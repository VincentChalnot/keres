//! Wire-format conformance tests.
//!
//! Every byte offset and bit position in `docs/PROTOCOL.md` is an external
//! contract: the keres-platform PHP `Model/` classes and the TypeScript
//! `boardUtils.ts` decode these bytes by hand. A silent layout change here
//! would corrupt every stored game on the platform without any Rust test
//! failing, so this file pins the layout against *independently computed*
//! expectations — hardcoded discriminants, hardcoded bit shifts and a
//! hardcoded starting position — rather than against the encoder itself.

mod common;

use common::*;
use keres_engine::api::{GAME_BYTES, MOVE_BYTES};
use keres_engine::board::{Board, Color, Piece, PieceType, Position, BOARD_SIZE};
use keres_engine::game::Game;
use keres_engine::moves::{Move, PotentialMove};

/// The `PieceType` discriminants the wire format promises, hardcoded so a
/// renumbering in `src/board.rs` fails here instead of silently shifting the
/// meaning of every stored board.
const PIECE_CODES: [(PieceType, u8); 7] = [
    (PieceType::Soldier, 1),
    (PieceType::Bishop, 2),
    (PieceType::Rook, 3),
    (PieceType::Paladin, 4),
    (PieceType::Guard, 5),
    (PieceType::Knight, 6),
    (PieceType::Ballista, 7),
];

/// Colour bit (bit 6): `1` = White, `0` = Black.
const COLORS: [(Color, u8); 2] = [(Color::White, 0b0100_0000), (Color::Black, 0b0000_0000)];

/// The king's dedicated payload: `C 111 000`.
const KING_PAYLOAD: u8 = 0b011_1000;

// ── Position ────────────────────────────────────────────────────────────────

#[test]
fn position_to_u8_is_y_times_nine_plus_x_for_every_square() {
    for y in 0..9usize {
        for x in 0..9usize {
            let expected = (y * 9 + x) as u8;
            assert_eq!(
                Position::new(x, y).to_u8(),
                expected,
                "({x},{y}) must encode as y * 9 + x"
            );
        }
    }
}

#[test]
fn position_try_from_u8_accepts_exactly_zero_through_eighty() {
    for value in 0..=80u8 {
        let position = Position::try_from_u8(value)
            .unwrap_or_else(|| panic!("{value} is a valid square index"));
        assert_eq!(position.x, value as usize % 9, "x is index % 9");
        assert_eq!(position.y, value as usize / 9, "y is index / 9");
        assert_eq!(position.to_u8(), value, "decode/encode must round trip");
    }
    for value in 81..=255u8 {
        assert!(
            Position::try_from_u8(value).is_none(),
            "{value} is off the 9x9 board and must be rejected"
        );
    }
}

#[tokio::test]
async fn new_board_square_bytes_live_at_y_times_nine_plus_x() {
    let body = new_game_bytes().await;

    // The two kings are the unambiguous landmarks of the starting position:
    // black on E9 = (4, 0), white on E1 = (4, 8).
    let black_king = Position::new(4, 0);
    let white_king = Position::new(4, 8);
    assert_eq!(black_king.to_u8(), 4, "(4,0) is index 4");
    assert_eq!(white_king.to_u8(), 76, "(4,8) is index 8 * 9 + 4");

    assert_eq!(
        body[black_king.to_u8() as usize],
        KING_PAYLOAD,
        "byte 4 must be the black king payload 0b0_0_111_000"
    );
    assert_eq!(
        body[white_king.to_u8() as usize],
        0b0100_0000 | KING_PAYLOAD,
        "byte 76 must be the white king payload 0b0_1_111_000"
    );
}

// ── Square byte encoding ────────────────────────────────────────────────────

#[test]
fn square_byte_is_color_bit_top_code_shifted_three_and_bottom_code() {
    for (color, color_bit) in COLORS {
        for (bottom, bottom_code) in PIECE_CODES {
            // Unstacked: UUU == 000.
            let single = Piece::new(color, bottom, None);
            let expected = color_bit | bottom_code;
            assert_eq!(
                single.to_u8(),
                expected,
                "{color:?} {bottom:?} (unstacked) must encode as 0b{expected:08b}"
            );
            assert_eq!(
                Piece::try_from_u8(expected),
                Ok(Some(single)),
                "0b{expected:08b} must decode back to {color:?} {bottom:?}"
            );

            for (top, top_code) in PIECE_CODES {
                let stacked = Piece::new(color, bottom, Some(top));
                let expected = color_bit | (top_code << 3) | bottom_code;
                assert_eq!(
                    stacked.to_u8(),
                    expected,
                    "{color:?} {top:?} on {bottom:?} must encode as 0b{expected:08b}"
                );
                assert_eq!(
                    Piece::try_from_u8(expected),
                    Ok(Some(stacked)),
                    "0b{expected:08b} must decode back to {color:?} {top:?} on {bottom:?}"
                );
            }
        }
    }
}

#[test]
fn king_square_byte_is_the_dedicated_color_one_one_one_zero_zero_zero_payload() {
    for (color, color_bit) in COLORS {
        let king = Piece::new(color, PieceType::King, None);
        let expected = color_bit | KING_PAYLOAD;
        assert_eq!(
            king.to_u8(),
            expected,
            "the {color:?} king must encode as 0b{expected:08b}"
        );
        assert_eq!(
            Piece::try_from_u8(expected),
            Ok(Some(king)),
            "0b{expected:08b} must always decode to a king"
        );
    }
}

#[test]
fn square_byte_with_bit_seven_set_is_always_rejected() {
    for value in 128..=255u8 {
        assert!(
            Piece::try_from_u8(value).is_err(),
            "0b{value:08b} has the reserved bit 7 set and must be rejected"
        );
    }
}

#[test]
fn square_byte_with_zero_bottom_code_is_rejected_unless_it_is_the_king() {
    for (color, color_bit) in COLORS {
        for uuu in 0..=7u8 {
            let value = color_bit | (uuu << 3);
            if uuu == 0b111 {
                assert!(
                    matches!(Piece::try_from_u8(value), Ok(Some(p)) if p.is_king()),
                    "0b{value:08b} is the {color:?} king payload"
                );
                continue;
            }
            if value == 0 {
                // The all-zero byte is the empty square, not a piece.
                assert_eq!(
                    Piece::try_from_u8(value),
                    Ok(None),
                    "0x00 is an empty square"
                );
                continue;
            }
            assert!(
                Piece::try_from_u8(value).is_err(),
                "0b{value:08b} has LLL == 000 without UUU == 111 and must be rejected"
            );
        }
    }
}

// ── Starting position ───────────────────────────────────────────────────────

/// The 81 board bytes of a fresh game, written out by hand from
/// `docs/PROTOCOL.md`. Deliberately *not* derived from `Board::new()`: any
/// change to the initial setup must show up as an edit to this literal, i.e.
/// as a conscious protocol change.
///
/// ```text
///        A    B    C    D    E    F    G    H    I
///      +----+----+----+----+----+----+----+----+----+
///   9  | bl | bn | bp | bg | bK | bg | bp | bn | bl |   black back rank
///   8  |    |    | br |    |    |    | bb |    |    |   black rook / bishop
///   7  | bs | bs | bs | bs | bs | bs | bs | bs | bs |   black soldiers
///   6  |    |    |    |    |    |    |    |    |    |
///   5  |    |    |    |    |    |    |    |    |    |
///   4  |    |    |    |    |    |    |    |    |    |
///   3  | ws | ws | ws | ws | ws | ws | ws | ws | ws |   white soldiers
///   2  |    |    | wb |    |    |    | wr |    |    |   white bishop / rook
///   1  | wl | wn | wp | wg | wK | wg | wp | wn | wl |   white back rank
///      +----+----+----+----+----+----+----+----+----+
///
/// l = Ballista, n = Knight, p = Paladin, g = Guard, K = King,
/// r = Rook, b = Bishop, s = Soldier.  White is mirrored through the board
/// centre (index 80 - i), which swaps the rook and bishop columns.
/// ```
const STARTING_BOARD: [u8; BOARD_SIZE] = [
    // Row y=0 (rank 9), black back rank: Ballista Knight Paladin Guard King Guard Paladin Knight Ballista
    0x07, 0x06, 0x04, 0x05, 0x38, 0x05, 0x04, 0x06, 0x07,
    // Row y=1 (rank 8): black rook on C8 (x=2), black bishop on G8 (x=6)
    0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
    // Row y=2 (rank 7): black soldiers
    0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, // Row y=3 (rank 6): empty
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Row y=4 (rank 5): empty
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Row y=5 (rank 4): empty
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    // Row y=6 (rank 3): white soldiers
    0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41, 0x41,
    // Row y=7 (rank 2): white bishop on C2 (x=2), white rook on G2 (x=6)
    0x00, 0x00, 0x42, 0x00, 0x00, 0x00, 0x43, 0x00, 0x00,
    // Row y=8 (rank 1), white back rank
    0x47, 0x46, 0x44, 0x45, 0x78, 0x45, 0x44, 0x46, 0x47,
];

#[tokio::test]
async fn new_returns_the_hardcoded_starting_position_flags_and_counter() {
    let captured = get("/new").await;
    assert_eq!(captured.status, 200, "GET /new must succeed");
    assert_eq!(
        captured.content_type.as_deref(),
        Some("application/octet-stream"),
        "/new is raw binary"
    );
    assert_eq!(captured.body.len(), GAME_BYTES, "a game is 83 bytes");
    assert_eq!(
        &captured.body[..BOARD_SIZE],
        &STARTING_BOARD[..],
        "bytes 0..=80 must be the documented starting layout"
    );
    assert_eq!(
        captured.body[BOARD_SIZE], 0x80,
        "byte 81 of a fresh game: white to move, nothing else set"
    );
    assert_eq!(
        captured.body[BOARD_SIZE + 1],
        0,
        "byte 82 of a fresh game: no moves without capture yet"
    );
}

// ── Game flags and counter ──────────────────────────────────────────────────

/// Build a game in an arbitrary turn/result state.
fn game_in_state(white_to_move: bool, game_over: bool, white_wins: bool, draw: bool) -> Game {
    let mut game = Game::from_board(Board::new());
    game.set_white_to_move(white_to_move);
    game.set_game_over(game_over, white_wins, draw);
    game
}

#[test]
fn game_flag_byte_packs_each_state_into_its_documented_bit() {
    // (white_to_move, game_over, white_wins, draw) -> the exact flags byte.
    let cases: [(bool, bool, bool, bool, u8); 6] = [
        (false, false, false, false, 0x00),
        (true, false, false, false, 0x80),
        (false, true, false, false, 0x40),
        (false, true, true, false, 0x60),
        (false, true, false, true, 0x50),
        (true, true, true, true, 0xF0),
    ];

    for (white_to_move, game_over, white_wins, draw, expected) in cases {
        let game = game_in_state(white_to_move, game_over, white_wins, draw);
        let flags = game.to_binary()[BOARD_SIZE];
        assert_eq!(
            flags, expected,
            "flags for (white_to_move={white_to_move}, game_over={game_over}, \
             white_wins={white_wins}, draw={draw}) must be 0b{expected:08b}"
        );
        assert_eq!(
            flags & 0b0000_1111,
            0,
            "bits 0-3 of the flags byte are unused and must stay zero"
        );
    }
}

#[test]
fn game_byte_eighty_two_is_the_raw_moves_without_capture_counter() {
    for counter in [0u8, 1, 39, 40, 200, 255] {
        let mut game = Game::from_board(Board::new());
        game.set_moves_without_capture(counter);
        assert_eq!(
            game.to_binary()[BOARD_SIZE + 1],
            counter,
            "byte 82 is the counter verbatim"
        );
    }
}

#[test]
fn game_binary_round_trip_preserves_board_flags_and_counter() {
    let mut game = game_in_state(false, true, false, true);
    game.set_moves_without_capture(41);

    let bytes = game.to_binary();
    let decoded = Game::from_binary(bytes).expect("our own encoding must decode");

    assert_eq!(decoded.board, game.board, "the board must survive");
    assert_eq!(decoded.is_white_to_move(), game.is_white_to_move());
    assert_eq!(decoded.is_game_over(), game.is_game_over());
    assert_eq!(decoded.white_wins(), game.white_wins());
    assert_eq!(decoded.is_draw(), game.is_draw());
    assert_eq!(
        decoded.moves_without_capture(),
        game.moves_without_capture()
    );
    assert_eq!(decoded.to_binary(), bytes, "re-encoding must be identical");
}

// ── Move / PotentialMove bit layouts ────────────────────────────────────────

/// from/to pairs spanning both ends of the 7-bit fields.
const MOVE_SAMPLES: [(u8, u8); 6] = [(0, 80), (80, 0), (0, 1), (54, 45), (76, 4), (40, 41)];

#[test]
fn move_u16_is_unstack_bit_fourteen_to_bits_seven_to_thirteen_from_bits_zero_to_six() {
    for (from, to) in MOVE_SAMPLES {
        for unstack in [false, true] {
            let expected = ((unstack as u16) << 14) | ((to as u16) << 7) | (from as u16);
            let mv = Move {
                from: Position::from_u8(from),
                to: Position::from_u8(to),
                unstack,
            };
            assert_eq!(
                mv.to_u16(),
                expected,
                "Move {from}->{to} (unstack={unstack}) must encode as 0x{expected:04x}"
            );

            let decoded = Move::try_from_u16(expected)
                .unwrap_or_else(|| panic!("0x{expected:04x} must decode as a Move"));
            assert_eq!(decoded.from.to_u8(), from, "bits 0-6 carry `from`");
            assert_eq!(decoded.to.to_u8(), to, "bits 7-13 carry `to`");
            assert_eq!(decoded.unstack, unstack, "bit 14 carries `unstack`");
        }
    }
}

#[test]
fn move_u16_with_bit_fifteen_set_is_always_rejected() {
    for (from, to) in MOVE_SAMPLES {
        for unstack in [false, true] {
            let valid = ((unstack as u16) << 14) | ((to as u16) << 7) | (from as u16);
            assert!(
                Move::try_from_u16(valid).is_some(),
                "0x{valid:04x} is a well-formed Move"
            );
            assert!(
                Move::try_from_u16(valid | 0x8000).is_none(),
                "0x{:04x} sets bit 15, which is not part of the Move layout",
                valid | 0x8000
            );
        }
    }
}

#[test]
fn potential_move_u16_is_force_unstack_bit_fifteen_unstackable_bit_fourteen_to_then_from() {
    for (from, to) in MOVE_SAMPLES {
        // `force_unstack` only ever appears together with `unstackable`.
        for (force_unstack, unstackable) in [(false, false), (false, true), (true, true)] {
            let expected = ((force_unstack as u16) << 15)
                | ((unstackable as u16) << 14)
                | ((to as u16) << 7)
                | (from as u16);
            let potential = PotentialMove {
                from: Position::from_u8(from),
                to: Position::from_u8(to),
                unstackable,
                force_unstack,
            };
            assert_eq!(
                potential.to_u16(),
                expected,
                "PotentialMove {from}->{to} (unstackable={unstackable}, \
                 force_unstack={force_unstack}) must encode as 0x{expected:04x}"
            );
            assert_eq!(
                PotentialMove::try_from_u16(expected),
                Some(potential),
                "0x{expected:04x} must decode back to the same PotentialMove"
            );
        }
    }
}

#[test]
fn potential_move_with_force_unstack_but_not_unstackable_is_rejected() {
    for (from, to) in MOVE_SAMPLES {
        let raw = 0x8000 | ((to as u16) << 7) | (from as u16);
        assert!(
            PotentialMove::try_from_u16(raw).is_none(),
            "0x{raw:04x} sets force_unstack without unstackable, which the \
             generator never produces"
        );
    }
}

// ── Byte order on the wire ──────────────────────────────────────────────────

#[tokio::test]
async fn moves_response_pairs_are_little_endian_u16s() {
    let captured = post("/moves", new_game_bytes().await).await;
    assert_eq!(captured.status, 200, "POST /moves must succeed");
    assert!(!captured.body.is_empty(), "the start position has moves");
    assert_eq!(
        captured.body.len() % MOVE_BYTES,
        0,
        "the move list is a whole number of u16s"
    );

    let expected: Vec<u16> = Game::new()
        .get_all_moves()
        .iter()
        .map(|p| p.to_u16())
        .collect();
    assert_eq!(
        captured.body.len(),
        expected.len() * MOVE_BYTES,
        "one 2-byte pair per legal PotentialMove"
    );

    // Low byte first: byte 0 of each pair holds `from` (bits 0-6) plus the
    // low bit of `to`, byte 1 holds the rest.
    for (index, (pair, &raw)) in captured
        .body
        .chunks_exact(MOVE_BYTES)
        .zip(expected.iter())
        .enumerate()
    {
        assert_eq!(
            pair[0],
            (raw & 0x00FF) as u8,
            "move {index}: first byte must be the low byte of 0x{raw:04x}"
        );
        assert_eq!(
            pair[1],
            (raw >> 8) as u8,
            "move {index}: second byte must be the high byte of 0x{raw:04x}"
        );
    }

    // The checks above only bite if the two bytes actually differ somewhere,
    // and a big-endian writer must produce something a decoder can tell
    // apart: at least one pair, read big-endian, is either not a valid
    // PotentialMove at all or names a different origin square.
    let pairs: Vec<[u8; 2]> = captured
        .body
        .chunks_exact(MOVE_BYTES)
        .map(|c| [c[0], c[1]])
        .collect();
    assert!(
        pairs.iter().any(|p| p[0] != p[1]),
        "byte order is only observable if some pair has two different bytes"
    );
    assert!(
        pairs.iter().any(|p| {
            let from_le = (u16::from_le_bytes(*p) & 0x7F) as u8;
            match PotentialMove::try_from_u16(u16::from_be_bytes(*p)) {
                None => true,
                Some(swapped) => swapped.from.to_u8() != from_le,
            }
        }),
        "a big-endian swap must change the decoded move, not go unnoticed"
    );
}

// ── HTTP layer pinned to the library encoding ───────────────────────────────

#[tokio::test]
async fn play_returns_exactly_what_try_make_produces_in_library() {
    let game_bytes = new_game_bytes().await;

    let listed = post("/moves", game_bytes.clone()).await;
    assert_eq!(listed.status, 200, "POST /moves must succeed");
    let raw_potential = *listed
        .u16s()
        .iter()
        .find(|raw| *raw & 0xC000 == 0)
        .expect("the start position has moves with no stacking choice");

    let raw_move = potential_to_move(raw_potential);
    let mut payload = game_bytes.clone();
    payload.extend_from_slice(&move_bytes(raw_move));

    let played = post("/play", payload).await;
    assert_eq!(played.status, 200, "POST /play must succeed");
    assert_eq!(
        played.content_type.as_deref(),
        Some("application/octet-stream"),
        "/play is raw binary"
    );

    let mv = Move::try_from_u16(raw_move).expect("a listed move must decode");
    let mut game = Game::new();
    game.try_make(&mv).expect("a listed move must be legal");

    assert_eq!(
        played.body,
        game.to_binary().to_vec(),
        "the HTTP result must be byte-identical to the in-library encoding of {mv}"
    );
}
