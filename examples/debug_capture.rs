use base64::{engine::general_purpose, Engine as _};
use keres_engine::engine::{MinimaxConfig, MinimaxEngine};
use keres_engine::{Board, Position};

fn main() {
    // Load the board from base64
    let board_str = "BwAEBTgFBABeAAADAAAAAAAAAQABAQAxAAAAAAABAAYBEQAAAAAAAAAAAAAAAAAAAABBAAAAQUFBQUFBAEEAAABCAAAAAABDR0ZERXhFRAAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();

    let mut board_data = [0; 83];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }

    let board = Board::from_binary(board_data).unwrap();

    println!("=== TESTING DIFFERENT DEPTHS ===");

    for depth in [2, 3, 4, 5, 6, 7, 8] {
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: depth,
            time_limit_ms: 10000,
            ..Default::default()
        });

        let start = std::time::Instant::now();
        let best_move = engine.find_best_move(&board).unwrap();
        let elapsed = start.elapsed();

        let stats = engine.get_statistics();

        let g9 = Position::new(6, 0);
        let i9 = Position::new(8, 0);
        let captures_dragon = best_move.from == g9 && best_move.to == i9;

        println!(
            "\nDepth {}: {} -> {} {}",
            depth,
            best_move.from.to_string(),
            best_move.to.to_string(),
            if captures_dragon {
                "✓ CAPTURES DRAGON+COMMANDER!"
            } else {
                ""
            }
        );
        println!(
            "  Positions: {}, Time: {:?}",
            stats.positions_evaluated, elapsed
        );
    }
}
