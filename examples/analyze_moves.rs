use arx_engine::{Board, Game, Position};
use base64::{engine::general_purpose, Engine as _};

fn main() {
    // Load the board from base64
    let board_str = "BwAEBTgFBABeAAADAAAAAAAAAQABAQAxAAAAAAABAAYBEQAAAAAAAAAAAAAAAAAAAABBAAAAQUFBQUFBAEEAAABCAAAAAABDR0ZERXhFRAAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();

    let mut board_data = [0; 83];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }

    let board = Board::from_binary(board_data).unwrap();

    // Analyze key moves
    println!("=== KEY PIECES AND MOVES ===\n");

    // Check D7
    let d7 = Position::new(3, 2);
    println!("D7 (x=3, y=2): {:?}", board.get_piece(&d7));

    // Check E6
    let e6 = Position::new(4, 3);
    println!("E6 (x=4, y=3): {:?}", board.get_piece(&e6));

    // Check G9
    let g9 = Position::new(6, 0);
    println!("G9 (x=6, y=0): {:?}", board.get_piece(&g9));

    // Check I9
    let i9 = Position::new(8, 0);
    println!("I9 (x=8, y=0): {:?}", board.get_piece(&i9));

    // Simulate both moves and see material change
    println!("\n=== MOVE ANALYSIS ===\n");

    let game = Game::from_board(board.clone());

    // Try G9->I9
    println!("1. G9->I9 (Paladin takes Dragon+Commander):");
    if let Ok(new_board) = game.apply_move_copy(arx_engine::game::Move {
        from: g9,
        to: i9,
        unstack: false,
    }) {
        println!("   Move successful!");
        println!("   Piece at I9 after: {:?}", new_board.get_piece(&i9));
        println!("   Piece at G9 after: {:?}", new_board.get_piece(&g9));

        // Count material
        let mut white_count = 0;
        let mut black_count = 0;
        for y in 0..9 {
            for x in 0..9 {
                if let Some(p) = new_board.get_piece(&Position::new(x, y)) {
                    if p.color == arx_engine::Color::White {
                        white_count += 1;
                    } else {
                        black_count += 1;
                    }
                }
            }
        }
        println!(
            "   White pieces: {}, Black pieces: {}",
            white_count, black_count
        );
    }

    // Try D7->E6
    println!("\n2. D7->E6:");
    if let Ok(new_board) = game.apply_move_copy(arx_engine::game::Move {
        from: d7,
        to: e6,
        unstack: false,
    }) {
        println!("   Move successful!");
        println!("   Piece at E6 after: {:?}", new_board.get_piece(&e6));
        println!("   Piece at D7 after: {:?}", new_board.get_piece(&d7));

        // Count material
        let mut white_count = 0;
        let mut black_count = 0;
        for y in 0..9 {
            for x in 0..9 {
                if let Some(p) = new_board.get_piece(&Position::new(x, y)) {
                    if p.color == arx_engine::Color::White {
                        white_count += 1;
                    } else {
                        black_count += 1;
                    }
                }
            }
        }
        println!(
            "   White pieces: {}, Black pieces: {}",
            white_count, black_count
        );
    }
}
