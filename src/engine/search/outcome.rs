//! Game-ending rules as the search sees them.

use crate::engine::constants::KING_VALUE;
use crate::game::{Game, UndoInfo};
use crate::game_over::check_game_over;

/// Lowest absolute score that means "the game is decided": a forced king
/// capture or annihilation (`KING_VALUE - ply`). Positional scores never get
/// near it — the whole material of one side is far below `KING_VALUE / 2`.
pub const DECIDED_SCORE: i32 = KING_VALUE / 2;

/// If the move just made (`undo` being its undo record, `game` the position
/// after it) ended the game, its NegaMax score for the side that made it:
/// `KING_VALUE - ply` for a win (so shallower wins score higher, like a king
/// capture), `0` for a draw. `None` while play continues.
///
/// Mirrors `check_game_over` exactly. That full board scan only runs after a
/// capture or once the 40-move counter is reached: no other move can change
/// the outcome (a quiet promotion only creates a paladin or rook, which never
/// turns sufficient material into insufficient material).
#[inline]
pub fn move_outcome(game: &Game, undo: &UndoInfo, ply: usize) -> Option<i32> {
    let win = KING_VALUE - ply as i32;
    if undo.is_king_captured() {
        return Some(win);
    }
    let counter = game.moves_without_capture();
    if counter != 0 && counter < 40 {
        return None;
    }
    let result = check_game_over(&game.board, counter);
    if !result.game_over {
        None
    } else if result.draw {
        Some(0)
    } else if result.white_wins == undo.was_white_to_move {
        Some(win)
    } else {
        Some(-win)
    }
}
