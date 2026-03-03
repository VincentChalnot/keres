pub mod board;
pub mod cli_rendering;
pub mod game;
pub mod game_over;
pub mod moves;
pub mod tui;

// Re-export main types
pub use board::{Board, Color, Piece, PieceType, Position, UndoInfo, BOARD_DIMENSION, BOARD_SIZE};
pub use game::{Game, Move, PotentialMove};
pub use moves::MoveGenerator;
pub use tui::run_tui;

