//! Zobrist hashing for Keres board positions.
//!
//! Provides a fast, collision-resistant hash for board states using
//! XOR of pre-computed random numbers indexed by (square, piece-encoding).

use crate::board::{Board, Position, BOARD_SIZE};
use std::sync::OnceLock;

const NUM_PIECE_CODES: usize = 128; // 7-bit piece encoding

struct ZobristKeys {
    piece_square: [[u64; NUM_PIECE_CODES]; BOARD_SIZE],
    side_to_move: u64,
}

static KEYS: OnceLock<ZobristKeys> = OnceLock::new();

fn init_keys() -> ZobristKeys {
    // Deterministic xorshift64 PRNG for reproducibility.
    let mut state: u64 = 0x12345678_DEADBEEF;
    let mut next = || -> u64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    let mut piece_square = [[0u64; NUM_PIECE_CODES]; BOARD_SIZE];
    for sq in 0..BOARD_SIZE {
        for pc in 0..NUM_PIECE_CODES {
            piece_square[sq][pc] = next();
        }
    }

    ZobristKeys {
        piece_square,
        side_to_move: next(),
    }
}

/// Compute the Zobrist hash for a board position.
pub fn hash_board(board: &Board) -> u64 {
    let keys = KEYS.get_or_init(init_keys);
    let mut h: u64 = 0;
    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            h ^= keys.piece_square[sq][piece.to_u8() as usize];
        }
    }
    if board.is_white_to_move() {
        h ^= keys.side_to_move;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn hash_is_deterministic() {
        let b = Board::new();
        let h1 = hash_board(&b);
        let h2 = hash_board(&b);
        assert_eq!(h1, h2);
    }

    #[test]
    fn different_side_to_move_gives_different_hash() {
        let b1 = Board::new();
        let mut b2 = Board::new();
        b2.set_white_to_move(false);
        assert_ne!(hash_board(&b1), hash_board(&b2));
    }

    #[test]
    fn hash_is_nonzero_for_starting_position() {
        let h = hash_board(&Board::new());
        assert_ne!(h, 0, "hash of starting position should be nonzero");
    }
}
