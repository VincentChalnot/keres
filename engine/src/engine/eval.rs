//! Static evaluation for Keres.
//!
//! Evaluates board positions using material, mobility, hanging pieces,
//! king safety, and center control.
//! Only runs at leaf nodes (depth=0) in Stage 1 search.

use std::collections::HashSet;
use crate::board::{Board, Color, PieceType, Position, BOARD_SIZE};
use crate::game::Game;

/// Heuristic weights.
pub const MOBILITY_WEIGHT: i32 = 2;
pub const HANGING_WEIGHT: i32 = 3;
pub const KING_SAFETY_WEIGHT: i32 = 4;
pub const CENTER_WEIGHT: i32 = 1;

/// Piece values for static evaluation (in centipawns).
pub const SOLDIER_VALUE: i32 = 10;
pub const GUARD_VALUE: i32 = 25;
pub const PALADIN_VALUE: i32 = 30;
pub const BISHOP_VALUE: i32 = 40;
pub const KNIGHT_VALUE: i32 = 40;
pub const BALLISTA_VALUE: i32 = 45;
pub const ROOK_VALUE: i32 = 60;
pub const KING_VALUE: i32 = 1000;

/// Material value for a piece type.
pub fn piece_value(pt: PieceType) -> i32 {
    match pt {
        PieceType::Soldier  => SOLDIER_VALUE,
        PieceType::Guard    => GUARD_VALUE,
        PieceType::Paladin  => PALADIN_VALUE,
        PieceType::Bishop   => BISHOP_VALUE,
        PieceType::Knight   => KNIGHT_VALUE,
        PieceType::Ballista => BALLISTA_VALUE,
        PieceType::Rook     => ROOK_VALUE,
        PieceType::King     => KING_VALUE,
    }
}

/// Mate score constant (used for terminal positions).
pub const MATE_SCORE: i32 = 100_000;

/// Total material value of a piece (bottom + optional top).
fn stack_value(piece: &crate::board::Piece) -> i32 {
    let mut val = piece_value(piece.bottom);
    if let Some(top) = piece.top {
        val += piece_value(top);
    }
    val
}

/// Returns true if the position is in the 3×3 center zone
/// (columns D–F = x 3–5, rows 4–6 in 1-indexed = y 3–5 in 0-indexed).
fn is_center(pos: &Position) -> bool {
    pos.x >= 3 && pos.x <= 5 && pos.y >= 3 && pos.y <= 5
}

/// Count the number of concrete moves represented by a slice of `PotentialMove`s.
/// Each `PotentialMove` with `unstackable=true` expands to two moves; all others to one.
fn count_moves(candidates: &[crate::game::PotentialMove]) -> usize {
    candidates.iter().map(|pm| {
        if pm.force_unstack { 1 } else if pm.unstackable { 2 } else { 1 }
    }).sum()
}

/// Squares attacked by each side and move counts for both sides.
struct AttackMap {
    white_attacks: HashSet<Position>,
    black_attacks: HashSet<Position>,
    white_move_count: usize,
    black_move_count: usize,
}

/// Compute the attack map by generating pseudo-legal moves for each side.
/// Reuses the existing move generator — the board's turn is temporarily flipped
/// to obtain the opponent's moves.
fn compute_attack_map(board: &Board) -> AttackMap {
    // Moves for the current side to move.
    let game = Game::from_board(*board);
    let cur_candidates = game.get_all_moves();
    let cur_move_count = count_moves(&cur_candidates);
    let cur_attacks: HashSet<Position> = cur_candidates.iter().map(|pm| pm.to).collect();

    // Moves for the opponent (flip the turn temporarily).
    let mut opp_board = *board;
    opp_board.set_white_to_move(!board.is_white_to_move());
    let opp_game = Game::from_board(opp_board);
    let opp_candidates = opp_game.get_all_moves();
    let opp_move_count = count_moves(&opp_candidates);
    let opp_attacks: HashSet<Position> = opp_candidates.iter().map(|pm| pm.to).collect();

    if board.is_white_to_move() {
        AttackMap {
            white_attacks: cur_attacks,
            white_move_count: cur_move_count,
            black_attacks: opp_attacks,
            black_move_count: opp_move_count,
        }
    } else {
        AttackMap {
            white_attacks: opp_attacks,
            white_move_count: opp_move_count,
            black_attacks: cur_attacks,
            black_move_count: cur_move_count,
        }
    }
}

/// Evaluate the board from the side-to-move's perspective.
/// Positive = advantage for the side to move.
///
/// Terminal positions return mate/draw scores.
/// Non-terminal positions combine material, mobility, hanging pieces,
/// king safety, and center control.
pub fn evaluate(board: &Board) -> i32 {
    if board.is_game_over() {
        if board.is_draw() {
            return 0;
        }
        // The side that just moved captured the king, so current
        // side-to-move is the *loser*.
        return -MATE_SCORE;
    }

    let my_color = board.color_to_move();

    // ── 1. Material ──────────────────────────────────────────────────────────
    let mut white_material: i32 = 0;
    let mut black_material: i32 = 0;

    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            let acc = if piece.color == Color::White {
                &mut white_material
            } else {
                &mut black_material
            };
            *acc += piece_value(piece.bottom);
            if let Some(top) = piece.top {
                *acc += piece_value(top);
            }
        }
    }

    let material_diff = white_material - black_material;
    let stm_material = if board.is_white_to_move() { material_diff } else { -material_diff };

    // ── Attack map (shared foundation) ───────────────────────────────────────
    let attack_map = compute_attack_map(board);
    let my_attacks = if board.is_white_to_move() { &attack_map.white_attacks } else { &attack_map.black_attacks };
    let opp_attacks = if board.is_white_to_move() { &attack_map.black_attacks } else { &attack_map.white_attacks };
    let my_move_count = if board.is_white_to_move() { attack_map.white_move_count } else { attack_map.black_move_count };
    let opp_move_count = if board.is_white_to_move() { attack_map.black_move_count } else { attack_map.white_move_count };

    // ── 2. Mobility ───────────────────────────────────────────────────────────
    let mobility = (my_move_count as i32 - opp_move_count as i32) * MOBILITY_WEIGHT;

    // ── 3. Hanging pieces ─────────────────────────────────────────────────────
    let mut hanging: i32 = 0;
    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            if piece.color == my_color {
                // Penalty: my piece attacked by opponent and not defended by me.
                if opp_attacks.contains(&pos) && !my_attacks.contains(&pos) {
                    hanging -= stack_value(piece) * HANGING_WEIGHT;
                }
            } else {
                // Bonus: opponent's piece attacked by me and not defended by them.
                if my_attacks.contains(&pos) && !opp_attacks.contains(&pos) {
                    hanging += stack_value(piece) * HANGING_WEIGHT;
                }
            }
        }
    }

    // ── 4. King safety ────────────────────────────────────────────────────────
    let mut my_king_pos: Option<Position> = None;
    let mut opp_king_pos: Option<Position> = None;
    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if let Some(piece) = board.get_piece(&pos) {
            if piece.is_king() {
                if piece.color == my_color {
                    my_king_pos = Some(pos);
                } else {
                    opp_king_pos = Some(pos);
                }
            }
        }
    }

    let my_king_threatened = my_king_pos.map_or(0i32, |kp| {
        Position::ALL_MOVES.iter().filter(|(dx, dy)| {
            kp.get_new(*dx, *dy).map_or(false, |adj| opp_attacks.contains(&adj))
        }).count() as i32
    });
    let opp_king_threatened = opp_king_pos.map_or(0i32, |kp| {
        Position::ALL_MOVES.iter().filter(|(dx, dy)| {
            kp.get_new(*dx, *dy).map_or(false, |adj| my_attacks.contains(&adj))
        }).count() as i32
    });
    let king_safety = (-my_king_threatened + opp_king_threatened) * KING_SAFETY_WEIGHT;

    // ── 5. Center control (Balistas excluded) ─────────────────────────────────
    let mut my_center: i32 = 0;
    let mut opp_center: i32 = 0;
    for sq in 0..BOARD_SIZE {
        let pos = Position::from_u8(sq as u8);
        if is_center(&pos) {
            if let Some(piece) = board.get_piece(&pos) {
                let count = |p: &crate::board::Piece| -> i32 {
                    let mut n = 0;
                    if p.bottom != PieceType::Ballista { n += 1; }
                    if let Some(top) = p.top { if top != PieceType::Ballista { n += 1; } }
                    n
                };
                if piece.color == my_color {
                    my_center += count(piece);
                } else {
                    opp_center += count(piece);
                }
            }
        }
    }
    let center = (my_center - opp_center) * CENTER_WEIGHT;

    stm_material + mobility + hanging + king_safety + center
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};

    fn empty_board(white_to_move: bool) -> Board {
        let mut binary = [0u8; crate::board::BOARD_SIZE + 2];
        if white_to_move {
            binary[crate::board::BOARD_SIZE] = 0b10000000;
        }
        let mut b = Board::from_binary(binary).unwrap();
        // Add kings so move generation works properly.
        b.set_piece(&Position::new(4, 0), Some(Piece::new(Color::Black, PieceType::King, None)));
        b.set_piece(&Position::new(4, 8), Some(Piece::new(Color::White, PieceType::King, None)));
        b
    }

    #[test]
    fn starting_position_is_zero() {
        let b = Board::new();
        let score = evaluate(&b);
        assert_eq!(score, 0, "symmetric start should score 0");
    }

    #[test]
    fn terminal_draw_is_zero() {
        let mut b = Board::new();
        b.set_game_over(true, false, true);
        assert_eq!(evaluate(&b), 0);
    }

    #[test]
    fn terminal_loss_is_negative_mate() {
        let mut b = Board::new();
        b.set_game_over(true, true, false); // white wins, but it's white to move => current side is "loser" semantically
        // With the king-capture convention: after king capture the turn has flipped,
        // so the side to move is always the loser.
        assert_eq!(evaluate(&b), -MATE_SCORE);
    }

    #[test]
    fn piece_values_are_correct() {
        assert_eq!(piece_value(PieceType::Soldier), 10);
        assert_eq!(piece_value(PieceType::Guard), 25);
        assert_eq!(piece_value(PieceType::Paladin), 30);
        assert_eq!(piece_value(PieceType::Bishop), 40);
        assert_eq!(piece_value(PieceType::Knight), 40);
        assert_eq!(piece_value(PieceType::Ballista), 45);
        assert_eq!(piece_value(PieceType::Rook), 60);
        assert_eq!(piece_value(PieceType::King), 1000);
    }

    #[test]
    fn weights_are_correct() {
        assert_eq!(MOBILITY_WEIGHT, 2);
        assert_eq!(HANGING_WEIGHT, 3);
        assert_eq!(KING_SAFETY_WEIGHT, 4);
        assert_eq!(CENTER_WEIGHT, 1);
    }

    #[test]
    fn material_advantage_reflected_in_score() {
        // White has an extra soldier — score should be positive for white.
        let mut b = empty_board(true);
        b.set_piece(&Position::new(4, 4), Some(Piece::new(Color::White, PieceType::Soldier, None)));
        let score = evaluate(&b);
        assert!(score > 0, "extra white soldier should give positive score for white to move");
    }

    #[test]
    fn material_advantage_negative_for_opponent() {
        // Same position but it's black to move.
        let mut b = empty_board(false);
        b.set_piece(&Position::new(4, 4), Some(Piece::new(Color::White, PieceType::Soldier, None)));
        let score = evaluate(&b);
        assert!(score < 0, "extra white soldier should give negative score for black to move");
    }

    #[test]
    fn is_center_identifies_3x3_zone() {
        // Corners of the center zone.
        assert!(is_center(&Position::new(3, 3)));
        assert!(is_center(&Position::new(5, 5)));
        assert!(is_center(&Position::new(4, 4)));
        // Outside center.
        assert!(!is_center(&Position::new(2, 4)));
        assert!(!is_center(&Position::new(6, 4)));
        assert!(!is_center(&Position::new(4, 2)));
        assert!(!is_center(&Position::new(4, 6)));
    }

    #[test]
    fn compute_attack_map_starting_position_is_symmetric() {
        let b = Board::new();
        let am = compute_attack_map(&b);
        // Starting position is symmetric so move counts should be equal.
        assert_eq!(am.white_move_count, am.black_move_count,
            "move counts should be equal at the starting position");
    }

    #[test]
    fn hanging_penalty_for_undefended_piece() {
        // Place a lone white soldier in the centre with a black rook nearby
        // so the soldier is attacked but not defended.
        // We verify that the hanging penalty is applied (score is lower than
        // a position without the threat).
        let b_nothreat = empty_board(true);
        // Now add a white soldier at (4,4) and a black rook at (4,1) (attacks along column).
        // White king stays at (4,8), black king stays at (4,0).
        let mut b_threat = empty_board(true);
        b_threat.set_piece(&Position::new(4, 4), Some(Piece::new(Color::White, PieceType::Soldier, None)));
        b_threat.set_piece(&Position::new(4, 1), Some(Piece::new(Color::Black, PieceType::Rook, None)));

        let score_safe = evaluate(&b_nothreat);
        let score_threat = evaluate(&b_threat);
        // With threat: white material advantage (+10) but hanging penalty (-30)
        // and black has a rook bonus from material. Score should be lower under threat.
        assert!(score_threat < score_safe,
            "threatened undefended piece should lower the score");
    }
}
