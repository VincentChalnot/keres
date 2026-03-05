//! Material evaluation: base piece values and weighted piece scoring.

use crate::board::{Color, Piece, PieceType, Position};
use crate::engine::constants::*;
use crate::engine::eval::mobility::mobility_bonus;
use crate::engine::eval::promotion::promotion_bonus;
use crate::engine::eval::pst::pst_bonus;
use crate::game::Game;

/// Return the base material value for a single piece type.
pub fn base_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Soldier => SOLDIER_VALUE,
        PieceType::Guard => GUARD_VALUE,
        PieceType::Paladin => PALADIN_VALUE,
        PieceType::Bishop => BISHOP_VALUE,
        PieceType::Knight => KNIGHT_VALUE,
        PieceType::Ballista => BALLISTA_VALUE,
        PieceType::Rook => ROOK_VALUE,
        PieceType::King => KING_VALUE,
    }
}

/// Compute the weighted value of a piece (or stack) at `pos` on `game`.
///
/// For a stack the base value is the sum of both piece base values, mobility
/// is the union of destinations, and all other modifiers are applied once to
/// the combined value.
pub fn weighted_value(piece: &Piece, pos: Position, game: &Game) -> i32 {
    let bv = stack_base_value(piece);
    let pst = pst_bonus(piece, pos);
    let mob = mobility_bonus(piece, pos, game);
    let promo = promotion_bonus(piece, pos);
    bv + pst + mob + promo
}

/// Base value for a piece or stack (sum of component base values).
pub fn stack_base_value(piece: &Piece) -> i32 {
    let bottom_val = base_value(piece.bottom);
    let top_val = piece.top.map(base_value).unwrap_or(0);
    bottom_val + top_val
}

/// Perspective row for PST / promotion lookup.
/// Returns 0 for a piece's own back rank, 8 for the promotion rank.
pub fn perspective_row(color: Color, y: usize) -> usize {
    match color {
        Color::White => 8 - y,
        Color::Black => y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_values_match_constants() {
        assert_eq!(base_value(PieceType::Soldier), SOLDIER_VALUE);
        assert_eq!(base_value(PieceType::King), KING_VALUE);
        assert_eq!(base_value(PieceType::Rook), ROOK_VALUE);
    }

    #[test]
    fn stack_base_value_sums_both_pieces() {
        let piece = Piece::new(Color::White, PieceType::Soldier, Some(PieceType::Bishop));
        assert_eq!(stack_base_value(&piece), SOLDIER_VALUE + BISHOP_VALUE);
    }

    #[test]
    fn perspective_row_white_back_rank_is_zero() {
        // White back rank is y=8
        assert_eq!(perspective_row(Color::White, 8), 0);
        // White promotion rank is y=0
        assert_eq!(perspective_row(Color::White, 0), 8);
    }

    #[test]
    fn perspective_row_black_back_rank_is_zero() {
        // Black back rank is y=0
        assert_eq!(perspective_row(Color::Black, 0), 0);
        // Black promotion rank is y=8
        assert_eq!(perspective_row(Color::Black, 8), 8);
    }
}
