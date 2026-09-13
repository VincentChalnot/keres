//! Tunable constants for the AI search engine.

/// Maximum search depth (ply from root).
pub const MAX_DEPTH: usize = 4;

/// Lowest engine strength level exposed to callers (e.g. the GUI difficulty
/// slider). See `SearchConfig::for_level`.
pub const MIN_LEVEL: u8 = 1;

/// Highest engine strength level exposed to callers.
pub const MAX_LEVEL: u8 = 10;

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

/// Hard ply cap on the quiescence extension, counted from the root (so it
/// also covers the `MAX_DEPTH` plies of the main search above it).
///
/// Quiescence only extends on captures and promotions, which are
/// self-limiting, so this never binds in real play; it exists so that no
/// caller-supplied position can drive the recursion deep enough to overflow
/// the stack — which would abort the whole server process, not just the one
/// request being served.
pub const MAX_QUIESCENCE_PLY: usize = 64;

/// Relative selection weight applied to king moves when picking an outright
/// blunder (`SearchConfig::blunder_chance`). `1.0` would treat every legal
/// move — king included — as equally likely to be the "random" blunder; this
/// pulls the king down to a fifth of that so the engine doesn't carelessly
/// walk its king into danger as readily as it hangs any other piece.
pub const KING_BLUNDER_WEIGHT: f32 = 0.2;

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
