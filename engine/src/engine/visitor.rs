//! NodeVisitor trait and implementations for search tree instrumentation.
//!
//! Provides a decoupled interface for collecting search tree data
//! without polluting the core search logic.

use crate::board::Board;
use crate::game::Move;

/// Trait for observing search tree nodes during the alpha-beta search.
///
/// Implementations must be `Send` to allow use across Rayon threads.
pub trait NodeVisitor: Send {
    /// Called when a leaf node is evaluated.
    fn on_leaf(&mut self, path: &[Move], score: i32, board: &Board);

    /// Called when entering an internal node (optional, for full tree recording).
    fn on_node(&mut self, depth: u8, mv: Move, alpha: i32, beta: i32);
}

/// No-op visitor with zero overhead. Used in production.
pub struct NoopVisitor;

impl NodeVisitor for NoopVisitor {
    #[inline(always)]
    fn on_leaf(&mut self, _path: &[Move], _score: i32, _board: &Board) {}

    #[inline(always)]
    fn on_node(&mut self, _depth: u8, _mv: Move, _alpha: i32, _beta: i32) {}
}

/// Debug node for tree recording.
#[derive(Clone, Debug, serde::Serialize)]
pub struct DebugNode {
    pub depth: u8,
    pub path: Vec<String>,
    pub score: Option<i32>,
    pub alpha: i32,
    pub beta: i32,
    pub is_leaf: bool,
    pub board_hash: u64,
}

/// Records the full search tree for debugging.
/// Serializes to JSONL format (one node per line).
pub struct TreeRecorder {
    pub nodes: Vec<DebugNode>,
}

impl TreeRecorder {
    pub fn new() -> Self {
        TreeRecorder { nodes: Vec::new() }
    }

    /// Serialize all nodes as JSONL to stdout.
    pub fn dump_jsonl(&self) {
        for node in &self.nodes {
            if let Ok(json) = serde_json::to_string(node) {
                println!("{}", json);
            }
        }
    }
}

impl NodeVisitor for TreeRecorder {
    fn on_leaf(&mut self, path: &[Move], score: i32, board: &Board) {
        let binary = board.to_binary();
        let board_hash = ahash::RandomState::with_seeds(0, 0, 0, 0)
            .hash_one(&binary[..81]);
        self.nodes.push(DebugNode {
            depth: path.len() as u8,
            path: path.iter().map(|m| m.to_string()).collect(),
            score: Some(score),
            alpha: i32::MIN,
            beta: i32::MAX,
            is_leaf: true,
            board_hash,
        });
    }

    fn on_node(&mut self, depth: u8, mv: Move, alpha: i32, beta: i32) {
        self.nodes.push(DebugNode {
            depth,
            path: vec![mv.to_string()],
            score: None,
            alpha,
            beta,
            is_leaf: false,
            board_hash: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Position};

    #[test]
    fn noop_visitor_compiles() {
        let mut v = NoopVisitor;
        let mv = Move {
            from: Position::new(0, 0),
            to: Position::new(1, 1),
            unstack: false,
        };
        v.on_leaf(&[mv], 42, &Board::new());
        v.on_node(0, mv, -100, 100);
    }

    #[test]
    fn tree_recorder_records_leaf() {
        let mut r = TreeRecorder::new();
        let mv = Move {
            from: Position::new(0, 0),
            to: Position::new(1, 1),
            unstack: false,
        };
        r.on_leaf(&[mv], 42, &Board::new());
        assert_eq!(r.nodes.len(), 1);
        assert!(r.nodes[0].is_leaf);
        assert_eq!(r.nodes[0].score, Some(42));
    }

    #[test]
    fn tree_recorder_records_node() {
        let mut r = TreeRecorder::new();
        let mv = Move {
            from: Position::new(0, 0),
            to: Position::new(1, 1),
            unstack: false,
        };
        r.on_node(2, mv, -100, 100);
        assert_eq!(r.nodes.len(), 1);
        assert!(!r.nodes[0].is_leaf);
        assert_eq!(r.nodes[0].alpha, -100);
        assert_eq!(r.nodes[0].beta, 100);
    }
}
