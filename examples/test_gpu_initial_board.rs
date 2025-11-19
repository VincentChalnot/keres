use arx_engine::board::Board;
use arx_engine::engine::MoveGenerationEngine;

fn main() {
    // Create initial board state
    let board = Board::new();
    let board_binary = board.to_binary();
    
    println!("Testing GPU move generation on initial board...");
    println!("White to move: {}", board.is_white_to_move());
    println!("Game over: {}", board.is_game_over());
    println!("Flags byte: 0b{:08b}", board_binary[81]);
    
    // Create GPU move generation engine
    let engine = match MoveGenerationEngine::new_sync() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Failed to create GPU engine: {}", e);
            eprintln!("This is expected if no GPU is available.");
            return;
        }
    };
    
    // Generate moves
    match engine.generate_moves(&board_binary) {
        Ok(moves) => {
            println!("Successfully generated {} moves", moves.len());
            if moves.len() > 0 {
                println!("SUCCESS: GPU move generation works!");
                // Show first few moves as examples
                for (i, mv) in moves.iter().take(5).enumerate() {
                    println!("  Move {}: 0x{:04x}", i+1, mv);
                }
            } else {
                println!("ERROR: No moves generated for initial board!");
            }
        }
        Err(e) => {
            println!("ERROR: Failed to generate moves: {}", e);
        }
    }
}
