pub mod board;
pub mod cli_rendering;
pub mod engine;
pub mod game;
pub mod game_over;
pub mod moves;
pub mod tui;

// Re-export main types
pub use board::{Board, Color, Piece, PieceType, Position, BOARD_DIMENSION, BOARD_SIZE};
pub use game::{Game, UndoInfo};
pub use moves::{Move, MoveGenerator, PotentialMove};
pub use tui::run_tui;
