//! Promotion bonus: reward soldiers that are close to promoting.

use crate::board::{Piece, PieceType, Position};
use crate::engine::constants::SOLDIER_VALUE;
use crate::engine::eval::material::perspective_row;

/// Promotion bonus for a piece (or stack) at the given position.
///
/// Only soldiers (bottom or top) receive a promotion bonus.
/// - Perspective row 6 (one row before last): +10% of `SOLDIER_VALUE`
/// - Perspective row 7 (last row before promotion): +20% of `SOLDIER_VALUE`
///
/// The Ballista does NOT receive a promotion bonus.
pub fn promotion_bonus(piece: &Piece, pos: Position) -> i32 {
    let mut bonus = 0i32;

    // Check the bottom piece.
    if piece.bottom == PieceType::Soldier {
        bonus += soldier_rank_bonus(piece.color, pos.y);
    }

    // Check the top piece.
    if piece.top == Some(PieceType::Soldier) {
        bonus += soldier_rank_bonus(piece.color, pos.y);
    }

    bonus
}

fn soldier_rank_bonus(color: crate::board::Color, y: usize) -> i32 {
    let row = perspective_row(color, y);
    match row {
        6 => SOLDIER_VALUE / 10,      // +10%
        7 => SOLDIER_VALUE * 2 / 10,  // +20%
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Color;

    #[test]
    fn promotion_bonus_row6_white() {
        // White perspective_row 6 means y = 8 - 6 = 2
        let piece = Piece::new(Color::White, PieceType::Soldier, None);
        let pos = Position::new(4, 2);
        assert_eq!(promotion_bonus(&piece, pos), SOLDIER_VALUE / 10);
    }

    #[test]
    fn promotion_bonus_row7_white() {
        // White perspective_row 7 means y = 8 - 7 = 1
        let piece = Piece::new(Color::White, PieceType::Soldier, None);
        let pos = Position::new(4, 1);
        assert_eq!(promotion_bonus(&piece, pos), SOLDIER_VALUE * 2 / 10);
    }

    #[test]
    fn promotion_bonus_row6_black() {
        // Black perspective_row 6 means y = 6
        let piece = Piece::new(Color::Black, PieceType::Soldier, None);
        let pos = Position::new(4, 6);
        assert_eq!(promotion_bonus(&piece, pos), SOLDIER_VALUE / 10);
    }

    #[test]
    fn promotion_bonus_row7_black() {
        // Black perspective_row 7 means y = 7
        let piece = Piece::new(Color::Black, PieceType::Soldier, None);
        let pos = Position::new(4, 7);
        assert_eq!(promotion_bonus(&piece, pos), SOLDIER_VALUE * 2 / 10);
    }

    #[test]
    fn promotion_bonus_zero_for_non_soldier() {
        let piece = Piece::new(Color::White, PieceType::Rook, None);
        let pos = Position::new(4, 1);
        assert_eq!(promotion_bonus(&piece, pos), 0);
    }

    #[test]
    fn promotion_bonus_stack_with_soldier_top() {
        // Stack: Rook (bottom) + Soldier (top) at white perspective row 7 (y=1)
        let piece = Piece::new(Color::White, PieceType::Rook, Some(PieceType::Soldier));
        let pos = Position::new(4, 1);
        assert_eq!(promotion_bonus(&piece, pos), SOLDIER_VALUE * 2 / 10);
    }
}
