//! Static Exchange Evaluation (SEE) for Keres.
//!
//! Provides functions for estimating the material outcome of capture
//! exchanges without doing a full recursive search. Used for:
//! - MVV-LVA move ordering (attacker/victim value)
//! - Qualifying captures in Stage 2 extensions (SEE ≥ 0)
//! - Futility pruning (max capturable piece value)

use crate::board::{Board, Color, Piece, PieceType, Position};
use super::config::ScoringWeights;

/// Material value for a single piece type in centipawns.
pub fn piece_value(pt: PieceType, weights: &ScoringWeights) -> i32 {
    match pt {
        PieceType::Soldier  => weights.soldier_pts as i32,
        PieceType::Bishop   => weights.bishop_pts as i32,
        PieceType::Rook     => weights.rook_pts as i32,
        PieceType::Paladin  => weights.paladin_pts as i32,
        PieceType::Guard    => weights.guard_pts as i32,
        PieceType::Knight   => weights.knight_pts as i32,
        PieceType::Ballista => weights.ballista_pts as i32,
        PieceType::King     => weights.king_pts as i32,
    }
}

/// Total material value of a piece including any stacked top piece.
pub fn total_piece_value(piece: &Piece, weights: &ScoringWeights) -> i32 {
    let mut val = piece_value(piece.bottom, weights);
    if let Some(top) = piece.top {
        val += piece_value(top, weights);
    }
    val
}

/// Value of the piece that is actually moving (for MVV-LVA attacker value).
/// When unstacking, only the top piece moves; otherwise the whole piece/stack.
pub fn attacker_value(piece: &Piece, unstack: bool, weights: &ScoringWeights) -> i32 {
    if unstack {
        if let Some(top) = piece.top {
            piece_value(top, weights)
        } else {
            piece_value(piece.bottom, weights)
        }
    } else {
        total_piece_value(piece, weights)
    }
}

/// Simplified Static Exchange Evaluation for a capture move.
///
/// Returns the estimated net material gain: `victim_value - attacker_value`.
/// Positive means the capture is likely winning (SEE ≥ 0).
///
/// This is a first-order approximation; a full SEE would simulate
/// all recaptures on the target square, but this is complex in Keres
/// due to stacking mechanics.
pub fn see_capture(
    board: &Board,
    from: &Position,
    to: &Position,
    unstack: bool,
    weights: &ScoringWeights,
) -> i32 {
    let attacker = match board.get_piece(from) {
        Some(p) => p,
        None => return 0,
    };
    let victim = match board.get_piece(to) {
        Some(p) => p,
        None => return 0,
    };
    if attacker.color == victim.color {
        return 0;
    }

    let victim_val = total_piece_value(victim, weights);
    let atk_val = attacker_value(attacker, unstack, weights);
    victim_val - atk_val
}

/// Maximum capturable piece value on the board for a given color.
/// Scans all opponent pieces and returns the highest total value.
/// Used for futility pruning at frontier nodes.
pub fn max_capturable_value(
    board: &Board,
    by_color: Color,
    weights: &ScoringWeights,
) -> i32 {
    let opponent = match by_color {
        Color::White => Color::Black,
        Color::Black => Color::White,
    };
    let mut max_val = 0i32;
    for sq in 0..81 {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            if piece.color == opponent {
                let val = total_piece_value(piece, weights);
                if val > max_val {
                    max_val = val;
                }
            }
        }
    }
    max_val
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};

    fn w() -> ScoringWeights { ScoringWeights::default() }

    fn empty_board() -> Board {
        let mut b = Board::new();
        for sq in 0..81 {
            b.set_piece(&Position::from_u8(sq), None);
        }
        b
    }

    #[test]
    fn soldier_captures_rook_is_positive() {
        let mut b = empty_board();
        let from = Position::new(0, 0);
        let to = Position::new(1, 1);
        b.set_piece(&from, Some(Piece::new(Color::White, PieceType::Soldier, None)));
        b.set_piece(&to, Some(Piece::new(Color::Black, PieceType::Rook, None)));
        let see = see_capture(&b, &from, &to, false, &w());
        assert!(see > 0, "soldier capturing rook should be positive SEE: {see}");
    }

    #[test]
    fn rook_captures_soldier_is_negative() {
        let mut b = empty_board();
        let from = Position::new(0, 0);
        let to = Position::new(1, 0);
        b.set_piece(&from, Some(Piece::new(Color::White, PieceType::Rook, None)));
        b.set_piece(&to, Some(Piece::new(Color::Black, PieceType::Soldier, None)));
        let see = see_capture(&b, &from, &to, false, &w());
        assert!(see < 0, "rook capturing soldier should be negative SEE: {see}");
    }

    #[test]
    fn equal_trade_is_zero() {
        let mut b = empty_board();
        let from = Position::new(0, 0);
        let to = Position::new(1, 1);
        b.set_piece(&from, Some(Piece::new(Color::White, PieceType::Knight, None)));
        b.set_piece(&to, Some(Piece::new(Color::Black, PieceType::Bishop, None)));
        let see = see_capture(&b, &from, &to, false, &w());
        assert_eq!(see, 0, "knight vs bishop should be equal trade: {see}");
    }

    #[test]
    fn max_capturable_finds_king() {
        let b = Board::new(); // has all pieces
        let max = max_capturable_value(&b, Color::White, &w());
        assert_eq!(max, w().king_pts as i32, "king should be the most valuable piece");
    }
}
