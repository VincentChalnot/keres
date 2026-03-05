//! Shared AI types used across the engine.

use crate::moves::Move;

/// Whether a transposition table score is exact, a lower bound, or an upper bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    /// The score is exact.
    Exact,
    /// The score is a lower bound (failed high / beta cutoff).
    LowerBound,
    /// The score is an upper bound (failed low).
    UpperBound,
}

/// The result returned from a NegaMax search call.
#[derive(Clone, Debug)]
pub struct SearchResult {
    /// NegaMax-relative score (positive = good for the side to move).
    pub score: i32,
    /// Best move found at this node (None at leaves / when no moves).
    pub best_move: Option<Move>,
}

/// Configuration flags that can disable individual engine features.
#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Enable transposition table.
    pub use_tt: bool,
    /// Enable alpha-beta pruning.
    pub use_alpha_beta: bool,
    /// Enable quiescence search.
    pub use_quiescence: bool,
    /// Enable killer move heuristic.
    pub use_killers: bool,
    /// Maximum search depth.
    pub max_depth: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            use_tt: true,
            use_alpha_beta: true,
            use_quiescence: true,
            use_killers: true,
            max_depth: crate::engine::constants::MAX_DEPTH,
        }
    }
}

/// Per-square evaluation detail (for verbose / debug mode).
#[derive(Clone, Debug)]
pub struct SquareEval {
    pub piece_type: String,
    pub color: String,
    pub base_value: i32,
    pub pst_bonus: i32,
    pub mobility_bonus: i32,
    pub promotion_bonus: i32,
    pub total: i32,
}

/// Full board evaluation detail (for verbose / debug mode).
#[derive(Clone, Debug)]
pub struct BoardEval {
    pub per_square: std::collections::HashMap<(usize, usize), SquareEval>,
    pub white_total: i32,
    pub black_total: i32,
    pub pinned_malus_white: i32,
    pub pinned_malus_black: i32,
    pub king_mobility_white: i32,
    pub king_mobility_black: i32,
    pub tempo: i32,
    pub final_score: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_config_defaults_are_all_enabled() {
        let cfg = SearchConfig::default();
        assert!(cfg.use_tt);
        assert!(cfg.use_alpha_beta);
        assert!(cfg.use_quiescence);
        assert!(cfg.use_killers);
    }

    #[test]
    fn bound_type_variants_are_distinct() {
        assert_ne!(BoundType::Exact, BoundType::LowerBound);
        assert_ne!(BoundType::Exact, BoundType::UpperBound);
        assert_ne!(BoundType::LowerBound, BoundType::UpperBound);
    }
}
