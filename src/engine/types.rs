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

/// Configuration flags that can disable individual engine features, plus the
/// dials that control how much the root move choice deviates from the
/// engine's true best move (see `for_level`).
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
    /// Softmax temperature (in eval-score units) used when picking the root
    /// move. `0.0` always plays the true best move (deterministic argmax).
    /// Above `0.0`, other root moves get picked with probability
    /// `exp((score - best) / temperature)`, so near-equal moves share the
    /// play and clearly worse ones fade out — small inaccuracies that
    /// compound over a game. Scores are clamped to the "decided" band first
    /// (see `search::outcome::DECIDED_SCORE`), so noise never picks a move
    /// that loses the king while a move that doesn't is available. Every root
    /// move is already searched to full depth independently (see
    /// `root_search`), so this costs no extra search time.
    pub noise_temperature: f32,
    /// Probability `[0.0, 1.0]` of playing the move a shallower,
    /// `blunder_depth`-ply search picks instead of searching at `max_depth`:
    /// the engine misses whatever only the deeper search would have seen,
    /// which is what a human blunder looks like (unlike a random move).
    pub blunder_chance: f32,
    /// Search depth used for a blunder (see `blunder_chance`); only
    /// meaningful below `max_depth`.
    pub blunder_depth: usize,
    /// Probability `[0.0, 1.0]` of ignoring a decided outcome. When the
    /// search sees a forced win the engine otherwise always plays the fastest
    /// one (an available king capture is always taken), and when every move
    /// loses it plays the longest defence. A slip falls back to the ordinary
    /// noisy pick instead, where all winning (or all losing) moves look
    /// alike — so the engine may miss the win or hang its king at once.
    pub decided_slip_chance: f32,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            use_tt: true,
            use_alpha_beta: true,
            use_quiescence: true,
            use_killers: true,
            max_depth: crate::engine::constants::MAX_DEPTH,
            noise_temperature: 0.0,
            blunder_chance: 0.0,
            blunder_depth: 1,
            decided_slip_chance: 0.0,
        }
    }
}

impl SearchConfig {
    /// Build a config for an engine strength `level`, clamped to
    /// `[MIN_LEVEL, MAX_LEVEL]`. This is the single place that maps a
    /// user-facing difficulty knob onto engine feature flags, so new dials
    /// get added here rather than scattered across callers.
    ///
    /// `use_tt` and `use_alpha_beta` stay on regardless of level: both are
    /// transparent optimizations that only affect search speed, not the
    /// move the engine picks, so disabling them would not weaken play —
    /// only slow it down. Quiescence/killers switch on once `max_depth >= 3`:
    /// below that the search is already so shallow that the extra tactical
    /// accuracy they buy just makes it feel erratic rather than weak.
    ///
    /// Strength comes from three dials, in decreasing order of effect: search
    /// depth, the chance of playing a shallower search's move
    /// (`blunder_chance`), and softmax noise among near-equal moves
    /// (`noise_temperature`). From level 4 up the engine never ignores a
    /// decided outcome (`decided_slip_chance`): it always takes a king
    /// capture and always defends a lost position for as long as it can.
    /// `MAX_LEVEL` is the full-strength engine (`MAX_DEPTH`, no noise).
    ///
    /// The values are tuned with the `arena` binary (`src/arena.rs`); the
    /// method and the measurements are in `docs/LEVELS.md` (keep it in sync).
    pub fn for_level(level: u8) -> Self {
        use crate::engine::constants::{MAX_LEVEL, MIN_LEVEL};
        // (max_depth, noise_temperature, blunder_chance, blunder_depth,
        //  decided_slip_chance), indexed by level - 1.
        const LADDER: [(usize, f32, f32, usize, f32); MAX_LEVEL as usize] = [
            // Elo over the level below, measured with `arena` (100–150 games,
            // 4 random opening plies, 95% intervals about ±50; 2026-10-08):
            (2, 4.0, 0.10, 1, 0.20), // no quiescence: hangs its king ~1% of moves
            (3, 10.0, 0.15, 1, 0.10), // +458
            (3, 8.0, 0.12, 1, 0.03), // +156
            (3, 7.0, 0.09, 2, 0.0),  // +100
            (3, 5.0, 0.07, 2, 0.0),  // +92
            (3, 3.0, 0.05, 2, 0.0),  // +111
            (3, 1.5, 0.03, 2, 0.0),  // +139
            (4, 4.0, 0.04, 3, 0.0),  // +115
            (4, 2.5, 0.015, 3, 0.0), // +120
            (4, 0.0, 0.0, 3, 0.0),   // +144
        ];
        let level = level.clamp(MIN_LEVEL, MAX_LEVEL);
        let (max_depth, noise_temperature, blunder_chance, blunder_depth, decided_slip_chance) =
            LADDER[(level - 1) as usize];
        let use_quiescence_and_killers = max_depth >= 3;

        SearchConfig {
            use_tt: true,
            use_alpha_beta: true,
            use_quiescence: use_quiescence_and_killers,
            use_killers: use_quiescence_and_killers,
            max_depth,
            noise_temperature,
            blunder_chance,
            blunder_depth,
            decided_slip_chance,
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
    fn for_level_gates_quiescence_and_killers_on_depth() {
        for level in 1..=10u8 {
            let cfg = SearchConfig::for_level(level);
            assert_eq!(cfg.use_quiescence, cfg.max_depth >= 3);
            assert_eq!(cfg.use_killers, cfg.max_depth >= 3);
            assert!(cfg.use_tt);
            assert!(cfg.use_alpha_beta);
        }
    }

    #[test]
    fn for_level_clamps_out_of_range_values() {
        assert_eq!(
            SearchConfig::for_level(0).noise_temperature,
            SearchConfig::for_level(1).noise_temperature
        );
        assert_eq!(SearchConfig::for_level(11).noise_temperature, 0.0);
    }

    #[test]
    fn max_level_is_the_full_strength_deterministic_engine() {
        let top = SearchConfig::for_level(crate::engine::constants::MAX_LEVEL);
        assert_eq!(top.max_depth, crate::engine::constants::MAX_DEPTH);
        assert_eq!(top.noise_temperature, 0.0);
        assert_eq!(top.blunder_chance, 0.0);
        assert_eq!(top.decided_slip_chance, 0.0);
    }

    #[test]
    fn from_level_4_up_a_decided_outcome_is_never_ignored() {
        for level in 4..=crate::engine::constants::MAX_LEVEL {
            assert_eq!(
                SearchConfig::for_level(level).decided_slip_chance,
                0.0,
                "level {level}"
            );
        }
    }

    #[test]
    fn every_level_has_usable_dials() {
        for level in 1..=crate::engine::constants::MAX_LEVEL {
            let cfg = SearchConfig::for_level(level);
            assert!(cfg.max_depth >= 1 && cfg.max_depth <= crate::engine::constants::MAX_DEPTH);
            assert!(cfg.noise_temperature >= 0.0, "level {level}");
            assert!((0.0..=1.0).contains(&cfg.blunder_chance), "level {level}");
            assert!(
                (0.0..=1.0).contains(&cfg.decided_slip_chance),
                "level {level}"
            );
            // A blunder must actually search less deep, or it is not a blunder.
            assert!(
                cfg.blunder_chance == 0.0
                    || (cfg.blunder_depth >= 1 && cfg.blunder_depth < cfg.max_depth),
                "level {level}"
            );
        }
    }

    #[test]
    fn bound_type_variants_are_distinct() {
        assert_ne!(BoundType::Exact, BoundType::LowerBound);
        assert_ne!(BoundType::Exact, BoundType::UpperBound);
        assert_ne!(BoundType::LowerBound, BoundType::UpperBound);
    }
}
