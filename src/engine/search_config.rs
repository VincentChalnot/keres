//! Search configuration for the Keres engine.
//!
//! Contains all parameters controlling the search behavior,
//! including depth, threading, and feature flags for benchmarking.

/// Configuration for the search engine.
#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Search depth (default: 4)
    pub depth: i32,
    /// Number of MultiPV passes to collect top-K moves (default: 3)
    pub top_moves: usize,
    /// Target number of distinct PV lines to collect across all passes (default: 5)
    pub expected_leaves: usize,
    /// Hard cap on the number of MultiPV passes regardless of expected_leaves (default: 3)
    pub max_passes: usize,
    /// Disable transposition table
    pub no_tt: bool,
    /// Disable alpha-beta pruning (pure minimax)
    pub no_alpha_beta: bool,
    /// Disable MVV-LVA + history move ordering
    pub no_move_ordering: bool,
    /// Disable killer move heuristic
    pub no_killers: bool,
    /// Enable TreeRecorder debug output
    pub debug_tree: bool,
    /// Number of threads for parallel search (default: num_cpus)
    pub threads: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            depth: 4,
            top_moves: 3,
            expected_leaves: 5,
            max_passes: 3,
            no_tt: false,
            no_alpha_beta: false,
            no_move_ordering: false,
            no_killers: false,
            debug_tree: false,
            threads: num_cpus::get().max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let cfg = SearchConfig::default();
        assert_eq!(cfg.depth, 4);
        assert_eq!(cfg.top_moves, 3);
        assert!(!cfg.no_tt);
        assert!(!cfg.no_alpha_beta);
        assert!(!cfg.no_move_ordering);
        assert!(!cfg.no_killers);
        assert!(!cfg.debug_tree);
        assert!(cfg.threads >= 1);
    }
}
