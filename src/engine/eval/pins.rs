//! Pinned-piece detection via ray casting.
//!
//! A piece is considered "pinned" when it sits between its own King and an
//! enemy long-range attacker along the same ray.  Pinned pieces receive a
//! material malus proportional to their base value.

use crate::board::{Board, Color, PieceType, Position};
use crate::engine::constants::PINNED_PENALTY_FACTOR;
use crate::engine::eval::material::base_value;

/// Compute the total pinned-piece malus for `color` on `board`.
///
/// Returns a non-negative value that should be SUBTRACTED from the score of
/// the pinned side (i.e., added as a bonus to the opponent).
pub fn pinned_malus(board: &Board, color: Color) -> i32 {
    let king_pos = match find_king(board, color) {
        Some(p) => p,
        None => return 0,
    };

    let mut total_malus = 0i32;

    // Check orthogonal rays for Rook / Paladin pins.
    for &(dx, dy) in &Position::ORTHOGONAL_MOVES {
        total_malus += ray_pin_malus(board, king_pos, dx, dy, color, false);
    }

    // Check diagonal rays for Bishop / Guard pins.
    for &(dx, dy) in &Position::DIAGONAL_MOVES {
        total_malus += ray_pin_malus(board, king_pos, dx, dy, color, true);
    }

    total_malus
}

/// Find the King's position for `color`.
fn find_king(board: &Board, color: Color) -> Option<Position> {
    for (pos, piece) in board.pieces() {
        if piece.color == color && piece.is_king() {
            return Some(pos);
        }
    }
    None
}

/// Walk one ray from `king_pos` in direction `(dx, dy)`.
///
/// - `diagonal`: true for diagonal rays (Bishop/Guard), false for orthogonal
///   (Rook/Paladin).
///
/// Returns the malus value if a pin is detected on this ray.
fn ray_pin_malus(
    board: &Board,
    king_pos: Position,
    dx: isize,
    dy: isize,
    friendly_color: Color,
    diagonal: bool,
) -> i32 {
    let mut pin_candidate: Option<(Position, i32)> = None; // (pos, base_value)

    let mut dist = 1isize;
    loop {
        let nx = king_pos.x as isize + dx * dist;
        let ny = king_pos.y as isize + dy * dist;
        if !Position::validate(nx, ny) {
            break;
        }
        let pos = Position::new(nx as usize, ny as usize);

        match board.get_piece(&pos) {
            None => {
                dist += 1;
                continue;
            }
            Some(piece) => {
                if piece.color == friendly_color {
                    if pin_candidate.is_some() {
                        // Second friendly piece on the ray — no pin possible.
                        break;
                    }
                    let bv = base_value(piece.bottom)
                        + piece.top.map(base_value).unwrap_or(0);
                    pin_candidate = Some((pos, bv));
                    dist += 1;
                    continue;
                }
                // Enemy piece.
                if let Some((_, bv)) = pin_candidate {
                    if can_pin_along_ray(piece, dist, diagonal) {
                        let malus = (PINNED_PENALTY_FACTOR * bv as f32) as i32;
                        return malus;
                    }
                }
                break;
            }
        }
    }

    0
}

/// Return true if `piece` can create a pin along this ray type at `dist`
/// squares from the pinned piece.
fn can_pin_along_ray(piece: &crate::board::Piece, dist: isize, diagonal: bool) -> bool {
    let bottom_pins = match piece.bottom {
        PieceType::Bishop if diagonal => true,
        PieceType::Rook if !diagonal => true,
        PieceType::Ballista if !diagonal => true,
        PieceType::Guard if diagonal && dist <= 2 => true,
        PieceType::Paladin if !diagonal && dist <= 2 => true,
        _ => false,
    };
    if bottom_pins {
        return true;
    }
    // Also check the top piece of a stack.
    if let Some(top) = piece.top {
        match top {
            PieceType::Bishop if diagonal => return true,
            PieceType::Rook if !diagonal => return true,
            PieceType::Ballista if !diagonal => return true,
            PieceType::Guard if diagonal && dist <= 2 => return true,
            PieceType::Paladin if !diagonal && dist <= 2 => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Piece};

    fn setup_pin(
        king_pos: Position,
        friendly_piece: Piece,
        friendly_pos: Position,
        enemy_piece: Piece,
        enemy_pos: Position,
    ) -> Board {
        let mut board = Board::empty();
        board.set_piece(&king_pos, Some(Piece::new(friendly_piece.color, PieceType::King, None)));
        board.set_piece(&friendly_pos, Some(friendly_piece));
        board.set_piece(&enemy_pos, Some(enemy_piece));
        board
    }

    #[test]
    fn test_pinned_rook_orthogonal_ray() {
        // White King at A1 (x=0,y=8), White Guard at A2 (x=0,y=7), Black Rook at A9 (x=0,y=0)
        let king_pos = Position::new(0, 8);
        let friendly_pos = Position::new(0, 7);
        let enemy_pos = Position::new(0, 0);

        let friendly_piece = Piece::new(Color::White, PieceType::Guard, None);
        let enemy_piece = Piece::new(Color::Black, PieceType::Rook, None);

        let board = setup_pin(king_pos, friendly_piece, friendly_pos, enemy_piece, enemy_pos);
        let malus = pinned_malus(&board, Color::White);
        assert!(malus > 0, "Expected pin malus but got 0");
    }

    #[test]
    fn test_pinned_bishop_diagonal_ray() {
        // White King at A1 (0,8), White Soldier at B2 (1,7), Black Bishop at C3 (2,6)
        let king_pos = Position::new(0, 8);
        let friendly_pos = Position::new(1, 7);
        let enemy_pos = Position::new(2, 6);

        let friendly_piece = Piece::new(Color::White, PieceType::Soldier, None);
        let enemy_piece = Piece::new(Color::Black, PieceType::Bishop, None);

        let board = setup_pin(king_pos, friendly_piece, friendly_pos, enemy_piece, enemy_pos);
        let malus = pinned_malus(&board, Color::White);
        assert!(malus > 0, "Expected diagonal pin malus but got 0");
    }

    #[test]
    fn test_no_pin_when_blocked() {
        // White King at A1, White Soldier at A2, White Rook at A3, Black Rook at A9
        // The Black Rook is blocked by two friendly pieces — no pin.
        let mut board = Board::empty();
        board.set_piece(&Position::new(0, 8), Some(Piece::new(Color::White, PieceType::King, None)));
        board.set_piece(&Position::new(0, 7), Some(Piece::new(Color::White, PieceType::Soldier, None)));
        board.set_piece(&Position::new(0, 6), Some(Piece::new(Color::White, PieceType::Rook, None)));
        board.set_piece(&Position::new(0, 0), Some(Piece::new(Color::Black, PieceType::Rook, None)));
        let malus = pinned_malus(&board, Color::White);
        assert_eq!(malus, 0, "No pin should be detected with two blockers");
    }
}
