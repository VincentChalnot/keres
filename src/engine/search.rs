//! Debug tree types for the Keres search engine.
//!
//! Contains `ScoredMove`, `DebugTree`, and `build_debug_tree` used by
//! the engine API and CLI debug-tree command.

use crate::board::Board;
use crate::game::Move;
use super::search_engine::PVLine;

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

/// Convert a centipawn score (from the root player's perspective) to a [0,1]
/// probability from white's perspective (for debug output).
fn cp_stm_to_white_sigmoid(cp: i32, root_white_to_move: bool) -> f32 {
    let white_cp = if root_white_to_move { cp } else { -cp };
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
    /// Hash of the board position — only set on leaf nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<u64>,
    pub white_to_move: bool,
    pub is_terminal: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DebugTree>,
}

/// Build a `DebugTree` from the principal-variation lines returned by Stage 1.
///
/// The root node represents the position analysed.  Each PV line is expanded
/// into a chain of intermediate nodes down to its leaf, so the resulting tree
/// covers the full search depth.  Leaf nodes carry the board hash; root and
/// intermediate nodes omit it (there is no transposition table at those plies).
pub fn build_debug_tree(board: &Board, pvlines: &[PVLine]) -> DebugTree {
    let root_white_to_move = board.is_white_to_move();
    let best_cp = pvlines.first().map(|pv| pv.score).unwrap_or(0);
    let root_score = cp_stm_to_white_sigmoid(best_cp, root_white_to_move);

    let pv_refs: Vec<&PVLine> = pvlines.iter().collect();
    let mut node_id = 1usize;
    let children = build_children(board, &pv_refs, 0, root_white_to_move, &mut node_id);

    DebugTree {
        node_id: 0,
        action: None,
        score: root_score,
        stage1_score: root_score,
        hash: None,
        white_to_move: root_white_to_move,
        is_terminal: board.is_game_over(),
        children,
    }
}

/// Recursively build child nodes for each distinct move at `depth` across all
/// `pvlines`.  Lines that share the same move at `depth` are merged into one
/// child node and their sub-trees are built recursively.
fn build_children<'a>(
    board: &Board,
    pvlines: &[&'a PVLine],
    depth: usize,
    root_white_to_move: bool,
    node_id: &mut usize,
) -> Vec<DebugTree> {
    // Group PV lines by the move they make at this depth, preserving the
    // order of first occurrence (so the highest-scoring line comes first).
    let mut groups: Vec<(Move, Vec<&'a PVLine>)> = Vec::new();
    for &pv in pvlines {
        if depth >= pv.moves.len() {
            continue;
        }
        let mv = pv.moves[depth];
        if let Some(g) = groups.iter_mut().find(|(m, _)| *m == mv) {
            g.1.push(pv);
        } else {
            groups.push((mv, vec![pv]));
        }
    }

    let mut children = Vec::new();
    for (mv, group) in groups {
        let current_id = *node_id;
        *node_id += 1;

        // Apply the move to derive the child board state.
        let mut child_board = *board;
        let _ = child_board.make(&mv);

        // A node is a leaf when no PV in its group has a move beyond this depth.
        let is_leaf = group.iter().all(|pv| depth + 1 >= pv.moves.len());

        // Leaf nodes carry the board hash; intermediate nodes omit it.
        // Use `leaf_board` from the first PV line in the group: this is the
        // position the engine actually evaluated (all PV moves replayed from
        // root), so its hash matches the transposition-table entry.
        let hash = if is_leaf {
            Some(hash_board(&group[0].leaf_board))
        } else {
            None
        };

        // Score comes from the best (first) PV passing through this node.
        let score = cp_stm_to_white_sigmoid(group[0].score, root_white_to_move);

        let sub_children = if is_leaf {
            Vec::new()
        } else {
            build_children(&child_board, &group, depth + 1, root_white_to_move, node_id)
        };

        children.push(DebugTree {
            node_id: current_id,
            action: Some(mv.to_string()),
            score,
            stage1_score: score,
            hash,
            white_to_move: child_board.is_white_to_move(),
            is_terminal: child_board.is_game_over(),
            children: sub_children,
        });
    }

    children
}
