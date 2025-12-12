use base64::{engine::general_purpose, Engine as _};
use keres_engine::{Board, Game, Position};

fn calculate_simple_material(board: &Board) -> i32 {
    let piece_values = [
        0, 10, 20, 100, 20, 25, 30,
        15, // Soldier, Jester, Commander, Paladin, Guard, Dragon, Ballista
    ];

    let mut white_material = 0;
    let mut black_material = 0;

    for y in 0..9 {
        for x in 0..9 {
            if let Some(piece) = board.get_piece(&Position::new(x, y)) {
                let mut value = match piece.bottom {
                    keres_engine::PieceType::Soldier => piece_values[1],
                    keres_engine::PieceType::Jester => piece_values[2],
                    keres_engine::PieceType::Commander => piece_values[3],
                    keres_engine::PieceType::Paladin => piece_values[4],
                    keres_engine::PieceType::Guard => piece_values[5],
                    keres_engine::PieceType::Dragon => piece_values[6],
                    keres_engine::PieceType::Ballista => piece_values[7],
                    keres_engine::PieceType::King => 10000,
                };

                if let Some(top) = piece.top {
                    let top_value = match top {
                        keres_engine::PieceType::Soldier => piece_values[1],
                        keres_engine::PieceType::Jester => piece_values[2],
                        keres_engine::PieceType::Commander => piece_values[3],
                        keres_engine::PieceType::Paladin => piece_values[4],
                        keres_engine::PieceType::Guard => piece_values[5],
                        keres_engine::PieceType::Dragon => piece_values[6],
                        keres_engine::PieceType::Ballista => piece_values[7],
                        keres_engine::PieceType::King => 10000,
                    };
                    let total = value + top_value;
                    let bonus = (total as f32 * 0.30) as i32;
                    value = total + bonus;
                }

                if piece.color == keres_engine::Color::White {
                    white_material += value;
                } else {
                    black_material += value;
                }
            }
        }
    }

    white_material - black_material
}

fn main() {
    // Load the board from base64
    let board_str = "BwAEBTgFBABeAAADAAAAAAAAAQABAQAxAAAAAAABAAYBEQAAAAAAAAAAAAAAAAAAAABBAAAAQUFBQUFBAEEAAABCAAAAAABDR0ZERXhFRAAAAAA==";
    let bytes = general_purpose::STANDARD.decode(board_str).unwrap();

    let mut board_data = [0; 83];
    for (i, &byte) in bytes.iter().enumerate() {
        board_data[i] = byte;
    }

    let board = Board::from_binary(board_data).unwrap();
    let game = Game::from_board(board.clone());

    println!("=== MATERIAL COMPARISON ===\n");

    let initial_material = calculate_simple_material(&board);
    println!(
        "Initial position: {} (positive = White advantage)",
        initial_material
    );

    // After G9->I9
    let g9 = Position::new(6, 0);
    let i9 = Position::new(8, 0);
    if let Ok(board_after_capture) = game.apply_move_copy(keres_engine::game::Move {
        from: g9,
        to: i9,
        unstack: false,
    }) {
        let material_after = calculate_simple_material(&board_after_capture);
        println!("\nAfter G9->I9 (capture Dragon+Commander):");
        println!(
            "  Material: {} (positive = White advantage)",
            material_after
        );
        println!(
            "  Change: {} (negative = Black gained material)",
            material_after - initial_material
        );
        println!("  Expected: Black should gain ~169 (Dragon 30 + Commander 100 + 30% = 169)");
    }

    // After D7->E6
    let d7 = Position::new(3, 2);
    let e6 = Position::new(4, 3);
    if let Ok(board_after_stack) = game.apply_move_copy(keres_engine::game::Move {
        from: d7,
        to: e6,
        unstack: false,
    }) {
        let material_after = calculate_simple_material(&board_after_stack);
        println!("\nAfter D7->E6 (stack Soldier on Dragon):");
        println!(
            "  Material: {} (positive = White advantage)",
            material_after
        );
        println!(
            "  Change: {} (should be near 0, just stacking own pieces)",
            material_after - initial_material
        );
        println!("  Note: Gain from stacking bonus = (10+30)*0.30 = 12");
    }
}
