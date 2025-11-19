//! Engine Configuration Variants
//!
//! This module provides pre-configured engine variants optimized for different play styles.
//! Each variant has been tuned for specific characteristics:
//!
//! - **Aggressive**: Prioritizes attacks, captures, and center control
//! - **Defensive**: Focuses on king safety and material preservation
//! - **Balanced**: Well-rounded approach (default configuration)
//! - **Tactical**: Deep tactical search with strong capture evaluation
//! - **Positional**: Emphasizes territorial control and piece placement
//!
//! # Example
//!
//! ```no_run
//! use arx_engine::engine::{MinimaxEngine, EngineVariant};
//!
//! // Create an aggressive engine
//! let mut engine = MinimaxEngine::with_variant(EngineVariant::Aggressive);
//!
//! // Or create a defensive engine
//! let mut engine = MinimaxEngine::with_variant(EngineVariant::Defensive);
//! ```

use super::MinimaxConfig;

/// Pre-configured engine variants
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineVariant {
    /// Aggressive play style - prioritizes attacks and captures
    Aggressive,
    /// Defensive play style - focuses on king safety and material preservation
    Defensive,
    /// Balanced play style - well-rounded approach (default)
    Balanced,
    /// Tactical play style - deep search with strong capture evaluation
    Tactical,
    /// Positional play style - emphasizes territory and piece placement
    Positional,
}

impl EngineVariant {
    /// Get the configuration for this variant
    pub fn config(self) -> MinimaxConfig {
        match self {
            EngineVariant::Aggressive => Self::aggressive_config(),
            EngineVariant::Defensive => Self::defensive_config(),
            EngineVariant::Balanced => Self::balanced_config(),
            EngineVariant::Tactical => Self::tactical_config(),
            EngineVariant::Positional => Self::positional_config(),
        }
    }

    /// Aggressive configuration
    /// - Higher material weight to value captures
    /// - Lower king safety to take risks
    /// - Higher territorial weight for aggressive positioning
    fn aggressive_config() -> MinimaxConfig {
        MinimaxConfig {
            max_depth: 6,
            use_quiescence: true,
            use_transposition_table: true,
            time_limit_ms: 4000,
            material_weight: 0.90, // Very high - prioritize captures
            territorial_weight: 0.07, // Medium - push forward
            mobility_weight: 0.02, // Low - don't worry about mobility
            king_safety_weight: 0.01, // Very low - take risks
            stack_bonus: 0.25, // Higher bonus for powerful stacks
        }
    }

    /// Defensive configuration
    /// - Higher king safety weight
    /// - Lower territorial weight (don't overextend)
    /// - Balanced material weight
    fn defensive_config() -> MinimaxConfig {
        MinimaxConfig {
            max_depth: 6,
            use_quiescence: true,
            use_transposition_table: true,
            time_limit_ms: 4000,
            material_weight: 0.75, // High but not dominant
            territorial_weight: 0.03, // Very low - stay back
            mobility_weight: 0.07, // Medium - keep options open
            king_safety_weight: 0.15, // Very high - protect the king
            stack_bonus: 0.15, // Lower - prefer spreading pieces
        }
    }

    /// Balanced configuration (default)
    /// - Well-rounded weights
    /// - Good all-around play
    fn balanced_config() -> MinimaxConfig {
        MinimaxConfig {
            max_depth: 6,
            use_quiescence: true,
            use_transposition_table: true,
            time_limit_ms: 4000,
            material_weight: 0.85,
            territorial_weight: 0.08,
            mobility_weight: 0.05,
            king_safety_weight: 0.02,
            stack_bonus: 0.20,
        }
    }

    /// Tactical configuration
    /// - Deeper quiescence search
    /// - Higher material weight for capture evaluation
    /// - Moderate other factors
    fn tactical_config() -> MinimaxConfig {
        MinimaxConfig {
            max_depth: 5, // Slightly lower depth to allow deeper quiescence
            use_quiescence: true,
            use_transposition_table: true,
            time_limit_ms: 5000, // More time for tactical calculation
            material_weight: 0.88, // Very high - find tactics
            territorial_weight: 0.04,
            mobility_weight: 0.06,
            king_safety_weight: 0.02,
            stack_bonus: 0.30, // High - value stacked piece tactics
        }
    }

    /// Positional configuration
    /// - Higher territorial and mobility weights
    /// - Lower material weight (willing to sacrifice for position)
    /// - Moderate king safety
    fn positional_config() -> MinimaxConfig {
        MinimaxConfig {
            max_depth: 6,
            use_quiescence: true,
            use_transposition_table: true,
            time_limit_ms: 4000,
            material_weight: 0.70, // Lower - willing to sacrifice
            territorial_weight: 0.15, // High - control the board
            mobility_weight: 0.10, // High - keep pieces active
            king_safety_weight: 0.05, // Medium - balance risk/reward
            stack_bonus: 0.20,
        }
    }

    /// Get a descriptive name for the variant
    pub fn name(self) -> &'static str {
        match self {
            EngineVariant::Aggressive => "Aggressive",
            EngineVariant::Defensive => "Defensive",
            EngineVariant::Balanced => "Balanced",
            EngineVariant::Tactical => "Tactical",
            EngineVariant::Positional => "Positional",
        }
    }

    /// Get a description of the variant's play style
    pub fn description(self) -> &'static str {
        match self {
            EngineVariant::Aggressive => "Prioritizes attacks, captures, and material gain. Takes risks with king safety.",
            EngineVariant::Defensive => "Focuses on king safety and solid defense. Avoids risky positions.",
            EngineVariant::Balanced => "Well-rounded approach with balanced priorities. Default configuration.",
            EngineVariant::Tactical => "Deep tactical search with strong capture evaluation. Finds complex combinations.",
            EngineVariant::Positional => "Emphasizes territorial control and piece activity. Values long-term advantages.",
        }
    }

    /// Get all available variants
    pub fn all() -> Vec<EngineVariant> {
        vec![
            EngineVariant::Aggressive,
            EngineVariant::Defensive,
            EngineVariant::Balanced,
            EngineVariant::Tactical,
            EngineVariant::Positional,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_variants_have_valid_configs() {
        for variant in EngineVariant::all() {
            let config = variant.config();
            
            // Check that weights sum to approximately 1.0
            let sum = config.material_weight
                + config.territorial_weight
                + config.mobility_weight
                + config.king_safety_weight;
            
            assert!(
                (sum - 1.0).abs() < 0.01,
                "Variant {} has weights that don't sum to ~1.0: {}",
                variant.name(),
                sum
            );
            
            // Check that all weights are positive
            assert!(config.material_weight > 0.0);
            assert!(config.territorial_weight >= 0.0);
            assert!(config.mobility_weight >= 0.0);
            assert!(config.king_safety_weight >= 0.0);
            
            // Check reasonable depth
            assert!(config.max_depth >= 3 && config.max_depth <= 8);
        }
    }

    #[test]
    fn test_variant_names_and_descriptions() {
        for variant in EngineVariant::all() {
            assert!(!variant.name().is_empty());
            assert!(!variant.description().is_empty());
        }
    }

    #[test]
    fn test_aggressive_has_high_material_weight() {
        let config = EngineVariant::Aggressive.config();
        assert!(config.material_weight > 0.85);
        assert!(config.king_safety_weight < 0.05);
    }

    #[test]
    fn test_defensive_has_high_king_safety_weight() {
        let config = EngineVariant::Defensive.config();
        assert!(config.king_safety_weight > 0.10);
        assert!(config.territorial_weight < 0.05);
    }

    #[test]
    fn test_positional_has_high_territorial_weight() {
        let config = EngineVariant::Positional.config();
        assert!(config.territorial_weight > 0.10);
        assert!(config.material_weight < 0.75);
    }
}
