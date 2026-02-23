//! Keres search tree — struct-of-arrays (SoA) architecture.
//!
//! Instead of storing each vertex as a single struct, the tree keeps
//! separate parallel vectors for boards, visit counts, reward sums,
//! parent links, expansion flags, inbound actions, and child-edge
//! lists.  This layout is cache-friendly for sweeps that touch only
//! one or two columns (e.g. selection reads only visits and rewards).

use crate::board::Board;
use crate::game::{Game, Move, PotentialMove};
use super::config::TreeParams;

// ══════════  SoA columns  ══════════

/// Parallel-array storage for all vertex data.
struct Columns {
    boards:     Vec<Board>,
    visit_ct:   Vec<u32>,
    reward_acc: Vec<f32>,
    parent_key: Vec<Option<usize>>,
    inbound_mv: Vec<Option<Move>>,
    arc_list:   Vec<Vec<(Move, usize)>>,
    is_open:    Vec<bool>,
}

impl Columns {
    fn with_capacity(cap: usize) -> Self {
        Columns {
            boards:     Vec::with_capacity(cap),
            visit_ct:   Vec::with_capacity(cap),
            reward_acc: Vec::with_capacity(cap),
            parent_key: Vec::with_capacity(cap),
            inbound_mv: Vec::with_capacity(cap),
            arc_list:   Vec::with_capacity(cap),
            is_open:    Vec::with_capacity(cap),
        }
    }

    fn insert_row(&mut self,
                  board: Board,
                  parent: Option<usize>,
                  via: Option<Move>) -> usize {
        let key = self.boards.len();
        self.boards.push(board);
        self.visit_ct.push(0);
        self.reward_acc.push(0.0);
        self.parent_key.push(parent);
        self.inbound_mv.push(via);
        self.arc_list.push(Vec::new());
        self.is_open.push(false);
        key
    }

    fn row_count(&self) -> usize { self.boards.len() }
}

// ══════════  Ranking function  ══════════

/// Compute the UCT selection priority of a vertex.  Vertices with
/// zero visits always receive `f32::MAX`.
///
/// `maximizing` should be `true` when the parent is white-to-move
/// (pick child that maximises white's reward) and `false` when the
/// parent is black-to-move (pick child that minimises white's reward).
fn rank_vertex(visits: u32, reward: f32,
               parent_total: u32, kappa: f32,
               maximizing: bool) -> f32 {
    if visits == 0 { return f32::MAX; }
    let mean_payoff = reward / (visits as f32);
    let ln_parent = f32::ln(parent_total as f32);
    let uncertainty = kappa * f32::sqrt(ln_parent / (visits as f32));
    // For white-to-move parent: high white score is good → mean_payoff + exploration
    // For black-to-move parent: low white score is good → (1 - mean_payoff) + exploration
    if maximizing {
        mean_payoff + uncertainty
    } else {
        (1.0 - mean_payoff) + uncertainty
    }
}

// ══════════  Move flattening  ══════════

fn flatten_candidate(pm: &PotentialMove, buf: &mut Vec<Move>) {
    if pm.force_unstack {
        buf.push(pm.to_move(true));
    } else {
        buf.push(pm.to_move(false));
        if pm.unstackable { buf.push(pm.to_move(true)); }
    }
}

// ══════════  KNode (public facade)  ══════════

/// Read-only snapshot of a single vertex exposed for external callers.
pub struct KNode {
    pub n:          u32,
    pub w:          f32,
    pub parent_idx: Option<usize>,
    pub edges:      Vec<(Move, usize)>,
    pub state:      Board,
    pub edge_in:    Option<Move>,
    pub expanded:   bool,
}

impl KNode {
    pub fn uct_score(&self, parent_agg: u32, kappa: f32, maximizing: bool) -> f32 {
        rank_vertex(self.n, self.w, parent_agg, kappa, maximizing)
    }
}

// ══════════  KTree  ══════════

pub struct KTree {
    pub pool:   Vec<KNode>,
    pub root:   usize,
    pub params: TreeParams,
    cols:       Columns,
}

impl KTree {
    // ── construction ──────────────────────────────

    pub fn with_root(board: Board, params: TreeParams) -> Self {
        let mut cols = Columns::with_capacity(1024);
        cols.insert_row(board, None, None);
        let root_snapshot = KNode {
            n: 0, w: 0.0, parent_idx: None,
            edges: Vec::new(), state: board,
            edge_in: None, expanded: false,
        };
        KTree { pool: vec![root_snapshot], root: 0, params, cols }
    }

    /// Allocate a new vertex in the SoA columns and in the public pool.
    fn alloc_vertex(&mut self, board: Board,
                    parent: usize, via: Move) -> usize {
        let key = self.cols.insert_row(board, Some(parent), Some(via));
        self.pool.push(KNode {
            n: 0, w: 0.0, parent_idx: Some(parent),
            edges: Vec::new(), state: board,
            edge_in: Some(via), expanded: false,
        });
        debug_assert_eq!(key, self.pool.len() - 1);
        key
    }

    /// Push column values into the public pool facade for a given key.
    fn refresh_facade(&mut self, key: usize) {
        self.pool[key].n = self.cols.visit_ct[key];
        self.pool[key].w = self.cols.reward_acc[key];
        self.pool[key].edges = self.cols.arc_list[key].clone();
        self.pool[key].expanded = self.cols.is_open[key];
    }

    // ── selection ─────────────────────────────────

    pub fn descend_to_leaf(&self) -> (usize, Vec<usize>) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut path = Vec::<usize>::with_capacity(48);
        let mut key = self.root;
        path.push(key);

        loop {
            // Check the three leaf conditions on the SoA columns directly.
            let terminal = self.cols.boards[key].is_game_over();
            let closed   = !self.cols.is_open[key];
            let no_arcs  = self.cols.arc_list[key].is_empty();
            if terminal || closed || no_arcs { break; }

            // Determine perspective: white-to-move maximises white's score.
            let maximizing = self.cols.boards[key].is_white_to_move();

            // Find the arc whose destination has the highest rank.
            let pv = self.cols.visit_ct[key];
            let kp = self.params.uct_c;
            let arcs = &self.cols.arc_list[key];

            // Rank a child node.  Already-visited leaves (visit_ct > 0 but
            // still not expanded, i.e. terminal or no-legal-move positions)
            // receive -infinity so they are never re-selected: the heuristic is
            // deterministic and re-evaluating them yields no new information.
            let rank_of = |dk: usize| -> f32 {
                let v = self.cols.visit_ct[dk];
                if v > 0 && !self.cols.is_open[dk] {
                    f32::NEG_INFINITY
                } else {
                    rank_vertex(v, self.cols.reward_acc[dk], pv, kp, maximizing)
                }
            };

            let mut champ_key = arcs[0].1;
            let mut champ_rank = rank_of(champ_key);
            let mut tied_count = 1usize;

            let mut idx = 1usize;
            while idx < arcs.len() {
                let dk = arcs[idx].1;
                let dr = rank_of(dk);
                if dr > champ_rank {
                    champ_rank = dr;
                    champ_key = dk;
                    tied_count = 1;
                } else if dr == champ_rank {
                    // Reservoir sampling: randomly replace the current champion
                    // with probability 1/tied_count so that all tied candidates are
                    // equally likely to win.  This prevents the deterministic
                    // "always pick arcs[0]" bias when multiple unvisited children
                    // share the f32::MAX rank.
                    tied_count += 1;
                    if rng.gen_bool(1.0 / tied_count as f64) {
                        champ_key = dk;
                    }
                }
                idx += 1;
            }

            // If every child is an already-evaluated leaf (all ranked -infinity),
            // the current node's subtree is fully exhausted: stop here rather
            // than descending into a position that would only be re-evaluated.
            if champ_rank == f32::NEG_INFINITY { break; }

            key = champ_key;
            path.push(key);
        }

        (key, path)
    }

    // ── virtual loss ──────────────────────────────

    pub fn inject_penalty(&mut self, route: &[usize]) {
        let delta = self.params.vl_penalty;
        for &key in route {
            self.cols.visit_ct[key] += delta;
            self.refresh_facade(key);
        }
    }

    pub fn retract_penalty(&mut self, route: &[usize]) {
        let delta = self.params.vl_penalty;
        for &key in route {
            let old = self.cols.visit_ct[key];
            self.cols.visit_ct[key] = if old >= delta { old - delta } else { 0 };
            self.refresh_facade(key);
        }
    }

    // ── expansion ─────────────────────────────────

    pub fn spawn_children(&mut self, slot: usize) {
        let brd = self.cols.boards[slot];
        let game_ref = Game::from_board(brd);
        let candidates = game_ref.get_all_moves();

        let mut moves_buf = Vec::<Move>::with_capacity(candidates.len() * 2);
        for candidate in &candidates {
            flatten_candidate(candidate, &mut moves_buf);
        }

        let mut new_arcs = Vec::<(Move, usize)>::with_capacity(moves_buf.len());
        for &mv in &moves_buf {
            if let Ok(next_brd) = game_ref.apply_move_copy(mv) {
                let child_key = self.alloc_vertex(next_brd, slot, mv);
                new_arcs.push((mv, child_key));
            }
        }

        self.cols.arc_list[slot] = new_arcs.clone();
        self.cols.is_open[slot] = true;
        self.pool[slot].edges = new_arcs;
        self.pool[slot].expanded = true;
    }

    // ── back-propagation ──────────────────────────
    // Scores are always from white's perspective (1.0 = white winning).
    // No flipping needed — every node stores the same viewpoint.

    pub fn feed_result(&mut self, route: &[usize], reward: f32) {
        for &key in route.iter().rev() {
            self.cols.visit_ct[key] += 1;
            self.cols.reward_acc[key] += reward;
            self.refresh_facade(key);
        }
    }

    // ── result extraction ─────────────────────────
    // Scores are from white's perspective.  White picks the child with
    // the highest mean reward; black picks the child with the lowest.

    pub fn pick_best_action(&self) -> Option<Move> {
        let root_arcs = &self.cols.arc_list[self.root];
        if root_arcs.is_empty() { return None; }

        let white_to_move = self.cols.boards[self.root].is_white_to_move();

        let mut winner_mv = root_arcs[0].0;
        let first_key = root_arcs[0].1;
        let first_n = self.cols.visit_ct[first_key];
        let mut winner_score = if first_n > 0 {
            self.cols.reward_acc[first_key] / (first_n as f32)
        } else {
            0.5
        };

        let mut ai = 1usize;
        loop {
            if ai >= root_arcs.len() { break; }
            let ck = root_arcs[ai].1;
            let cn = self.cols.visit_ct[ck];
            let cs = if cn > 0 {
                self.cols.reward_acc[ck] / (cn as f32)
            } else {
                0.5
            };
            let is_better = if white_to_move { cs > winner_score } else { cs < winner_score };
            if is_better { winner_score = cs; winner_mv = root_arcs[ai].0; }
            ai += 1;
        }
        Some(winner_mv)
    }

    // ── accessors ─────────────────────────────────

    pub fn root_n(&self) -> u32 { self.cols.visit_ct[self.root] }
    pub fn pool_len(&self) -> usize { self.cols.row_count() }
    pub fn board_of(&self, key: usize) -> &Board { &self.cols.boards[key] }

    // ── terminal detection ───────────────────────────

    /// If the current (expanded) node has a terminal child that
    /// represents a **forced win** for the current mover — i.e. the
    /// player whose turn it is has at least one child that is already
    /// game-over in their favour — return the guaranteed terminal
    /// score (1.0 = white wins, 0.0 = black wins).
    ///
    /// Returns `None` for terminal nodes themselves, for nodes with no
    /// children yet (not yet expanded), and for nodes where no child
    /// is a decisive terminal position.
    pub fn immediate_terminal_score(&self, slot: usize) -> Option<f32> {
        let board = &self.cols.boards[slot];
        if board.is_game_over() { return None; }

        let white_to_move = board.is_white_to_move();

        for &(_, ck) in &self.cols.arc_list[slot] {
            let child = &self.cols.boards[ck];
            if child.is_game_over() && !child.is_draw() {
                // White to move and child is a white win → forced win for white.
                if white_to_move && child.white_wins() {
                    return Some(1.0);
                }
                // Black to move and child is a black win → forced win for black.
                if !white_to_move && !child.white_wins() {
                    return Some(0.0);
                }
            }
        }
        None
    }

    // ── debug export ─────────────────────────────

    /// Export the search tree as a serialisable structure for debugging.
    /// Only includes nodes with at least one visit.
    pub fn export_debug(&self) -> DebugTree {
        self.export_subtree(self.root)
    }

    fn export_subtree(&self, key: usize) -> DebugTree {
        let n = self.cols.visit_ct[key];
        let w = self.cols.reward_acc[key];
        let mean = if n > 0 { w / (n as f32) } else { 0.0 };
        let board = &self.cols.boards[key];

        let action_label = self.cols.inbound_mv[key].map(|m| m.to_string());
        let white_to_move = board.is_white_to_move();

        let children: Vec<DebugTree> = self.cols.arc_list[key].iter()
            .filter(|(_, ck)| self.cols.visit_ct[*ck] > 0)
            .map(|(_, ck)| self.export_subtree(*ck))
            .collect();

        DebugTree {
            node_id: key,
            action: action_label,
            visits: n,
            total_reward: w,
            mean_value: mean,
            white_to_move,
            is_terminal: board.is_game_over(),
            children,
        }
    }
}

/// Serialisable snapshot of the MCTS tree for external debugging tools.
#[derive(serde::Serialize)]
pub struct DebugTree {
    pub node_id: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub visits: u32,
    pub total_reward: f32,
    pub mean_value: f32,
    pub white_to_move: bool,
    pub is_terminal: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DebugTree>,
}

// ══════════  Tests  ══════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    macro_rules! assert_approx {
        ($got:expr, $want:expr, $msg:expr) => {
            let g = $got; let w = $want;
            assert!((g - w).abs() < 1e-5, "{}: got {} want {}", $msg, g, w);
        };
    }

    fn fresh() -> KTree { KTree::with_root(Board::new(), TreeParams::default()) }
    fn with_kids() -> KTree { let mut t = fresh(); t.spawn_children(0); t }

    #[test]
    fn rank_vertex_gives_max_for_zero_visits() {
        assert_eq!(rank_vertex(0, 0.0, 100, 1.0, true), f32::MAX);
        assert_eq!(rank_vertex(0, 0.0, 100, 1.0, false), f32::MAX);
    }

    #[test]
    fn rank_vertex_gives_finite_for_nonzero_visits() {
        let r = rank_vertex(20, 8.0, 200, 1.414, true);
        assert!(r.is_finite() && r >= 0.0, "expected finite nonneg, got {r}");
    }

    #[test]
    fn rank_vertex_black_perspective_inverts() {
        // 10 visits, 8.0 total reward → mean 0.8 (good for white)
        let white_rank = rank_vertex(10, 8.0, 100, 1.414, true);
        let black_rank = rank_vertex(10, 8.0, 100, 1.414, false);
        // White should prefer this (high rank), black should not (low rank)
        assert!(white_rank > black_rank,
            "white {white_rank} should exceed black {black_rank} for white-favorable position");
    }

    #[test]
    fn penalty_symmetry() {
        let mut t = fresh();
        let before = t.root_n();
        t.inject_penalty(&[0]);
        let mid = t.root_n();
        t.retract_penalty(&[0]);
        let after = t.root_n();
        assert_eq!((before, mid, after), (0, 10, 0));
    }

    #[test]
    fn lone_root_is_leaf() {
        let t = fresh();
        let (lk, bc) = t.descend_to_leaf();
        assert!(lk == 0 && bc.len() == 1, "leaf={lk}, trail={bc:?}");
    }

    #[test]
    fn spawn_creates_children_and_marks_open() {
        let t = with_kids();
        assert!(t.cols.is_open[0], "root must be open after spawn");
        let ec = t.cols.arc_list[0].len();
        assert!(ec > 0, "opening must have legal moves; got {ec}");
        assert_eq!(t.pool_len(), 1 + ec);
    }

    #[test]
    fn propagation_stores_uniform_white_score() {
        let mut t = with_kids();
        let ck = t.cols.arc_list[0][0].1;
        t.feed_result(&[0, ck], 0.8);
        // With white-perspective scoring, both nodes store the same reward
        assert_approx!(t.cols.reward_acc[ck], 0.8, "child reward");
        assert_approx!(t.cols.reward_acc[0], 0.8, "root reward");
        assert_eq!(t.cols.visit_ct[ck], 1);
        assert_eq!(t.cols.visit_ct[0], 1);
    }

    #[test]
    fn best_action_follows_value_leader() {
        let mut t = with_kids();
        // White to move from initial position: pick_best_action takes highest mean
        let (expected_mv, boosted_key) = t.cols.arc_list[0][2];
        // Collect all child keys first to avoid borrow conflict
        let all_children: Vec<usize> = t.cols.arc_list[0].iter().map(|&(_, ck)| ck).collect();
        // Give the boosted child a high score, others mediocre
        for &ck in &all_children {
            if ck == boosted_key {
                t.cols.visit_ct[ck] = 100;
                t.cols.reward_acc[ck] = 95.0; // mean 0.95 — very good for white
            } else {
                t.cols.visit_ct[ck] = 100;
                t.cols.reward_acc[ck] = 50.0; // mean 0.5
            }
            t.refresh_facade(ck);
        }
        assert_eq!(t.pick_best_action().unwrap(), expected_mv);
    }

    #[test]
    fn game_over_root_stays_leaf() {
        let mut b = Board::new();
        b.set_game_over(true, true, false);
        let mut t = KTree::with_root(b, TreeParams::default());
        t.spawn_children(0);
        assert_eq!(t.descend_to_leaf().0, 0);
    }

    #[test]
    fn immediate_terminal_score_returns_none_for_unexpanded_node() {
        let t = fresh();
        // Root has no children yet → no forced result.
        assert!(t.immediate_terminal_score(0).is_none());
    }

    #[test]
    fn immediate_terminal_score_returns_none_for_no_terminal_children() {
        let t = with_kids();
        // Opening position: no child is immediately game-over.
        assert!(t.immediate_terminal_score(0).is_none());
    }

    #[test]
    fn immediate_terminal_score_detects_white_win() {
        // Build a tree with a synthetic child that is a white-win terminal.
        let mut t = fresh();
        // The root board is white-to-move.
        assert!(t.cols.boards[0].is_white_to_move());

        // Manually insert a terminal child (white wins) into the root's arc list.
        let mut terminal_board = Board::new();
        terminal_board.set_game_over(true, true, false); // white wins
        // We need a Move to insert; borrow any valid move from the opening.
        t.spawn_children(0); // creates real children
        // Replace the first child's board with the terminal board.
        let first_child_key = t.cols.arc_list[0][0].1;
        t.cols.boards[first_child_key] = terminal_board;
        t.pool[first_child_key].state = terminal_board;

        let score = t.immediate_terminal_score(0);
        assert_eq!(score, Some(1.0),
            "white-to-move parent with white-win child should yield forced score 1.0");
    }

    #[test]
    fn immediate_terminal_score_detects_black_win() {
        // Build a tree rooted at a black-to-move position.
        let mut b = Board::new();
        b.set_white_to_move(false);
        let mut t = KTree::with_root(b, TreeParams::default());
        t.spawn_children(0);

        // Replace the first child's board with a black-win terminal.
        let first_child_key = t.cols.arc_list[0][0].1;
        let mut terminal_board = Board::new();
        terminal_board.set_game_over(true, false, false); // black wins
        t.cols.boards[first_child_key] = terminal_board;
        t.pool[first_child_key].state = terminal_board;

        let score = t.immediate_terminal_score(0);
        assert_eq!(score, Some(0.0),
            "black-to-move parent with black-win child should yield forced score 0.0");
    }

    #[test]
    fn immediate_terminal_score_ignores_draw_children() {
        // A draw child should NOT be considered a forced result.
        let mut t = fresh();
        t.spawn_children(0);
        let first_child_key = t.cols.arc_list[0][0].1;
        let mut draw_board = Board::new();
        draw_board.set_game_over(true, false, true); // draw
        t.cols.boards[first_child_key] = draw_board;
        t.pool[first_child_key].state = draw_board;

        assert!(t.immediate_terminal_score(0).is_none(),
            "draw child should not produce a forced terminal score");
    }

    #[test]
    fn immediate_terminal_score_returns_none_for_terminal_slot() {
        let mut b = Board::new();
        b.set_game_over(true, true, false);
        let t = KTree::with_root(b, TreeParams::default());
        assert!(t.immediate_terminal_score(0).is_none(),
            "terminal slot itself should always return None");
    }

    // ── new-behaviour tests ───────────────────────────────────────────────

    /// A visited terminal child must not be re-selected as a leaf.
    /// With N-1 unvisited children and 1 already-visited terminal child,
    /// `descend_to_leaf` must always land on one of the unvisited children.
    #[test]
    fn visited_terminal_child_is_not_reselected() {
        let mut t = with_kids(); // root expanded, all children unvisited
        let term_child_key = t.cols.arc_list[0][0].1;

        // Make the first child a visited terminal (game-over board, 1 visit).
        let mut term_board = Board::new();
        term_board.set_game_over(true, true, false);
        t.cols.boards[term_child_key] = term_board;
        t.pool[term_child_key].state  = term_board;
        // is_open stays false (terminal, never expanded) — that's the key.
        t.cols.visit_ct[term_child_key]   = 1;
        t.cols.reward_acc[term_child_key] = 1.0;
        t.refresh_facade(term_child_key);
        // Also give the root 1 visit so UCT exploration term is well-defined.
        t.cols.visit_ct[0]   = 1;
        t.cols.reward_acc[0] = 0.5;
        t.refresh_facade(0);

        // Run many descents: the terminal child must never be chosen.
        for _ in 0..200 {
            let (leaf, _) = t.descend_to_leaf();
            assert_ne!(leaf, term_child_key,
                "descend_to_leaf selected the already-visited terminal child");
        }
    }

    /// When multiple children are unvisited (all tied at f32::MAX), the
    /// selection must not always land on the first arc.  Over enough trials
    /// every child should be chosen at least once.
    #[test]
    fn unvisited_children_are_chosen_with_variety() {
        let t = with_kids(); // root expanded, all children unvisited
        let child_count = t.cols.arc_list[0].len();
        assert!(child_count >= 3, "need at least 3 children for this test");

        let mut seen = std::collections::HashSet::new();
        // 500 descents should be more than enough to hit every child at least once.
        for _ in 0..500 {
            let (leaf, _) = t.descend_to_leaf();
            seen.insert(leaf);
        }
        // All unvisited children should have been selected at some point.
        assert!(seen.len() > 1,
            "selection was always the same child — tie-breaking is not random");
        // We don't require *all* to be seen (the budget might not be enough for
        // huge branching factors), but at least 3 distinct choices is a robust
        // signal that randomness is working.
        assert!(seen.len() >= 3,
            "only {} distinct children seen in 500 descents; expected randomness",
            seen.len());
    }

    /// Each unique leaf must be evaluated at most once across a full MCTS run.
    /// We count evaluator calls per board and assert none exceeds 1.
    #[test]
    fn each_leaf_evaluated_at_most_once() {
        use std::sync::{Arc, Mutex};
        use crate::engine::gpu_batch_processor::{CpuEvaluator, Evaluator};
        use crate::engine::config::{EngineConfig, ScoringWeights};

        // Wrap a CpuEvaluator and track which boards it sees.
        struct CountingEvaluator {
            inner: CpuEvaluator,
            // Store (board, count) pairs; Board is PartialEq so we can search.
            calls: Arc<Mutex<Vec<(Board, usize)>>>,
        }
        impl Evaluator for CountingEvaluator {
            fn score_positions(&self, boards: &[Board]) -> Vec<f32> {
                let mut calls = self.calls.lock().unwrap();
                for b in boards {
                    if let Some(entry) = calls.iter_mut().find(|(k, _)| k == b) {
                        entry.1 += 1;
                    } else {
                        calls.push((*b, 1));
                    }
                }
                drop(calls);
                self.inner.score_positions(boards)
            }
        }

        let call_log: Arc<Mutex<Vec<(Board, usize)>>> = Default::default();
        let evaluator = CountingEvaluator {
            inner: CpuEvaluator { weights: ScoringWeights::default() },
            calls: Arc::clone(&call_log),
        };

        let mut cfg = EngineConfig::default();
        cfg.iterations = 200;

        let eng = crate::engine::mcts_engine::MctsEngine::with_evaluator(
            cfg, Box::new(evaluator));
        let _ = eng.find_move(&Board::new()).expect("should find a move");

        // No board position should have been evaluated more than once.
        let calls = call_log.lock().unwrap();
        let max_calls = calls.iter().map(|(_, c)| *c).max().unwrap_or(0);
        assert_eq!(max_calls, 1,
            "at least one position was evaluated {} times (expected ≤1)", max_calls);
    }
}
