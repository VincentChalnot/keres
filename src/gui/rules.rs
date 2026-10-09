//! Static rule text for the fullscreen rules reference (`show_rules`) and
//! the first-launch mini help modal (`show_help`) — see `App`. Kept as data
//! here rather than inline in `main.rs` so the two modals can share it.
//!
//! Sourced from playkeres.com/rules and cross-checked against the engine's
//! own rules (`crate::game_over::check_promotion`/`check_game_over`,
//! `crate::moves::MoveGenerator`) so the two clients never disagree.

use crate::icons::PieceIcon;

pub struct PieceRule {
    pub icon: PieceIcon,
    pub letter: char,
    pub name: &'static str,
    pub movement: &'static str,
    pub promotion: Option<&'static str>,
}

/// Ordered so each promoting piece sits next to what it promotes into.
pub const PIECES: [PieceRule; 8] = [
    PieceRule {
        icon: PieceIcon::King,
        letter: 'K',
        name: "KING",
        movement: "1 SQUARE, ANY DIRECTION. CANNOT STACK.",
        promotion: None,
    },
    PieceRule {
        icon: PieceIcon::Soldier,
        letter: 'S',
        name: "SOLDIER",
        movement: "1 SQUARE, DIAGONALLY FORWARD.",
        promotion: Some("PROMOTES TO PALADIN AT THE FAR RANK"),
    },
    PieceRule {
        icon: PieceIcon::Paladin,
        letter: 'P',
        name: "PALADIN",
        movement: "1-2 SQUARES, ORTHOGONALLY.",
        promotion: None,
    },
    PieceRule {
        icon: PieceIcon::Ballista,
        letter: 'L',
        name: "BALLISTA",
        movement: "UNLIMITED SQUARES, FORWARD ONLY.",
        promotion: Some("PROMOTES TO ROOK AT THE FAR RANK"),
    },
    PieceRule {
        icon: PieceIcon::Rook,
        letter: 'R',
        name: "ROOK",
        movement: "UNLIMITED SQUARES, ORTHOGONALLY.",
        promotion: None,
    },
    PieceRule {
        icon: PieceIcon::Bishop,
        letter: 'B',
        name: "BISHOP",
        movement: "UNLIMITED SQUARES, DIAGONALLY.",
        promotion: None,
    },
    PieceRule {
        icon: PieceIcon::Guard,
        letter: 'G',
        name: "GUARD",
        movement: "1-2 SQUARES, DIAGONALLY.",
        promotion: None,
    },
    PieceRule {
        icon: PieceIcon::Knight,
        letter: 'N',
        name: "KNIGHT",
        movement: "L-SHAPED JUMP, OVER OTHER PIECES.",
        promotion: None,
    },
];

/// General, non-piece-specific rules for the top of the fullscreen modal.
pub const GENERAL: [&str; 5] = [
    "9X9 BOARD - CAPTURE THE ENEMY KING TO WIN",
    "STACK ONTO YOUR OWN PIECE TO COMBINE ITS MOVES WITH YOURS",
    "MOVE THE WHOLE STACK, OR JUST THE TOP PIECE, WHEN LEGAL",
    "CAPTURING A STACK REMOVES IT ENTIRELY",
    "DRAW AFTER 40 MOVES WITHOUT A CAPTURE, OR INSUFFICIENT MATERIAL",
];

/// Look up a piece's rule entry by its one-letter notation (see
/// `render::letter_for`) — every `PieceType` has exactly one entry here, so
/// this never returns `None` for a real piece.
pub fn find(letter: char) -> &'static PieceRule {
    PIECES
        .iter()
        .find(|p| p.letter == letter)
        .expect("every PieceType has a rules entry")
}

/// The subset of rules that doesn't overlap with chess, for a player who
/// already knows chess and just wants to start playing (see the first-launch
/// `show_help` modal).
pub const QUICK_TIPS: [&str; 3] = [
    "EVERY PIECE MOVES AND CAPTURES THE SAME WAY",
    "SOLDIERS PROMOTE TO PALADINS, BALLISTAS TO ROOKS",
    "CAPTURE THE ENEMY KING TO WIN",
];
