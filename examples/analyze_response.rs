use arx_engine::{Game, Board, Position};
use arx_engine::engine::{MinimaxEngine, MinimaxConfig};
use base64::{engine::general_purpose, Engine as _};

fn main() {
    // Load the board from base64
    let board_str = "BwAEBTgFBABeAAADAAAAAAAAAQABAQAxAAAAAAABAAYBEQAAAAAAAAAAAAAAAAAAAABBAAAAQUFBQUFBAEEAAABCAAAAAABDR0ZERXhFRAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();
    
    let mut board_data = [0; 82];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }
    
    let board = Board::from_binary(board_data).unwrap();
    let game = Game::from_board(board.clone());
    
    println!("=== ANALYZING WHITE'S RESPONSE ===\n");
    
    // After Black plays G9->I9
    let g9 = Position::new(6, 0);
    let i9 = Position::new(8, 0);
    
    if let Ok(board_after_capture) = game.apply_move_copy(arx_engine::game::Move {
        from: g9,
        to: i9,
        unstack: false,
    }) {
        println!("After Black G9->I9 (captured Dragon+Commander):");
        println!("Now it's White's turn.\n");
        
        // What does White do?
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 4,
            time_limit_ms: 5000,
            ..Default::default()
        });
        
        let white_response = engine.find_best_move(&board_after_capture).unwrap();
        println!("White's best response: {} -> {}", 
            white_response.from.to_string(), 
            white_response.to.to_string());
        
        // Check if White can recapture
        if white_response.to == i9 {
            println!("  ^^ White recaptures at I9!");
            if let Some(piece) = board_after_capture.get_piece(&white_response.from) {
                println!("  Using: {:?} (top: {:?})", piece.bottom, piece.top);
            }
        }
        
        // Apply White's response
        if let Ok(board_after_white) = Game::from_board(board_after_capture).apply_move_copy(white_response) {
            println!("\nAfter White's response:");
            println!("  Piece at I9: {:?}", board_after_white.get_piece(&i9));
        }
    }
    
    println!("\n=== ANALYZING WHITE'S RESPONSE TO D7->E6 ===\n");
    
    // After Black plays D7->E6
    let d7 = Position::new(3, 2);
    let e6 = Position::new(4, 3);
    
    if let Ok(board_after_stack) = game.apply_move_copy(arx_engine::game::Move {
        from: d7,
        to: e6,
        unstack: false,
    }) {
        println!("After Black D7->E6 (stacked Soldier on Dragon):");
        println!("Now it's White's turn.\n");
        
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 4,
            time_limit_ms: 5000,
            ..Default::default()
        });
        
        let white_response = engine.find_best_move(&board_after_stack).unwrap();
        println!("White's best response: {} -> {}", 
            white_response.from.to_string(), 
            white_response.to.to_string());
    }
}
