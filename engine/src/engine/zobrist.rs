//! Zobrist hashing for incremental board position hashing.
//!
//! The Zobrist table maps (piece_encoding, square) pairs to random u64 values.
//! Piece encoding uses the same 7-bit scheme as `Piece::to_u8()`:
//!   - bit 6: color (0=Black, 1=White)
//!   - bits 3-5: top piece code (0=none)
//!   - bits 0-2: bottom piece code
//!
//! The table is lazily initialized at startup with a fixed seed for
//! reproducibility. `ZOBRIST_SIDE_TO_MOVE` is XORed whenever it is Black's
//! turn to move, so the hash reflects whose turn it is.

use std::sync::OnceLock;

use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// Number of squares on the board (9×9).
const NUM_SQUARES: usize = 81;

/// Number of distinct piece encodings (7 bits → 0..=127).
const NUM_PIECE_ENCODINGS: usize = 128;

/// Flat Zobrist table: indexed by `piece_byte as usize * NUM_SQUARES + square`.
static ZOBRIST_TABLE: OnceLock<Box<[u64; NUM_PIECE_ENCODINGS * NUM_SQUARES]>> = OnceLock::new();

/// Hash XORed when it is Black's turn to move (flip on each half-move).
static ZOBRIST_SIDE_TO_MOVE: OnceLock<u64> = OnceLock::new();

fn init_table() -> (Box<[u64; NUM_PIECE_ENCODINGS * NUM_SQUARES]>, u64) {
    // Fixed seed for reproducibility across engine restarts.
    let mut rng = StdRng::seed_from_u64(0xCAFE_BABE_DEAD_BEEF);
    let mut table = Box::new([0u64; NUM_PIECE_ENCODINGS * NUM_SQUARES]);
    for entry in table.iter_mut() {
        *entry = rng.gen();
    }
    let side_to_move = rng.gen();
    (table, side_to_move)
}

/// Return a reference to the global Zobrist table, initializing it on first call.
fn table() -> &'static [u64; NUM_PIECE_ENCODINGS * NUM_SQUARES] {
    ZOBRIST_TABLE.get_or_init(|| {
        let (t, stm) = init_table();
        ZOBRIST_SIDE_TO_MOVE.get_or_init(|| stm);
        t
    })
}

/// The hash constant for "Black to move".  XOR this into the hash whenever the
/// side to move changes from White to Black or vice-versa.
pub fn side_to_move_hash() -> u64 {
    // Ensure the table (and thus the side-to-move value) is initialized.
    table();
    *ZOBRIST_SIDE_TO_MOVE.get().expect("zobrist side_to_move not initialized")
}

/// Look up the Zobrist value for a piece encoding at a given square index (0–80).
///
/// `piece_byte` is the raw byte produced by `Piece::to_u8()`.
/// `square` is the absolute board index `Position::to_absolute()`.
///
/// Empty squares (`piece_byte == 0`) are never passed to this function; their
/// hash contribution is defined to be 0 by convention.  Only call this for
/// non-empty squares.
#[inline(always)]
pub fn piece_hash(piece_byte: u8, square: usize) -> u64 {
    debug_assert!(square < NUM_SQUARES, "square index out of range");
    debug_assert!(piece_byte != 0, "piece_hash called with empty-square encoding (0); empty squares do not contribute to the hash");
    table()[piece_byte as usize * NUM_SQUARES + square]
}

/// Compute a full Zobrist hash from scratch for the given board state.
///
/// Empty squares contribute 0 to the hash (they are skipped).
/// This is O(81) and is used only during initialization.  During search the
/// hash is kept incrementally via XOR updates in `Game::make_inner`.
pub fn compute_hash_from_board(
    board: &crate::board::Board,
    white_to_move: bool,
) -> u64 {
    let mut hash = 0u64;
    for sq in 0..NUM_SQUARES {
        let pos = crate::board::Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            // piece.to_u8() is always non-zero for a real piece.
            hash ^= piece_hash(piece.to_u8(), sq);
        }
        // Empty squares (piece_byte == 0) contribute 0; they are skipped.
    }
    if !white_to_move {
        hash ^= side_to_move_hash();
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};

    #[test]
    fn hash_is_deterministic() {
        let board = Board::new();
        let h1 = compute_hash_from_board(&board, true);
        let h2 = compute_hash_from_board(&board, true);
        assert_eq!(h1, h2);
    }

    #[test]
    fn empty_board_hash_is_zero_white_to_move() {
        let board = Board::empty();
        let h = compute_hash_from_board(&board, true);
        // No pieces → XOR of nothing; white to move does not XOR side_to_move.
        assert_eq!(h, 0);
    }

    #[test]
    fn side_to_move_flips_hash() {
        let board = Board::new();
        let h_white = compute_hash_from_board(&board, true);
        let h_black = compute_hash_from_board(&board, false);
        assert_ne!(h_white, h_black);
        assert_eq!(h_white ^ h_black, side_to_move_hash());
    }

    #[test]
    fn different_boards_have_different_hashes() {
        let board1 = Board::new();
        let board2 = Board::empty();
        assert_ne!(
            compute_hash_from_board(&board1, true),
            compute_hash_from_board(&board2, true)
        );
    }

    #[test]
    fn piece_hash_xor_inverts() {
        // XORing twice should cancel out (Zobrist property).
        let mut h: u64 = 0x1234_5678_9ABC_DEF0;
        let piece = Piece::new(Color::White, PieceType::Rook, None);
        let sq = 42usize;
        h ^= piece_hash(piece.to_u8(), sq);
        h ^= piece_hash(piece.to_u8(), sq);
        assert_eq!(h, 0x1234_5678_9ABC_DEF0);
    }

    #[test]
    fn stacked_piece_has_unique_hash() {
        // A stack at a square should differ from the constituent single pieces.
        let sq = 10usize;
        let soldier = Piece::new(Color::White, PieceType::Soldier, None);
        let guard = Piece::new(Color::White, PieceType::Guard, None);
        let stack = Piece::new(Color::White, PieceType::Soldier, Some(PieceType::Guard));
        // stack hash ≠ soldier hash ≠ guard hash (very high probability with random table)
        let h_stack = piece_hash(stack.to_u8(), sq);
        let h_soldier = piece_hash(soldier.to_u8(), sq);
        let h_guard = piece_hash(guard.to_u8(), sq);
        assert_ne!(h_stack, h_soldier);
        assert_ne!(h_stack, h_guard);
        assert_ne!(h_stack, h_soldier ^ h_guard);
    }

    #[test]
    fn king_at_different_squares_gives_different_hashes() {
        let king = Piece::new(Color::Black, PieceType::King, None);
        let h1 = piece_hash(king.to_u8(), 0);
        let h2 = piece_hash(king.to_u8(), 1);
        assert_ne!(h1, h2);
    }

    #[test]
    fn incrementally_matches_from_scratch() {
        // Verify that adding a piece incrementally gives the same result as
        // computing from scratch.
        let mut board = Board::empty();
        board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::White, PieceType::Rook, None)),
        );
        let from_scratch = compute_hash_from_board(&board, true);

        // Incremental: start from empty hash (empty board, white to move = 0) and XOR the piece in.
        let incremental = piece_hash(
            Piece::new(Color::White, PieceType::Rook, None).to_u8(),
            Position::new(4, 4).to_absolute(),
        );
        assert_eq!(from_scratch, incremental);
    }
}
