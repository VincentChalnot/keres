//! NegaMax recursive search function.

use crate::engine::constants::KING_VALUE;
use crate::engine::search::alpha_beta::{should_cutoff, update_alpha};
use crate::engine::search::killer::KillerTable;
use crate::engine::search::loop_detection::LoopDetector;
use crate::engine::search::move_ordering::order_moves;
use crate::engine::search::quiescence::quiescence;
use crate::engine::tree_recorder::TreeRecorder;
use crate::engine::tt::{board_hash, TranspositionTable};
use crate::engine::types::{BoundType, SearchConfig};
use crate::game::Game;
use crate::moves::Move;

/// NegaMax search with alpha-beta pruning.
///
/// Returns the NegaMax-relative score for the current side to move.
/// `depth` is the current ply from the root (0 = root).
///
/// `tt_ptr` is an optional raw mutable pointer to a `TranspositionTable`.
/// Using a raw pointer avoids Rust's borrow checker limitations in recursive
/// single-threaded search (the pointer is never aliased within a single branch).
#[allow(clippy::too_many_arguments)]
pub fn negamax(
    game: &mut Game,
    depth: usize,
    mut alpha: i32,
    beta: i32,
    config: &SearchConfig,
    loop_detector: &mut LoopDetector,
    killers: &mut KillerTable,
    tt_ptr: Option<*mut TranspositionTable>,
    recorder: Option<&TreeRecorder>,
    parent_id: u64,
) -> i32 {
    // ── 1. Loop detection ────────────────────────────────────────────────────
    let hash = board_hash(&game);

    if loop_detector.push(hash) {
        loop_detector.pop(hash);
        return 0; // Repetition → draw.
    }

    // ── 2. Transposition table lookup ────────────────────────────────────────
    let max_depth = config.max_depth;
    let remaining_depth = (max_depth.saturating_sub(depth)) as u8;

    let mut tt_best_move: Option<Move> = None;
    if config.use_tt {
        if let Some(ptr) = tt_ptr {
            // Safety: single-threaded branch; no aliased mutable access.
            let tt_ref = unsafe { &*ptr };
            if let Some(entry) = tt_ref.get(hash) {
                tt_best_move = entry.best_move;
                if entry.remaining_depth >= remaining_depth {
                    let score = entry.score;
                    match entry.bound_type {
                        BoundType::Exact => {
                            loop_detector.pop(hash);
                            return score;
                        }
                        BoundType::LowerBound => {
                            alpha = update_alpha(alpha, score);
                        }
                        BoundType::UpperBound => {
                            let new_beta = beta.min(score);
                            if should_cutoff(alpha, new_beta) {
                                loop_detector.pop(hash);
                                return score;
                            }
                        }
                    }
                    if should_cutoff(alpha, beta) {
                        loop_detector.pop(hash);
                        return score;
                    }
                }
            }
        }
    }
    let _ = tt_best_move; // used indirectly via order_moves

    // ── 3. Leaf / quiescence ─────────────────────────────────────────────────
    if depth >= max_depth {
        loop_detector.pop(hash);
        if config.use_quiescence {
            return quiescence(game, alpha, beta, recorder, depth, parent_id);
        } else {
            return if game.is_white_to_move() {
                crate::engine::eval::evaluate_absolute(game)
            } else {
                -crate::engine::eval::evaluate_absolute(game)
            };
        }
    }

    // ── 4. Generate and order moves ──────────────────────────────────────────
    let potential_moves = game.get_all_moves();
    let mut moves: Vec<Move> = potential_moves
        .iter()
        .flat_map(|pm| pm.to_moves())
        .collect();

    if moves.is_empty() {
        loop_detector.pop(hash);
        return -KING_VALUE;
    }

    {
        // Safety: read-only borrow; no mutable access to TT during ordering.
        let tt_read: Option<&TranspositionTable> = tt_ptr.map(|p| unsafe { &*p });
        order_moves(&mut moves, game, depth, tt_read, hash, killers);
    }

    // ── 5. Search each move ──────────────────────────────────────────────────
    let original_alpha = alpha;
    let mut best_score = i32::MIN + 1;
    let mut best_move: Option<Move> = None;

    for mv in &moves {
        let undo = game.make_unchecked(mv);

        if undo.is_king_captured() {
            game.unmake(mv, undo);
            best_score = KING_VALUE;
            best_move = Some(*mv);
            break;
        }

        let node_id = recorder
            .map(|r| r.record_node(parent_id, depth as u8, mv, 0))
            .unwrap_or(0);

        let score = negamax(
            game,
            depth + 1,
            -beta,
            -alpha,
            config,
            loop_detector,
            killers,
            tt_ptr,
            recorder,
            node_id,
        );

        game.unmake(mv, undo);

        if let Some(r) = recorder {
            r.update_score(node_id, -score);
        }

        let negamax_score = -score;

        if negamax_score > best_score {
            best_score = negamax_score;
            best_move = Some(*mv);
        }

        alpha = update_alpha(alpha, negamax_score);

        if config.use_alpha_beta && should_cutoff(alpha, beta) {
            if config.use_killers && game.board.get_piece(&mv.to).is_none() {
                killers.store(depth, *mv);
            }
            break;
        }
    }

    loop_detector.pop(hash);

    // ── 6. Determine bound type ──────────────────────────────────────────────
    let bound_type = if best_score >= beta {
        BoundType::LowerBound
    } else if best_score <= original_alpha {
        BoundType::UpperBound
    } else {
        BoundType::Exact
    };

    // ── 7. Store in TT ───────────────────────────────────────────────────────
    if config.use_tt {
        if let Some(ptr) = tt_ptr {
            // Safety: single-threaded branch; no aliased mutable access.
            let tt_mut = unsafe { &mut *ptr };
            tt_mut.store(hash, remaining_depth, best_score, bound_type, best_move);
        }
    }

    best_score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};
    use crate::engine::constants::MAX_KILLER_DEPTH;
    use crate::engine::search::killer::KillerTable;
    use crate::engine::search::loop_detection::LoopDetector;
    use crate::engine::tt::TranspositionTable;
    use crate::engine::types::SearchConfig;
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

    fn run_negamax(game: &mut Game, max_depth: usize) -> i32 {
        let config = SearchConfig {
            max_depth,
            ..Default::default()
        };
        let mut ld = LoopDetector::new();
        let mut killers = KillerTable::new(MAX_KILLER_DEPTH);
        let mut tt = TranspositionTable::new(1024);
        let tt_ptr = Some(&mut tt as *mut TranspositionTable);
        negamax(
            game, 0, -10_000, 10_000, &config, &mut ld, &mut killers, tt_ptr, None, 0,
        )
    }

    #[test]
    fn negamax_depth1_returns_nonzero_for_asymmetric_position() {
        let mut game = minimal_game();
        game.board.set_piece(
            &Position::new(3, 5),
            Some(Piece::new(Color::White, PieceType::Rook, None)),
        );
        let score = run_negamax(&mut game, 1);
        assert!(score > 0, "expected positive score, got {}", score);
    }

    #[test]
    fn negamax_finds_capture() {
        let mut game = minimal_game();
        game.board.set_piece(
            &Position::new(4, 5),
            Some(Piece::new(Color::White, PieceType::Rook, None)),
        );
        game.board.set_piece(
            &Position::new(4, 3),
            Some(Piece::new(Color::Black, PieceType::Soldier, None)),
        );
        let score = run_negamax(&mut game, 2);
        assert!(
            score > 0,
            "expected positive score after capturing soldier, got {}",
            score
        );
    }
}

