//! Repetition detection for the search, Stockfish-style.
//!
//! A line that comes back to a position scores as a draw (`0`) when either
//!
//! - the position already occurred **inside the search tree**, after the root
//!   (the line is cycling: one recurrence is enough to know it can be
//!   repeated into a draw), or
//! - the position already occurred **twice** in the real game up to and
//!   including the root, so reaching it again is the third occurrence — the
//!   real game rule (`game_over::REPETITION_DRAW_OCCURRENCES`) ends the game
//!   there.
//!
//! A position seen only once up to the root is *not* a draw: the game goes on
//! after its second occurrence, and the search must not flee from it.
//!
//! "Same position" is [`Game::board_hash`] — the same key the game rule counts
//! with — so the search and `Game::make` can never disagree about a real third
//! occurrence.

use crate::game::Game;
use crate::game_over::REPETITION_DRAW_OCCURRENCES;
use std::collections::{HashMap, HashSet};

/// How many times each position occurred in the real game up to and
/// including the root (the root is the position being searched).
pub type PositionCounts = HashMap<u64, usize>;

/// Count the positions of `game`'s real history and its current position
/// (the search root). Built once per root search and shared by every thread.
pub fn position_counts(game: &Game) -> PositionCounts {
    let mut counts = PositionCounts::with_capacity(game.position_history().len() + 1);
    for &hash in game.position_history() {
        *counts.entry(hash).or_insert(0) += 1;
    }
    *counts.entry(game.board_hash()).or_insert(0) += 1;
    counts
}

/// Repetition state for a single search thread: the real-game counts (fixed
/// for the whole search) plus the positions on the current search path.
pub struct LoopDetector<'a> {
    before_root: Option<&'a PositionCounts>,
    path: HashSet<u64>,
}

impl LoopDetector<'static> {
    /// A detector that knows no real-game history.
    pub fn new() -> Self {
        LoopDetector {
            before_root: None,
            path: HashSet::new(),
        }
    }
}

impl<'a> LoopDetector<'a> {
    /// A detector seeded with the real game's position counts (see
    /// [`position_counts`]).
    pub fn with_history(before_root: &'a PositionCounts) -> Self {
        LoopDetector {
            before_root: Some(before_root),
            path: HashSet::new(),
        }
    }

    /// Enter the position `hash` on the search path.
    ///
    /// Returns `true` if reaching it is a draw by repetition (see the module
    /// docs); the position is then *not* pushed, and the caller must not
    /// [`pop`](Self::pop) it. Returns `false` after pushing it; the caller
    /// pops it when leaving the node.
    pub fn push(&mut self, hash: u64) -> bool {
        if !self.path.insert(hash) {
            return true;
        }
        let earlier = self
            .before_root
            .and_then(|counts| counts.get(&hash))
            .copied()
            .unwrap_or(0);
        if earlier + 1 >= REPETITION_DRAW_OCCURRENCES {
            self.path.remove(&hash);
            return true;
        }
        false
    }

    /// Remove a position pushed by a `push` that returned `false`.
    pub fn pop(&mut self, hash: u64) {
        self.path.remove(&hash);
    }

    /// Whether `hash` is currently on the search path.
    pub fn contains(&self, hash: u64) -> bool {
        self.path.contains(&hash)
    }
}

impl Default for LoopDetector<'static> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(entries: &[(u64, usize)]) -> PositionCounts {
        entries.iter().copied().collect()
    }

    #[test]
    fn new_hash_is_not_a_draw() {
        let mut ld = LoopDetector::new();
        assert!(!ld.push(42));
    }

    #[test]
    fn a_recurrence_inside_the_tree_is_a_draw() {
        let mut ld = LoopDetector::new();
        assert!(!ld.push(42));
        assert!(ld.push(42));
    }

    #[test]
    fn a_draw_does_not_unmark_the_earlier_occurrence() {
        let mut ld = LoopDetector::new();
        assert!(!ld.push(42));
        assert!(ld.push(42));
        // The draw returned without pushing; the first visit is still on
        // the path, so a sibling branch reaching 42 is a draw too.
        assert!(ld.contains(42));
        assert!(ld.push(42));
    }

    #[test]
    fn pop_removes_hash() {
        let mut ld = LoopDetector::new();
        assert!(!ld.push(42));
        ld.pop(42);
        assert!(!ld.contains(42));
        assert!(!ld.push(42));
    }

    #[test]
    fn different_hashes_do_not_conflict() {
        let mut ld = LoopDetector::new();
        assert!(!ld.push(1));
        assert!(!ld.push(2));
    }

    #[test]
    fn a_position_seen_once_before_the_root_is_not_a_draw() {
        let history = counts(&[(7, 1)]);
        let mut ld = LoopDetector::with_history(&history);
        assert!(!ld.push(7), "this would be only the 2nd occurrence");
        assert!(ld.push(7), "…but a second visit inside the tree is a cycle");
    }

    #[test]
    fn a_position_seen_twice_up_to_the_root_is_a_draw() {
        let history = counts(&[(7, 2), (8, 3)]);
        let mut ld = LoopDetector::with_history(&history);
        assert!(ld.push(7), "this would be the 3rd occurrence");
        assert!(ld.push(8));
        assert!(!ld.contains(7), "a draw leaves nothing on the path");
        assert!(!ld.push(9));
    }
}
