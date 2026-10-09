//! Root search orchestration: parallel root-move evaluation using Rayon.

pub mod alpha_beta;
pub mod killer;
pub mod loop_detection;
pub mod move_ordering;
pub mod negamax;
pub mod outcome;
pub mod quiescence;
pub mod rng;

use crate::engine::constants::MAX_KILLER_DEPTH;
use crate::engine::search::killer::KillerTable;
use crate::engine::search::loop_detection::LoopDetector;
use crate::engine::search::negamax::negamax;
use crate::engine::search::rng::Rng;
use crate::engine::tree_recorder::TreeRecorder;
use crate::engine::tt::TranspositionTable;
use crate::engine::types::SearchConfig;
use crate::game::Game;
use crate::moves::Move;
use rayon::prelude::*;
use std::time::Instant;

/// Statistics collected during a root search.
#[derive(Debug, Default)]
pub struct SearchStats {
    /// Number of leaf or quiescence nodes visited.
    pub nodes_visited: u64,
    /// Time elapsed during the search.
    pub elapsed: std::time::Duration,
}

/// Result of a root search.
#[derive(Debug)]
pub struct RootSearchResult {
    /// Best move found.
    pub best_move: Option<Move>,
    /// Score for the best move (NegaMax-relative).
    pub best_score: i32,
    /// Principal variation (list of moves from root to leaf).
    pub pv: Vec<Move>,
    /// Every legal root move with its score (NegaMax-relative, exact: each
    /// root move is searched with a full window), best first. Game-ending
    /// root moves are included with their outcome score.
    pub root_scores: Vec<(Move, i32)>,
    /// Search statistics.
    pub stats: SearchStats,
}

/// Run a full root search on `game` using Rayon at the root level.
///
/// Each root move is evaluated in its own Rayon task, with a per-thread
/// transposition table shard (shared via DashMap) and independent killer/
/// loop-detector state.
///
/// `game_history` is an optional slice of board hashes for positions already
/// played in the real game. These are pre-loaded into each thread's
/// `LoopDetector` so that the engine avoids repeating any previously seen
/// position (draw by repetition from the 1st recurrence).
pub fn root_search(
    game: &Game,
    config: &SearchConfig,
    game_history: &[u64],
    recorder: Option<&TreeRecorder>,
) -> RootSearchResult {
    let start = Instant::now();
    let mut rng = Rng::new();

    // A blunder: play whatever a shallower search finds (see
    // `SearchConfig::blunder_chance`). Rolled up front so the full-depth
    // search is skipped entirely when it would not be used.
    if config.blunder_chance > 0.0
        && config.blunder_depth >= 1
        && config.blunder_depth < config.max_depth
        && rng.next_f32() < config.blunder_chance
    {
        let shallow = SearchConfig {
            max_depth: config.blunder_depth,
            blunder_chance: 0.0,
            ..config.clone()
        };
        return root_search(game, &shallow, game_history, recorder);
    }

    // Generate all root moves.
    let potential_moves = game.get_all_moves();
    let root_moves: Vec<Move> = potential_moves
        .iter()
        .flat_map(|pm| pm.to_moves())
        .collect();

    if root_moves.is_empty() {
        return RootSearchResult {
            best_move: None,
            best_score: -crate::engine::constants::KING_VALUE,
            pv: vec![],
            root_scores: vec![],
            stats: SearchStats {
                nodes_visited: 0,
                elapsed: start.elapsed(),
            },
        };
    }

    // Evaluate each root move in parallel.
    let mut results: Vec<(Move, i32)> = root_moves
        .par_iter()
        .map(|&mv| {
            let mut game_clone = game.clone();
            let undo = game_clone.make_unchecked(&mv);
            if let Some(outcome) = outcome::move_outcome(&game_clone, &undo, 0) {
                game_clone.unmake(&mv, undo);
                return (mv, outcome);
            }

            let mut local_tt = TranspositionTable::new(crate::engine::constants::TT_SIZE / 8);
            let tt_ptr = Some(&mut local_tt as *mut TranspositionTable);
            let mut ld = LoopDetector::new();
            // Pre-populate with real-game history so the engine avoids repeating
            // any already-played position (draw by repetition from 1st recurrence).
            for &h in game_history {
                let _ = ld.push(h);
            }
            let root_hash = game.board_hash();
            let _ = ld.push(root_hash);

            let mut killers = KillerTable::new(MAX_KILLER_DEPTH);

            // The root move is a node of its own: its replies hang under it,
            // not directly under the (unrecorded) root position `0`.
            let node_id = recorder.map(TreeRecorder::next_id).unwrap_or(0);

            let score = -negamax(
                &mut game_clone,
                1,
                -crate::engine::constants::KING_VALUE,
                crate::engine::constants::KING_VALUE,
                config,
                &mut ld,
                &mut killers,
                tt_ptr,
                recorder,
                node_id,
            );

            if let Some(r) = recorder {
                r.record(node_id, 0, 0, &mv, score);
            }

            game_clone.unmake(&mv, undo);
            (mv, score)
        })
        .collect();

    // Pick the root move to play. `results` already holds every legal root
    // move searched to full depth, so `select_root_move` picks among
    // already-computed scores — see `SearchConfig::noise_temperature` /
    // `decided_slip_chance` for what makes it deviate from the true best move.
    let (best_move, best_score) = select_root_move(&results, config, &mut rng)
        .map(|(mv, s)| (Some(mv), s))
        .unwrap_or((None, -crate::engine::constants::KING_VALUE));

    // Extract PV by re-running a single-threaded search and following best moves.
    let pv = if let Some(bm) = best_move {
        extract_pv(game, bm, config)
    } else {
        vec![]
    };
    // Stable sort: equal scores keep move-generation order.
    results.sort_by_key(|&(_, score)| std::cmp::Reverse(score));

    RootSearchResult {
        best_move,
        best_score,
        pv,
        root_scores: results,
        stats: SearchStats {
            nodes_visited: 0, // simplified; full counting omitted for brevity
            elapsed: start.elapsed(),
        },
    }
}

/// Pick which already-searched root move to actually play.
///
/// `results` holds every legal root move with its full-depth score (NegaMax
/// relative to the side to move). With `config.noise_temperature` at `0.0`
/// (the default) this always returns the highest-scoring move.
///
/// When the outcome is decided — the best move wins by force, or every move
/// loses by force (`|best| >= DECIDED_SCORE`) — it also returns the
/// highest-scoring move: the fastest win, or the defence that survives the
/// longest. Only a `decided_slip_chance` roll lets noise in there.
///
/// Otherwise every move is picked with probability proportional to
/// `exp((score - best) / temperature)`, scores clamped to
/// `±DECIDED_SCORE`: the best move is always the likeliest, near-equal ones
/// get a real chance, clearly worse ones fade out fast, and a move that loses
/// the king is never picked while one that doesn't exists.
fn select_root_move(
    results: &[(Move, i32)],
    config: &SearchConfig,
    rng: &mut Rng,
) -> Option<(Move, i32)> {
    let best = results.iter().copied().max_by_key(|&(_, s)| s)?;
    if config.noise_temperature <= 0.0 {
        return Some(best);
    }
    let decided = best.1.abs() >= outcome::DECIDED_SCORE;
    if decided && !(config.decided_slip_chance > 0.0 && rng.next_f32() < config.decided_slip_chance)
    {
        return Some(best);
    }

    let clamp = |s: i32| s.clamp(-outcome::DECIDED_SCORE, outcome::DECIDED_SCORE);
    let best_score = clamp(best.1);
    let weights: Vec<f32> = results
        .iter()
        .map(|&(_, s)| ((clamp(s) - best_score) as f32 / config.noise_temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();
    let mut pick = rng.next_f32() * total;
    for (i, &w) in weights.iter().enumerate() {
        pick -= w;
        if pick <= 0.0 {
            return Some(results[i]);
        }
    }
    results.last().copied()
}

/// Re-run the search from `root` → `first_move` and walk down the TT best
/// moves to extract the principal variation.
fn extract_pv(game: &Game, first_move: Move, config: &SearchConfig) -> Vec<Move> {
    let mut pv = vec![first_move];
    let mut game_clone = game.clone();
    let undo = game_clone.make_unchecked(&first_move);
    if undo.is_king_captured() {
        game_clone.unmake(&first_move, undo);
        return pv;
    }

    let mut tt = TranspositionTable::new(crate::engine::constants::TT_SIZE);
    let tt_ptr = Some(&mut tt as *mut TranspositionTable);
    let mut ld = LoopDetector::new();
    let mut killers = KillerTable::new(MAX_KILLER_DEPTH);

    let _ = negamax(
        &mut game_clone,
        1,
        -crate::engine::constants::KING_VALUE,
        crate::engine::constants::KING_VALUE,
        config,
        &mut ld,
        &mut killers,
        tt_ptr,
        None,
        0,
    );

    game_clone.unmake(&first_move, undo);

    // Walk the TT best moves starting from depth 2.
    let mut depth = 2usize;
    let mut cur_game = game.clone();
    cur_game.make_unchecked(&first_move);
    let max_depth = config.max_depth;

    while depth <= max_depth {
        let hash = cur_game.board_hash();
        if let Some(entry) = tt.get(hash) {
            if let Some(bm) = entry.best_move {
                pv.push(bm);
                cur_game.make_unchecked(&bm);
                depth += 1;
                continue;
            }
        }
        break;
    }

    pv
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};
    use crate::game::Game;

    fn minimal_game() -> Game {
        let mut board = Board::empty();
        board.set_piece(
            &Position::new(4, 8),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        board.set_piece(
            &Position::new(4, 0),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );
        Game::from_board(board)
    }

    #[test]
    fn root_search_returns_a_move() {
        let game = minimal_game();
        let config = SearchConfig {
            max_depth: 2,
            ..Default::default()
        };
        let result = root_search(&game, &config, &[], None);
        assert!(
            result.best_move.is_some(),
            "Expected a move from root search"
        );
    }

    #[test]
    fn root_search_with_material_advantage_has_positive_score() {
        let mut game = minimal_game();
        game.board.set_piece(
            &Position::new(3, 5),
            Some(Piece::new(Color::White, PieceType::Rook, None)),
        );
        let config = SearchConfig {
            max_depth: 2,
            ..Default::default()
        };
        let result = root_search(&game, &config, &[], None);
        assert!(
            result.best_score > 0,
            "Expected positive score for material advantage, got {}",
            result.best_score
        );
    }

    #[test]
    fn root_search_accepts_game_history_and_returns_move() {
        // Build a minimal game and pass the initial position hash as game history.
        // The call must not panic and must still return a valid move even when
        // the engine is instructed to treat a revisit to the root as a draw.
        let game = minimal_game();
        let initial_hash = game.board_hash();

        let config = SearchConfig {
            max_depth: 2,
            ..Default::default()
        };
        let result = root_search(&game, &config, &[initial_hash], None);
        // A move should still be found (the engine selects among non-repeating moves).
        assert!(
            result.best_move.is_some(),
            "Expected a move even with game history present"
        );
    }

    /// One distinct dummy move per score, in order.
    fn results_with(scores: &[i32]) -> Vec<(Move, i32)> {
        scores
            .iter()
            .enumerate()
            .map(|(i, &s)| {
                (
                    Move {
                        from: Position::new(i, 0),
                        to: Position::new(i, 1),
                        unstack: false,
                    },
                    s,
                )
            })
            .collect()
    }

    fn noisy(temperature: f32, slip: f32) -> SearchConfig {
        SearchConfig {
            noise_temperature: temperature,
            decided_slip_chance: slip,
            ..Default::default()
        }
    }

    /// Scores picked by `select_root_move` over `trials` draws.
    fn picks(results: &[(Move, i32)], config: &SearchConfig, trials: usize) -> Vec<i32> {
        let mut rng = Rng::seeded(99);
        (0..trials)
            .map(|_| select_root_move(results, config, &mut rng).unwrap().1)
            .collect()
    }

    const KV: i32 = crate::engine::constants::KING_VALUE;

    #[test]
    fn select_root_move_is_deterministic_argmax_at_zero_noise() {
        let results = results_with(&[10, 30, 20]);
        assert!(picks(&results, &SearchConfig::default(), 50)
            .iter()
            .all(|&s| s == 30));
    }

    #[test]
    fn select_root_move_with_noise_sometimes_picks_a_non_best_move() {
        let results = results_with(&[10, 30, 20]);
        assert!(picks(&results, &noisy(15.0, 0.0), 200)
            .iter()
            .any(|&s| s != 30));
    }

    #[test]
    fn select_root_move_returns_none_for_empty_results() {
        let mut rng = Rng::seeded(3);
        assert!(select_root_move(&[], &noisy(15.0, 0.0), &mut rng).is_none());
    }

    #[test]
    fn a_forced_win_is_always_taken_by_the_fastest_route_without_slips() {
        // An immediate king capture (KV), a slower forced win, quiet moves.
        let results = results_with(&[10, KV - 2, KV, 5]);
        assert!(picks(&results, &noisy(50.0, 0.0), 500)
            .iter()
            .all(|&s| s == KV));
    }

    #[test]
    fn a_lost_position_is_defended_for_as_long_as_possible_without_slips() {
        // Every move loses the king; the middle one survives longest.
        let results = results_with(&[-(KV - 1), -(KV - 3), -KV]);
        assert!(picks(&results, &noisy(50.0, 0.0), 500)
            .iter()
            .all(|&s| s == -(KV - 3)));
    }

    #[test]
    fn noise_never_hangs_the_king_while_a_safe_move_exists() {
        let results = results_with(&[0, -40, -(KV - 1)]);
        let picked = picks(&results, &noisy(20.0, 1.0), 20_000);
        assert!(picked.iter().all(|&s| s != -(KV - 1)));
        assert!(
            picked.contains(&-40),
            "noise should still pick worse safe moves"
        );
    }

    #[test]
    fn a_slip_can_miss_the_fastest_win() {
        let results = results_with(&[KV, KV - 2, 0]);
        let picked = picks(&results, &noisy(5.0, 1.0), 500);
        assert!(picked.contains(&(KV - 2)));
        assert!(
            picked.iter().all(|&s| s != 0),
            "a slip still plays a winning move"
        );
    }

    #[test]
    fn root_search_sees_the_win_by_capturing_every_non_king_piece() {
        let mut game = minimal_game();
        game.board.set_piece(
            &Position::new(0, 4),
            Some(Piece::new(Color::White, PieceType::Rook, None)),
        );
        game.board.set_piece(
            &Position::new(0, 1),
            Some(Piece::new(Color::Black, PieceType::Guard, None)),
        );
        let config = SearchConfig {
            max_depth: 2,
            ..Default::default()
        };
        let result = root_search(&game, &config, &[], None);
        assert_eq!(
            result.best_score, KV,
            "capturing the last guard wins at once"
        );
        assert_eq!(result.best_move.unwrap().to, Position::new(0, 1));
    }

    #[test]
    fn root_search_scores_the_fortieth_quiet_move_as_a_draw() {
        // White is a rook up but has no capture, and its next move is the
        // 40th without one: every move ends the game drawn.
        let mut game = minimal_game();
        game.board.set_piece(
            &Position::new(3, 5),
            Some(Piece::new(Color::White, PieceType::Rook, None)),
        );
        game.board.set_piece(
            &Position::new(8, 1),
            Some(Piece::new(Color::Black, PieceType::Guard, None)),
        );
        let config = SearchConfig {
            max_depth: 2,
            ..Default::default()
        };
        game.set_moves_without_capture(30);
        assert!(root_search(&game, &config, &[], None).best_score > 0);
        game.set_moves_without_capture(39);
        assert_eq!(root_search(&game, &config, &[], None).best_score, 0);
    }
}
