pub mod board;
pub mod cli_rendering;
pub mod engine;
pub mod game;
pub mod tui;

// Re-export main types
pub use board::{Board, Color, Piece, PieceType, Position, BOARD_DIMENSION, BOARD_SIZE};
pub use game::{Game, Move, PotentialMove};
pub use tui::run_tui;
// Re-export main engine types (others available via engine::*)
pub use engine::{EngineConfig, MctsEngine, SearchStatistics};
