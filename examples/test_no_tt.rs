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

    println!("Testing with different configurations:\n");

    // Test 1: No transposition table, no quiescence
    let mut engine1 = MinimaxEngine::with_config(MinimaxConfig {
        max_depth: 6,
        use_transposition_table: false,
        use_quiescence: false,
        time_limit_ms: 10000,
        material_weight: 1.0,
        territorial_weight: 0.0,
        mobility_weight: 0.0,
        king_safety_weight: 0.0,
        ..Default::default()
    });

    let move1 = engine1.find_best_move(&board).unwrap();
    println!("1. Pure material (weight=1.0), no TT, no quiescence:");
    println!(
        "   {} -> {} {}",
        move1.from.to_string(),
        move1.to.to_string(),
        if move1.from == g9 && move1.to == i9 {
            "✓"
        } else {
            "✗"
        }
    );

    // Test 2: Material only, with TT
    let mut engine2 = MinimaxEngine::with_config(MinimaxConfig {
        max_depth: 6,
        use_transposition_table: true,
        use_quiescence: false,
        time_limit_ms: 10000,
        material_weight: 1.0,
        territorial_weight: 0.0,
        mobility_weight: 0.0,
        king_safety_weight: 0.0,
        ..Default::default()
    });

    let move2 = engine2.find_best_move(&board).unwrap();
    println!("\n2. Pure material (weight=1.0), with TT, no quiescence:");
    println!(
        "   {} -> {} {}",
        move2.from.to_string(),
        move2.to.to_string(),
        if move2.from == g9 && move2.to == i9 {
            "✓"
        } else {
            "✗"
        }
    );

    // Test 3: Default weights
    let mut engine3 = MinimaxEngine::with_config(MinimaxConfig {
        max_depth: 6,
        time_limit_ms: 10000,
        ..Default::default()
    });

    let move3 = engine3.find_best_move(&board).unwrap();
    println!("\n3. Default config:");
    println!(
        "   {} -> {} {}",
        move3.from.to_string(),
        move3.to.to_string(),
        if move3.from == g9 && move3.to == i9 {
            "✓"
        } else {
            "✗"
        }
    );
}
