use arx_engine::{Game, Board, Position, Color};
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
    
    // Display the board state
    println!("Board loaded. White to move: {}", board.is_white_to_move());
    
    // Get all moves
    let game = Game::from_board(board.clone());
    let moves = game.get_all_moves();
    println!("\nTotal legal moves: {}", moves.len());
    
    // Find White pieces
    for y in 0..9 {
        for x in 0..9 {
            let pos = Position::new(x, y);
            if let Some(piece) = board.get_piece(&pos) {
                if piece.color == Color::White {
                    println!("White piece at {}: {:?} (top: {:?})", 
                        pos.to_string(), piece.bottom, piece.top);
                }
            }
        }
    }
    
    // Check I9 (x=8, y=0)
    let i9 = Position::new(8, 0);
    println!("\nPiece at I9: {:?}", board.get_piece(&i9));
    
    // Check G9 (x=6, y=0)
    let g9 = Position::new(6, 0);
    println!("Piece at G9: {:?}", board.get_piece(&g9));
    
    // Test minimax with current default depth (4)
    let mut engine = MinimaxEngine::new();
    println!("\nRunning minimax with depth 4...");
    let best_move = engine.find_best_move(&board).unwrap();
    println!("Best move: {} -> {}", best_move.from.to_string(), best_move.to.to_string());
    
    // Test with depth 6
    let mut engine6 = MinimaxEngine::with_config(MinimaxConfig {
        max_depth: 6,
        ..Default::default()
    });
    println!("\nRunning minimax with depth 6...");
    let best_move6 = engine6.find_best_move(&board).unwrap();
    println!("Best move (depth 6): {} -> {}", best_move6.from.to_string(), best_move6.to.to_string());
    
    let stats = engine6.get_statistics();
    println!("Positions evaluated: {}", stats.positions_evaluated);
    println!("Search time: {}ms", stats.search_time_ms);
}
