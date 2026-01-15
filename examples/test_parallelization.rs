use keres_engine::engine::{MinimaxEngine, MinimaxConfig};
use keres_engine::Board;
use std::time::Instant;

fn main() {
    println!("Testing Minimax CPU Parallelization");
    println!("====================================\n");
    
    let board = Board::new();
    
    // Test with different configurations
    let configs = vec![
        (2, "Shallow search (depth 2)"),
        (4, "Medium search (depth 4)"),
    ];
    
    for (depth, description) in configs {
        println!("{}", description);
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: depth,
            time_limit_ms: 10000,
            use_quiescence: false,
            ..Default::default()
        });
        
        let start = Instant::now();
        match engine.find_best_move(&board) {
            Ok(best_move) => {
                let elapsed = start.elapsed();
                let stats = engine.get_statistics();
                
                println!("  Best move: {} -> {}", 
                    best_move.from.to_string(), 
                    best_move.to.to_string());
                println!("  Time: {:?}", elapsed);
                println!("  Positions evaluated: {}", stats.positions_evaluated);
                println!("  Alpha-beta cutoffs: {}", stats.ab_cutoffs);
                println!();
            }
            Err(e) => {
                println!("  Error: {}\n", e);
            }
        }
    }
    
    println!("Parallelization test completed!");
    println!("Each root move is now evaluated in its own thread.");
}
