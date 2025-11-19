//! Monte Carlo Tree Search Engine for Arx
//!
//! This module provides a GPU-accelerated MCTS engine for evaluating board positions
//! and finding optimal moves. The engine is completely independent from the main
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

use rand::Rng;
use rayon::prelude::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::board::Board;
use crate::game::{Game, Move, PotentialMove};

mod gpu_context;
pub use gpu_context::{get_shared_context, GpuContext};

mod gpu_move_gen;
pub use gpu_move_gen::MoveGenerationEngine;

mod gpu_batch_sim;
pub use gpu_batch_sim::BatchSimulationEngine;

mod minimax;
pub use minimax::{MinimaxEngine, MinimaxConfig, MinimaxStatistics};

mod variants;
pub use variants::EngineVariant;

pub mod tournament;

/// Piece values for evaluation (based on chess piece values, scaled with Soldier=1)
const PIECE_VALUES: [i32; 8] = [
    0, // Index 0: unused
    1, // Soldier
    3, // Jester (like Bishop)
    5, // Commander (like Rook)
    3, // Paladin (like Bishop-lite)
    3, // Guard (like Bishop-lite)
    3, // Dragon (like Knight)
    5, // Ballista (like Rook-lite)
];

const KING_VALUE: i32 = 1000; // King is invaluable

/// Engine configuration
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Maximum search depth
    pub max_depth: u32,
    /// Number of simulations per move evaluation
    pub simulations_per_move: u32,
    /// Exploration constant for UCB1
    pub exploration_constant: f32,
    /// Batch size for GPU processing (number of simulations processed in parallel)
    pub gpu_batch_size: usize,
    /// Enable GPU-accelerated batch simulation (if false, uses CPU fallback)
    pub use_gpu_simulation: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            simulations_per_move: 100,
            exploration_constant: 1.414,
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

    /// Evaluate a board position and return the value
    /// Positive values favor the current player
    fn evaluate_board(&self, board: &Board) -> i32 {
        let white_to_move = board.is_white_to_move();
        let mut white_value = 0;
        let mut black_value = 0;

        // Iterate through all positions on the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = crate::board::Position::new(x, y);
                if let Some(piece) = board.get_piece(&pos) {
                    // Add value for bottom piece
                    let bottom_value = match piece.bottom {
                        crate::board::PieceType::Soldier => PIECE_VALUES[1],
                        crate::board::PieceType::Jester => PIECE_VALUES[2],
                        crate::board::PieceType::Commander => PIECE_VALUES[3],
                        crate::board::PieceType::Paladin => PIECE_VALUES[4],
                        crate::board::PieceType::Guard => PIECE_VALUES[5],
                        crate::board::PieceType::Dragon => PIECE_VALUES[6],
                        crate::board::PieceType::Ballista => PIECE_VALUES[7],
                        crate::board::PieceType::King => KING_VALUE,
                    };

                    if piece.color == crate::board::Color::White {
                        white_value += bottom_value;
                    } else {
                        black_value += bottom_value;
                    }

                    // Add value for top piece if stacked
                    if let Some(top_piece) = piece.top {
                        let top_value = match top_piece {
                            crate::board::PieceType::Soldier => PIECE_VALUES[1],
                            crate::board::PieceType::Jester => PIECE_VALUES[2],
                            crate::board::PieceType::Commander => PIECE_VALUES[3],
                            crate::board::PieceType::Paladin => PIECE_VALUES[4],
                            crate::board::PieceType::Guard => PIECE_VALUES[5],
                            crate::board::PieceType::Dragon => PIECE_VALUES[6],
                            crate::board::PieceType::Ballista => PIECE_VALUES[7],
                            crate::board::PieceType::King => KING_VALUE,
                        };

                        if piece.color == crate::board::Color::White {
                            white_value += top_value;
                        } else {
                            black_value += top_value;
                        }
                    }
                }
            }
        }

        // Return value from perspective of current player
        if white_to_move {
            white_value - black_value
        } else {
            black_value - white_value
        }
    }

    /// Apply a move to a board state using proper game logic
    fn apply_move(&self, board: &Board, mv: &Move) -> Result<Board, String> {
        let game = Game::from_board(board.clone());
        game.apply_move_copy(*mv)
    }

    /// Run simulations from a given board state
    /// Run simulations from a given board state
    fn simulate(&self, board: &Board, depth: u32) -> i32 {
        // Terminal condition: max depth reached or game over
        if depth >= self.config.max_depth {
            return self.evaluate_board(board);
        }

        // Generate legal moves using the Game API
        let game = Game::from_board(board.clone());
        let potential_moves = game.get_all_moves();

        if potential_moves.is_empty() {
            return self.evaluate_board(board);
        }

        // Simple rollout: pick random move and continue
        let mut rng = rand::thread_rng();
        let random_potential_move = &potential_moves[rng.gen_range(0..potential_moves.len())];

        // Decide whether to unstack based on force_unstack
        let unstack = random_potential_move.force_unstack;
        let mv = random_potential_move.to_move(unstack);

        match self.apply_move(board, &mv) {
            Ok(new_board) => -self.simulate(&new_board, depth + 1), // Negate for opponent's perspective
            Err(_) => self.evaluate_board(board), // Invalid move, evaluate current position
        }
    }

    /// Find the best move using MCTS with GPU acceleration and multi-threading
    pub fn find_best_move(&mut self, board: &Board) -> Result<Move, String> {
        // Reset search-specific stats
        let search_start_moves = self.stats.total_moves.load(Ordering::Relaxed);

        // Generate all legal moves using Game API
        let game = Game::from_board(board.clone());
        let potential_moves = game.get_all_moves();

        if potential_moves.is_empty() {
            return Err("No legal moves available".to_string());
        }

        if potential_moves.len() == 1 {
            // Return the only move, deciding on unstack based on force_unstack
            let unstack = potential_moves[0].force_unstack;
            return Ok(potential_moves[0].to_move(unstack));
        }

        // Convert board to binary for GPU communication
        let board_binary = board.to_binary();

        // Use GPU for move generation
        let moves_u16 = self.move_gen.generate_moves(&board_binary)?;

        // Use GPU batch processing if available
        let best_move_u16 = if let Some(ref batch_sim) = self.batch_sim {
            self.find_best_move_gpu(&board_binary, &moves_u16, batch_sim, search_start_moves)?
        } else {
            self.find_best_move_cpu(board, &potential_moves, search_start_moves)?
        };

        // Convert the u16 back to a Move
        let potential_move = PotentialMove::from_u16(best_move_u16);
        let unstack = potential_move.force_unstack;
        Ok(potential_move.to_move(unstack))
    }

    /// GPU-accelerated move evaluation with batch processing
    fn find_best_move_gpu(
        &self,
        board: &[u8; 83],
        moves: &[u16],
        batch_sim: &BatchSimulationEngine,
        _search_start_moves: u64,
    ) -> Result<u16, String> {
        // Evaluate each move using parallel processing
        let move_scores: Vec<(u16, i32, u32)> = moves
            .par_iter()
            .map(|&mv| {
                let mut total_score = 0i32;
                let mut valid_simulations = 0u32;
                let mut moves_evaluated = 0u64;

                // Process simulations in batches
                let batch_size = self.config.gpu_batch_size;
                let num_batches =
                    (self.config.simulations_per_move as usize + batch_size - 1) / batch_size;

                for batch_idx in 0..num_batches {
                    let sims_in_batch = batch_size
                        .min(self.config.simulations_per_move as usize - batch_idx * batch_size);

                    // Prepare batch: apply initial move and create boards for simulation
                    let mut batch_boards = Vec::with_capacity(sims_in_batch);
                    let mut batch_moves = Vec::with_capacity(sims_in_batch);

                    for _ in 0..sims_in_batch {
                        batch_boards.push(*board);
                        batch_moves.push(mv);
                    }

                    // Process batch on GPU
                    match batch_sim.process_batch(&batch_boards, &batch_moves) {
                        Ok(results) => {
                            self.stats.gpu_batches.fetch_add(1, Ordering::Relaxed);

                            for result in results {
                                if result.valid {
                                    // Negate score for opponent's perspective
                                    total_score -= result.score;
                                    valid_simulations += 1;
                                    moves_evaluated += 1;
                                }
                            }
                        }
                        Err(_) => {
                            // Fall back to CPU for this batch
                            // Convert binary board to Board object for CPU processing
                            let board_obj = match Board::from_binary(*board) {
                                Ok(b) => b,
                                Err(_) => continue,
                            };

                            // Decode the move
                            let potential_move = PotentialMove::from_u16(mv);
                            let unstack = potential_move.force_unstack;
                            let move_obj = potential_move.to_move(unstack);

                            self.stats
                                .cpu_sims
                                .fetch_add(sims_in_batch as u64, Ordering::Relaxed);
                            for _ in 0..sims_in_batch {
                                if let Ok(new_board) = self.apply_move(&board_obj, &move_obj) {
                                    let score = -self.simulate(&new_board, 1);
                                    total_score += score;
                                    valid_simulations += 1;
                                    moves_evaluated += 1;
                                }
                            }
                        }
                    }
                }

                self.stats
                    .simulations
                    .fetch_add(valid_simulations as u64, Ordering::Relaxed);
                self.stats
                    .total_moves
                    .fetch_add(moves_evaluated, Ordering::Relaxed);

                (mv, total_score, valid_simulations)
            })
            .collect();

        // Find move with best average score
        let best_move = move_scores
            .iter()
            .filter(|(_, _, sims)| *sims > 0)
            .max_by(|a, b| {
                let avg_a = a.1 as f32 / a.2 as f32;
                let avg_b = b.1 as f32 / b.2 as f32;
                avg_a
                    .partial_cmp(&avg_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(mv, _, _)| *mv)
            .ok_or("No valid moves found")?;

        Ok(best_move)
    }

    /// CPU-based move evaluation with multi-threading (fallback)
    fn find_best_move_cpu(
        &self,
        board: &Board,
        potential_moves: &[PotentialMove],
        _search_start_moves: u64,
    ) -> Result<u16, String> {
        // Evaluate each move using parallel processing
        let move_scores: Vec<(u16, i32, u32)> = potential_moves
            .par_iter()
            .map(|potential_mv| {
                let mut total_score = 0;
                let mut simulations = 0;

                // Decide on unstack based on force_unstack
                let unstack = potential_mv.force_unstack;
                let mv = potential_mv.to_move(unstack);

                for _ in 0..self.config.simulations_per_move {
                    match self.apply_move(board, &mv) {
                        Ok(new_board) => {
                            let score = -self.simulate(&new_board, 1);
                            total_score += score;
                            simulations += 1;
                        }
                        Err(_) => continue, // Skip invalid moves
                    }
                }

                self.stats
                    .simulations
                    .fetch_add(simulations as u64, Ordering::Relaxed);
                self.stats
                    .cpu_sims
                    .fetch_add(simulations as u64, Ordering::Relaxed);
                self.stats
                    .total_moves
                    .fetch_add(simulations as u64, Ordering::Relaxed);

                (potential_mv.to_u16(), total_score, simulations)
            })
            .collect();

        // Find move with best average score
        let best_move = move_scores
            .iter()
            .filter(|(_, _, sims)| *sims > 0)
            .max_by(|a, b| {
                let avg_a = a.1 as f32 / a.2 as f32;
                let avg_b = b.1 as f32 / b.2 as f32;
                avg_a
                    .partial_cmp(&avg_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(mv, _, _)| *mv)
            .ok_or("No valid moves found")?;

        Ok(best_move)
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
    fn test_board_evaluation() {
        let engine = MctsEngine::new();
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        let engine = engine.unwrap();

        // Test empty board
        let mut board = Board::new();
        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = crate::board::Position::new(x, y);
                board.set_piece(&pos, None);
            }
        }
        let eval = engine.evaluate_board(&board);
        assert_eq!(eval, 0, "Empty board should evaluate to 0");

        // Test board with one white soldier
        let pos = crate::board::Position::new(4, 4); // Center
        let piece = crate::board::Piece {
            color: crate::board::Color::White,
            bottom: crate::board::PieceType::Soldier,
            top: None,
        };
        board.set_piece(&pos, Some(piece));
        let eval = engine.evaluate_board(&board);
        assert_eq!(
            eval, 1,
            "Board with one white soldier should evaluate to 1 for white"
        );
    }

    #[test]
    fn test_engine_config() {
        let config = EngineConfig {
            max_depth: 5,
            simulations_per_move: 200,
            exploration_constant: 2.0,
            gpu_batch_size: 128,
            use_gpu_simulation: true,
        };
        let engine = MctsEngine::with_config(config.clone());
        if let Err(e) = &engine {
            println!("Skipping test: GPU not available - {}", e);
            return;
        }
        let engine = engine.unwrap();
        assert_eq!(engine.config().max_depth, 5);
        assert_eq!(engine.config().simulations_per_move, 200);
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
