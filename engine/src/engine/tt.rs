//! Lock-free transposition table for the Keres search engine.
//!
//! Uses XOR-based verification to detect torn reads from concurrent
//! writes. Each entry is stored as two `AtomicU64` values:
//! `key_xor_data` and `data`. On read, `key_xor_data ^ data` must
//! equal the original hash to be valid.

use std::sync::atomic::{AtomicU64, Ordering};

/// Bound type stored in transposition table entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Exact = 0,
    LowerBound = 1,
    UpperBound = 2,
}

/// A transposition table entry.
#[derive(Clone, Copy, Debug)]
pub struct TtEntry {
    pub depth: i8,
    pub bound: Bound,
    pub score: i32,
    pub best_move: u16, // Move encoded as u16; 0 = no move
}

impl TtEntry {
    fn pack(&self) -> u64 {
        let d = self.depth as u8 as u64;
        let b = self.bound as u64;
        let m = self.best_move as u64;
        let s = self.score as u32 as u64;
        d | (b << 8) | (m << 10) | (s << 26)
    }

    fn unpack(data: u64) -> Self {
        let depth = data as u8 as i8;
        let bound = match (data >> 8) & 3 {
            0 => Bound::Exact,
            1 => Bound::LowerBound,
            _ => Bound::UpperBound,
        };
        let best_move = ((data >> 10) & 0xFFFF) as u16;
        let score = ((data >> 26) & 0xFFFFFFFF) as u32 as i32;
        TtEntry { depth, bound, score, best_move }
    }
}

/// Thread-safe, lock-free transposition table.
pub struct TranspositionTable {
    keys: Vec<AtomicU64>,
    data: Vec<AtomicU64>,
    mask: usize,
}

impl TranspositionTable {
    /// Create a TT with the given number of entries (rounded up to power of 2).
    pub fn new(size: usize) -> Self {
        let size = size.next_power_of_two();
        let mut keys = Vec::with_capacity(size);
        let mut data = Vec::with_capacity(size);
        for _ in 0..size {
            keys.push(AtomicU64::new(0));
            data.push(AtomicU64::new(0));
        }
        TranspositionTable { keys, data, mask: size - 1 }
    }

    /// Probe the TT. Returns the entry if the stored hash matches.
    pub fn probe(&self, hash: u64) -> Option<TtEntry> {
        let idx = (hash as usize) & self.mask;
        let stored_key = self.keys[idx].load(Ordering::Relaxed);
        let stored_data = self.data[idx].load(Ordering::Relaxed);
        if stored_key ^ stored_data == hash {
            Some(TtEntry::unpack(stored_data))
        } else {
            None
        }
    }

    /// Store an entry. Uses always-replace policy.
    pub fn store(&self, hash: u64, entry: TtEntry) {
        let idx = (hash as usize) & self.mask;
        let packed = entry.pack();
        self.keys[idx].store(hash ^ packed, Ordering::Relaxed);
        self.data[idx].store(packed, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_probe_roundtrip() {
        let tt = TranspositionTable::new(1024);
        let entry = TtEntry {
            depth: 4,
            bound: Bound::Exact,
            score: 150,
            best_move: 0x1234,
        };
        tt.store(0xDEADBEEF, entry);
        let probed = tt.probe(0xDEADBEEF).expect("entry should be found");
        assert_eq!(probed.depth, 4);
        assert_eq!(probed.bound, Bound::Exact);
        assert_eq!(probed.score, 150);
        assert_eq!(probed.best_move, 0x1234);
    }

    #[test]
    fn probe_miss_returns_none() {
        let tt = TranspositionTable::new(1024);
        assert!(tt.probe(0xDEADBEEF).is_none());
    }

    #[test]
    fn negative_score_roundtrips() {
        let tt = TranspositionTable::new(1024);
        let entry = TtEntry {
            depth: 3,
            bound: Bound::LowerBound,
            score: -500,
            best_move: 0,
        };
        tt.store(42, entry);
        let probed = tt.probe(42).unwrap();
        assert_eq!(probed.score, -500);
        assert_eq!(probed.bound, Bound::LowerBound);
    }

    #[test]
    fn upper_bound_roundtrips() {
        let tt = TranspositionTable::new(1024);
        let entry = TtEntry {
            depth: 7,
            bound: Bound::UpperBound,
            score: 9999,
            best_move: 0xABCD,
        };
        tt.store(12345, entry);
        let probed = tt.probe(12345).unwrap();
        assert_eq!(probed.depth, 7);
        assert_eq!(probed.bound, Bound::UpperBound);
        assert_eq!(probed.score, 9999);
        assert_eq!(probed.best_move, 0xABCD);
    }
}
