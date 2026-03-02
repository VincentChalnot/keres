//! Debug tree types for the Keres search engine.
//!
//! Contains `ScoredMove`, `DebugTree`, and `build_debug_tree` used by
//! the engine API and CLI debug-tree command.

use crate::board::Board;
use crate::game::Move;

const SCORE_SIGMOID_SCALE: f32 = 2000.0;

// ── Public types ─────────────────────────────────────────────────────────────

/// A move paired with its centipawn score (from the root side-to-move's
/// perspective: higher = better for the root player).
#[derive(Clone, Debug)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

// ── Debug tree ───────────────────────────────────────────────────────────────

/// Convert a centipawn score (side-to-move perspective) to a [0,1]
/// probability from white's perspective (for debug output).
fn cp_stm_to_white_sigmoid(cp: i32, white_to_move: bool) -> f32 {
    let white_cp = if white_to_move { cp } else { -cp };
    let x = -(white_cp as f32) / SCORE_SIGMOID_SCALE;
    1.0 / (1.0 + x.exp())
}

/// Hash the board binary (excluding last 2 bytes) using ahash.
fn hash_board(board: &Board) -> u64 {
    let binary = board.to_binary();
    ahash::RandomState::with_seeds(0, 0, 0, 0).hash_one(&binary[..81])
}

/// Serialisable snapshot of the search tree for external debugging tools.
#[derive(serde::Serialize)]
pub struct DebugTree {
    pub node_id: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub score: f32,
    pub stage1_score: f32,
    pub hash: u64,
    pub white_to_move: bool,
    pub is_terminal: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DebugTree>,
}

/// Build a `DebugTree` from the search results.
///
/// The root node represents the position analyzed; its children are
/// the scored moves. The `score` of each child reflects the
/// Stage 2 score (if available) or Stage 1 score otherwise.
pub fn build_debug_tree(
    board: &Board,
    stage1: &[ScoredMove],
    stage2: &[ScoredMove],
) -> DebugTree {
    let white_to_move = board.is_white_to_move();
    let best_cp = stage2.first().or(stage1.first()).map(|sm| sm.score).unwrap_or(0);
    let best_sigmoid = cp_stm_to_white_sigmoid(best_cp, white_to_move);
    let root_hash = hash_board(board);

    let children: Vec<DebugTree> = stage1.iter().enumerate().map(|(i, sm)| {
        let s2_score = stage2.iter().find(|s| s.mv == sm.mv).map(|s| s.score);
        let final_cp = s2_score.unwrap_or(sm.score);
        // Compute the board hash after applying this move.
        // The undo token is intentionally discarded — we only need the resulting position.
        let mut child_board = *board;
        let _ = child_board.make(&sm.mv);
        let child_hash = hash_board(&child_board);
        DebugTree {
            node_id: i + 1,
            action: Some(sm.mv.to_string()),
            score: cp_stm_to_white_sigmoid(final_cp, white_to_move),
            stage1_score: cp_stm_to_white_sigmoid(sm.score, white_to_move),
            hash: child_hash,
            white_to_move: !white_to_move,
            is_terminal: false,
            children: Vec::new(),
        }
    }).collect();

    DebugTree {
        node_id: 0,
        action: None,
        score: best_sigmoid,
        stage1_score: best_sigmoid,
        hash: root_hash,
        white_to_move,
        is_terminal: board.is_game_over(),
        children,
    }
}
