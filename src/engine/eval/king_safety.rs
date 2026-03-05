//! King-safety evaluation: mobility term for the King.

use crate::board::{Board, Color, Position};
use crate::engine::constants::KING_MOBILITY_WEIGHT;
use crate::game::Game;
use crate::moves::MoveGenerator;

/// Compute the king-mobility term contribution to the absolute evaluation.
///
/// Returns `king_mobility_white_score - king_mobility_black_score`, where each
/// component is `king_move_count × KING_MOBILITY_WEIGHT`.
pub fn king_mobility_term(game: &Game) -> i32 {
    let white_mob = king_mobility_for(game, Color::White);
    let black_mob = king_mobility_for(game, Color::Black);
    (white_mob - black_mob) * KING_MOBILITY_WEIGHT
}

/// Count the number of moves available to the King of `color`.
pub fn king_mobility_for(game: &Game, color: Color) -> i32 {
    let king_pos = match find_king(&game.board, color) {
        Some(p) => p,
        None => return 0,
    };
    let color_flag = color == Color::White;
    let generator = MoveGenerator::new(&game.board, color_flag);
    let moves = generator.get_moves(&king_pos);
    moves.len() as i32
}

fn find_king(board: &Board, color: Color) -> Option<Position> {
    for (pos, piece) in board.pieces() {
        if piece.color == color && piece.is_king() {
            return Some(pos);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};
    use crate::game::Game;

    #[test]
    fn king_in_center_has_more_mobility_than_corner() {
        let mut board_center = Board::empty();
        let mut board_corner = Board::empty();
        let white_king = Piece::new(Color::White, PieceType::King, None);
        let black_king = Piece::new(Color::Black, PieceType::King, None);

        board_center.set_piece(&Position::new(4, 4), Some(white_king));
        board_center.set_piece(&Position::new(0, 0), Some(black_king));
        board_corner.set_piece(&Position::new(0, 8), Some(white_king));
        board_corner.set_piece(&Position::new(8, 0), Some(black_king));

        let game_center = Game::from_board(board_center);
        let game_corner = Game::from_board(board_corner);

        let mob_center = king_mobility_for(&game_center, Color::White);
        let mob_corner = king_mobility_for(&game_corner, Color::White);

        assert!(
            mob_center > mob_corner,
            "Center king should have more mobility: center={} corner={}",
            mob_center,
            mob_corner
        );
    }
}
