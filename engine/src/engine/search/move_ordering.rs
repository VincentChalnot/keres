//! Move ordering: sort moves for efficient alpha-beta pruning.
//!
//! Priority:
//! 1. Hash move from the transposition table.
//! 2. Winning captures ordered by MVV-LVA (using weighted values).
//! 3. Killer moves.
//! 4. Remaining quiet moves.

use crate::engine::eval::material::weighted_value;
use crate::engine::search::killer::KillerTable;
use crate::engine::tt::TranspositionTable;
use crate::game::Game;
use crate::moves::Move;

/// Score assigned to the TT hash move (must be higher than any capture).
const HASH_MOVE_SCORE: i32 = i32::MAX;
/// Score assigned to killer moves (between captures and quiet moves).
const KILLER_SCORE: i32 = 10_000;
/// Base score for quiet moves.
const QUIET_BASE: i32 = 0;

/// Assign a sorting score to each move (higher = searched first).
fn score_move(
    mv: Move,
    game: &Game,
    depth: usize,
    tt_hash_move: Option<Move>,
    killers: &KillerTable,
) -> i32 {
    // 1. TT hash move.
    if tt_hash_move == Some(mv) {
        return HASH_MOVE_SCORE;
    }

    // 2. Captures: MVV-LVA using weighted values.
    if let Some(victim) = game.board.get_piece(&mv.to) {
        if victim.color != game.color_to_move() {
            let attacker = game.board.get_piece(&mv.from).unwrap();
            let victim_val = weighted_value(victim, mv.to, game);
            let attacker_val = weighted_value(attacker, mv.from, game);
            return 50_000 + victim_val - attacker_val;
        }
    }

    // 3. Killer moves.
    if killers.is_killer(depth, mv) {
        return KILLER_SCORE;
    }

    // 4. Quiet move.
    QUIET_BASE
}

/// Sort `moves` in-place, best first, using the engine's move ordering heuristics.
pub fn order_moves(
    moves: &mut [Move],
    game: &Game,
    depth: usize,
    tt: Option<&TranspositionTable>,
    hash: u64,
    killers: &KillerTable,
) {
    let tt_hash_move = tt.and_then(|t| t.get(hash)).and_then(|e| e.best_move);
    moves.sort_unstable_by_key(|&mv| {
        std::cmp::Reverse(score_move(mv, game, depth, tt_hash_move, killers))
    });
}

/// Extract only capture moves from `moves`, sorted by MVV-LVA.
/// Also includes promotion moves (soldier reaching the back rank).
pub fn order_captures(moves: &mut Vec<Move>, game: &Game) {    moves.retain(|mv| is_capture_or_promotion(mv, game));
    moves.sort_unstable_by_key(|mv| {
        let victim_val = game
            .board
            .get_piece(&mv.to)
            .filter(|p| p.color != game.color_to_move())
            .map(|p| weighted_value(p, mv.to, game))
            .unwrap_or(0);
        let attacker_val = game
            .board
            .get_piece(&mv.from)
            .map(|p| weighted_value(p, mv.from, game))
            .unwrap_or(0);
        std::cmp::Reverse(victim_val - attacker_val)
    });
}

/// Return true if `mv` is a capture or a promotion.
pub fn is_capture_or_promotion(mv: &Move, game: &Game) -> bool {
    // Capture: enemy piece at destination.
    if let Some(dest) = game.board.get_piece(&mv.to) {
        if dest.color != game.color_to_move() {
            return true;
        }
    }
    // Promotion: moving piece is a soldier and destination is the promotion rank.
    if let Some(piece) = game.board.get_piece(&mv.from) {
        let is_soldier = piece.bottom == crate::board::PieceType::Soldier
            || piece.top == Some(crate::board::PieceType::Soldier);
        if is_soldier {
            let promo_rank = match piece.color {
                crate::board::Color::White => 0,
                crate::board::Color::Black => 8,
            };
            if mv.to.y == promo_rank {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{Board, Color, Piece, PieceType, Position};
    use crate::game::Game;

    fn empty_game_white_to_move() -> Game {
        let mut board = Board::empty();
        board.set_piece(&Position::new(4, 8), Some(Piece::new(Color::White, PieceType::King, None)));
        board.set_piece(&Position::new(4, 0), Some(Piece::new(Color::Black, PieceType::King, None)));
        Game::from_board(board)
    }

    #[test]
    fn captures_are_identified_correctly() {
        let mut game = empty_game_white_to_move();
        game.board.set_piece(&Position::new(3, 5), Some(Piece::new(Color::White, PieceType::Rook, None)));
        game.board.set_piece(&Position::new(3, 3), Some(Piece::new(Color::Black, PieceType::Soldier, None)));

        let mv_capture = Move {
            from: Position::new(3, 5),
            to: Position::new(3, 3),
            unstack: false,
        };
        let mv_quiet = Move {
            from: Position::new(3, 5),
            to: Position::new(3, 4),
            unstack: false,
        };

        assert!(is_capture_or_promotion(&mv_capture, &game));
        assert!(!is_capture_or_promotion(&mv_quiet, &game));
    }
}
