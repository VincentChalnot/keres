//! Tempo bonus: small advantage for the side to move.

use crate::engine::constants::TEMPO_BONUS;

/// Return the tempo contribution to the absolute evaluation score.
/// Positive when White is to move, negative when Black is to move.
pub fn tempo_score(white_to_move: bool) -> i32 {
    if white_to_move {
        TEMPO_BONUS
    } else {
        -TEMPO_BONUS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempo_is_positive_for_white() {
        assert_eq!(tempo_score(true), TEMPO_BONUS);
    }

    #[test]
    fn tempo_is_negative_for_black() {
        assert_eq!(tempo_score(false), -TEMPO_BONUS);
    }

    #[test]
    fn tempo_values_are_symmetric() {
        assert_eq!(tempo_score(true), -tempo_score(false));
    }
}
