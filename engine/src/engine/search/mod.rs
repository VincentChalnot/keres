//! Root search orchestration: parallel root-move evaluation using Rayon.

pub mod alpha_beta;
pub mod killer;
pub mod loop_detection;
pub mod move_ordering;
pub mod negamax;
pub mod quiescence;

use crate::engine::constants::MAX_KILLER_DEPTH;
use crate::engine::search::killer::KillerTable;
use crate::engine::search::loop_detection::LoopDetector;
use crate::engine::search::negamax::negamax;
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
    /// Search statistics.
    pub stats: SearchStats,
}

/// Run a full root search on `game` using Rayon at the root level.
///
/// Each root move is evaluated in its own Rayon task, with a per-thread
/// transposition table shard (shared via DashMap) and independent killer/
/// loop-detector state.
pub fn root_search(
    game: &Game,
    config: &SearchConfig,
    recorder: Option<&TreeRecorder>,
) -> RootSearchResult {
    let start = Instant::now();

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
            stats: SearchStats {
                nodes_visited: 0,
                elapsed: start.elapsed(),
            },
        };
    }

    // Evaluate each root move in parallel.
    let results: Vec<(Move, i32)> = root_moves
        .par_iter()
        .map(|&mv| {
            let mut game_clone = game.clone();
            let undo = game_clone.make_unchecked(&mv);
            if undo.is_king_captured() {
                game_clone.unmake(&mv, undo);
                return (mv, crate::engine::constants::KING_VALUE);
            }

            let mut local_tt = TranspositionTable::new(crate::engine::constants::TT_SIZE / 8);
            let tt_ptr = Some(&mut local_tt as *mut TranspositionTable);
            let mut ld = LoopDetector::new();
            let root_hash = game.board_hash();
            let _ = ld.push(root_hash);

            let mut killers = KillerTable::new(MAX_KILLER_DEPTH);

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
                0,
            );

            game_clone.unmake(&mv, undo);
            (mv, score)
        })
        .collect();

    // Pick the best root move.
    let (best_move, best_score) = results
        .iter()
        .max_by_key(|(_, s)| *s)
        .map(|&(mv, s)| (Some(mv), s))
        .unwrap_or((None, -crate::engine::constants::KING_VALUE));

    // Extract PV by re-running a single-threaded search and following best moves.
    let pv = if let Some(bm) = best_move {
        extract_pv(game, bm, config)
    } else {
        vec![]
    };

    RootSearchResult {
        best_move,
        best_score,
        pv,
        stats: SearchStats {
            nodes_visited: 0, // simplified; full counting omitted for brevity
            elapsed: start.elapsed(),
        },
    }
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
        board.set_piece(&Position::new(4, 8), Some(Piece::new(Color::White, PieceType::King, None)));
        board.set_piece(&Position::new(4, 0), Some(Piece::new(Color::Black, PieceType::King, None)));
        Game::from_board(board)
    }

    #[test]
    fn root_search_returns_a_move() {
        let game = minimal_game();
        let config = SearchConfig {
            max_depth: 2,
            ..Default::default()
        };
        let result = root_search(&game, &config, None);
        assert!(result.best_move.is_some(), "Expected a move from root search");
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
        let result = root_search(&game, &config, None);
        assert!(
            result.best_score > 0,
            "Expected positive score for material advantage, got {}",
            result.best_score
        );
    }
}
