//! Mobility evaluation: bonus based on the number of reachable empty squares.

use crate::board::{Color, Piece, Position};
use crate::engine::constants::MOBILITY_WEIGHT;
use crate::game::Game;
use crate::moves::MoveGenerator;
use std::collections::HashSet;

/// Count the number of reachable squares (captures + empty) for a given piece
/// type acting as the sole active piece at `pos`.
///
/// For a stack, returns the union of destinations for both components.
pub fn mobility_bonus(piece: &Piece, pos: Position, game: &Game) -> i32 {
    let count = reachable_square_count(piece, pos, game);
    count as i32 * MOBILITY_WEIGHT
}

/// Return the number of unique reachable squares for `piece` at `pos`.
fn reachable_square_count(piece: &Piece, pos: Position, game: &Game) -> usize {
    // We use the game's move generator filtered to the piece's color.
    // The generator already knows `white_to_move`; to handle both colors we
    // ask for that color's moves.
    let color_flag = piece.color == Color::White;
    let generator = MoveGenerator::new(&game.board, color_flag);
    let potential_moves = generator.get_moves(&pos);

    let mut destinations: HashSet<u8> = HashSet::new();
    for pm in &potential_moves {
        destinations.insert(pm.to.to_u8());
    }
    destinations.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};
    use crate::game::Game;

    #[test]
    fn mobility_rook_in_center_has_bonus() {
        let mut board = Board::empty();
        let white_rook = Piece::new(Color::White, PieceType::Rook, None);
        let white_king = Piece::new(Color::White, PieceType::King, None);
        let black_king = Piece::new(Color::Black, PieceType::King, None);
        board.set_piece(&Position::new(4, 4), Some(white_rook));
        board.set_piece(&Position::new(8, 8), Some(white_king));
        board.set_piece(&Position::new(0, 0), Some(black_king));
        let game = Game::from_board(board);
        let piece = game.board.get_piece(&Position::new(4, 4)).unwrap();
        let bonus = mobility_bonus(piece, Position::new(4, 4), &game);
        assert!(bonus > 0);
    }
}
