use base64::{engine::general_purpose, Engine as _};
use keres_engine::{Board, Game, Position};

fn main() {
    let board_str = "BwAEADgFXgAAAAADAAAAAAAAAAAAAAAFAAAAAAkBAAABAAAAAAAAAAAAAAAAAAAAAEJBAAAAQUFBAABBAEEAAAAAAAAAAAAAR0ZERXhFRAAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();

    let mut board_data = [0; 83];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }

    let board = Board::from_binary(board_data).unwrap();
    let game = Game::from_board(board.clone());

    // Simulate F9->E8
    let f9 = Position::new(5, 0);
    let e8 = Position::new(4, 1);

    if let Ok(board_after_guard_move) = game.apply_move_copy(keres_engine::game::Move {
        from: f9,
        to: e8,
        unstack: false,
    }) {
        println!("After Black F9->E8, checking White's options from G9...\n");

        let g9 = Position::new(6, 0);
        let game_after = Game::from_board(board_after_guard_move.clone());

        let g9_moves = game_after.get_moves(&g9);
        println!("White Knight+Rook at G9 can move to:");
        for m in &g9_moves {
            println!("  {} -> {}", m.from.to_string(), m.to.to_string());
            let e9 = Position::new(4, 0);
            if m.to == e9 {
                println!("    ^^^ THIS CAPTURES THE BLACK KING!");
            }
        }

        // Check if E9 is in the list
        let e9 = Position::new(4, 0);
        let can_capture_king = g9_moves.iter().any(|m| m.to == e9);

        if can_capture_king {
            println!("\n✓ G9 CAN capture the king at E9");
        } else {
            println!("\n✗ G9 CANNOT capture the king at E9");
            println!("\nLet's check what piece is at E9:");
            println!("  E9: {:?}", board_after_guard_move.get_piece(&e9));
        }
    }
}
