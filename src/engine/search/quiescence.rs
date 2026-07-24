//! Quiescence search: extend the search at noisy positions to avoid the
//! horizon effect.

use crate::engine::constants::DELTA_MARGIN;
use crate::engine::constants::KING_VALUE;
use crate::engine::eval::evaluate_absolute;
use crate::engine::search::move_ordering::order_captures;
use crate::engine::tree_recorder::TreeRecorder;
use crate::game::Game;

/// Quiescence search from the current position.
///
/// `alpha` and `beta` are NegaMax-relative bounds.
/// Returns a NegaMax-relative score.
pub fn quiescence(
    game: &mut Game,
    alpha: i32,
    beta: i32,
    recorder: Option<&TreeRecorder>,
    depth: usize,
    parent_id: u64,
) -> i32 {
    // Stand-pat: evaluate the position statically (NegaMax-relative).
    let stand_pat = if game.is_white_to_move() {
        evaluate_absolute(game)
    } else {
        -evaluate_absolute(game)
    };

    if stand_pat >= beta {
        return stand_pat;
    }

    let mut alpha = alpha.max(stand_pat);

    // Generate only captures and promotion moves.
    let potential_moves = game.get_all_moves();
    let mut moves: Vec<crate::moves::Move> = potential_moves
        .iter()
        .flat_map(|pm| pm.to_moves())
        .collect();
    order_captures(&mut moves, game);

    for mv in moves {
        // Delta pruning: skip if the best possible material gain cannot raise alpha.
        let material_gain = estimate_capture_gain(&mv, game);
        if stand_pat + material_gain + DELTA_MARGIN < alpha {
            continue;
        }

        let undo = game.make_unchecked(&mv);
        if undo.is_king_captured() {
            game.unmake(&mv, undo);
            // Prefer shallower king captures
            return KING_VALUE - depth as i32;
        }

        let node_id = recorder
            .map(|r| r.record_node(parent_id, depth as u8, &mv, 0))
            .unwrap_or(0);

        let score = -quiescence(game, -beta, -alpha, recorder, depth + 1, node_id);
        game.unmake(&mv, undo);

        if let Some(r) = recorder {
            r.update_score(node_id, score);
        }

        if score >= beta {
            return score;
        }
        alpha = alpha.max(score);
    }

    alpha
}

/// Estimate the material gain from a capture move (used for delta pruning).
fn estimate_capture_gain(mv: &crate::moves::Move, game: &Game) -> i32 {
    use crate::engine::eval::material::stack_base_value;
    game.board
        .get_piece(&mv.to)
        .filter(|p| p.color != game.color_to_move())
        .map(stack_base_value)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};
    use crate::game::Game;

    fn minimal_game(white_to_move: bool) -> Game {
        let mut board = Board::empty();
        board.set_piece(
            &Position::new(4, 8),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        board.set_piece(
            &Position::new(4, 0),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );
        let mut game = Game::from_board(board);
        game.set_white_to_move(white_to_move);
        game
    }

    #[test]
    fn quiescence_returns_finite_score_on_empty_board() {
        let mut game = minimal_game(true);
        let score = quiescence(&mut game, -10_000, 10_000, None, 0, 0);
        assert!(score.abs() < 10_000);
    }

    #[test]
    fn quiescence_captures_free_piece() {
        // White rook can capture a free black soldier.
        let mut game = minimal_game(true);
        game.board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::White, PieceType::Rook, None)),
        );
        game.board.set_piece(
            &Position::new(4, 2),
            Some(Piece::new(Color::Black, PieceType::Soldier, None)),
        );
        let score = quiescence(&mut game, -10_000, 10_000, None, 0, 0);
        // Score should be positive (White ahead) after capturing the soldier.
        assert!(
            score > 0,
            "expected positive score after free capture, got {}",
            score
        );
    }
}
