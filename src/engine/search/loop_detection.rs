//! Loop detection: maintains a stack of board hashes along the current search
//! path and detects repeated positions (cycles → drawn / 0 score).

use std::collections::HashSet;

/// Path-hash stack for a single search thread.
pub struct LoopDetector {
    seen: HashSet<u64>,
}

impl LoopDetector {
    /// Create a new, empty loop detector.
    pub fn new() -> Self {
        LoopDetector {
            seen: HashSet::new(),
        }
    }

    /// Push a board hash onto the stack.  Returns `true` if the hash was
    /// already present (i.e., this position is a cycle/draw).
    pub fn push(&mut self, hash: u64) -> bool {
        !self.seen.insert(hash)
    }

    /// Remove a hash from the stack (called after returning from a branch).
    pub fn pop(&mut self, hash: u64) {
        self.seen.remove(&hash);
    }

    /// Check without modifying whether `hash` is already on the path.
    pub fn contains(&self, hash: u64) -> bool {
        self.seen.contains(&hash)
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_hash_is_not_duplicate() {
        let mut ld = LoopDetector::new();
        assert!(!ld.push(42));
    }

    #[test]
    fn repeated_hash_is_detected() {
        let mut ld = LoopDetector::new();
        ld.push(42);
        assert!(ld.push(42));
    }

    #[test]
    fn pop_removes_hash() {
        let mut ld = LoopDetector::new();
        ld.push(42);
        ld.pop(42);
        assert!(!ld.contains(42));
    }

    #[test]
    fn different_hashes_do_not_conflict() {
        let mut ld = LoopDetector::new();
        ld.push(1);
        assert!(!ld.push(2));
    }
}
