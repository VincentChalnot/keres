//! Tunable constants for the AI search engine.

/// Maximum search depth (ply from root).
pub const MAX_DEPTH: usize = 4;

/// Weight applied to mobility count (reachable empty squares × weight).
pub const MOBILITY_WEIGHT: i32 = 2;

/// Weight applied to king mobility in the king-safety term.
pub const KING_MOBILITY_WEIGHT: i32 = 3;

/// Fraction of a piece's base value applied as a malus when it is pinned.
pub const PINNED_PENALTY_FACTOR: f32 = 0.20;

/// Small bonus for the side to move.
pub const TEMPO_BONUS: i32 = 15;

/// Delta-pruning margin in quiescence search.
pub const DELTA_MARGIN: i32 = 50;

// ── Piece base values ────────────────────────────────────────────────────────

pub const SOLDIER_VALUE: i32 = 10;
pub const GUARD_VALUE: i32 = 25;
pub const PALADIN_VALUE: i32 = 30;
pub const BISHOP_VALUE: i32 = 40;
pub const KNIGHT_VALUE: i32 = 40;
pub const BALLISTA_VALUE: i32 = 45;
pub const ROOK_VALUE: i32 = 60;
pub const KING_VALUE: i32 = 1000;

/// Transposition table size (number of slots, must be a power of two).
pub const TT_SIZE: usize = 1 << 20; // ~1 million entries

/// Number of killer-move slots per depth level.
pub const KILLER_SLOTS: usize = 2;

/// Maximum depth used when dimensioning the killer table.
pub const MAX_KILLER_DEPTH: usize = MAX_DEPTH + 8;
