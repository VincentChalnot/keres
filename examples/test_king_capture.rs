use arx_engine::{Game, Board, Position};
use arx_engine::engine::{MinimaxEngine, MinimaxConfig};
use base64::{engine::general_purpose, Engine as _};

fn main() {
    let board_str = "BwAEADgFXgAAAAADAAAAAAAAAAAAAAAFAAAAAAkBAAABAAAAAAAAAAAAAAAAAAAAAEJBAAAAQUFBAABBAEEAAAAAAAAAAAAAR0ZERXhFRAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();
    
    let mut board_data = [0; 82];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }
    
    let board = Board::from_binary(board_data).unwrap();
    let game = Game::from_board(board.clone());
    
    // Simulate F9->E8
    let f9 = Position::new(5, 0);
    let e8 = Position::new(4, 1);
    
    println!("Simulating Black's move F9->E8...");
    if let Ok(board_after_guard_move) = game.apply_move_copy(arx_engine::game::Move {
        from: f9,
        to: e8,
        unstack: false,
    }) {
        println!("Move successful. Now it's White's turn.\n");
        
        // Check what White can do
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 2,
            time_limit_ms: 3000,
            ..Default::default()
        });
        
        let white_response = engine.find_best_move(&board_after_guard_move).unwrap();
        println!("White's best move: {} -> {}", 
            white_response.from.to_string(),
            white_response.to.to_string()
        );
        
        let e9 = Position::new(4, 0);
        let g9 = Position::new(6, 0);
        
        if white_response.from == g9 && white_response.to == e9 {
            println!("✓ White captures the Black King!");
        }
        
        // Apply White's move
        if let Ok(board_after_white) = Game::from_board(board_after_guard_move).apply_move_copy(white_response) {
            println!("\nAfter White's move:");
            println!("  Black King at E9: {:?}", board_after_white.get_piece(&e9));
            println!("  Is game over? {}", board_after_white.is_game_over());
        }
    }
}
