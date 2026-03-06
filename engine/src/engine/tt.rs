//! Transposition table for caching search results.

use crate::engine::types::BoundType;
use crate::moves::Move;

/// A single entry in the transposition table.
#[derive(Clone, Debug)]
pub struct TtEntry {
    /// Zobrist hash of the board position.
    pub hash: u64,
    /// Remaining depth at which this entry was computed (MAX_DEPTH - depth).
    pub remaining_depth: u8,
    /// Stored score (NegaMax-relative for the side that stored it).
    pub score: i32,
    /// Bound type for the stored score.
    pub bound_type: BoundType,
    /// Best move found when this entry was stored.
    pub best_move: Option<Move>,
}

/// Fixed-size transposition table with a simple replacement policy.
pub struct TranspositionTable {
    entries: Vec<Option<TtEntry>>,
    size: usize,
}

impl TranspositionTable {
    /// Create a new transposition table with the given number of slots.
    /// `size` is rounded down to a power of two internally.
    pub fn new(size: usize) -> Self {
        let actual_size = size.next_power_of_two();
        TranspositionTable {
            entries: vec![None; actual_size],
            size: actual_size,
        }
    }

    fn index(&self, hash: u64) -> usize {
        (hash as usize) & (self.size - 1)
    }

    /// Look up a position hash.  Returns a reference to the entry if the hash matches.
    pub fn get(&self, hash: u64) -> Option<&TtEntry> {
        let idx = self.index(hash);
        self.entries[idx].as_ref().filter(|e| e.hash == hash)
    }

    /// Store an entry, replacing an existing one only if the new entry has
    /// greater or equal `remaining_depth`.
    pub fn store(
        &mut self,
        hash: u64,
        remaining_depth: u8,
        score: i32,
        bound_type: BoundType,
        best_move: Option<Move>,
    ) {
        let idx = self.index(hash);
        let replace = match &self.entries[idx] {
            None => true,
            Some(existing) => remaining_depth >= existing.remaining_depth,
        };
        if replace {
            self.entries[idx] = Some(TtEntry {
                hash,
                remaining_depth,
                score,
                bound_type,
                best_move,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;
    use crate::board::Board;
    use crate::engine::constants::TT_SIZE;

    fn make_hash() -> u64 {
        let game = Game::new();
        game.zobrist_hash()
    }

    #[test]
    fn tt_store_and_retrieve() {
        let mut tt = TranspositionTable::new(TT_SIZE);
        let hash = make_hash();
        tt.store(hash, 3, 42, BoundType::Exact, None);
        let entry = tt.get(hash).expect("entry should be present");
        assert_eq!(entry.score, 42);
        assert_eq!(entry.bound_type, BoundType::Exact);
        assert_eq!(entry.remaining_depth, 3);
    }

    #[test]
    fn tt_replacement_policy_replaces_on_equal_depth() {
        let mut tt = TranspositionTable::new(TT_SIZE);
        let hash = make_hash();
        tt.store(hash, 2, 10, BoundType::UpperBound, None);
        tt.store(hash, 2, 20, BoundType::Exact, None);
        let entry = tt.get(hash).expect("entry should be present");
        assert_eq!(entry.score, 20);
    }

    #[test]
    fn tt_replacement_policy_keeps_deeper_entry() {
        let mut tt = TranspositionTable::new(TT_SIZE);
        let hash = make_hash();
        tt.store(hash, 5, 99, BoundType::Exact, None);
        tt.store(hash, 2, 1, BoundType::Exact, None);
        let entry = tt.get(hash).expect("entry should be present");
        assert_eq!(entry.score, 99);
    }

    #[test]
    fn board_hash_is_deterministic() {
        let h1 = make_hash();
        let h2 = make_hash();
        assert_eq!(h1, h2);
    }

    #[test]
    fn board_hash_differs_for_different_boards() {
        let game1 = Game::new();
        let game2 = Game::from_board(Board::empty());
        assert_ne!(game1.zobrist_hash(), game2.zobrist_hash());
    }

    #[test]
    fn zobrist_hash_restored_after_unmake() {
        let mut game = Game::new();
        let original_hash = game.zobrist_hash();
        let mv = crate::moves::Move {
            from: crate::board::Position::new(0, 6),
            to: crate::board::Position::new(0, 5),
            unstack: false,
        };
        let undo = game.make(&mv);
        assert_ne!(game.zobrist_hash(), original_hash, "hash must change after make");
        game.unmake(&mv, undo);
        assert_eq!(game.zobrist_hash(), original_hash, "hash must be restored after unmake");
    }

    #[test]
    fn zobrist_hash_incremental_matches_from_scratch() {
        use crate::engine::zobrist;
        let mut game = Game::new();
        let mv = crate::moves::Move {
            from: crate::board::Position::new(0, 6),
            to: crate::board::Position::new(0, 5),
            unstack: false,
        };
        game.make(&mv);
        let incremental = game.zobrist_hash();
        let from_scratch = zobrist::compute_hash_from_board(&game.board, game.is_white_to_move());
        assert_eq!(
            incremental, from_scratch,
            "incremental hash must match full recomputation"
        );
    }
}
