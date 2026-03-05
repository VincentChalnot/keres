//! Piece-square tables: positional bonuses indexed by piece type, row, and column.
//!
//! All tables are from the perspective of the owning side (row 0 = own back
//! rank, row 8 = opponent's back rank / promotion rank).  Use
//! `material::perspective_row` to convert absolute board rows before
//! indexing.

use crate::board::{Color, Piece, PieceType, Position};
use crate::engine::eval::material::perspective_row;

/// PST dimensions: [perspective_row 0..=8][col 0..=8].
type PstTable = [[i32; 9]; 9];

// ---------------------------------------------------------------------------
// Individual PST tables
// ---------------------------------------------------------------------------

/// Soldier PST: reward advancement, slight centre preference.
const SOLDIER_PST: PstTable = [
    // row 0 (own back rank)
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [1, 1, 1, 1, 1, 1, 1, 1, 1],
    [1, 1, 2, 2, 2, 2, 2, 1, 1],
    [2, 2, 3, 3, 3, 3, 3, 2, 2],
    [3, 3, 4, 4, 4, 4, 4, 3, 3],
    [4, 4, 5, 5, 5, 5, 5, 4, 4],
    [5, 5, 6, 6, 6, 6, 6, 5, 5],
    // row 8 (promotion rank)
    [6, 6, 7, 7, 7, 7, 7, 6, 6],
];

/// Guard PST: slight centre preference.
const GUARD_PST: PstTable = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 1, 1, 1, 1, 1, 1, 0],
    [0, 1, 2, 2, 2, 2, 2, 1, 0],
    [0, 1, 2, 3, 3, 3, 2, 1, 0],
    [0, 1, 2, 3, 4, 3, 2, 1, 0],
    [0, 1, 2, 3, 3, 3, 2, 1, 0],
    [0, 1, 2, 2, 2, 2, 2, 1, 0],
    [0, 1, 1, 1, 1, 1, 1, 1, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
];

/// Paladin PST: mirrors Guard (orthogonal version).
const PALADIN_PST: PstTable = GUARD_PST;

/// Bishop PST: centre and forward preference.
const BISHOP_PST: PstTable = [
    [-2, -1, -1, -1, -1, -1, -1, -1, -2],
    [-1, 0, 0, 0, 0, 0, 0, 0, -1],
    [-1, 0, 1, 1, 1, 1, 1, 0, -1],
    [-1, 0, 1, 2, 2, 2, 1, 0, -1],
    [-1, 0, 1, 2, 3, 2, 1, 0, -1],
    [-1, 0, 1, 2, 2, 2, 1, 0, -1],
    [-1, 0, 1, 1, 1, 1, 1, 0, -1],
    [-1, 0, 0, 0, 0, 0, 0, 0, -1],
    [-2, -1, -1, -1, -1, -1, -1, -1, -2],
];

/// Rook PST: slightly reward open files and 7th/8th rank.
const ROOK_PST: PstTable = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [2, 2, 2, 2, 2, 2, 2, 2, 2],
    [3, 3, 3, 3, 3, 3, 3, 3, 3],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
];

/// Knight PST: mild centre preference.
const KNIGHT_PST: PstTable = [
    [-4, -2, -2, -2, -2, -2, -2, -2, -4],
    [-2, -1, 0, 0, 0, 0, 0, -1, -2],
    [-2, 0, 1, 2, 2, 2, 1, 0, -2],
    [-2, 0, 2, 3, 3, 3, 2, 0, -2],
    [-2, 0, 2, 3, 4, 3, 2, 0, -2],
    [-2, 0, 2, 3, 3, 3, 2, 0, -2],
    [-2, 0, 1, 2, 2, 2, 1, 0, -2],
    [-2, -1, 0, 0, 0, 0, 0, -1, -2],
    [-4, -2, -2, -2, -2, -2, -2, -2, -4],
];

/// Ballista PST: reward central columns and advancement.
const BALLISTA_PST: PstTable = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 1, 1, 1, 1, 1, 0, 0],
    [0, 0, 1, 2, 2, 2, 1, 0, 0],
    [0, 0, 1, 2, 3, 2, 1, 0, 0],
    [0, 0, 1, 2, 3, 2, 1, 0, 0],
    [0, 0, 1, 2, 3, 2, 1, 0, 0],
    [0, 0, 1, 2, 2, 2, 1, 0, 0],
    [0, 0, 1, 1, 1, 1, 1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0],
];

/// King PST: stay in back rank early (lower rows = own back rank).
const KING_PST: PstTable = [
    [3, 4, 2, 0, 0, 0, 2, 4, 3],
    [2, 2, 1, -1, -1, -1, 1, 2, 2],
    [-2, -2, -2, -2, -2, -2, -2, -2, -2],
    [-3, -3, -3, -3, -3, -3, -3, -3, -3],
    [-4, -4, -4, -4, -4, -4, -4, -4, -4],
    [-5, -5, -5, -5, -5, -5, -5, -5, -5],
    [-6, -6, -6, -6, -6, -6, -6, -6, -6],
    [-7, -7, -7, -7, -7, -7, -7, -7, -7],
    [-8, -8, -8, -8, -8, -8, -8, -8, -8],
];

fn table_for(pt: PieceType) -> &'static PstTable {
    match pt {
        PieceType::Soldier => &SOLDIER_PST,
        PieceType::Guard => &GUARD_PST,
        PieceType::Paladin => &PALADIN_PST,
        PieceType::Bishop => &BISHOP_PST,
        PieceType::Rook => &ROOK_PST,
        PieceType::Knight => &KNIGHT_PST,
        PieceType::Ballista => &BALLISTA_PST,
        PieceType::King => &KING_PST,
    }
}

/// PST bonus for a single piece type at the given board position.
fn pst_single(pt: PieceType, color: Color, pos: Position) -> i32 {
    let row = perspective_row(color, pos.y);
    let col = pos.x;
    table_for(pt)[row][col]
}

/// PST bonus for a piece or stack at `pos`.
/// For a stack the bonus is the sum of both component bonuses.
pub fn pst_bonus(piece: &Piece, pos: Position) -> i32 {
    let bottom_bonus = pst_single(piece.bottom, piece.color, pos);
    let top_bonus = piece
        .top
        .map(|t| pst_single(t, piece.color, pos))
        .unwrap_or(0);
    bottom_bonus + top_bonus
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Piece;

    #[test]
    fn pst_soldier_advances_score_increases() {
        let white_soldier = Piece::new(Color::White, PieceType::Soldier, None);
        // White soldiers advance toward lower y
        let back = pst_bonus(&white_soldier, Position::new(4, 8));
        let front = pst_bonus(&white_soldier, Position::new(4, 1));
        assert!(
            front > back,
            "forward white soldier should score higher: back={} front={}",
            back,
            front
        );
    }

    #[test]
    fn pst_bonus_for_stack_sums_both() {
        let stacked = Piece::new(Color::White, PieceType::Soldier, Some(PieceType::Bishop));
        let soldier_only = Piece::new(Color::White, PieceType::Soldier, None);
        let bishop_only = Piece::new(Color::White, PieceType::Bishop, None);
        let pos = Position::new(4, 4);
        assert_eq!(
            pst_bonus(&stacked, pos),
            pst_bonus(&soldier_only, pos) + pst_bonus(&bishop_only, pos)
        );
    }
}
