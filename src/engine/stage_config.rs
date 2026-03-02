//! Stage configuration for the two-stage search architecture.
//!
//! `StageConfig` fully drives the behavior of the search engine at runtime.
//! Both stages share the same alpha-beta core — the config determines what
//! is enabled or disabled.

use serde::{Deserialize, Serialize};

/// Configuration that fully controls the behavior of a search stage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StageConfig {
    // Search depth
    pub depth: u8,
    /// If true, increment depth by 1 if it is odd (Stage 2 only).
    pub force_even_depth: bool,

    // Alpha-beta optimizations
    /// Enable alpha-beta pruning (default: true). When false, runs pure minimax.
    pub alpha_beta: bool,
    /// Enable MVV-LVA + history heuristic move ordering (default: true).
    pub move_ordering: bool,
    /// Enable killer move heuristic (default: true).
    pub killer_moves: bool,
    /// Enable transposition table (default: true).
    pub transposition_table: bool,

    // Stage 2 only (must default to false in Stage 1)
    /// Enable null move pruning.
    pub null_move_pruning: bool,
    /// Null move reduction R value (default: 2).
    pub null_move_reduction: u8,
    /// Enable Late Move Reductions.
    pub lmr: bool,
    /// Enable selective extensions (extend 1 ply on SEE >= 0 captures at leaf nodes).
    pub selective_extensions: bool,

    // MultiPV (Stage 1 only — Stage 2 does not use MultiPV)
    /// Hard cap on the number of MultiPV passes (default: 3).
    pub max_passes: u8,
    /// Target number of distinct PV lines to collect across all passes (default: 5).
    pub expected_leaves: usize,

    // Debug
    /// Enable TreeRecorder debug output (default: false).
    pub tree_recorder: bool,
}

impl StageConfig {
    /// Default configuration for Stage 1 search.
    pub fn stage1() -> Self {
        Self {
            depth: 4,
            force_even_depth: false,
            alpha_beta: true,
            move_ordering: true,
            killer_moves: true,
            transposition_table: true,
            null_move_pruning: false,
            null_move_reduction: 2,
            lmr: false,
            selective_extensions: false,
            max_passes: 3,
            expected_leaves: 5,
            tree_recorder: false,
        }
    }

    /// Default configuration for Stage 2 search.
    pub fn stage2() -> Self {
        Self {
            depth: 6,
            force_even_depth: true,
            alpha_beta: true,
            move_ordering: true,
            killer_moves: true,
            transposition_table: true,
            null_move_pruning: true,
            null_move_reduction: 2,
            lmr: true,
            selective_extensions: true,
            max_passes: 1,
            expected_leaves: 1,
            tree_recorder: false,
        }
    }

    /// Effective search depth, adjusted for force_even_depth.
    pub fn effective_depth(&self) -> u8 {
        let d = self.depth;
        if self.force_even_depth && d % 2 != 0 {
            d + 1
        } else {
            d
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage1_defaults() {
        let cfg = StageConfig::stage1();
        assert_eq!(cfg.depth, 4);
        assert!(!cfg.force_even_depth);
        assert!(cfg.alpha_beta);
        assert!(cfg.move_ordering);
        assert!(cfg.killer_moves);
        assert!(cfg.transposition_table);
        assert!(!cfg.null_move_pruning);
        assert_eq!(cfg.null_move_reduction, 2);
        assert!(!cfg.lmr);
        assert!(!cfg.selective_extensions);
        assert_eq!(cfg.max_passes, 3);
        assert_eq!(cfg.expected_leaves, 5);
        assert!(!cfg.tree_recorder);
    }

    #[test]
    fn stage2_defaults() {
        let cfg = StageConfig::stage2();
        assert_eq!(cfg.depth, 6);
        assert!(cfg.force_even_depth);
        assert!(cfg.alpha_beta);
        assert!(cfg.move_ordering);
        assert!(cfg.killer_moves);
        assert!(cfg.transposition_table);
        assert!(cfg.null_move_pruning);
        assert_eq!(cfg.null_move_reduction, 2);
        assert!(cfg.lmr);
        assert!(cfg.selective_extensions);
        assert_eq!(cfg.max_passes, 1);
        assert_eq!(cfg.expected_leaves, 1);
        assert!(!cfg.tree_recorder);
    }

    #[test]
    fn force_even_depth_works() {
        let mut cfg = StageConfig::stage2();
        cfg.depth = 5;
        assert_eq!(cfg.effective_depth(), 6);
        cfg.depth = 6;
        assert_eq!(cfg.effective_depth(), 6);
        cfg.force_even_depth = false;
        cfg.depth = 5;
        assert_eq!(cfg.effective_depth(), 5);
    }

    #[test]
    fn serde_roundtrip() {
        let cfg = StageConfig::stage1();
        let json = serde_json::to_string(&cfg).unwrap();
        let cfg2: StageConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg.depth, cfg2.depth);
        assert_eq!(cfg.alpha_beta, cfg2.alpha_beta);
        assert_eq!(cfg.null_move_pruning, cfg2.null_move_pruning);
        assert_eq!(cfg.lmr, cfg2.lmr);
        assert_eq!(cfg.selective_extensions, cfg2.selective_extensions);
        assert_eq!(cfg.max_passes, cfg2.max_passes);
        assert_eq!(cfg.expected_leaves, cfg2.expected_leaves);
    }
}
