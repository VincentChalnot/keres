//! Material-only static evaluation for Keres.
//!
//! Evaluates board positions by pure material counting.
//! Only runs at leaf nodes (depth=0) in Stage 1 search.

use crate::board::{Board, Color, PieceType, Position, BOARD_SIZE};

/// Piece values for static evaluation (in centipawns).
pub const SOLDIER_VALUE: i32 = 10;
pub const GUARD_VALUE: i32 = 25;
pub const PALADIN_VALUE: i32 = 30;
pub const BISHOP_VALUE: i32 = 40;
pub const KNIGHT_VALUE: i32 = 40;
pub const BALLISTA_VALUE: i32 = 45;
pub const ROOK_VALUE: i32 = 60;
pub const KING_VALUE: i32 = 1000;

/// Material value for a piece type.
pub fn piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Soldier  => SOLDIER_VALUE,
        PieceType::Guard    => GUARD_VALUE,
        PieceType::Paladin  => PALADIN_VALUE,
        PieceType::Bishop   => BISHOP_VALUE,
        PieceType::Knight   => KNIGHT_VALUE,
        PieceType::Ballista => BALLISTA_VALUE,
        PieceType::Rook     => ROOK_VALUE,
        PieceType::King     => KING_VALUE,
    }
}

/// Mate score constant (used for terminal positions).
pub const MATE_SCORE: i32 = 100_000;

/// Evaluate the board from the side-to-move's perspective.
/// Positive = advantage for the side to move.
///
/// Terminal positions return mate/draw scores.
/// Non-terminal positions use pure material counting.
pub fn evaluate(board: &Board) -> i32 {
    if board.is_game_over() {
        if board.is_draw() {
            return 0;
        }
        // The side that just moved captured the king, so current
        // side-to-move is the *loser*.
        return -MATE_SCORE;
    }

    let mut white_material: i32 = 0;
    let mut black_material: i32 = 0;

    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            let acc = if piece.color == Color::White {
                &mut white_material
            } else {
                &mut black_material
            };
            *acc += piece_value(piece.bottom);
            if let Some(top) = piece.top {
                *acc += piece_value(top);
            }
        }
    }

    let diff = white_material - black_material;
    if board.is_white_to_move() { diff } else { -diff }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn starting_position_is_zero() {
        let b = Board::new();
        let score = evaluate(&b);
        assert_eq!(score, 0, "symmetric start should score 0");
    }

    #[test]
    fn terminal_draw_is_zero() {
        let mut b = Board::new();
        b.set_game_over(true, false, true);
        assert_eq!(evaluate(&b), 0);
    }

    #[test]
    fn terminal_loss_is_negative_mate() {
        let mut b = Board::new();
        b.set_game_over(true, true, false); // white wins, but it's white to move => current side is "loser" semantically
        // With the king-capture convention: after king capture the turn has flipped,
        // so the side to move is always the loser.
        assert_eq!(evaluate(&b), -MATE_SCORE);
    }

    #[test]
    fn piece_values_are_correct() {
        assert_eq!(piece_value(PieceType::Soldier), 10);
        assert_eq!(piece_value(PieceType::Guard), 25);
        assert_eq!(piece_value(PieceType::Paladin), 30);
        assert_eq!(piece_value(PieceType::Bishop), 40);
        assert_eq!(piece_value(PieceType::Knight), 40);
        assert_eq!(piece_value(PieceType::Ballista), 45);
        assert_eq!(piece_value(PieceType::Rook), 60);
        assert_eq!(piece_value(PieceType::King), 1000);
    }
}
