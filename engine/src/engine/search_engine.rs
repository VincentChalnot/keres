//! Search result types and SearchEngine trait for the Keres engine.
//!
//! Defines the shared result format across all search implementations
//! and the trait that all engines must implement.

use crate::board::Board;
use crate::game::Move;
use super::search_config::SearchConfig;

/// A single principal variation line.
#[derive(Clone, Debug)]
pub struct PVLine {
    /// The root move (first move in the chain).
    pub root_move: Move,
    /// Complete move chain from root to leaf.
    pub moves: Vec<Move>,
    /// Minimax score from the root player's perspective.
    pub score: i32,
    /// Board state at the leaf position.
    pub leaf_board: Board,
}

/// Search result returned by all engine implementations.
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// Best move found.
    pub best_move: Move,
    /// Score of the best move.
    pub score: i32,
    /// Search depth used.
    pub depth: u8,
    /// Total nodes visited during search.
    pub nodes_visited: u64,
    /// Top-K principal variations (sorted by score, best first).
    pub top_moves: Vec<PVLine>,
}

/// Runtime statistics for a search.
#[derive(Clone, Debug, Default)]
pub struct SearchStats {
    /// Total nodes visited.
    pub nodes_visited: u64,
    /// TT hits at leaf nodes.
    pub tt_hits: u64,
    /// Total leaf nodes evaluated.
    pub tt_probes: u64,
    /// Time elapsed in seconds.
    pub elapsed_secs: f64,
}

impl SearchStats {
    /// TT hit rate as a percentage.
    pub fn tt_hit_rate(&self) -> f64 {
        if self.tt_probes == 0 { 0.0 } else { self.tt_hits as f64 / self.tt_probes as f64 * 100.0 }
    }

    /// Nodes per second.
    pub fn nps(&self) -> f64 {
        if self.elapsed_secs <= 0.0 { 0.0 } else { self.nodes_visited as f64 / self.elapsed_secs }
    }
}

/// Trait implemented by all search engines.
pub trait SearchEngine {
    /// Run a search from the given position with the given config.
    fn search(&mut self, board: &Board, config: &SearchConfig) -> SearchResult;

    /// Get the total nodes visited (across all searches).
    fn nodes_visited(&self) -> u64;

    /// Reset all statistics counters.
    fn reset_stats(&mut self);
}

/// Stage 2 mock: simply returns the best move from Stage 1.
pub struct MockStage2;

impl SearchEngine for MockStage2 {
    fn search(&mut self, _board: &Board, _config: &SearchConfig) -> SearchResult {
        // Stage 2 is not yet implemented — this is a placeholder.
        // In production, Stage 1 results are passed to Stage 2 via config
        // or a separate entry point.
        unimplemented!("Stage 2 not yet implemented — use Stage 1 results directly")
    }

    fn nodes_visited(&self) -> u64 { 0 }
    fn reset_stats(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Position;

    #[test]
    fn search_stats_rates() {
        let stats = SearchStats {
            nodes_visited: 1000,
            tt_hits: 250,
            tt_probes: 500,
            elapsed_secs: 2.0,
        };
        assert!((stats.tt_hit_rate() - 50.0).abs() < 0.01);
        assert!((stats.nps() - 500.0).abs() < 0.01);
    }

    #[test]
    fn pvline_creation() {
        let mv = Move {
            from: Position::new(0, 6),
            to: Position::new(1, 5),
            unstack: false,
        };
        let pv = PVLine {
            root_move: mv,
            moves: vec![mv],
            score: 42,
            leaf_board: Board::new(),
        };
        assert_eq!(pv.root_move, mv);
        assert_eq!(pv.score, 42);
    }

    #[test]
    #[should_panic(expected = "not yet implemented")]
    fn mock_stage2_panics() {
        let mut mock = MockStage2;
        let _ = mock.search(&Board::new(), &SearchConfig::default());
    }
}
