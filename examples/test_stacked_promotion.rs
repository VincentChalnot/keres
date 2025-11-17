// Test to demonstrate the issue: when a stack reaches the promotion zone,
// both pieces should be promoted if they are Soldier or Ballista

use arx_engine::board::{Board, Color, Piece, PieceType, Position};
use arx_engine::game::{Game, Move};

fn main() {
    // Test case 1: White Soldier on top of Soldier stack
    // Both should be promoted when reaching y=0
    let mut game = Game::new();
    let stack_pos = Position::new(4, 1);
    
    // Create a stack: Soldier (bottom) + Soldier (top)
    game.board.set_piece(&stack_pos, Some(Piece::new(
        Color::White, 
        PieceType::Soldier,  // bottom
        Some(PieceType::Soldier)  // top
    )));
    
    println!("Before move:");
    if let Some(piece) = game.board.get_piece(&stack_pos) {
        println!("  Position (4,1): bottom={:?}, top={:?}", piece.bottom, piece.top);
    }
    
    // Move the stack to y=0 (promotion zone for white)
    let mv = Move {
        from: stack_pos,
        to: Position::new(3, 0),
        unstack: false,
    };
    
    match game.apply_move_copy(mv) {
        Ok(new_board) => {
            println!("\nAfter move to (3,0):");
            if let Some(piece) = new_board.get_piece(&Position::new(3, 0)) {
                println!("  Position (3,0): bottom={:?}, top={:?}", piece.bottom, piece.top);
                
                // Expected: both should be Paladin
                // Actual (bug): only top is promoted
                if piece.bottom == PieceType::Paladin && piece.top == Some(PieceType::Paladin) {
                    println!("✓ PASS: Both pieces promoted correctly!");
                } else {
                    println!("✗ FAIL: Expected both to be Paladin");
                    println!("  Bottom: {:?} (expected Paladin)", piece.bottom);
                    println!("  Top: {:?} (expected Some(Paladin))", piece.top);
                }
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    
    // Test case 2: White Ballista on top of Ballista stack
    println!("\n\n--- Test case 2: Ballista stack ---");
    let mut game2 = Game::new();
    let stack_pos2 = Position::new(4, 1);
    
    game2.board.set_piece(&stack_pos2, Some(Piece::new(
        Color::White, 
        PieceType::Ballista,  // bottom
        Some(PieceType::Ballista)  // top
    )));
    
    println!("Before move:");
    if let Some(piece) = game2.board.get_piece(&stack_pos2) {
        println!("  Position (4,1): bottom={:?}, top={:?}", piece.bottom, piece.top);
    }
    
    let mv2 = Move {
        from: stack_pos2,
        to: Position::new(4, 0),
        unstack: false,
    };
    
    match game2.apply_move_copy(mv2) {
        Ok(new_board) => {
            println!("\nAfter move to (4,0):");
            if let Some(piece) = new_board.get_piece(&Position::new(4, 0)) {
                println!("  Position (4,0): bottom={:?}, top={:?}", piece.bottom, piece.top);
                
                // Expected: both should be Commander
                if piece.bottom == PieceType::Commander && piece.top == Some(PieceType::Commander) {
                    println!("✓ PASS: Both pieces promoted correctly!");
                } else {
                    println!("✗ FAIL: Expected both to be Commander");
                    println!("  Bottom: {:?} (expected Commander)", piece.bottom);
                    println!("  Top: {:?} (expected Some(Commander))", piece.top);
                }
            }
        }
        Err(e) => println!("Error: {}", e),
    }
    
    // Test case 3: Mixed stack - Soldier on top of Guard
    // Only Soldier should be promoted
    println!("\n\n--- Test case 3: Soldier on Guard stack ---");
    let mut game3 = Game::new();
    let stack_pos3 = Position::new(4, 1);
    
    game3.board.set_piece(&stack_pos3, Some(Piece::new(
        Color::White, 
        PieceType::Guard,  // bottom - should NOT be promoted
        Some(PieceType::Soldier)  // top - should be promoted
    )));
    
    println!("Before move:");
    if let Some(piece) = game3.board.get_piece(&stack_pos3) {
        println!("  Position (4,1): bottom={:?}, top={:?}", piece.bottom, piece.top);
    }
    
    let mv3 = Move {
        from: stack_pos3,
        to: Position::new(3, 0),
        unstack: false,
    };
    
    match game3.apply_move_copy(mv3) {
        Ok(new_board) => {
            println!("\nAfter move to (3,0):");
            if let Some(piece) = new_board.get_piece(&Position::new(3, 0)) {
                println!("  Position (3,0): bottom={:?}, top={:?}", piece.bottom, piece.top);
                
                // Expected: Guard stays Guard, Soldier becomes Paladin
                if piece.bottom == PieceType::Guard && piece.top == Some(PieceType::Paladin) {
                    println!("✓ PASS: Only Soldier promoted correctly!");
                } else {
                    println!("✗ FAIL: Expected Guard+Paladin");
                    println!("  Bottom: {:?} (expected Guard)", piece.bottom);
                    println!("  Top: {:?} (expected Some(Paladin))", piece.top);
                }
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}
