//! Alpha-beta pruning helper functions.

/// Returns `true` if a beta cutoff should be triggered (fail-high).
#[inline]
pub fn should_cutoff(alpha: i32, beta: i32) -> bool {
    alpha >= beta
}

/// Update alpha and return the new value.
#[inline]
pub fn update_alpha(alpha: i32, score: i32) -> i32 {
    alpha.max(score)
}

/// Update beta and return the new value.
#[inline]
pub fn update_beta(beta: i32, score: i32) -> i32 {
    beta.min(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutoff_when_alpha_equals_beta() {
        assert!(should_cutoff(5, 5));
    }

    #[test]
    fn cutoff_when_alpha_exceeds_beta() {
        assert!(should_cutoff(10, 5));
    }

    #[test]
    fn no_cutoff_when_alpha_less_than_beta() {
        assert!(!should_cutoff(3, 10));
    }

    #[test]
    fn update_alpha_takes_max() {
        assert_eq!(update_alpha(3, 7), 7);
        assert_eq!(update_alpha(7, 3), 7);
    }

    #[test]
    fn update_beta_takes_min() {
        assert_eq!(update_beta(10, 7), 7);
        assert_eq!(update_beta(7, 10), 7);
    }
}
