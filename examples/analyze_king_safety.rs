use base64::{engine::general_purpose, Engine as _};
use keres_engine::engine::{MinimaxConfig, MinimaxEngine};
use keres_engine::{Board, Color, Position};

fn main() {
    let board_str = "BwAEADgFXgAAAAADAAAAAAAAAAAAAAAFAAAAAAkBAAABAAAAAAAAAAAAAAAAAAAAAEJBAAAAQUFBAABBAEEAAAAAAAAAAAAAR0ZERXhFRAAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();

    let mut board_data = [0; 83];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }

    let board = Board::from_binary(board_data).unwrap();

    println!("=== BOARD ANALYSIS ===");
    println!(
        "Turn to move: {}\n",
        if board.is_white_to_move() {
            "White"
        } else {
            "Black"
        }
    );

    // Find all pieces
    println!("=== BLACK PIECES ===");
    for y in 0..9 {
        for x in 0..9 {
            let pos = Position::new(x, y);
            if let Some(piece) = board.get_piece(&pos) {
                if piece.color == Color::Black {
                    println!(
                        "Black piece at {}: {:?} (top: {:?})",
                        pos.to_string(),
                        piece.bottom,
                        piece.top
                    );
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
                    println!(
                        "White piece at {}: {:?} (top: {:?})",
                        pos.to_string(),
                        piece.bottom,
                        piece.top
                    );
                }
            }
        }
    }

    // Key positions
    let f9 = Position::new(5, 0); // F9 - Black Guard
    let e9 = Position::new(4, 0); // E9 - Black King
    let g9 = Position::new(6, 0); // G9 - Should have White piece that can capture king

    println!("\n=== KEY POSITIONS ===");
    println!("F9 (x=5, y=0): {:?}", board.get_piece(&f9));
    println!("E9 (x=4, y=0): {:?}", board.get_piece(&e9));
    println!("G9 (x=6, y=0): {:?}", board.get_piece(&g9));

    // Test engine multiple times
    println!("\n=== ENGINE TESTS ===");
    for i in 1..=5 {
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 6,
            time_limit_ms: 5000,
            ..Default::default()
        });

        let best_move = engine.find_best_move(&board).unwrap();
        println!(
            "Test {}: {} -> {}",
            i,
            best_move.from.to_string(),
            best_move.to.to_string()
        );

        if best_move.from == f9 {
            println!("  ✗ ERROR: Moved F9 Guard, exposing king!");
        }
    }
}
