//! Killer move table: stores up to N quiet moves that recently caused
//! beta cutoffs.  Used to improve move ordering.

use crate::engine::constants::KILLER_SLOTS;
use crate::moves::Move;

/// Per-depth killer move storage.
pub struct KillerTable {
    table: Vec<[Option<Move>; KILLER_SLOTS]>,
}

impl KillerTable {
    /// Create a killer table sized for depths 0 through `max_depth` (inclusive).
    pub fn new(max_depth: usize) -> Self {
        KillerTable {
            table: vec![[None; KILLER_SLOTS]; max_depth + 1],
        }
    }

    /// Record a new killer move at `depth`.
    pub fn store(&mut self, depth: usize, mv: Move) {
        let depth = depth.min(self.table.len() - 1);
        // Shift existing killers and insert the new one at slot 0.
        let entry = &mut self.table[depth];
        if entry[0] == Some(mv) {
            return; // Already the primary killer.
        }
        entry[KILLER_SLOTS - 1] = entry[KILLER_SLOTS - 2];
        entry[0] = Some(mv);
    }

    /// Return the killer move slots for `depth`.
    pub fn get(&self, depth: usize) -> &[Option<Move>] {
        let depth = depth.min(self.table.len() - 1);
        &self.table[depth]
    }

    /// Check whether `mv` is a killer at `depth`.
    pub fn is_killer(&self, depth: usize, mv: Move) -> bool {
        self.get(depth).contains(&Some(mv))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;
    use crate::engine::constants::MAX_KILLER_DEPTH;

    fn make_move(fx: usize, fy: usize, tx: usize, ty: usize) -> Move {
        Move {
            from: Position::new(fx, fy),
            to: Position::new(tx, ty),
            unstack: false,
        }
    }

    #[test]
    fn store_and_retrieve_killer() {
        let mut kt = KillerTable::new(MAX_KILLER_DEPTH);
        let mv = make_move(0, 0, 1, 1);
        kt.store(3, mv);
        assert!(kt.is_killer(3, mv));
    }

    #[test]
    fn second_killer_does_not_evict_first() {
        let mut kt = KillerTable::new(MAX_KILLER_DEPTH);
        let mv1 = make_move(0, 0, 1, 1);
        let mv2 = make_move(2, 2, 3, 3);
        kt.store(2, mv1);
        kt.store(2, mv2);
        assert!(kt.is_killer(2, mv1));
        assert!(kt.is_killer(2, mv2));
    }

    #[test]
    fn duplicate_killer_not_stored_twice() {
        let mut kt = KillerTable::new(MAX_KILLER_DEPTH);
        let mv = make_move(4, 4, 5, 5);
        kt.store(1, mv);
        kt.store(1, mv);
        // The same move stored twice should still only appear once.
        let killers = kt.get(1);
        let count = killers.iter().filter(|k| **k == Some(mv)).count();
        assert_eq!(count, 1);
    }
}
