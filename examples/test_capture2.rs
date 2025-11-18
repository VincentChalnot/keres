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
    
    println!("=== BOARD STATE ===");
    println!("Turn to move: {}", if board.is_white_to_move() { "White" } else { "Black" });
    
    // Find all pieces
    println!("\n=== BLACK PIECES ===");
    for y in 0..9 {
        for x in 0..9 {
            let pos = Position::new(x, y);
            if let Some(piece) = board.get_piece(&pos) {
                if piece.color == Color::Black {
                    println!("Black piece at {}: {:?} (top: {:?})", 
                        pos.to_string(), piece.bottom, piece.top);
                }
            }
        }
    }
    
    println!("\n=== WHITE PIECES ===");
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
    
    // Check key positions
    // I9 should be row 0 (y=0), column I (x=8)
    let i9 = Position::new(8, 0);
    println!("\n=== KEY POSITIONS ===");
    println!("I9 (x=8, y=0): {:?}", board.get_piece(&i9));
    
    // G9 should be row 0 (y=0), column G (x=6)
    let g9 = Position::new(6, 0);
    println!("G9 (x=6, y=0): {:?}", board.get_piece(&g9));
    
    // Check what moves are available from G9
    let game = Game::from_board(board.clone());
    let g9_moves = game.get_moves(&g9);
    println!("\nMoves from G9 (Black Paladin):");
    for m in &g9_moves {
        println!("  {} -> {}", m.from.to_string(), m.to.to_string());
        if m.to == i9 {
            println!("    ^^^ This captures the White Dragon+Commander!");
        }
    }
    
    // Test minimax
    println!("\n=== MINIMAX TESTING ===");
    let mut engine = MinimaxEngine::with_config(MinimaxConfig {
        max_depth: 6,
        time_limit_ms: 5000,
        ..Default::default()
    });
    
    println!("Running minimax (Black to move)...");
    let best_move = engine.find_best_move(&board).unwrap();
    println!("Best move: {} -> {}", best_move.from.to_string(), best_move.to.to_string());
    
    if best_move.from == g9 && best_move.to == i9 {
        println!("✓ Minimax correctly found the capture!");
    } else {
        println!("✗ Minimax missed the capture of Dragon+Commander!");
    }
    
    let stats = engine.get_statistics();
    println!("\nStatistics:");
    println!("  Positions evaluated: {}", stats.positions_evaluated);
    println!("  Search time: {}ms", stats.search_time_ms);
}
