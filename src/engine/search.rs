//! Two-stage alpha-beta search engine for Keres.
//!
//! ## Stage 1 — Exhaustive Parallel Brute Force (depth=4)
//! Scores every legal root move with guaranteed tactical coverage.
//! Uses alpha-beta with MVV-LVA + killer moves + history heuristic + TT.
//!
//! ## Stage 2 — Selective Deep Analysis (depth 5–7)
//! Confirms or refutes the top-K candidates from Stage 1.
//! Enables null move pruning, SEE-based extensions, LMR, aspiration
//! windows, and futility pruning.

use crate::board::Board;
use crate::game::{Game, Move, PotentialMove};
use super::config::{EngineConfig, ScoringWeights};
use super::evaluator::eval_centipawns;
use super::tt::{TranspositionTable, TtEntry, Bound};
use super::zobrist::hash_board;
use super::see;

use std::sync::Arc;

// ── Constants ────────────────────────────────────────────────────────────────

const INFINITY: i32 = 100_000;
const MATE_SCORE: i32 = 50_000;

// Move-ordering priority tiers
const HASH_MOVE_SCORE: i32 = 1_000_000;
const CAPTURE_BASE_SCORE: i32 = 100_000;
const KILLER_SCORE_1: i32 = 90_000;
const KILLER_SCORE_2: i32 = 80_000;

// Stage 2 parameters
const STAGE2_DELTA: i32 = 75;
const STAGE2_MAX_CANDIDATES: usize = 6;
const ASPIRATION_DELTA: i32 = 50;
const NULL_MOVE_R: i32 = 2;

/// Default Stage 1 search depth.
pub const STAGE1_DEPTH: i32 = 4;
/// Default Stage 2 search depth.
pub const STAGE2_DEPTH: i32 = 6;

const SCORE_SIGMOID_SCALE: f32 = 2000.0;

// Late Move Reduction thresholds
const LMR_MIN_MOVE_INDEX: usize = 3;
const LMR_DEEP_MOVE_INDEX: usize = 6;
const LMR_SHALLOW_REDUCTION: i32 = 1;
const LMR_DEEP_REDUCTION: i32 = 2;

// ── Public types ─────────────────────────────────────────────────────────────

/// A move paired with its centipawn score (from the root side-to-move's
/// perspective: higher = better for the root player).
#[derive(Clone, Debug)]
pub struct ScoredMove {
    pub mv: Move,
    pub score: i32,
}

/// Aggregate statistics for the two-stage search.
#[derive(Clone, Debug)]
pub struct SearchStats {
    pub stage1_moves: usize,
    pub stage2_candidates: usize,
    pub best_score: i32,
    pub nodes_searched: usize,
}

// ── Internal data structures ─────────────────────────────────────────────────

/// Killer move table: 2 slots per depth level (max 64 ply).
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

/// History heuristic table: indexed by [from_square][to_square].
struct HistoryTable {
    table: [[i32; 81]; 81],
}

impl HistoryTable {
    fn new() -> Self { HistoryTable { table: [[0; 81]; 81] } }

    fn get(&self, mv: &Move) -> i32 {
        self.table[mv.from.to_absolute()][mv.to.to_absolute()]
    }

    fn update(&mut self, mv: &Move, depth: i32) {
        self.table[mv.from.to_absolute()][mv.to.to_absolute()] += depth * depth;
    }
}

/// Per-thread search context carrying weights, TT handle, and heuristic tables.
struct SearchContext {
    weights: ScoringWeights,
    tt: Arc<TranspositionTable>,
    killers: KillerTable,
    history: HistoryTable,
    nodes_searched: usize,
}

impl SearchContext {
    fn new(weights: ScoringWeights, tt: Arc<TranspositionTable>) -> Self {
        SearchContext {
            weights, tt,
            killers: KillerTable::new(),
            history: HistoryTable::new(),
            nodes_searched: 0,
        }
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

/// Score and sort moves according to the ordering hierarchy:
/// 1. Hash move  2. MVV-LVA captures  3. Killer moves  4. History heuristic
fn order_moves(
    moves: &[Move],
    board: &Board,
    tt_move: Option<Move>,
    killers: &KillerTable,
    history: &HistoryTable,
    depth: usize,
    weights: &ScoringWeights,
) -> Vec<Move> {
    let game = Game::from_board(*board);
    let mut scored: Vec<(Move, i32)> = moves.iter().map(|&mv| {
        if Some(mv) == tt_move {
            return (mv, HASH_MOVE_SCORE);
        }
        if game.is_capture(&mv) {
            let victim_val = game.capture_value(&mv) as i32;
            let atk_val = if let Some(p) = board.get_piece(&mv.from) {
                see::attacker_value(p, mv.unstack, weights)
            } else { 0 };
            return (mv, CAPTURE_BASE_SCORE + victim_val * 10 - atk_val);
        }
        let killer_sc = killers.score(&mv, depth);
        if killer_sc > 0 { return (mv, killer_sc); }
        (mv, history.get(&mv))
    }).collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(mv, _)| mv).collect()
}

/// Evaluate the board from the side-to-move's perspective in centipawns.
fn eval_stm(board: &Board, weights: &ScoringWeights) -> i32 {
    let raw = eval_centipawns(board, weights);
    if board.is_white_to_move() { raw } else { -raw }
}

/// Convert a centipawn score (side-to-move perspective) to a [0,1]
/// probability from white's perspective (for debug output).
fn cp_stm_to_white_sigmoid(cp: i32, white_to_move: bool) -> f32 {
    let white_cp = if white_to_move { cp } else { -cp };
    let x = -(white_cp as f32) / SCORE_SIGMOID_SCALE;
    1.0 / (1.0 + x.exp())
}

// ══════════  Stage 1: Exhaustive depth-4 alpha-beta  ══════════════════════════

/// Run Stage 1: exhaustive alpha-beta at the given depth across all root moves.
///
/// Returns a sorted list of `ScoredMove`s (best first from the root player's
/// perspective) and the total number of nodes searched.
pub fn stage1_search(
    board: &Board,
    depth: i32,
    weights: ScoringWeights,
    tt: Arc<TranspositionTable>,
    num_threads: usize,
) -> (Vec<ScoredMove>, usize) {
    if board.is_game_over() {
        return (Vec::new(), 0);
    }

    let game = Game::from_board(*board);
    let candidates = game.get_all_moves();
    let moves = flatten_moves(&candidates);

    if moves.is_empty() {
        return (Vec::new(), 0);
    }

    let (scored_moves, total_nodes) = if num_threads <= 1 || moves.len() <= 1 {
        // Single-threaded fallback
        let mut ctx = SearchContext::new(weights, Arc::clone(&tt));
        let results: Vec<ScoredMove> = moves.iter().filter_map(|&mv| {
            game.apply_move_copy(mv).ok().map(|child| {
                let score = -alphabeta_s1(&child, depth - 1, -INFINITY, INFINITY, &mut ctx, 1);
                ScoredMove { mv, score }
            })
        }).collect();
        let nodes = ctx.nodes_searched;
        (results, nodes)
    } else {
        // Multi-threaded: distribute root moves across threads
        use std::sync::Mutex;
        let results = Mutex::new(Vec::with_capacity(moves.len()));
        let node_counts = Mutex::new(0usize);

        std::thread::scope(|s| {
            let chunk_size = (moves.len() + num_threads - 1) / num_threads;
            let chunks: Vec<&[Move]> = moves.chunks(chunk_size).collect();
            let mut handles = Vec::new();

            for chunk in chunks {
                let tt = Arc::clone(&tt);
                let results = &results;
                let node_counts = &node_counts;
                let board_copy = *board;

                handles.push(s.spawn(move || {
                    let mut ctx = SearchContext::new(weights, tt);
                    let game = Game::from_board(board_copy);
                    let mut local_results = Vec::new();

                    for &mv in chunk {
                        if let Ok(child) = game.apply_move_copy(mv) {
                            let score = -alphabeta_s1(
                                &child, depth - 1, -INFINITY, INFINITY, &mut ctx, 1,
                            );
                            local_results.push(ScoredMove { mv, score });
                        }
                    }

                    results.lock().unwrap().extend(local_results);
                    *node_counts.lock().unwrap() += ctx.nodes_searched;
                }));
            }

            for h in handles { h.join().unwrap(); }
        });

        (results.into_inner().unwrap(), node_counts.into_inner().unwrap())
    };

    // Sort: best (highest) score first (side-to-move's perspective)
    let mut result = scored_moves;
    result.sort_by(|a, b| b.score.cmp(&a.score));
    (result, total_nodes)
}

/// Alpha-beta for Stage 1.
/// No null-move pruning, no LMR, no extensions — pure brute force.
fn alphabeta_s1(
    board: &Board,
    depth: i32,
    mut alpha: i32,
    beta: i32,
    ctx: &mut SearchContext,
    ply: i32,
) -> i32 {
    ctx.nodes_searched += 1;

    // Terminal node
    if board.is_game_over() {
        if board.is_draw() { return 0; }
        // After a king capture the turn has flipped, so the current
        // side-to-move is always the *loser*.
        return -(MATE_SCORE - ply);
    }

    // Horizon — return static eval
    if depth <= 0 {
        return eval_stm(board, &ctx.weights);
    }

    // TT probe
    let hash = hash_board(board);
    let tt_move = if let Some(entry) = ctx.tt.probe(hash) {
        if entry.depth >= depth as i8 {
            match entry.bound {
                Bound::Exact => return entry.score,
                Bound::LowerBound => {
                    if entry.score >= beta { return entry.score; }
                    if entry.score > alpha { alpha = entry.score; }
                }
                Bound::UpperBound => {
                    if entry.score <= alpha { return entry.score; }
                }
            }
        }
        if entry.best_move != 0 { Some(Move::from_u16(entry.best_move)) } else { None }
    } else {
        None
    };

    // Generate & order moves
    let game = Game::from_board(*board);
    let candidates = game.get_all_moves();
    let moves = flatten_moves(&candidates);
    if moves.is_empty() {
        return eval_stm(board, &ctx.weights);
    }

    let ordered = order_moves(
        &moves, board, tt_move, &ctx.killers, &ctx.history,
        depth as usize, &ctx.weights,
    );

    let mut best_score = -INFINITY;
    let mut best_move = ordered[0];
    let original_alpha = alpha;

    for mv in &ordered {
        if let Ok(child) = game.apply_move_copy(*mv) {
            let score = -alphabeta_s1(&child, depth - 1, -beta, -alpha, ctx, ply + 1);
            if score > best_score {
                best_score = score;
                best_move = *mv;
            }
            if score > alpha { alpha = score; }
            if alpha >= beta {
                if !game.is_capture(mv) {
                    ctx.killers.store(*mv, depth as usize);
                    ctx.history.update(mv, depth);
                }
                break;
            }
        }
    }

    // TT store
    let bound = if best_score <= original_alpha {
        Bound::UpperBound
    } else if best_score >= beta {
        Bound::LowerBound
    } else {
        Bound::Exact
    };
    ctx.tt.store(hash, TtEntry {
        depth: depth as i8, bound, score: best_score,
        best_move: best_move.to_u16(),
    });

    best_score
}

// ══════════  Stage 2: Selective deep search  ══════════════════════════════════

/// Filter Stage 1 results into Stage 2 candidates.
///
/// Keeps moves within `δ = 75 pts` of the best Stage 1 score, hard-capped
/// at 6 candidates. Auto-includes captures with SEE ≥ 0.
pub fn filter_candidates(
    stage1_results: &[ScoredMove],
    board: &Board,
    weights: &ScoringWeights,
) -> Vec<ScoredMove> {
    if stage1_results.is_empty() { return Vec::new(); }

    let best_score = stage1_results[0].score;

    let mut candidates: Vec<ScoredMove> = stage1_results.iter()
        .filter(|sm| sm.score >= best_score - STAGE2_DELTA)
        .cloned()
        .collect();

    // Auto-include captures with SEE ≥ 0 that weren't already selected
    let game = Game::from_board(*board);
    for sm in stage1_results {
        if game.is_capture(&sm.mv) {
            let see_val = see::see_capture(
                board, &sm.mv.from, &sm.mv.to, sm.mv.unstack, weights,
            );
            if see_val >= 0 && !candidates.iter().any(|c| c.mv == sm.mv) {
                candidates.push(sm.clone());
            }
        }
    }

    candidates.truncate(STAGE2_MAX_CANDIDATES);
    candidates
}

/// Run Stage 2: selective deep search on the filtered candidates.
///
/// Uses aspiration windows around the Stage 1 score; falls back to
/// a full-window re-search on failure.
pub fn stage2_search(
    board: &Board,
    candidates: &[ScoredMove],
    max_depth: i32,
    weights: ScoringWeights,
    tt: Arc<TranspositionTable>,
) -> (Vec<ScoredMove>, usize) {
    if candidates.is_empty() { return (Vec::new(), 0); }

    let mut ctx = SearchContext::new(weights, tt);
    let game = Game::from_board(*board);
    let mut results = Vec::new();

    for sm in candidates {
        if let Ok(child) = game.apply_move_copy(sm.mv) {
            let lo = sm.score - ASPIRATION_DELTA;
            let hi = sm.score + ASPIRATION_DELTA;

            let mut score = -alphabeta_s2(
                &child, max_depth - 1, -hi, -lo, &mut ctx, true, 1,
            );

            // Re-search with full window on aspiration fail
            if score <= lo || score >= hi {
                score = -alphabeta_s2(
                    &child, max_depth - 1, -INFINITY, INFINITY, &mut ctx, true, 1,
                );
            }

            results.push(ScoredMove { mv: sm.mv, score });
        }
    }

    let nodes = ctx.nodes_searched;
    results.sort_by(|a, b| b.score.cmp(&a.score));
    (results, nodes)
}

/// Alpha-beta for Stage 2 with the full technique stack:
/// - Null-move pruning (R=2, depth ≥ 5, not consecutive)
/// - SEE-based capture extensions
/// - Late Move Reductions (depth ≥ 5, move index ≥ 3)
/// - Futility pruning at frontier nodes (depth=1)
/// - Full TT reuse from Stage 1
fn alphabeta_s2(
    board: &Board,
    depth: i32,
    mut alpha: i32,
    beta: i32,
    ctx: &mut SearchContext,
    allow_null: bool,
    ply: i32,
) -> i32 {
    ctx.nodes_searched += 1;

    if board.is_game_over() {
        if board.is_draw() { return 0; }
        return -(MATE_SCORE - ply);
    }

    if depth <= 0 {
        return eval_stm(board, &ctx.weights);
    }

    // TT probe
    let hash = hash_board(board);
    let tt_move = if let Some(entry) = ctx.tt.probe(hash) {
        if entry.depth >= depth as i8 {
            match entry.bound {
                Bound::Exact => return entry.score,
                Bound::LowerBound => {
                    if entry.score >= beta { return entry.score; }
                    if entry.score > alpha { alpha = entry.score; }
                }
                Bound::UpperBound => {
                    if entry.score <= alpha { return entry.score; }
                }
            }
        }
        if entry.best_move != 0 { Some(Move::from_u16(entry.best_move)) } else { None }
    } else {
        None
    };

    let static_eval = eval_stm(board, &ctx.weights);

    // Futility pruning at frontier nodes (depth == 1)
    if depth == 1 {
        let max_cap = see::max_capturable_value(
            board, board.color_to_move(), &ctx.weights,
        );
        if static_eval + max_cap < alpha {
            return static_eval;
        }
    }

    // Null-move pruning (only depth ≥ 5, not consecutive)
    if allow_null && depth >= 5 && static_eval >= beta {
        let mut null_board = *board;
        null_board.set_white_to_move(!board.is_white_to_move());

        let null_score = -alphabeta_s2(
            &null_board, depth - 1 - NULL_MOVE_R, -beta, -(beta - 1),
            ctx, false, ply + 1,
        );

        if null_score >= beta {
            return beta;
        }
    }

    // Generate & order moves
    let game = Game::from_board(*board);
    let candidates = game.get_all_moves();
    let moves = flatten_moves(&candidates);
    if moves.is_empty() {
        return static_eval;
    }

    let ordered = order_moves(
        &moves, board, tt_move, &ctx.killers, &ctx.history,
        depth as usize, &ctx.weights,
    );

    let mut best_score = -INFINITY;
    let mut best_move = ordered[0];
    let original_alpha = alpha;

    for (i, mv) in ordered.iter().enumerate() {
        if let Ok(child) = game.apply_move_copy(*mv) {
            let is_capture = game.is_capture(mv);

            // Selective extension: extend for SEE ≥ 0 captures
            let mut extension = 0;
            if is_capture {
                let see_val = see::see_capture(
                    board, &mv.from, &mv.to, mv.unstack, &ctx.weights,
                );
                if see_val >= 0 { extension = 1; }
            }

            let mut search_depth = depth - 1 + extension;

            // LMR: reduce late quiet moves at depth ≥ 5
            let mut reduced = false;
            if depth >= 5 && i >= LMR_MIN_MOVE_INDEX && !is_capture && extension == 0 {
                let reduction = if i >= LMR_DEEP_MOVE_INDEX { LMR_DEEP_REDUCTION } else { LMR_SHALLOW_REDUCTION };
                search_depth = (search_depth - reduction).max(1);
                reduced = true;
            }

            let mut score = -alphabeta_s2(
                &child, search_depth, -beta, -alpha, ctx, true, ply + 1,
            );

            // Re-search at full depth if LMR reduced search raised alpha
            if reduced && score > alpha {
                score = -alphabeta_s2(
                    &child, depth - 1 + extension, -beta, -alpha, ctx, true, ply + 1,
                );
            }

            if score > best_score { best_score = score; best_move = *mv; }
            if score > alpha { alpha = score; }
            if alpha >= beta {
                if !is_capture {
                    ctx.killers.store(*mv, depth as usize);
                    ctx.history.update(mv, depth);
                }
                break;
            }
        }
    }

    // TT store
    let bound = if best_score <= original_alpha {
        Bound::UpperBound
    } else if best_score >= beta {
        Bound::LowerBound
    } else {
        Bound::Exact
    };
    ctx.tt.store(hash, TtEntry {
        depth: depth as i8, bound, score: best_score,
        best_move: best_move.to_u16(),
    });

    best_score
}

// ══════════  Pipeline  ═══════════════════════════════════════════════════════

/// Run the complete two-stage search pipeline.
pub fn two_stage_search(
    board: &Board,
    config: &EngineConfig,
) -> Result<(Move, SearchStats), String> {
    if board.is_game_over() {
        return Err("cannot search from a terminal position".into());
    }

    let tt = Arc::new(TranspositionTable::new(1 << 20)); // ~1 M entries
    let weights = config.weights;

    // Stage 1
    let (stage1_results, s1_nodes) = stage1_search(
        board, config.stage1_depth, weights, Arc::clone(&tt), config.threads,
    );
    if stage1_results.is_empty() {
        return Err("no legal moves found".into());
    }

    if config.disable_stage2 {
        let best = &stage1_results[0];
        return Ok((best.mv, SearchStats {
            stage1_moves: stage1_results.len(),
            stage2_candidates: 0,
            best_score: best.score,
            nodes_searched: s1_nodes,
        }));
    }

    // Filter → Stage 2
    let candidates = filter_candidates(&stage1_results, board, &weights);
    let (stage2_results, s2_nodes) = stage2_search(
        board, &candidates, STAGE2_DEPTH, weights, tt,
    );

    let best = if stage2_results.is_empty() {
        &stage1_results[0]
    } else {
        &stage2_results[0]
    };

    Ok((best.mv, SearchStats {
        stage1_moves: stage1_results.len(),
        stage2_candidates: candidates.len(),
        best_score: best.score,
        nodes_searched: s1_nodes + s2_nodes,
    }))
}

/// Run the two-stage pipeline and return per-stage results for debugging.
pub fn two_stage_search_debug(
    board: &Board,
    config: &EngineConfig,
) -> Result<(Move, SearchStats, Vec<ScoredMove>, Vec<ScoredMove>), String> {
    if board.is_game_over() {
        return Err("cannot search from a terminal position".into());
    }

    let tt = Arc::new(TranspositionTable::new(1 << 20));
    let weights = config.weights;

    let (stage1_results, s1_nodes) = stage1_search(
        board, config.stage1_depth, weights, Arc::clone(&tt), config.threads,
    );
    if stage1_results.is_empty() {
        return Err("no legal moves found".into());
    }

    if config.disable_stage2 {
        let best = &stage1_results[0];
        return Ok((best.mv, SearchStats {
            stage1_moves: stage1_results.len(),
            stage2_candidates: 0,
            best_score: best.score,
            nodes_searched: s1_nodes,
        }, stage1_results, Vec::new()));
    }

    let candidates = filter_candidates(&stage1_results, board, &weights);
    let (stage2_results, s2_nodes) = stage2_search(
        board, &candidates, STAGE2_DEPTH, weights, tt,
    );

    let best = if stage2_results.is_empty() {
        &stage1_results[0]
    } else {
        &stage2_results[0]
    };

    Ok((best.mv, SearchStats {
        stage1_moves: stage1_results.len(),
        stage2_candidates: candidates.len(),
        best_score: best.score,
        nodes_searched: s1_nodes + s2_nodes,
    }, stage1_results, stage2_results))
}

/// Serialisable snapshot of the search tree for external debugging tools.
#[derive(serde::Serialize)]
pub struct DebugTree {
    pub node_id: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub score: f32,
    pub stage1_score: f32,
    pub zobrist_key: u64,
    pub white_to_move: bool,
    pub is_terminal: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<DebugTree>,
}

/// Build a `DebugTree` from the two-stage search results.
///
/// The root node represents the position analyzed; its children are
/// the Stage 1 scored moves.  The `score` of each child reflects the
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
        DebugTree {
            node_id: i + 1,
            action: Some(sm.mv.to_string()),
            score: cp_stm_to_white_sigmoid(final_cp, white_to_move),
            stage1_score: cp_stm_to_white_sigmoid(sm.score, white_to_move),
            zobrist_key: 0, // child zobrist not computed here
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
        zobrist_key: root_hash,
        white_to_move,
        is_terminal: board.is_game_over(),
        children,
    }
}

// ══════════  Tests  ══════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn stage1_finds_moves_from_opening() {
        let board = Board::new();
        let tt = Arc::new(TranspositionTable::new(1 << 16));
        let (results, nodes) = stage1_search(&board, 4, ScoringWeights::default(), tt, 1);
        assert!(!results.is_empty(), "should find legal moves from opening");
        assert!(nodes > 0, "should search some nodes");
    }

    #[test]
    fn stage1_game_over_returns_empty() {
        let mut board = Board::new();
        board.set_game_over(true, true, false);
        let tt = Arc::new(TranspositionTable::new(1024));
        let (results, _) = stage1_search(&board, 4, ScoringWeights::default(), tt, 1);
        assert!(results.is_empty());
    }

    #[test]
    fn filter_caps_at_six() {
        let board = Board::new();
        let weights = ScoringWeights::default();
        let game = Game::from_board(board);
        let candidates = game.get_all_moves();
        let moves = flatten_moves(&candidates);
        // Create many moves with the same score so they all pass the δ filter
        let fake: Vec<ScoredMove> = moves.iter().take(20).map(|&mv| {
            ScoredMove { mv, score: 0 }
        }).collect();
        let filtered = filter_candidates(&fake, &board, &weights);
        assert!(filtered.len() <= STAGE2_MAX_CANDIDATES,
            "should cap at {} candidates, got {}", STAGE2_MAX_CANDIDATES, filtered.len());
    }

    #[test]
    fn two_stage_finds_legal_move() {
        let board = Board::new();
        let mut config = EngineConfig::default();
        config.threads = 1; // deterministic for testing
        let (mv, stats) = two_stage_search(&board, &config).expect("should find a move");
        assert!(stats.nodes_searched > 0);
        let game = Game::from_board(board);
        let all_moves = game.get_all_moves();
        let legal = flatten_moves(&all_moves);
        assert!(legal.contains(&mv), "engine returned illegal move {mv:?}");
    }

    #[test]
    fn pipeline_terminal_returns_error() {
        let mut board = Board::new();
        board.set_game_over(true, true, false);
        let config = EngineConfig::default();
        assert!(two_stage_search(&board, &config).is_err());
    }

    #[test]
    fn debug_pipeline_returns_all_stages() {
        let board = Board::new();
        let mut config = EngineConfig::default();
        config.threads = 1;
        let (mv, stats, s1, s2) = two_stage_search_debug(&board, &config)
            .expect("should find a move");
        assert!(!s1.is_empty(), "stage 1 should have results");
        assert!(!s2.is_empty(), "stage 2 should have results");
        assert!(stats.stage2_candidates > 0);
        let legal = flatten_moves(&Game::from_board(board).get_all_moves());
        assert!(legal.contains(&mv));
    }
}
