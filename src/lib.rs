pub mod api;
pub mod board;
pub mod cli_rendering;
pub mod engine;
pub mod game;
pub mod game_over;
pub mod moves;

// Re-export main types
pub use board::{Board, Color, Piece, PieceType, Position, BOARD_DIMENSION, BOARD_SIZE};
pub use game::{Game, UndoInfo};
pub use moves::{Move, MoveGenerator, PotentialMove};
