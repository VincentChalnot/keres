use arx_engine::{engine::{MctsEngine, EngineConfig}, Game};

fn main() {
    println!("Arx Engine - MCTS GPU Engine Example");
    println!("=====================================\n");

    // Create a new game
    let mut game = Game::new();

    // For this example, we'll use Easy difficulty with GPU acceleration
    let config = EngineConfig {
        max_depth: 12,
        simulations_per_move: 10000,
        exploration_constant: 1.414,
        gpu_batch_size: 2048,
        use_gpu_simulation: true,
    };

    println!("Creating MCTS engine with following difficulty...");
    println!("  Max depth: {}", config.max_depth);
    println!("  Simulations per move: {}", config.simulations_per_move);
    println!("  GPU batch size: {}", config.gpu_batch_size);
    println!("  GPU simulation: {}", config.use_gpu_simulation);
    
    let mut engine = match MctsEngine::with_config(config) {
        Ok(e) => {
            println!("✓ Engine created successfully\n");
            e
        }
        Err(e) => {
            eprintln!("✗ Failed to create engine: {}", e);
            eprintln!("This may happen if no GPU is available.");
            return;
        }
    };

    println!("Playing first 500 moves with the engine:\n");

    for move_num in 1..=500 {
        // Check if game is over
        if game.board.is_game_over() {
            println!("Game over!");
            break;
        }

        // Find best move
        println!("Move {}: Thinking...", move_num);
        let start = std::time::Instant::now();
        
        let best_move = match engine.find_best_move(&game.board) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("✗ Failed to find move: {}", e);
                break;
            }
        };
        
        let elapsed = start.elapsed();

        // Get statistics
        let stats = engine.get_statistics();
        
        // Display the move
        let from_str = best_move.from.to_string();
        let to_str = best_move.to.to_string();
        let unstack_str = if best_move.unstack { " (unstack)" } else { "" };
        
        println!("  Best move: {} -> {}{}", from_str, to_str, unstack_str);
        println!("  Time: {:.3}s", elapsed.as_secs_f64());
        println!("  Statistics:");
        println!("    - Total moves evaluated: {}", stats.total_moves_evaluated);
        println!("    - Simulations run: {}", stats.simulations_run);
        println!("    - GPU batches processed: {}", stats.gpu_batches_processed);
        println!("    - CPU simulations: {}", stats.cpu_simulations);
        println!("    - Avg moves/simulation: {:.2}", stats.avg_moves_per_simulation());

        // Apply the move
        match game.apply_move(best_move) {
            Ok(_) => println!("  ✓ Move applied\n"),
            Err(e) => {
                eprintln!("  ✗ Failed to apply move: {}", e);
                break;
            }
        }
    }

    // Final statistics
    let final_stats = engine.get_statistics();
    println!("═══════════════════════════════════════");
    println!("Final Statistics:");
    println!("  Total moves evaluated: {}", final_stats.total_moves_evaluated);
    println!("  Total simulations run: {}", final_stats.simulations_run);
    println!("  GPU batches processed: {}", final_stats.gpu_batches_processed);
    println!("  CPU simulations: {}", final_stats.cpu_simulations);
    println!("  Average moves per simulation: {:.2}", final_stats.avg_moves_per_simulation());
    println!("═══════════════════════════════════════");

    println!("\nExample completed!");
    println!("\nConfiguration tips:");
    println!("- Increase max_depth for stronger play (but slower)");
    println!("- Increase simulations_per_move for more accurate evaluation");
    println!("- Increase gpu_batch_size to process more simulations in parallel");
    println!("- Set use_gpu_simulation to false to use CPU-only mode");
    println!("- Decrease all parameters for faster but weaker play");
}
