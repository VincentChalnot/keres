//! Board position evaluators for the Keres search engine.
//!
//! One implementation is provided:
//! - `CpuEvaluator`: pure-Rust heuristic evaluation with quiescence search

use std::collections::HashMap;
use parking_lot::RwLock;
use crate::board::{Board, Color, PieceType, Position, BOARD_SIZE};
use crate::game::{Game, Move};
use super::config::ScoringWeights;

/// Scale factor for the sigmoid used to convert raw material scores to [0, 1].
/// A raw difference of `SCORE_SIGMOID_SCALE` points corresponds to roughly 0.73
/// in the sigmoid output.
const SCORE_SIGMOID_SCALE: f32 = 2000.0;

/// Additive margin (in raw points) added to a capture's victim value during
/// delta pruning to allow for positional gains that are hard to quantify.
const DELTA_PRUNING_MARGIN: f32 = 150.0;

/// Divisor used to convert raw points into an approximate [0, 1] score delta
/// for delta pruning comparisons.  Chosen so that at equilibrium the derivative
/// of the sigmoid roughly matches the conversion (4 × SCORE_SIGMOID_SCALE).
const DELTA_PRUNING_SCALE: f32 = 8000.0;

/// Trait implemented by anything that can assign a [0,1] score to
/// one or more board positions.  `Send + Sync` is required so evaluators
/// can be shared across threads via `Arc<dyn Evaluator>`.
pub trait Evaluator: Send + Sync {
    fn score_positions(&self, boards: &[Board]) -> Vec<f32>;
}

// ── Shared helpers ──────────────────────────────────────────────────────────

/// Map a `PieceType` to the integer discriminant used by `ScoringWeights::material_value`.
fn piece_disc(pt: &PieceType) -> u32 {
    match pt {
        PieceType::Soldier  => 1,
        PieceType::Bishop   => 2,
        PieceType::Rook     => 3,
        PieceType::Paladin  => 4,
        PieceType::Guard    => 5,
        PieceType::Knight   => 6,
        PieceType::Ballista => 7,
        PieceType::King     => 8,
    }
}

// ══════════  CpuEvaluator  ══════════

pub struct CpuEvaluator {
    pub weights: ScoringWeights,
    cache: RwLock<HashMap<[u8; 83], f32>>,
}

impl CpuEvaluator {
    pub fn new(weights: ScoringWeights) -> Self {
        CpuEvaluator {
            weights,
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn evaluate_single(&self, board: &Board) -> f32 {
        if board.is_game_over() {
            if board.is_draw() { return 0.5; }
            return if board.white_wins() { 1.0 } else { 0.0 };
        }
        // Check cache first
        let key = {
            let bin = board.to_binary();
            let mut k = [0u8; 83];
            k.copy_from_slice(&bin[..83]);
            k
        };
        {
            let cache = self.cache.read();
            if let Some(&cached) = cache.get(&key) {
                return cached;
            }
        }
        let score = self.quiescence(board, 0);
        {
            let mut cache = self.cache.write();
            cache.insert(key, score);
        }
        score
    }

    fn quiescence(&self, board: &Board, depth: u8) -> f32 {
        let stand_pat = self.static_eval(board);
        if depth >= 3 { return stand_pat; }
        if board.is_game_over() {
            if board.is_draw() { return 0.5; }
            return if board.white_wins() { 1.0 } else { 0.0 };
        }

        let game = Game::from_board(*board);
        let all_moves = game.get_all_moves();

        // Only consider captures of pieces worth more than a soldier
        let mut captures: Vec<(Move, u32)> = Vec::new();
        for pm in &all_moves {
            if pm.force_unstack {
                let mv_unstack = pm.to_move(true);
                if game.is_capture(&mv_unstack) {
                    let val = game.capture_value(&mv_unstack);
                    if val > self.weights.soldier_pts {
                        captures.push((mv_unstack, val));
                    }
                }
            } else {
                let mv = pm.to_move(false);
                if game.is_capture(&mv) {
                    let val = game.capture_value(&mv);
                    if val > self.weights.soldier_pts {
                        captures.push((mv, val));
                    }
                }
                if pm.unstackable {
                    let mv_unstack = pm.to_move(true);
                    if game.is_capture(&mv_unstack) {
                        let val = game.capture_value(&mv_unstack);
                        if val > self.weights.soldier_pts {
                            captures.push((mv_unstack, val));
                        }
                    }
                }
            }
        }

        if captures.is_empty() {
            return stand_pat;
        }

        // Sort by MVV: highest victim value first
        captures.sort_by_key(|(_, val)| std::cmp::Reverse(*val));

        let white_to_move = board.is_white_to_move();
        let mut best = stand_pat;

        for (capture, victim_pts) in &captures {
            // Delta pruning: skip if even the best-case gain from this capture
            // cannot improve the current best.  The score improvement is
            // approximated in [0,1] space using the sigmoid scale constants.
            let delta_bound = (*victim_pts as f32 + DELTA_PRUNING_MARGIN) / DELTA_PRUNING_SCALE;
            if white_to_move && stand_pat + delta_bound < best { continue; }
            if !white_to_move && stand_pat - delta_bound > best { continue; }

            if let Ok(child_board) = game.apply_move_copy(*capture) {
                let score = self.quiescence(&child_board, depth + 1);
                if white_to_move {
                    if score > best { best = score; }
                } else {
                    if score < best { best = score; }
                }
            }
        }

        best
    }

    fn static_eval(&self, board: &Board) -> f32 {
        if board.is_game_over() {
            if board.is_draw() { return 0.5; }
            return if board.white_wins() { 1.0 } else { 0.0 };
        }

        let mut white_score: f32 = 0.0;
        let mut black_score: f32 = 0.0;

        for sq in 0..BOARD_SIZE {
            let pos = Position::from_u8(sq as u8);
            if let Some(piece) = board.get_piece(&pos) {
                let accumulator = if piece.color == Color::White {
                    &mut white_score
                } else {
                    &mut black_score
                };

                // Material for bottom piece
                let bottom_val = self.weights.material_value(piece_disc(&piece.bottom));
                *accumulator += bottom_val as f32;

                // Material for top piece (stacked)
                if let Some(ref top_type) = piece.top {
                    let top_val = self.weights.material_value(piece_disc(top_type));
                    *accumulator += top_val as f32;
                }

                // Centrality bonus
                let dx = if pos.x > 4 { pos.x - 4 } else { 4 - pos.x };
                let dy = if pos.y > 4 { pos.y - 4 } else { 4 - pos.y };
                let manhattan = dx + dy;
                let centrality_bonus = ((8 - manhattan) as f32) * self.weights.centrality_wt as f32;
                *accumulator += centrality_bonus;

                // Advance bonus for soldiers and ballistas near promotion
                let top_piece_type = piece.top.as_ref().unwrap_or(&piece.bottom);
                let is_advanceable = matches!(top_piece_type,
                    PieceType::Soldier | PieceType::Ballista);
                if is_advanceable {
                    let advance_rank = match piece.color {
                        Color::White => if pos.y > 0 { 8 - pos.y } else { 8 },
                        Color::Black => pos.y,
                    };
                    let advance_bonus = (advance_rank as f32) * self.weights.advance_wt as f32;
                    *accumulator += advance_bonus;
                }
            }
        }

        let diff = white_score - black_score;
        let exponent = -diff / SCORE_SIGMOID_SCALE;
        1.0 / (1.0 + exponent.exp())
    }
}

impl Evaluator for CpuEvaluator {
    fn score_positions(&self, boards: &[Board]) -> Vec<f32> {
        boards.iter().map(|b| self.evaluate_single(b)).collect()
    }
}

/// Evaluate the board in centipawns from white's perspective.
/// Positive = white advantage, negative = black advantage.
/// Used by the alpha-beta search engine for leaf-node evaluation.
pub fn eval_centipawns(board: &Board, weights: &ScoringWeights) -> i32 {
    if board.is_game_over() {
        if board.is_draw() { return 0; }
        return if board.white_wins() { 50_000 } else { -50_000 };
    }

    let mut white_score: i32 = 0;
    let mut black_score: i32 = 0;

    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            let accumulator = if piece.color == Color::White {
                &mut white_score
            } else {
                &mut black_score
            };

            *accumulator += weights.material_value(piece_disc(&piece.bottom)) as i32;
            if let Some(ref top_type) = piece.top {
                *accumulator += weights.material_value(piece_disc(top_type)) as i32;
            }

            let dx = if pos.x > 4 { pos.x - 4 } else { 4 - pos.x };
            let dy = if pos.y > 4 { pos.y - 4 } else { 4 - pos.y };
            let manhattan = dx + dy;
            *accumulator += ((8 - manhattan) as i32) * weights.centrality_wt as i32;

            let top_piece_type = piece.top.as_ref().unwrap_or(&piece.bottom);
            let is_advanceable = matches!(top_piece_type,
                PieceType::Soldier | PieceType::Ballista);
            if is_advanceable {
                let advance_rank = match piece.color {
                    Color::White => if pos.y > 0 { 8 - pos.y } else { 8 },
                    Color::Black => pos.y,
                };
                *accumulator += (advance_rank as i32) * weights.advance_wt as i32;
            }
        }
    }

    white_score - black_score
}

/// Compute a cheap material + centrality score for `board` using `weights`,
/// without quiescence search or caching.  Returns a value in [0.0, 1.0]
/// (1.0 = white winning, 0.5 = equal, 0.0 = black winning).
pub fn static_eval_board(board: &Board, weights: &ScoringWeights) -> f32 {
    if board.is_game_over() {
        if board.is_draw() { return 0.5; }
        return if board.white_wins() { 1.0 } else { 0.0 };
    }

    let mut white_score: f32 = 0.0;
    let mut black_score: f32 = 0.0;

    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            let accumulator = if piece.color == Color::White {
                &mut white_score
            } else {
                &mut black_score
            };

            *accumulator += weights.material_value(piece_disc(&piece.bottom)) as f32;
            if let Some(ref top_type) = piece.top {
                *accumulator += weights.material_value(piece_disc(top_type)) as f32;
            }

            let dx = if pos.x > 4 { pos.x - 4 } else { 4 - pos.x };
            let dy = if pos.y > 4 { pos.y - 4 } else { 4 - pos.y };
            let manhattan = dx + dy;
            *accumulator += ((8 - manhattan) as f32) * weights.centrality_wt as f32;

            let top_piece_type = piece.top.as_ref().unwrap_or(&piece.bottom);
            let is_advanceable = matches!(top_piece_type,
                PieceType::Soldier | PieceType::Ballista);
            if is_advanceable {
                let advance_rank = match piece.color {
                    Color::White => if pos.y > 0 { 8 - pos.y } else { 8 },
                    Color::Black => pos.y,
                };
                *accumulator += (advance_rank as f32) * weights.advance_wt as f32;
            }
        }
    }

    let diff = white_score - black_score;
    let exponent = -diff / SCORE_SIGMOID_SCALE;
    1.0 / (1.0 + exponent.exp())
}

// ══════════  Tests  ══════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    fn cpu_eval() -> CpuEvaluator {
        CpuEvaluator::new(ScoringWeights::default())
    }

    #[test]
    fn starting_position_near_half() {
        let ev = cpu_eval();
        let sc = ev.score_positions(&[Board::new()]);
        assert!((sc[0] - 0.5).abs() < 0.05,
            "symmetric start should score ~0.5, got {}", sc[0]);
    }

    #[test]
    fn terminal_win_scores_one() {
        let mut b = Board::new();
        b.set_game_over(true, true, false);
        let sc = cpu_eval().score_positions(&[b]);
        assert!((sc[0] - 1.0).abs() < 1e-5, "white wins should be 1.0, got {}", sc[0]);
    }

    #[test]
    fn terminal_loss_scores_zero() {
        let mut b = Board::new();
        b.set_game_over(true, false, false);
        let sc = cpu_eval().score_positions(&[b]);
        assert!(sc[0].abs() < 1e-5, "black wins should be 0.0, got {}", sc[0]);
    }

    #[test]
    fn evaluation_is_symmetric_regardless_of_turn() {
        let b_white = Board::new();
        let mut b_black = Board::new();
        b_black.set_white_to_move(false);
        let ev = cpu_eval();
        let sw = ev.score_positions(&[b_white])[0];
        let sb = ev.score_positions(&[b_black])[0];
        assert!((sw - sb).abs() < 1e-5,
            "evaluation should be same regardless of turn: white={sw}, black={sb}");
    }

    #[test]
    fn terminal_draw_scores_half() {
        let mut b = Board::new();
        b.set_game_over(true, false, true);
        let sc = cpu_eval().score_positions(&[b]);
        assert!((sc[0] - 0.5).abs() < 1e-5, "draw should be 0.5, got {}", sc[0]);
    }

    #[test]
    fn batch_returns_correct_count() {
        let batch = vec![Board::new(); 7];
        let sc = cpu_eval().score_positions(&batch);
        assert_eq!(sc.len(), 7);
    }
}
