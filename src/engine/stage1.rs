//! Stage 1 — Exhaustive alpha-beta minimax search for Keres.
//!
//! Fixed-depth alpha-beta with:
//! - MVV-LVA capture ordering (full stack value)
//! - Killer moves (2 slots per depth)
//! - History heuristic ([side][from][to])
//! - Transposition table at leaf nodes only (ahash-based)
//! - Rayon parallelism at root level
//! - MultiPV via leaf blacklisting
//! - PV tracking (complete move chain per leaf)

use crate::board::{Board, Position};
use crate::game::{Game, Move, PotentialMove};
use super::eval;
use super::search_config::SearchConfig;
use super::stage_config::StageConfig;
use super::search_engine::{SearchResult, PVLine, SearchStats};

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// ── Constants ────────────────────────────────────────────────────────────────

const INFINITY: i32 = 200_000;
const WORST_SCORE: i32 = i32::MIN + 1; // avoid overflow on negation

// Move-ordering priority tiers
const CAPTURE_BASE_SCORE: i32 = 100_000;
const KILLER_SCORE_1: i32 = 90_000;
const KILLER_SCORE_2: i32 = 80_000;

// ── Transposition table (leaf-only, ahash-based) ────────────────────────────

/// Hash the board binary (excluding last 2 bytes: flags + counter)
/// using ahash with AES-NI acceleration.
fn hash_board(board: &Board) -> u64 {
    let binary = board.to_binary();
    ahash::RandomState::with_seeds(0, 0, 0, 0).hash_one(&binary[..81])
}

/// Lock-free leaf-only transposition table.
pub struct LeafTT {
    keys: Vec<AtomicU64>,
    scores: Vec<AtomicU64>,  // stores score as u64 (bit cast)
    mask: usize,
    enabled: bool,
}

impl LeafTT {
    pub(crate) fn new(size: usize, enabled: bool) -> Self {
        let size = size.next_power_of_two();
        let mut keys = Vec::with_capacity(size);
        let mut scores = Vec::with_capacity(size);
        for _ in 0..size {
            keys.push(AtomicU64::new(0));
            scores.push(AtomicU64::new(0));
        }
        LeafTT { keys, scores, mask: size - 1, enabled }
    }

    fn probe(&self, hash: u64) -> Option<i32> {
        if !self.enabled { return None; }
        let idx = (hash as usize) & self.mask;
        let stored_key = self.keys[idx].load(Ordering::Relaxed);
        if stored_key == hash {
            let score_bits = self.scores[idx].load(Ordering::Relaxed);
            Some(score_bits as i32)
        } else {
            None
        }
    }

    fn store(&self, hash: u64, score: i32) {
        if !self.enabled { return; }
        let idx = (hash as usize) & self.mask;
        self.keys[idx].store(hash, Ordering::Relaxed);
        self.scores[idx].store(score as u32 as u64, Ordering::Relaxed);
    }
}

// ── Move helpers ─────────────────────────────────────────────────────────────

/// Flatten `PotentialMove`s into concrete `Move`s.
fn flatten_moves(candidates: &[PotentialMove]) -> Vec<Move> {
    let mut moves = Vec::with_capacity(candidates.len() * 2);
    for pm in candidates {
        if pm.force_unstack {
            moves.push(pm.to_move(true));
        } else {
            moves.push(pm.to_move(false));
            if pm.unstackable {
                moves.push(pm.to_move(true));
            }
        }
    }
    moves
}

// ── Killer move table ────────────────────────────────────────────────────────

struct KillerTable {
    killers: [[Option<Move>; 2]; 64],
}

impl KillerTable {
    fn new() -> Self { KillerTable { killers: [[None; 2]; 64] } }

    fn store(&mut self, mv: Move, depth: usize) {
        if depth >= 64 { return; }
        if self.killers[depth][0] != Some(mv) {
            self.killers[depth][1] = self.killers[depth][0];
            self.killers[depth][0] = Some(mv);
        }
    }

    fn score(&self, mv: &Move, depth: usize) -> i32 {
        if depth >= 64 { return 0; }
        if self.killers[depth][0].as_ref() == Some(mv) { return KILLER_SCORE_1; }
        if self.killers[depth][1].as_ref() == Some(mv) { return KILLER_SCORE_2; }
        0
    }
}

// ── History heuristic table ──────────────────────────────────────────────────

struct HistoryTable {
    // [side: 0=white,1=black][from_sq][to_sq]
    table: [[[i32; 81]; 81]; 2],
}

impl HistoryTable {
    fn new() -> Self { HistoryTable { table: [[[0; 81]; 81]; 2] } }

    fn get(&self, mv: &Move, side: usize) -> i32 {
        self.table[side][mv.from.to_absolute()][mv.to.to_absolute()]
    }

    fn update(&mut self, mv: &Move, side: usize, depth: i32) {
        self.table[side][mv.from.to_absolute()][mv.to.to_absolute()] += depth * depth;
    }
}

// ── Move ordering ────────────────────────────────────────────────────────────

/// Score and sort moves for alpha-beta ordering.
fn order_moves(
    moves: &[Move],
    board: &Board,
    killers: &KillerTable,
    history: &HistoryTable,
    depth: usize,
    config: &StageConfig,
) -> Vec<Move> {
    if !config.move_ordering {
        return moves.to_vec();
    }

    let game = Game::from_board(*board);
    let side = if board.is_white_to_move() { 0 } else { 1 };

    let mut scored: Vec<(Move, i32)> = moves.iter().map(|&mv| {
        // MVV-LVA captures
        if game.is_capture(&mv) {
            let victim_val = capture_value_eval(board, &mv);
            let atk_val = attacker_value_eval(board, &mv);
            return (mv, CAPTURE_BASE_SCORE + victim_val * 10 - atk_val);
        }
        // Killer moves
        if config.killer_moves {
            let killer_sc = killers.score(&mv, depth);
            if killer_sc > 0 { return (mv, killer_sc); }
        }
        // History heuristic
        (mv, history.get(&mv, side))
    }).collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(mv, _)| mv).collect()
}

/// Score and sort moves for alpha-beta ordering (legacy SearchConfig version).
fn order_moves_legacy(
    moves: &[Move],
    board: &Board,
    killers: &KillerTable,
    history: &HistoryTable,
    depth: usize,
    config: &SearchConfig,
) -> Vec<Move> {
    if config.no_move_ordering {
        return moves.to_vec();
    }

    let game = Game::from_board(*board);
    let side = if board.is_white_to_move() { 0 } else { 1 };

    let mut scored: Vec<(Move, i32)> = moves.iter().map(|&mv| {
        // MVV-LVA captures
        if game.is_capture(&mv) {
            let victim_val = capture_value_eval(board, &mv);
            let atk_val = attacker_value_eval(board, &mv);
            return (mv, CAPTURE_BASE_SCORE + victim_val * 10 - atk_val);
        }
        // Killer moves
        if !config.no_killers {
            let killer_sc = killers.score(&mv, depth);
            if killer_sc > 0 { return (mv, killer_sc); }
        }
        // History heuristic
        (mv, history.get(&mv, side))
    }).collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(mv, _)| mv).collect()
}

/// Compute capture value (victim's total material) using eval piece values.
fn capture_value_eval(board: &Board, mv: &Move) -> i32 {
    if let Some(piece) = board.get_piece(&mv.to) {
        let mut val = eval::piece_value(piece.bottom);
        if let Some(top) = piece.top {
            val += eval::piece_value(top);
        }
        val
    } else {
        0
    }
}

/// Compute attacker value for MVV-LVA ordering.
fn attacker_value_eval(board: &Board, mv: &Move) -> i32 {
    if let Some(piece) = board.get_piece(&mv.from) {
        if mv.unstack {
            if let Some(top) = piece.top {
                eval::piece_value(top)
            } else {
                eval::piece_value(piece.bottom)
            }
        } else {
            let mut val = eval::piece_value(piece.bottom);
            if let Some(top) = piece.top {
                val += eval::piece_value(top);
            }
            val
        }
    } else {
        0
    }
}

/// Check if a move is a capture.
fn is_capture(board: &Board, mv: &Move) -> bool {
    if let Some(dest) = board.get_piece(&mv.to) {
        dest.color != board.color_to_move()
    } else {
        false
    }
}

/// Check if the king of the side to move is under direct attack.
fn is_in_check(board: &Board) -> bool {
    let my_color = board.color_to_move();
    // Find my king
    let mut king_pos = None;
    for sq in 0..81 {
        let pos = Position::from_u8(sq);
        if let Some(piece) = board.get_piece(&pos) {
            if piece.is_king() && piece.color == my_color {
                king_pos = Some(pos);
                break;
            }
        }
    }
    let king_pos = match king_pos {
        Some(p) => p,
        None => return false,
    };

    // Check if any opponent move targets the king's square
    let mut opp_board = *board;
    opp_board.set_white_to_move(!board.is_white_to_move());
    let opp_game = Game::from_board(opp_board);
    let opp_moves = opp_game.get_all_moves();
    opp_moves.iter().any(|pm| pm.to == king_pos)
}

/// Check if the side to move has at least one non-king piece.
fn has_non_king_pieces(board: &Board) -> bool {
    let my_color = board.color_to_move();
    for sq in 0..81 {
        let pos = Position::from_u8(sq);
        if let Some(piece) = board.get_piece(&pos) {
            if piece.color == my_color && !piece.is_king() {
                return true;
            }
        }
    }
    false
}

/// Simple SEE: check if any opponent capture at this position has
/// a victim value >= attacker value (SEE >= 0).
fn has_see_ge_zero_captures(board: &Board) -> bool {
    let my_color = board.color_to_move();
    // Look at opponent's perspective
    let mut opp_board = *board;
    opp_board.set_white_to_move(!board.is_white_to_move());
    let opp_game = Game::from_board(opp_board);
    let opp_moves = opp_game.get_all_moves();

    for pm in &opp_moves {
        // Flatten the potential move properly
        let unstack = pm.force_unstack;
        let mv = pm.to_move(unstack);
        if let Some(victim) = board.get_piece(&mv.to) {
            if victim.color == my_color {
                let victim_val = eval::piece_value(victim.bottom)
                    + victim.top.map_or(0, |t| eval::piece_value(t));
                let attacker_val = if let Some(atk) = board.get_piece(&mv.from) {
                    if mv.unstack {
                        atk.top.map_or(eval::piece_value(atk.bottom), |t| eval::piece_value(t))
                    } else {
                        eval::piece_value(atk.bottom)
                            + atk.top.map_or(0, |t| eval::piece_value(t))
                    }
                } else {
                    0
                };
                if victim_val >= attacker_val {
                    return true;
                }
            }
        }
    }
    false
}

// ── Per-thread search state ──────────────────────────────────────────────────

pub(crate) struct SearchState {
    killers: KillerTable,
    history: HistoryTable,
    nodes: u64,
    tt_hits: u64,
    tt_probes: u64,
    /// Tracks whether the previous move was a null move (to prevent consecutive null moves).
    consecutive_null: bool,
    /// Tracks how many extension plies have been added on the current path.
    extension_plies: u8,
}

impl SearchState {
    fn new() -> Self {
        SearchState {
            killers: KillerTable::new(),
            history: HistoryTable::new(),
            nodes: 0,
            tt_hits: 0,
            tt_probes: 0,
            consecutive_null: false,
            extension_plies: 0,
        }
    }
}

// ── Stage 1 Alpha-Beta ───────────────────────────────────────────────────────

/// Run Stage 1 search on a given board. Returns a SearchResult with top-K moves.
pub fn stage1_search(board: &Board, config: &SearchConfig) -> (SearchResult, SearchStats) {
    if board.is_game_over() {
        let dummy_mv = Move {
            from: Position::new(0, 0),
            to: Position::new(0, 0),
            unstack: false,
        };
        return (SearchResult {
            best_move: dummy_mv,
            score: 0,
            depth: config.depth as u8,
            nodes_visited: 0,
            top_moves: Vec::new(),
        }, SearchStats::default());
    }

    let game = Game::from_board(*board);
    let candidates = game.get_all_moves();
    let all_moves = flatten_moves(&candidates);

    if all_moves.is_empty() {
        // No legal moves — return a dummy result
        let dummy_mv = Move {
            from: Position::new(0, 0),
            to: Position::new(0, 0),
            unstack: false,
        };
        return (SearchResult {
            best_move: dummy_mv,
            score: 0,
            depth: config.depth as u8,
            nodes_visited: 0,
            top_moves: Vec::new(),
        }, SearchStats::default());
    }

    let tt = Arc::new(LeafTT::new(1 << 20, !config.no_tt));
    let mut blacklist: HashSet<u64> = HashSet::new();
    let mut all_pvs: Vec<PVLine> = Vec::new();

    let total_nodes = Arc::new(AtomicU64::new(0));
    let total_tt_hits = Arc::new(AtomicU64::new(0));
    let total_tt_probes = Arc::new(AtomicU64::new(0));

    let max_passes = config.max_passes.min(all_moves.len());

    for _pass in 0..max_passes {
        if all_pvs.len() >= config.expected_leaves {
            break;
        }

        // Score all root moves for this pass
        let scored = score_root_moves(
            board,
            &all_moves,
            config,
            &tt,
            &blacklist,
            &total_nodes,
            &total_tt_hits,
            &total_tt_probes,
        );

        if scored.is_empty() {
            break;
        }

        // Collect ALL PV lines that share the best score for this pass.
        // `scored` is sorted descending so take_while stops as soon as score drops.
        let best_score = scored[0].score;
        for pv in scored.iter().take_while(|p| p.score == best_score) {
            let leaf_hash = hash_board(&pv.leaf_board);
            blacklist.insert(leaf_hash);
            all_pvs.push(pv.clone());
        }
    }

    let nodes = total_nodes.load(Ordering::Relaxed);
    let hits = total_tt_hits.load(Ordering::Relaxed);
    let probes = total_tt_probes.load(Ordering::Relaxed);

    let best_pv = all_pvs.first().cloned();
    let (best_move, best_score) = if let Some(ref pv) = best_pv {
        (pv.root_move, pv.score)
    } else {
        (all_moves[0], 0)
    };

    (
        SearchResult {
            best_move,
            score: best_score,
            depth: config.depth as u8,
            nodes_visited: nodes,
            top_moves: all_pvs,
        },
        SearchStats {
            nodes_visited: nodes,
            tt_hits: hits,
            tt_probes: probes,
            elapsed_secs: 0.0, // filled in by caller
        },
    )
}

/// Run Stage 1 search using a `StageConfig` (new API).
pub fn stage1_search_with_config(board: &Board, config: &StageConfig, threads: usize) -> (SearchResult, SearchStats, Arc<LeafTT>) {
    if board.is_game_over() {
        let dummy_mv = Move {
            from: Position::new(0, 0),
            to: Position::new(0, 0),
            unstack: false,
        };
        return (SearchResult {
            best_move: dummy_mv,
            score: 0,
            depth: config.depth,
            nodes_visited: 0,
            top_moves: Vec::new(),
        }, SearchStats::default(), Arc::new(LeafTT::new(1 << 20, config.transposition_table)));
    }

    let game = Game::from_board(*board);
    let candidates = game.get_all_moves();
    let all_moves = flatten_moves(&candidates);

    if all_moves.is_empty() {
        let dummy_mv = Move {
            from: Position::new(0, 0),
            to: Position::new(0, 0),
            unstack: false,
        };
        return (SearchResult {
            best_move: dummy_mv,
            score: 0,
            depth: config.depth,
            nodes_visited: 0,
            top_moves: Vec::new(),
        }, SearchStats::default(), Arc::new(LeafTT::new(1 << 20, config.transposition_table)));
    }

    let tt = Arc::new(LeafTT::new(1 << 20, config.transposition_table));
    let mut blacklist: HashSet<u64> = HashSet::new();
    let mut all_pvs: Vec<PVLine> = Vec::new();

    let total_nodes = Arc::new(AtomicU64::new(0));
    let total_tt_hits = Arc::new(AtomicU64::new(0));
    let total_tt_probes = Arc::new(AtomicU64::new(0));

    let max_passes = (config.max_passes as usize).min(all_moves.len());

    for _pass in 0..max_passes {
        if all_pvs.len() >= config.expected_leaves {
            break;
        }

        let scored = score_root_moves_with_config(
            board,
            &all_moves,
            config,
            threads,
            &tt,
            &blacklist,
            &total_nodes,
            &total_tt_hits,
            &total_tt_probes,
        );

        if scored.is_empty() {
            break;
        }

        let best_score = scored[0].score;
        for pv in scored.iter().take_while(|p| p.score == best_score) {
            let leaf_hash = hash_board(&pv.leaf_board);
            blacklist.insert(leaf_hash);
            all_pvs.push(pv.clone());
        }
    }

    let nodes = total_nodes.load(Ordering::Relaxed);
    let hits = total_tt_hits.load(Ordering::Relaxed);
    let probes = total_tt_probes.load(Ordering::Relaxed);

    let best_pv = all_pvs.first().cloned();
    let (best_move, best_score) = if let Some(ref pv) = best_pv {
        (pv.root_move, pv.score)
    } else {
        (all_moves[0], 0)
    };

    (
        SearchResult {
            best_move,
            score: best_score,
            depth: config.depth,
            nodes_visited: nodes,
            top_moves: all_pvs,
        },
        SearchStats {
            nodes_visited: nodes,
            tt_hits: hits,
            tt_probes: probes,
            elapsed_secs: 0.0,
        },
        tt,
    )
}

/// Score all root moves in parallel using Rayon. Returns sorted PVLines.
fn score_root_moves(
    board: &Board,
    moves: &[Move],
    config: &SearchConfig,
    tt: &Arc<LeafTT>,
    blacklist: &HashSet<u64>,
    total_nodes: &Arc<AtomicU64>,
    total_tt_hits: &Arc<AtomicU64>,
    total_tt_probes: &Arc<AtomicU64>,
) -> Vec<PVLine> {
    if config.threads <= 1 || moves.len() <= 1 {
        // Single-threaded
        let mut state = SearchState::new();
        let mut results: Vec<PVLine> = Vec::new();

        for &mv in moves {
            let mut board_copy = *board;
            let undo = board_copy.make(&mv);

            let mut child_pv = Vec::new();
            let score = -alphabeta(
                &mut board_copy,
                config.depth - 1,
                -INFINITY,
                INFINITY,
                &mut state,
                tt,
                blacklist,
                1,
                &mut child_pv,
                config,
            );

            board_copy.unmake(&mv, undo);

            let mut pv_chain = vec![mv];
            pv_chain.extend(child_pv);

            // Replay PV to get leaf board state
            let mut leaf = *board;
            for pv_mv in &pv_chain {
                let _ = leaf.make(pv_mv);
            }

            results.push(PVLine {
                root_move: mv,
                moves: pv_chain,
                score,
                leaf_board: leaf,
            });
        }

        total_nodes.fetch_add(state.nodes, Ordering::Relaxed);
        total_tt_hits.fetch_add(state.tt_hits, Ordering::Relaxed);
        total_tt_probes.fetch_add(state.tt_probes, Ordering::Relaxed);

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    } else {
        // Multi-threaded with Rayon
        use rayon::prelude::*;

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(config.threads)
            .build()
            .unwrap();

        let results: Vec<PVLine> = pool.install(|| {
            moves.par_iter().map(|&mv| {
                let mut state = SearchState::new();
                let mut board_copy = *board;
                let undo = board_copy.make(&mv);

                let mut child_pv = Vec::new();
                let score = -alphabeta(
                    &mut board_copy,
                    config.depth - 1,
                    -INFINITY,
                    INFINITY,
                    &mut state,
                    tt,
                    blacklist,
                    1,
                    &mut child_pv,
                    config,
                );

                board_copy.unmake(&mv, undo);

                total_nodes.fetch_add(state.nodes, Ordering::Relaxed);
                total_tt_hits.fetch_add(state.tt_hits, Ordering::Relaxed);
                total_tt_probes.fetch_add(state.tt_probes, Ordering::Relaxed);

                let mut pv_chain = vec![mv];
                pv_chain.extend(child_pv);

                // Replay PV to get leaf board state
                let mut leaf = *board;
                for pv_mv in &pv_chain {
                    let _ = leaf.make(pv_mv);
                }

                PVLine {
                    root_move: mv,
                    moves: pv_chain,
                    score,
                    leaf_board: leaf,
                }
            }).collect()
        });

        let mut results = results;
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

/// Score all root moves using StageConfig. Returns sorted PVLines.
fn score_root_moves_with_config(
    board: &Board,
    moves: &[Move],
    config: &StageConfig,
    threads: usize,
    tt: &Arc<LeafTT>,
    blacklist: &HashSet<u64>,
    total_nodes: &Arc<AtomicU64>,
    total_tt_hits: &Arc<AtomicU64>,
    total_tt_probes: &Arc<AtomicU64>,
) -> Vec<PVLine> {
    let depth = config.effective_depth() as i32;

    if threads <= 1 || moves.len() <= 1 {
        let mut state = SearchState::new();
        let mut results: Vec<PVLine> = Vec::new();

        for &mv in moves {
            let mut board_copy = *board;
            let undo = board_copy.make(&mv);

            let mut child_pv = Vec::new();
            let score = -alphabeta_staged(
                &mut board_copy,
                depth - 1,
                -INFINITY,
                INFINITY,
                config,
                &mut state,
                tt,
                blacklist,
                1,
                &mut child_pv,
            );

            board_copy.unmake(&mv, undo);

            let mut pv_chain = vec![mv];
            pv_chain.extend(child_pv);

            let mut leaf = *board;
            for pv_mv in &pv_chain {
                let _ = leaf.make(pv_mv);
            }

            results.push(PVLine {
                root_move: mv,
                moves: pv_chain,
                score,
                leaf_board: leaf,
            });
        }

        total_nodes.fetch_add(state.nodes, Ordering::Relaxed);
        total_tt_hits.fetch_add(state.tt_hits, Ordering::Relaxed);
        total_tt_probes.fetch_add(state.tt_probes, Ordering::Relaxed);

        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    } else {
        use rayon::prelude::*;

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();

        let results: Vec<PVLine> = pool.install(|| {
            moves.par_iter().map(|&mv| {
                let mut state = SearchState::new();
                let mut board_copy = *board;
                let undo = board_copy.make(&mv);

                let mut child_pv = Vec::new();
                let score = -alphabeta_staged(
                    &mut board_copy,
                    depth - 1,
                    -INFINITY,
                    INFINITY,
                    config,
                    &mut state,
                    tt,
                    blacklist,
                    1,
                    &mut child_pv,
                );

                board_copy.unmake(&mv, undo);

                total_nodes.fetch_add(state.nodes, Ordering::Relaxed);
                total_tt_hits.fetch_add(state.tt_hits, Ordering::Relaxed);
                total_tt_probes.fetch_add(state.tt_probes, Ordering::Relaxed);

                let mut pv_chain = vec![mv];
                pv_chain.extend(child_pv);

                let mut leaf = *board;
                for pv_mv in &pv_chain {
                    let _ = leaf.make(pv_mv);
                }

                PVLine {
                    root_move: mv,
                    moves: pv_chain,
                    score,
                    leaf_board: leaf,
                }
            }).collect()
        });

        let mut results = results;
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results
    }
}

/// Alpha-beta minimax for Stage 1 (legacy SearchConfig version).
/// Uses make/unmake pattern — no board cloning.
fn alphabeta(
    board: &mut Board,
    depth: i32,
    mut alpha: i32,
    beta: i32,
    state: &mut SearchState,
    tt: &Arc<LeafTT>,
    blacklist: &HashSet<u64>,
    ply: i32,
    pv: &mut Vec<Move>,
    config: &SearchConfig,
) -> i32 {
    state.nodes += 1;

    // Terminal node
    if board.is_game_over() {
        if board.is_draw() { return 0; }
        return -(eval::MATE_SCORE - ply);
    }

    // Leaf node — evaluate
    if depth <= 0 {
        // Check blacklist
        let leaf_hash = hash_board(board);
        if blacklist.contains(&leaf_hash) {
            // Return worst score to force this PV to be avoided
            return WORST_SCORE;
        }

        // TT probe at leaf
        state.tt_probes += 1;
        if let Some(cached_score) = tt.probe(leaf_hash) {
            state.tt_hits += 1;
            return cached_score;
        }

        let score = eval::evaluate(board);
        tt.store(leaf_hash, score);
        return score;
    }

    // Generate and order moves
    let game = Game::from_board(*board);
    let candidates = game.get_all_moves();
    let moves = flatten_moves(&candidates);

    if moves.is_empty() {
        return eval::evaluate(board);
    }

    let ordered = order_moves_legacy(
        &moves, board, &state.killers, &state.history,
        depth as usize, config,
    );

    let mut best_score = -INFINITY;
    let mut best_pv: Vec<Move> = Vec::new();
    let side = if board.is_white_to_move() { 0 } else { 1 };

    if config.no_alpha_beta {
        // Pure minimax (no pruning)
        for &mv in &ordered {
            let undo = board.make(&mv);

            let mut child_pv = Vec::new();
            let score = -alphabeta(
                board, depth - 1, -INFINITY, INFINITY,
                state, tt, blacklist, ply + 1, &mut child_pv, config,
            );

            board.unmake(&mv, undo);

            if score > best_score {
                best_score = score;
                best_pv = vec![mv];
                best_pv.extend(child_pv);
            }
        }
    } else {
        // Alpha-beta pruning
        for &mv in &ordered {
            let undo = board.make(&mv);

            let mut child_pv = Vec::new();
            let score = -alphabeta(
                board, depth - 1, -beta, -alpha,
                state, tt, blacklist, ply + 1, &mut child_pv, config,
            );

            board.unmake(&mv, undo);

            if score > best_score {
                best_score = score;
                best_pv = vec![mv];
                best_pv.extend(child_pv);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                // Beta cutoff — store killer/history
                if !is_capture(board, &mv) {
                    if !config.no_killers {
                        state.killers.store(mv, depth as usize);
                    }
                    state.history.update(&mv, side, depth);
                }
                break;
            }
        }
    }

    // Update PV chain
    if !best_pv.is_empty() {
        *pv = best_pv;
    }

    best_score
}

/// Unified alpha-beta search parameterized by `StageConfig`.
/// Handles all feature flags: null move pruning, LMR, selective extensions.
pub(crate) fn alphabeta_staged(
    board: &mut Board,
    depth: i32,
    mut alpha: i32,
    beta: i32,
    config: &StageConfig,
    state: &mut SearchState,
    tt: &Arc<LeafTT>,
    blacklist: &HashSet<u64>,
    ply: i32,
    pv: &mut Vec<Move>,
) -> i32 {
    state.nodes += 1;

    // Terminal node
    if board.is_game_over() {
        if board.is_draw() { return 0; }
        return -(eval::MATE_SCORE - ply);
    }

    // Leaf node — evaluate (with possible selective extension)
    if depth <= 0 {
        // Selective extensions: at leaf nodes, if any opponent capture has SEE >= 0,
        // extend by 1 ply (hard cap: 2 extension plies per path).
        if config.selective_extensions && state.extension_plies < 2 {
            if has_see_ge_zero_captures(board) {
                state.extension_plies += 1;
                let score = alphabeta_staged(
                    board, 1, alpha, beta, config, state, tt, blacklist, ply + 1, pv,
                );
                state.extension_plies -= 1;
                return score;
            }
        }

        // Check blacklist
        let leaf_hash = hash_board(board);
        if blacklist.contains(&leaf_hash) {
            return WORST_SCORE;
        }

        // TT probe at leaf
        state.tt_probes += 1;
        if let Some(cached_score) = tt.probe(leaf_hash) {
            state.tt_hits += 1;
            return cached_score;
        }

        let score = eval::evaluate(board);
        tt.store(leaf_hash, score);
        return score;
    }

    // Null move pruning (Stage 2 only)
    if config.null_move_pruning
        && depth >= 3
        && !state.consecutive_null
        && !is_in_check(board)
        && has_non_king_pieces(board)
    {
        let prev = board.make_null_move();
        state.consecutive_null = true;

        let null_score = -alphabeta_staged(
            board,
            depth - 1 - config.null_move_reduction as i32,
            -beta,
            -beta + 1,
            config,
            state,
            tt,
            blacklist,
            ply + 1,
            &mut Vec::new(),
        );

        state.consecutive_null = false;
        board.unmake_null_move(prev);

        if null_score >= beta {
            return beta;
        }
    }

    // Generate and order moves
    let game = Game::from_board(*board);
    let candidates = game.get_all_moves();
    let moves = flatten_moves(&candidates);

    if moves.is_empty() {
        return eval::evaluate(board);
    }

    let ordered = order_moves(
        &moves, board, &state.killers, &state.history,
        depth as usize, config,
    );

    let mut best_score = -INFINITY;
    let mut best_pv: Vec<Move> = Vec::new();
    let side = if board.is_white_to_move() { 0 } else { 1 };

    if !config.alpha_beta {
        // Pure minimax (no pruning)
        for &mv in &ordered {
            let undo = board.make(&mv);

            let mut child_pv = Vec::new();
            let score = -alphabeta_staged(
                board, depth - 1, -INFINITY, INFINITY,
                config, state, tt, blacklist, ply + 1, &mut child_pv,
            );

            board.unmake(&mv, undo);

            if score > best_score {
                best_score = score;
                best_pv = vec![mv];
                best_pv.extend(child_pv);
            }
        }
    } else {
        // Alpha-beta pruning with LMR
        for (move_idx, &mv) in ordered.iter().enumerate() {
            let is_capture_move = is_capture(board, &mv);
            let undo = board.make(&mv);

            let mut child_pv = Vec::new();
            let mut score;

            // Late Move Reductions: for quiet moves beyond the first 3, at depth >= 3
            let do_lmr = config.lmr
                && depth >= 3
                && move_idx >= 3
                && !is_capture_move;

            if do_lmr {
                // Reduced search
                score = -alphabeta_staged(
                    board, depth - 2, -beta, -alpha,
                    config, state, tt, blacklist, ply + 1, &mut child_pv,
                );
                // Re-search at full depth if score > alpha
                if score > alpha {
                    child_pv.clear();
                    score = -alphabeta_staged(
                        board, depth - 1, -beta, -alpha,
                        config, state, tt, blacklist, ply + 1, &mut child_pv,
                    );
                }
            } else {
                score = -alphabeta_staged(
                    board, depth - 1, -beta, -alpha,
                    config, state, tt, blacklist, ply + 1, &mut child_pv,
                );
            }

            board.unmake(&mv, undo);

            if score > best_score {
                best_score = score;
                best_pv = vec![mv];
                best_pv.extend(child_pv);
            }
            if score > alpha {
                alpha = score;
            }
            if alpha >= beta {
                // Beta cutoff — store killer/history
                if !is_capture_move {
                    if config.killer_moves {
                        state.killers.store(mv, depth as usize);
                    }
                    state.history.update(&mv, side, depth);
                }
                break;
            }
        }
    }

    // Update PV chain
    if !best_pv.is_empty() {
        *pv = best_pv;
    }

    best_score
}

// ── Stage 2 Engine ───────────────────────────────────────────────────────────

/// Stage 2 refines the selection among Stage 1 candidates.
pub struct Stage2Engine {
    pub config: StageConfig,
    pub tt: Arc<LeafTT>,
}

impl Stage2Engine {
    /// Create a new Stage2Engine reusing the given TT from Stage 1.
    pub fn new(config: StageConfig, tt: Arc<LeafTT>) -> Self {
        Stage2Engine { config, tt }
    }

    /// Search: for each candidate PVLine from Stage 1, replay only the root move,
    /// then run alpha-beta from that position. Return the candidate with the best score.
    pub fn search(&self, board: &Board, candidates: &[PVLine]) -> SearchResult {
        let depth = self.config.effective_depth() as i32;
        let mut best_score = -INFINITY;
        let mut best_idx = 0;
        let mut total_nodes: u64 = 0;

        let mut refined: Vec<(usize, i32)> = Vec::new();

        for (i, pv) in candidates.iter().enumerate() {
            let mut board_copy = *board;
            let undo = board_copy.make(&pv.root_move);

            let mut state = SearchState::new();
            let mut child_pv = Vec::new();

            let score = -alphabeta_staged(
                &mut board_copy,
                depth - 1,
                -INFINITY,
                INFINITY,
                &self.config,
                &mut state,
                &self.tt,
                &HashSet::new(),
                1,
                &mut child_pv,
            );

            board_copy.unmake(&pv.root_move, undo);
            total_nodes += state.nodes;

            refined.push((i, score));

            if score > best_score {
                best_score = score;
                best_idx = i;
            }
        }

        let best_pv = &candidates[best_idx];

        // Build top_moves with refined scores
        let mut top_moves: Vec<PVLine> = refined.iter().map(|&(idx, score)| {
            let mut pv = candidates[idx].clone();
            pv.score = score;
            pv
        }).collect();
        top_moves.sort_by(|a, b| b.score.cmp(&a.score));

        SearchResult {
            best_move: best_pv.root_move,
            score: best_score,
            depth: self.config.effective_depth(),
            nodes_visited: total_nodes,
            top_moves,
        }
    }
}

/// Check whether all PVLines share the same root move.
pub fn all_same_root_move(top_moves: &[PVLine]) -> bool {
    if top_moves.is_empty() {
        return true;
    }
    let first = top_moves[0].root_move;
    top_moves.iter().all(|pv| pv.root_move == first)
}

// ══════════  Tests  ══════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    fn test_config() -> SearchConfig {
        SearchConfig {
            depth: 4,
            top_moves: 3,
            threads: 1,
            ..SearchConfig::default()
        }
    }

    #[test]
    fn stage1_finds_moves_from_opening() {
        let board = Board::new();
        let (result, stats) = stage1_search(&board, &test_config());
        assert!(!result.top_moves.is_empty(), "should find legal moves");
        assert!(stats.nodes_visited > 0, "should search some nodes");
    }

    #[test]
    fn stage1_game_over_returns_empty() {
        let mut board = Board::new();
        board.set_game_over(true, true, false);
        let (result, _) = stage1_search(&board, &test_config());
        assert!(result.top_moves.is_empty());
    }

    #[test]
    fn stage1_returns_legal_move() {
        let board = Board::new();
        let (result, _) = stage1_search(&board, &test_config());
        let game = Game::from_board(board);
        let all_moves = game.get_all_moves();
        let legal = flatten_moves(&all_moves);
        assert!(legal.contains(&result.best_move),
            "engine returned illegal move {:?}", result.best_move);
    }

    #[test]
    fn multipv_returns_multiple_lines() {
        let board = Board::new();
        let cfg = SearchConfig {
            depth: 2,
            top_moves: 3,
            threads: 1,
            ..SearchConfig::default()
        };
        let (result, _) = stage1_search(&board, &cfg);
        // Should have at least 2 PV lines (may be less if positions are the same)
        assert!(result.top_moves.len() >= 1, "should have at least 1 PV line");
    }

    #[test]
    fn no_tt_flag_works() {
        let board = Board::new();
        let cfg = SearchConfig {
            depth: 2,
            no_tt: true,
            threads: 1,
            ..SearchConfig::default()
        };
        let (result, stats) = stage1_search(&board, &cfg);
        assert!(!result.top_moves.is_empty());
        assert_eq!(stats.tt_hits, 0, "TT should be disabled");
    }

    #[test]
    fn no_alpha_beta_flag_works() {
        let board = Board::new();
        let cfg = SearchConfig {
            depth: 2,
            no_alpha_beta: true,
            threads: 1,
            ..SearchConfig::default()
        };
        let (result, _) = stage1_search(&board, &cfg);
        assert!(!result.top_moves.is_empty());
    }

    #[test]
    fn pv_chain_has_root_move() {
        let board = Board::new();
        let cfg = SearchConfig {
            depth: 2,
            top_moves: 1,
            threads: 1,
            ..SearchConfig::default()
        };
        let (result, _) = stage1_search(&board, &cfg);
        if let Some(pv) = result.top_moves.first() {
            assert_eq!(pv.root_move, pv.moves[0],
                "first move in PV chain should be the root move");
        }
    }

    #[test]
    fn stage_config_stage1_search_works() {
        let board = Board::new();
        let config = StageConfig::stage1();
        let (result, stats, _tt) = stage1_search_with_config(&board, &config, 1);
        assert!(!result.top_moves.is_empty(), "should find legal moves");
        assert!(stats.nodes_visited > 0, "should search some nodes");
    }

    #[test]
    fn stage2_engine_refines_candidates() {
        let board = Board::new();
        let s1_config = StageConfig::stage1();
        let (s1_result, _, tt) = stage1_search_with_config(&board, &s1_config, 1);

        if s1_result.top_moves.len() > 1 && !all_same_root_move(&s1_result.top_moves) {
            let mut s2_config = StageConfig::stage2();
            s2_config.depth = 2; // shallow for test speed
            let s2_engine = Stage2Engine::new(s2_config, tt);
            let s2_result = s2_engine.search(&board, &s1_result.top_moves);
            assert!(!s2_result.top_moves.is_empty());
        }
    }

    #[test]
    fn all_same_root_move_works() {
        let mv1 = Move { from: Position::new(0, 6), to: Position::new(0, 5), unstack: false };
        let mv2 = Move { from: Position::new(1, 6), to: Position::new(1, 5), unstack: false };
        let pv1 = PVLine { root_move: mv1, moves: vec![mv1], score: 10, leaf_board: Board::new() };
        let pv2 = PVLine { root_move: mv1, moves: vec![mv1], score: 5, leaf_board: Board::new() };
        let pv3 = PVLine { root_move: mv2, moves: vec![mv2], score: 8, leaf_board: Board::new() };

        assert!(all_same_root_move(&[pv1.clone(), pv2.clone()]));
        assert!(!all_same_root_move(&[pv1, pv3]));
        assert!(all_same_root_move(&[]));
    }
}
