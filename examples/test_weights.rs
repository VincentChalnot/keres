use arx_engine::engine::{MinimaxConfig, MinimaxEngine};
use arx_engine::{Board, Position};
use base64::{engine::general_purpose, Engine as _};

fn main() {
    let board_str = "BwAEBTgFBABeAAADAAAAAAAAAQABAQAxAAAAAAABAAYBEQAAAAAAAAAAAAAAAAAAAABBAAAAQUFBQUFBAEEAAABCAAAAAABDR0ZERXhFRAAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();

    let mut board_data = [0; 83];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }

    let board = Board::from_binary(board_data).unwrap();

    let g9 = Position::new(6, 0);
    let i9 = Position::new(8, 0);

    println!("Testing different material weights:\n");

    for material_weight in [0.50, 0.60, 0.70, 0.80, 0.90, 1.00] {
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 6,
            time_limit_ms: 5000,
            material_weight,
            territorial_weight: 0.15,
            mobility_weight: 0.10,
            king_safety_weight: 0.05,
            ..Default::default()
        });

        let best_move = engine.find_best_move(&board).unwrap();
        let correct = best_move.from == g9 && best_move.to == i9;

        println!(
            "Material weight {:.2}: {} -> {} {}",
            material_weight,
            best_move.from.to_string(),
            best_move.to.to_string(),
            if correct { "✓" } else { "✗" }
        );
    }
}
