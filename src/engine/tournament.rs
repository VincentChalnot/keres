//! Engine Tournament Framework
//!
//! This module provides tools to run matches between different engine configurations
//! to compare their performance. This is useful for:
//! - Testing new engine variants
//! - Tuning engine parameters
//! - Benchmarking improvements
//!
//! # Example
//!
//! ```no_run
//! use arx_engine::engine::{MinimaxEngine, EngineVariant, tournament::*};
//!
//! // Create a match between two variants
//! let aggressive = MinimaxEngine::with_variant(EngineVariant::Aggressive);
//! let defensive = MinimaxEngine::with_variant(EngineVariant::Defensive);
//!
//! let config = MatchConfig {
//!     num_games: 10,
//!     time_per_move_ms: 3000,
//!     max_moves_per_game: 100,
//! };
//!
//! let result = run_match(aggressive, defensive, config);
//! println!("Player 1 won {} games", result.player1_wins);
//! println!("Player 2 won {} games", result.player2_wins);
//! println!("Draws: {}", result.draws);
//! ```

use crate::game::Game;
use super::{MinimaxEngine, MinimaxConfig};
use std::time::{Duration, Instant};

/// Configuration for a match between two engines
#[derive(Clone, Debug)]
pub struct MatchConfig {
    /// Number of games to play
    pub num_games: usize,
    /// Time limit per move in milliseconds
    pub time_per_move_ms: u64,
    /// Maximum number of moves per game before declaring a draw
    pub max_moves_per_game: usize,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            num_games: 10,
            time_per_move_ms: 3000,
            max_moves_per_game: 150,
        }
    }
}

/// Result of a single game
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameResult {
    /// Player 1 (White) wins
    Player1Win,
    /// Player 2 (Black) wins
    Player2Win,
    /// Draw
    Draw,
}

/// Statistics for a single game
#[derive(Clone, Debug)]
pub struct GameStats {
    /// Result of the game
    pub result: GameResult,
    /// Number of moves made
    pub num_moves: usize,
    /// Total time taken by player 1
    pub player1_time: Duration,
    /// Total time taken by player 2
    pub player2_time: Duration,
    /// Average positions evaluated per move by player 1
    pub player1_avg_positions: f64,
    /// Average positions evaluated per move by player 2
    pub player2_avg_positions: f64,
}

/// Result of a match between two engines
#[derive(Clone, Debug)]
pub struct MatchResult {
    /// Number of games won by player 1
    pub player1_wins: usize,
    /// Number of games won by player 2
    pub player2_wins: usize,
    /// Number of draws
    pub draws: usize,
    /// Statistics for each game
    pub game_stats: Vec<GameStats>,
    /// Player 1 configuration name/description
    pub player1_name: String,
    /// Player 2 configuration name/description
    pub player2_name: String,
}

impl MatchResult {
    /// Calculate win percentage for player 1
    pub fn player1_win_percentage(&self) -> f64 {
        let total_games = self.player1_wins + self.player2_wins + self.draws;
        if total_games == 0 {
            return 0.0;
        }
        (self.player1_wins as f64 / total_games as f64) * 100.0
    }

    /// Calculate win percentage for player 2
    pub fn player2_win_percentage(&self) -> f64 {
        let total_games = self.player1_wins + self.player2_wins + self.draws;
        if total_games == 0 {
            return 0.0;
        }
        (self.player2_wins as f64 / total_games as f64) * 100.0
    }

    /// Calculate draw percentage
    pub fn draw_percentage(&self) -> f64 {
        let total_games = self.player1_wins + self.player2_wins + self.draws;
        if total_games == 0 {
            return 0.0;
        }
        (self.draws as f64 / total_games as f64) * 100.0
    }

    /// Get average game length
    pub fn avg_game_length(&self) -> f64 {
        if self.game_stats.is_empty() {
            return 0.0;
        }
        let total_moves: usize = self.game_stats.iter().map(|s| s.num_moves).sum();
        total_moves as f64 / self.game_stats.len() as f64
    }

    /// Print a formatted summary
    pub fn print_summary(&self) {
        println!("═══════════════════════════════════════════════════");
        println!("Match Results: {} vs {}", self.player1_name, self.player2_name);
        println!("═══════════════════════════════════════════════════");
        println!("Total games: {}", self.player1_wins + self.player2_wins + self.draws);
        println!();
        println!("{} wins: {} ({:.1}%)", self.player1_name, self.player1_wins, self.player1_win_percentage());
        println!("{} wins: {} ({:.1}%)", self.player2_name, self.player2_wins, self.player2_win_percentage());
        println!("Draws: {} ({:.1}%)", self.draws, self.draw_percentage());
        println!();
        println!("Average game length: {:.1} moves", self.avg_game_length());
        
        if !self.game_stats.is_empty() {
            let avg_p1_time: Duration = self.game_stats.iter()
                .map(|s| s.player1_time)
                .sum::<Duration>() / self.game_stats.len() as u32;
            let avg_p2_time: Duration = self.game_stats.iter()
                .map(|s| s.player2_time)
                .sum::<Duration>() / self.game_stats.len() as u32;
            
            println!("Average time per game:");
            println!("  {}: {:.2}s", self.player1_name, avg_p1_time.as_secs_f64());
            println!("  {}: {:.2}s", self.player2_name, avg_p2_time.as_secs_f64());
            
            let avg_p1_pos: f64 = self.game_stats.iter()
                .map(|s| s.player1_avg_positions)
                .sum::<f64>() / self.game_stats.len() as f64;
            let avg_p2_pos: f64 = self.game_stats.iter()
                .map(|s| s.player2_avg_positions)
                .sum::<f64>() / self.game_stats.len() as f64;
            
            println!("Average positions evaluated per move:");
            println!("  {}: {:.0}", self.player1_name, avg_p1_pos);
            println!("  {}: {:.0}", self.player2_name, avg_p2_pos);
        }
        println!("═══════════════════════════════════════════════════");
    }
}

/// Run a match between two engine configurations
pub fn run_match(
    mut player1: MinimaxEngine,
    mut player2: MinimaxEngine,
    config: MatchConfig,
) -> MatchResult {
    run_match_with_names(
        &mut player1,
        &mut player2,
        config,
        "Player 1".to_string(),
        "Player 2".to_string(),
    )
}

/// Run a match between two engine configurations with custom names
pub fn run_match_with_names(
    player1: &mut MinimaxEngine,
    player2: &mut MinimaxEngine,
    config: MatchConfig,
    player1_name: String,
    player2_name: String,
) -> MatchResult {
    let mut player1_wins = 0;
    let mut player2_wins = 0;
    let mut draws = 0;
    let mut game_stats = Vec::new();

    println!("Starting match: {} vs {}", player1_name, player2_name);
    println!("Configuration:");
    println!("  Games: {}", config.num_games);
    println!("  Time per move: {}ms", config.time_per_move_ms);
    println!("  Max moves per game: {}", config.max_moves_per_game);
    println!();

    // Update time limits for both engines
    player1.set_config(MinimaxConfig {
        time_limit_ms: config.time_per_move_ms,
        ..player1.config().clone()
    });
    player2.set_config(MinimaxConfig {
        time_limit_ms: config.time_per_move_ms,
        ..player2.config().clone()
    });

    for game_num in 1..=config.num_games {
        println!("Game {}/{}...", game_num, config.num_games);
        
        // Alternate who plays white
        let result = if game_num % 2 == 1 {
            play_game(player1, player2, config.max_moves_per_game)
        } else {
            // Swap colors
            let game_result = play_game(player2, player1, config.max_moves_per_game);
            // Swap result back
            GameStats {
                result: match game_result.result {
                    GameResult::Player1Win => GameResult::Player2Win,
                    GameResult::Player2Win => GameResult::Player1Win,
                    GameResult::Draw => GameResult::Draw,
                },
                num_moves: game_result.num_moves,
                player1_time: game_result.player2_time,
                player2_time: game_result.player1_time,
                player1_avg_positions: game_result.player2_avg_positions,
                player2_avg_positions: game_result.player1_avg_positions,
            }
        };

        match result.result {
            GameResult::Player1Win => {
                player1_wins += 1;
                println!("  Result: {} wins!", player1_name);
            }
            GameResult::Player2Win => {
                player2_wins += 1;
                println!("  Result: {} wins!", player2_name);
            }
            GameResult::Draw => {
                draws += 1;
                println!("  Result: Draw");
            }
        }
        
        println!("  Moves: {}", result.num_moves);
        println!("  {} time: {:.2}s", player1_name, result.player1_time.as_secs_f64());
        println!("  {} time: {:.2}s", player2_name, result.player2_time.as_secs_f64());
        println!();

        game_stats.push(result);
    }

    MatchResult {
        player1_wins,
        player2_wins,
        draws,
        game_stats,
        player1_name,
        player2_name,
    }
}

/// Play a single game between two engines
fn play_game(
    player1: &mut MinimaxEngine,
    player2: &mut MinimaxEngine,
    max_moves: usize,
) -> GameStats {
    let mut game = Game::new();
    let mut player1_time = Duration::ZERO;
    let mut player2_time = Duration::ZERO;
    let mut player1_positions = Vec::new();
    let mut player2_positions = Vec::new();
    let mut move_count = 0;

    while move_count < max_moves {
        if game.board.is_game_over() {
            break;
        }

        let (engine, time_accumulator, positions_accumulator) = if game.board.is_white_to_move() {
            (player1 as &mut MinimaxEngine, &mut player1_time, &mut player1_positions)
        } else {
            (player2 as &mut MinimaxEngine, &mut player2_time, &mut player2_positions)
        };

        // Find and apply move
        let start = Instant::now();
        match engine.find_best_move(&game.board) {
            Ok(best_move) => {
                let elapsed = start.elapsed();
                *time_accumulator += elapsed;
                
                let stats = engine.get_statistics();
                positions_accumulator.push(stats.positions_evaluated);
                
                match game.apply_move(best_move) {
                    Ok(_) => {
                        move_count += 1;
                    }
                    Err(_) => {
                        // Invalid move, game is a draw
                        break;
                    }
                }
            }
            Err(_) => {
                // No moves available
                break;
            }
        }
    }

    // Determine result
    let result = if game.board.is_game_over() {
        if game.board.is_draw() {
            GameResult::Draw
        } else if game.board.white_wins() {
            GameResult::Player1Win
        } else {
            GameResult::Player2Win
        }
    } else {
        // Max moves reached
        GameResult::Draw
    };

    let player1_avg_positions = if player1_positions.is_empty() {
        0.0
    } else {
        player1_positions.iter().sum::<u64>() as f64 / player1_positions.len() as f64
    };

    let player2_avg_positions = if player2_positions.is_empty() {
        0.0
    } else {
        player2_positions.iter().sum::<u64>() as f64 / player2_positions.len() as f64
    };

    GameStats {
        result,
        num_moves: move_count,
        player1_time,
        player2_time,
        player1_avg_positions,
        player2_avg_positions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_result_percentages() {
        let result = MatchResult {
            player1_wins: 7,
            player2_wins: 2,
            draws: 1,
            game_stats: vec![],
            player1_name: "Test1".to_string(),
            player2_name: "Test2".to_string(),
        };

        assert!((result.player1_win_percentage() - 70.0).abs() < 0.1);
        assert!((result.player2_win_percentage() - 20.0).abs() < 0.1);
        assert!((result.draw_percentage() - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_match_config_default() {
        let config = MatchConfig::default();
        assert_eq!(config.num_games, 10);
        assert!(config.time_per_move_ms > 0);
        assert!(config.max_moves_per_game > 0);
    }
}
