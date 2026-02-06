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

/// Compute the selection priority of a vertex.  Vertices with
/// zero visits always receive `f32::MAX`.
fn rank_vertex(visits: u32, reward: f32,
               parent_total: u32, kappa: f32) -> f32 {
    if visits == 0 { return f32::MAX; }
    let mean_payoff = reward / (visits as f32);
    let ln_parent = f32::ln(parent_total as f32);
    let uncertainty = kappa * f32::sqrt(ln_parent / (visits as f32));
    mean_payoff + uncertainty
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
    pub fn uct_score(&self, parent_agg: u32, kappa: f32) -> f32 {
        rank_vertex(self.n, self.w, parent_agg, kappa)
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
        let mut path = Vec::<usize>::with_capacity(48);
        let mut key = self.root;
        path.push(key);

        loop {
            // Check the three leaf conditions on the SoA columns directly.
            let terminal = self.cols.boards[key].is_game_over();
            let closed   = !self.cols.is_open[key];
            let no_arcs  = self.cols.arc_list[key].is_empty();
            if terminal || closed || no_arcs { break; }

            // Find the arc whose destination has the highest rank.
            let pv = self.cols.visit_ct[key];
            let kp = self.params.uct_c;
            let arcs = &self.cols.arc_list[key];

            let mut champ_key = arcs[0].1;
            let mut champ_rank = rank_vertex(
                self.cols.visit_ct[champ_key],
                self.cols.reward_acc[champ_key], pv, kp);

            let mut idx = 1usize;
            let bound = arcs.len();
            loop {
                if idx >= bound { break; }
                let dk = arcs[idx].1;
                let dr = rank_vertex(
                    self.cols.visit_ct[dk],
                    self.cols.reward_acc[dk], pv, kp);
                if dr > champ_rank { champ_rank = dr; champ_key = dk; }
                idx += 1;
            }

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

    pub fn feed_result(&mut self, route: &[usize], reward: f32) {
        let mut signal = reward;
        for &key in route.iter().rev() {
            self.cols.visit_ct[key] += 1;
            self.cols.reward_acc[key] += signal;
            self.refresh_facade(key);
            signal = 1.0 - signal;
        }
    }

    // ── result extraction ─────────────────────────

    pub fn pick_best_action(&self) -> Option<Move> {
        let root_arcs = &self.cols.arc_list[self.root];
        if root_arcs.is_empty() { return None; }

        let mut winner_mv = root_arcs[0].0;
        let mut winner_vc = self.cols.visit_ct[root_arcs[0].1];
        let mut ai = 1usize;
        loop {
            if ai >= root_arcs.len() { break; }
            let vc = self.cols.visit_ct[root_arcs[ai].1];
            if vc > winner_vc { winner_vc = vc; winner_mv = root_arcs[ai].0; }
            ai += 1;
        }
        Some(winner_mv)
    }

    // ── accessors ─────────────────────────────────

    pub fn root_n(&self) -> u32 { self.cols.visit_ct[self.root] }
    pub fn pool_len(&self) -> usize { self.cols.row_count() }
    pub fn board_of(&self, key: usize) -> &Board { &self.cols.boards[key] }
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
        assert_eq!(rank_vertex(0, 0.0, 100, 1.0), f32::MAX);
    }

    #[test]
    fn rank_vertex_gives_finite_for_nonzero_visits() {
        let r = rank_vertex(20, 8.0, 200, 1.414);
        assert!(r.is_finite() && r >= 0.0, "expected finite nonneg, got {r}");
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
    fn propagation_flips_signal_per_layer() {
        let mut t = with_kids();
        let ck = t.cols.arc_list[0][0].1;
        t.feed_result(&[0, ck], 0.8);
        assert_approx!(t.cols.reward_acc[ck], 0.8, "child reward");
        assert_approx!(t.cols.reward_acc[0], 0.2, "root reward");
        assert_eq!(t.cols.visit_ct[ck], 1);
        assert_eq!(t.cols.visit_ct[0], 1);
    }

    #[test]
    fn best_action_follows_visit_leader() {
        let mut t = with_kids();
        let (expected_mv, boosted_key) = t.cols.arc_list[0][2];
        t.cols.visit_ct[boosted_key] = 7777;
        t.refresh_facade(boosted_key);
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
}
