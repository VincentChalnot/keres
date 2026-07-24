//! Evaluation module: position evaluation from White's absolute perspective.

pub mod king_safety;
pub mod material;
pub mod mobility;
pub mod pins;
pub mod promotion;
pub mod pst;
pub mod tempo;

use crate::board::Color;
use crate::engine::eval::king_safety::king_mobility_term;
use crate::engine::eval::material::weighted_value;
use crate::engine::eval::pins::pinned_malus;
use crate::engine::eval::tempo::tempo_score;
use crate::engine::types::{BoardEval, SquareEval};
use crate::game::Game;

/// Evaluate the position from White's absolute perspective.
///
/// Returns a positive score when White dominates, negative when Black dominates.
/// This is NOT NegaMax-relative; the single conversion happens in quiescence search.
pub fn evaluate_absolute(game: &Game) -> i32 {
    let mut score = 0i32;

    for (pos, piece) in game.board.pieces() {
        let wv = weighted_value(piece, pos, game);
        if piece.color == Color::White {
            score += wv;
        } else {
            score -= wv;
        }
    }

    // Pin malus: pinned-White pieces hurt White, pinned-Black pieces hurt Black.
    score += pinned_malus(&game.board, Color::Black);
    score -= pinned_malus(&game.board, Color::White);

    // King mobility term.
    score += king_mobility_term(game);

    // Tempo bonus.
    score += tempo_score(game.is_white_to_move());

    score
}

/// Verbose evaluation for debug / CLI tooling.  Not called from the search tree.
pub fn evaluate_verbose(game: &Game) -> BoardEval {
    use std::collections::HashMap;

    let mut per_square: HashMap<(usize, usize), SquareEval> = HashMap::new();
    let mut white_total = 0i32;
    let mut black_total = 0i32;

    for (pos, piece) in game.board.pieces() {
        use crate::engine::eval::material::stack_base_value;
        use crate::engine::eval::mobility::mobility_bonus;
        use crate::engine::eval::promotion::promotion_bonus;
        use crate::engine::eval::pst::pst_bonus;

        let bv = stack_base_value(piece);
        let pst = pst_bonus(piece, pos);
        let mob = mobility_bonus(piece, pos, game);
        let promo = promotion_bonus(piece, pos);
        let total = bv + pst + mob + promo;

        if piece.color == Color::White {
            white_total += total;
        } else {
            black_total += total;
        }

        per_square.insert(
            (pos.x, pos.y),
            SquareEval {
                piece_type: format!("{:?}", piece.bottom),
                color: format!("{:?}", piece.color),
                base_value: bv,
                pst_bonus: pst,
                mobility_bonus: mob,
                promotion_bonus: promo,
                total,
            },
        );
    }

    let pinned_malus_white = pinned_malus(&game.board, Color::White);
    let pinned_malus_black = pinned_malus(&game.board, Color::Black);
    let king_mobility_white = king_safety::king_mobility_for(game, Color::White);
    let king_mobility_black = king_safety::king_mobility_for(game, Color::Black);
    let tempo = tempo_score(game.is_white_to_move());

    let final_score = white_total - black_total + pinned_malus_black - pinned_malus_white
        + (king_mobility_white - king_mobility_black)
            * crate::engine::constants::KING_MOBILITY_WEIGHT
        + tempo;

    BoardEval {
        per_square,
        white_total,
        black_total,
        pinned_malus_white,
        pinned_malus_black,
        king_mobility_white,
        king_mobility_black,
        tempo,
        final_score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn evaluate_initial_position_is_near_zero() {
        // The starting position is symmetric, so the score should be close to
        // zero (exact value depends on PST and tempo bonus).
        let game = Game::new();
        let score = evaluate_absolute(&game);
        // Allow up to TEMPO_BONUS difference for the side to move.
        assert!(
            score.abs() <= 50,
            "Initial position score should be near 0, got {}",
            score
        );
    }

    #[test]
    fn evaluate_verbose_final_matches_absolute() {
        let game = Game::new();
        let abs = evaluate_absolute(&game);
        let verbose = evaluate_verbose(&game);
        assert_eq!(abs, verbose.final_score);
    }
}
