//! Monte Carlo Tree Search Engine for Arx
//!
//! This module provides a GPU-accelerated MCTS engine for evaluating board positions
//! and finding optimal moves. The engine is completely independent of the main
//! game logic (board.rs and game.rs) and implements its own simplified move application
//! and evaluation functions.
//!
//! # Features
//!
//! - GPU-accelerated move generation via compute shaders
//! - GPU-accelerated batch simulation for move application and evaluation
//! - Multi-threaded CPU processing with Rayon
//! - Configurable search depth and simulation count
//! - Piece value-based position evaluation
//! - Adjustable engine strength
//! - Statistics tracking (moves evaluated, simulations run)
//!
//! # Example
//!
//! ```no_run
//! use arx_engine::engine::{MctsEngine, EngineConfig};
//! use arx_engine::{Game, Board};
//!
//! // Create engine with custom configuration
//! let config = EngineConfig {
//!     max_depth: 3,
//!     simulations_per_move: 100,
//!     exploration_constant: 1.414,
//!     gpu_batch_size: 256,
//!     use_gpu_simulation: true,
//! };
//! let mut engine = MctsEngine::with_config(config).expect("Failed to create engine");
//!
//! // Find best move for a board position
//! let game = Game::new();
//! let best_move = engine.find_best_move(&game.board).expect("No legal moves");
//!
//! // Get search statistics
//! let stats = engine.get_statistics();
//! println!("Moves evaluated: {}", stats.total_moves_evaluated);
//! println!("Simulations run: {}", stats.simulations_run);
//! ```

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::board::Board;
use crate::game::Move;

mod gpu_context;
pub use gpu_context::{get_shared_context, GpuContext};

mod gpu_move_gen;
pub use gpu_move_gen::MoveGenerationEngine;

mod gpu_batch_sim;
pub use gpu_batch_sim::BatchSimulationEngine;

mod minimax;
pub use minimax::{MinimaxConfig, MinimaxEngine, MinimaxStatistics};

/// Search parameters for MCTS evaluation
#[derive(Clone, Debug)]
pub struct SearchParams {
    /// Maximum search depth
    pub max_depth: u32,
    /// Number of simulations per move evaluation
    pub simulations_per_move: u32,
    /// Exploration constant for UCB1
    pub exploration_constant: f32,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            max_depth: 3,
            simulations_per_move: 100,
            exploration_constant: 1.414,
        }
    }
}

/// A move with its evaluated score
#[derive(Clone, Debug)]
pub struct ScoredMove {
    /// The move
    pub mv: Move,
    /// The evaluated score (from White's perspective: positive=good for White, negative=good for Black)
    pub score: i32,
    /// Number of valid simulations that contributed to this score
    pub simulations: u32,
}

/// Engine configuration
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Batch size for GPU processing (number of simulations processed in parallel)
    pub gpu_batch_size: usize,
    /// Enable GPU-accelerated batch simulation (if false, uses CPU fallback)
    pub use_gpu_simulation: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            gpu_batch_size: 256,
            use_gpu_simulation: true,
        }
    }
}

/// Statistics for MCTS search
#[derive(Clone, Debug, Default)]
pub struct SearchStatistics {
    /// Total number of moves evaluated across all simulations
    pub total_moves_evaluated: u64,
    /// Number of simulations run
    pub simulations_run: u64,
    /// Number of moves evaluated in the most recent search
    pub last_search_moves: u64,
    /// Number of GPU batches processed
    pub gpu_batches_processed: u64,
    /// Number of CPU simulations (fallback)
    pub cpu_simulations: u64,
}

impl SearchStatistics {
    /// Reset statistics
    pub fn reset(&mut self) {
        self.total_moves_evaluated = 0;
        self.simulations_run = 0;
        self.last_search_moves = 0;
        self.gpu_batches_processed = 0;
        self.cpu_simulations = 0;
    }

    /// Get average moves per simulation
    pub fn avg_moves_per_simulation(&self) -> f64 {
        if self.simulations_run == 0 {
            0.0
        } else {
            self.total_moves_evaluated as f64 / self.simulations_run as f64
        }
    }
}

/// Monte Carlo Tree Search Engine
pub struct MctsEngine {
    config: EngineConfig,
    move_gen: MoveGenerationEngine,
    batch_sim: Option<BatchSimulationEngine>,
    stats: Arc<AtomicStats>,
}

/// Atomic statistics for thread-safe updates
struct AtomicStats {
    total_moves: AtomicU64,
    simulations: AtomicU64,
    gpu_batches: AtomicU64,
    cpu_sims: AtomicU64,
}

impl AtomicStats {
    fn new() -> Self {
        Self {
            total_moves: AtomicU64::new(0),
            simulations: AtomicU64::new(0),
            gpu_batches: AtomicU64::new(0),
            cpu_sims: AtomicU64::new(0),
        }
    }

    fn to_statistics(&self, last_search_moves: u64) -> SearchStatistics {
        SearchStatistics {
            total_moves_evaluated: self.total_moves.load(Ordering::Relaxed),
            simulations_run: self.simulations.load(Ordering::Relaxed),
            last_search_moves,
            gpu_batches_processed: self.gpu_batches.load(Ordering::Relaxed),
            cpu_simulations: self.cpu_sims.load(Ordering::Relaxed),
        }
    }

    fn reset(&self) {
        self.total_moves.store(0, Ordering::Relaxed);
        self.simulations.store(0, Ordering::Relaxed);
        self.gpu_batches.store(0, Ordering::Relaxed);
        self.cpu_sims.store(0, Ordering::Relaxed);
    }
}

impl MctsEngine {
    /// Create a new MCTS engine with default configuration
    pub fn new() -> Result<Self, String> {
        Self::with_config(EngineConfig::default())
    }

    /// Create a new MCTS engine with custom configuration
    pub fn with_config(config: EngineConfig) -> Result<Self, String> {
        let move_gen = MoveGenerationEngine::new_sync()?;

        // Try to create batch simulation engine if GPU simulation is enabled
        let batch_sim = if config.use_gpu_simulation {
            match BatchSimulationEngine::new_sync() {
                Ok(engine) => {
                    eprintln!("✓ GPU batch simulation engine initialized");
                    Some(engine)
                }
                Err(e) => {
                    eprintln!("⚠ GPU batch simulation unavailable: {}", e);
                    eprintln!("  Falling back to CPU simulation");
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config,
            move_gen,
            batch_sim,
            stats: Arc::new(AtomicStats::new()),
        })
    }

    /// Evaluate all legal moves for a board position
    /// Evaluate all legal moves for a board position using GPU
    /// Returns a list of moves with their scores (from White's perspective)
    pub fn evaluate_moves(
        &mut self,
        board: &Board,
        params: &SearchParams,
    ) -> Result<Vec<ScoredMove>, String> {
        // Convert board to binary for GPU communication
        let board_binary = board.to_binary();

        // Use GPU for move generation - returns GPU buffer
        let moves_buffer = self.move_gen.generate_moves_buffer(&board_binary)?;

        if self.move_gen.is_buffer_empty(&moves_buffer)? {
            return Ok(Vec::new());
        }

        // Use GPU batch simulation engine (required)
        let batch_sim = self
            .batch_sim
            .as_ref()
            .ok_or("GPU batch simulation engine required but not available")?;

        self.evaluate_moves_gpu(&board_binary, &moves_buffer, batch_sim, params)
    }

    /// Select the best move for the current player from evaluated moves
    pub fn select_best_move(board: &Board, scored_moves: &[ScoredMove]) -> Result<Move, String> {
        if scored_moves.is_empty() {
            return Err("No moves to select from".to_string());
        }

        let white_to_move = board.is_white_to_move();

        // Paranoid approach: pick highest for White, lowest for Black
        let best = if white_to_move {
            scored_moves.iter().max_by_key(|sm| sm.score)
        } else {
            scored_moves.iter().min_by_key(|sm| sm.score)
        };

        best.map(|sm| sm.mv)
            .ok_or_else(|| "No valid moves found".to_string())
    }

    /// GPU-accelerated move evaluation with batch processing
    fn evaluate_moves_gpu(
        &self,
        board: &[u8; 83],
        moves_buffer: &wgpu::Buffer,
        batch_sim: &BatchSimulationEngine,
        params: &SearchParams,
    ) -> Result<Vec<ScoredMove>, String> {
        // Directly use the GPU buffer for move evaluation in batch_sim
        batch_sim.evaluate_moves_gpu(board, moves_buffer, params)
    }

    /// Get search statistics
    pub fn get_statistics(&self) -> SearchStatistics {
        let current_moves = self.stats.total_moves.load(Ordering::Relaxed);
        self.stats.to_statistics(current_moves)
    }

    /// Reset search statistics
    pub fn reset_statistics(&mut self) {
        self.stats.reset();
    }

    /// Get the current configuration
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Update the configuration
    pub fn set_config(&mut self, config: EngineConfig) {
        // Check if we need to initialize batch sim before moving config
        let use_gpu = config.use_gpu_simulation;
        self.config = config;

        // Try to initialize batch sim if needed
        if use_gpu && self.batch_sim.is_none() {
            if let Ok(batch_sim) = BatchSimulationEngine::new_sync() {
                eprintln!("✓ GPU batch simulation engine initialized");
                self.batch_sim = Some(batch_sim);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = MctsEngine::new();
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        assert!(engine.is_ok());
    }

    #[test]
    fn test_engine_config() {
        let config = EngineConfig {
            gpu_batch_size: 128,
            use_gpu_simulation: true,
        };
        let engine = MctsEngine::with_config(config.clone());
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        let engine = engine.unwrap();
        assert_eq!(engine.config().gpu_batch_size, 128);
        assert_eq!(engine.config().use_gpu_simulation, true);
    }

    #[test]
    fn test_statistics() {
        let engine = MctsEngine::new();
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        let mut engine = engine.unwrap();

        // Get initial stats
        let stats = engine.get_statistics();
        assert_eq!(stats.total_moves_evaluated, 0);
        assert_eq!(stats.simulations_run, 0);

        // Reset stats
        engine.reset_statistics();
        let stats = engine.get_statistics();
        assert_eq!(stats.total_moves_evaluated, 0);
    }
}
