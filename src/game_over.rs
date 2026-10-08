use crate::board::{Board, Color, Piece, PieceType, Position};

pub struct GameOverResult {
    pub game_over: bool,
    pub white_wins: bool,
    pub draw: bool,
}

impl GameOverResult {
    pub fn ongoing() -> Self {
        GameOverResult {
            game_over: false,
            white_wins: false,
            draw: false,
        }
    }
}

pub fn check_promotion(piece: &Piece, position: &Position) -> Option<Piece> {
    let reached_opposite_side = match piece.color {
        Color::White => position.y == 0,
        Color::Black => position.y == 8,
    };
    if !reached_opposite_side {
        return None;
    }

    let promote_piece_type = |piece_type: PieceType| -> PieceType {
        match piece_type {
            PieceType::Soldier => PieceType::Paladin,
            PieceType::Ballista => PieceType::Rook,
            _ => piece_type,
        }
    };

    let bottom_needs_promotion =
        piece.bottom == PieceType::Soldier || piece.bottom == PieceType::Ballista;
    let top_needs_promotion = piece.top.is_some()
        && (piece.top == Some(PieceType::Soldier) || piece.top == Some(PieceType::Ballista));

    if !bottom_needs_promotion && !top_needs_promotion {
        return None;
    }

    let promoted_bottom = promote_piece_type(piece.bottom);
    let promoted_top = piece.top.map(promote_piece_type);
    Some(Piece::new(piece.color, promoted_bottom, promoted_top))
}

/// Decide whether `board` (with `moves_without_capture` quiet moves played
/// since the last capture) is a finished game.
///
/// One pass over the board, no allocation: the search calls this on every
/// capture it explores (see `engine::search::outcome`), not just `Game::make`.
pub fn check_game_over(board: &Board, moves_without_capture: u8) -> GameOverResult {
    let mut white = SideMaterial::default();
    let mut black = SideMaterial::default();
    for (pos, piece) in board.pieces() {
        match piece.color {
            Color::White => white.add(piece, pos),
            Color::Black => black.add(piece, pos),
        }
    }

    if !white.has_king {
        return GameOverResult {
            game_over: true,
            white_wins: false,
            draw: false,
        };
    }
    if !black.has_king {
        return GameOverResult {
            game_over: true,
            white_wins: true,
            draw: false,
        };
    }

    // Capturing every one of the opponent's pieces except their king wins
    // the game outright, even if the winning side's own material would
    // otherwise be judged insufficient to checkmate.
    if black.non_king == 0 && white.non_king > 0 {
        return GameOverResult {
            game_over: true,
            white_wins: true,
            draw: false,
        };
    }
    if white.non_king == 0 && black.non_king > 0 {
        return GameOverResult {
            game_over: true,
            white_wins: false,
            draw: false,
        };
    }

    if moves_without_capture >= 40 {
        return GameOverResult {
            game_over: true,
            white_wins: false,
            draw: true,
        };
    }

    if white.is_insufficient() && black.is_insufficient() {
        return GameOverResult {
            game_over: true,
            white_wins: false,
            draw: true,
        };
    }

    GameOverResult::ongoing()
}

/// What one side has on the board, as far as the game-over rules care.
struct SideMaterial {
    has_king: bool,
    /// Number of non-king squares (a stack counts once).
    non_king: u32,
    /// The first non-king piece was an unstacked knight (only meaningful
    /// while `non_king == 1`).
    lone_knight: bool,
    /// Every non-king piece seen so far is colour-bound (bishops/guards,
    /// stacks thereof) and they all stand on the same square colour.
    colour_bound: bool,
    square_colour: Option<bool>,
}

impl Default for SideMaterial {
    fn default() -> Self {
        SideMaterial {
            has_king: false,
            non_king: 0,
            lone_knight: false,
            colour_bound: true,
            square_colour: None,
        }
    }
}

impl SideMaterial {
    fn add(&mut self, piece: &Piece, pos: Position) {
        if piece.is_king() {
            self.has_king = true;
            return;
        }
        self.non_king += 1;
        self.lone_knight =
            self.non_king == 1 && piece.bottom == PieceType::Knight && piece.top.is_none();
        // Both halves of a stack have to be colour-bound for the stack to be.
        // A soldier riding a bishop can unstack, advance and promote, so it
        // is not insufficient material.
        let is_colour_bound = |t: PieceType| t == PieceType::Bishop || t == PieceType::Guard;
        let piece_colour_bound =
            is_colour_bound(piece.bottom) && piece.top.map(is_colour_bound).unwrap_or(true);
        // A square is "white" when (x + y) is even.
        let square_is_white = (pos.x + pos.y).is_multiple_of(2);
        let first = *self.square_colour.get_or_insert(square_is_white);
        if !piece_colour_bound || first != square_is_white {
            self.colour_bound = false;
        }
    }

    /// Bare king, king + lone knight, or king + colour-bound pieces all on
    /// one square colour.
    fn is_insufficient(&self) -> bool {
        self.non_king == 0 || (self.non_king == 1 && self.lone_knight) || self.colour_bound
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::BOARD_DIMENSION;

    const ALL_TYPES: [PieceType; 8] = [
        PieceType::Soldier,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Paladin,
        PieceType::Guard,
        PieceType::Knight,
        PieceType::Ballista,
        PieceType::King,
    ];

    /// Every type that never promotes, whatever rank it reaches.
    const NON_PROMOTING_TYPES: [PieceType; 6] = [
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Paladin,
        PieceType::Guard,
        PieceType::Knight,
        PieceType::King,
    ];

    const COLORS: [Color; 2] = [Color::White, Color::Black];

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn at(x: usize, y: usize) -> Position {
        Position::new(x, y)
    }

    fn single(color: Color, bottom: PieceType) -> Piece {
        Piece::new(color, bottom, None)
    }

    fn stacked(color: Color, bottom: PieceType, top: PieceType) -> Piece {
        Piece::new(color, bottom, Some(top))
    }

    fn king(color: Color) -> Piece {
        single(color, PieceType::King)
    }

    fn board_of(pieces: &[(Piece, Position)]) -> Board {
        let mut board = Board::empty();
        for (piece, pos) in pieces {
            board.set_piece(pos, Some(*piece));
        }
        board
    }

    /// The rank on which `color` promotes: y=0 is Black's back rank (so White
    /// promotes there) and y=8 is White's.
    fn promotion_rank(color: Color) -> usize {
        match color {
            Color::White => 0,
            Color::Black => 8,
        }
    }

    fn other_rank(color: Color) -> usize {
        match color {
            Color::White => 8,
            Color::Black => 0,
        }
    }

    fn promote_type(piece_type: PieceType) -> PieceType {
        match piece_type {
            PieceType::Soldier => PieceType::Paladin,
            PieceType::Ballista => PieceType::Rook,
            other => other,
        }
    }

    fn promotes(piece_type: PieceType) -> bool {
        piece_type == PieceType::Soldier || piece_type == PieceType::Ballista
    }

    fn assert_win(result: &GameOverResult, white_wins: bool, case: &str) {
        assert!(result.game_over, "{case}: expected the game to be over");
        assert_eq!(
            result.white_wins, white_wins,
            "{case}: expected white_wins == {white_wins}"
        );
        assert!(!result.draw, "{case}: a win must not be reported as a draw");
    }

    fn assert_draw(result: &GameOverResult, case: &str) {
        assert!(result.game_over, "{case}: expected the game to be over");
        assert!(result.draw, "{case}: expected a draw");
        assert!(
            !result.white_wins,
            "{case}: a draw must not also claim white wins"
        );
    }

    fn assert_ongoing(result: &GameOverResult, case: &str) {
        assert!(!result.game_over, "{case}: expected the game to continue");
        assert!(!result.white_wins, "{case}: expected no winner");
        assert!(!result.draw, "{case}: expected no draw");
    }

    // ── check_promotion: single pieces ───────────────────────────────────────

    #[test]
    fn a_soldier_promotes_to_a_paladin_only_on_its_own_far_rank() {
        for color in COLORS {
            let soldier = single(color, PieceType::Soldier);

            let promoted = check_promotion(&soldier, &at(4, promotion_rank(color)))
                .unwrap_or_else(|| panic!("{color:?} soldier should promote on its far rank"));
            assert_eq!(promoted, single(color, PieceType::Paladin), "{color:?}");

            assert_eq!(
                check_promotion(&soldier, &at(4, other_rank(color))),
                None,
                "a {color:?} soldier on its own back rank must not promote"
            );
        }
    }

    #[test]
    fn a_ballista_promotes_to_a_rook_only_on_its_own_far_rank() {
        for color in COLORS {
            let ballista = single(color, PieceType::Ballista);

            let promoted = check_promotion(&ballista, &at(0, promotion_rank(color)))
                .unwrap_or_else(|| panic!("{color:?} ballista should promote on its far rank"));
            assert_eq!(promoted, single(color, PieceType::Rook), "{color:?}");

            assert_eq!(
                check_promotion(&ballista, &at(0, other_rank(color))),
                None,
                "a {color:?} ballista on its own back rank must not promote"
            );
        }
    }

    #[test]
    fn a_promotable_piece_off_the_promotion_rank_does_not_promote() {
        for color in COLORS {
            for piece_type in [PieceType::Soldier, PieceType::Ballista] {
                for y in 0..BOARD_DIMENSION {
                    if y == promotion_rank(color) {
                        continue;
                    }
                    let piece = single(color, piece_type);
                    assert_eq!(
                        check_promotion(&piece, &at(4, y)),
                        None,
                        "{color:?} {piece_type:?} at y={y} is not on a promotion rank"
                    );
                }
            }
        }
    }

    #[test]
    fn piece_types_that_never_promote_stay_unchanged_on_the_promotion_rank() {
        for color in COLORS {
            for piece_type in NON_PROMOTING_TYPES {
                let piece = single(color, piece_type);
                assert_eq!(
                    check_promotion(&piece, &at(4, promotion_rank(color))),
                    None,
                    "{color:?} {piece_type:?} must not promote even on the far rank"
                );
            }
        }
    }

    // ── check_promotion: stacks ──────────────────────────────────────────────

    #[test]
    fn a_stack_promotes_each_half_independently_and_keeps_its_arrangement() {
        // (bottom, top, expected bottom, expected top)
        let cases = [
            (
                PieceType::Soldier,
                PieceType::Soldier,
                Some((PieceType::Paladin, PieceType::Paladin)),
            ),
            (
                PieceType::Ballista,
                PieceType::Ballista,
                Some((PieceType::Rook, PieceType::Rook)),
            ),
            // Top-only promotion: the guard underneath is untouched.
            (
                PieceType::Guard,
                PieceType::Soldier,
                Some((PieceType::Guard, PieceType::Paladin)),
            ),
            // Bottom-only promotion: the guard on top is untouched.
            (
                PieceType::Soldier,
                PieceType::Guard,
                Some((PieceType::Paladin, PieceType::Guard)),
            ),
            // Mixed promotion of both halves into different types.
            (
                PieceType::Soldier,
                PieceType::Ballista,
                Some((PieceType::Paladin, PieceType::Rook)),
            ),
            // Neither half promotes.
            (PieceType::Guard, PieceType::Bishop, None),
        ];

        for color in COLORS {
            for (bottom, top, expected) in cases {
                let piece = stacked(color, bottom, top);
                let result = check_promotion(&piece, &at(4, promotion_rank(color)));
                let case = format!("{color:?} {top:?} on {bottom:?}");

                match expected {
                    None => assert_eq!(result, None, "{case}: nothing to promote"),
                    Some((expected_bottom, expected_top)) => {
                        let promoted =
                            result.unwrap_or_else(|| panic!("{case}: expected a promotion"));
                        assert_eq!(promoted.color, color, "{case}: color must be preserved");
                        assert_eq!(promoted.bottom, expected_bottom, "{case}: bottom");
                        assert_eq!(promoted.top, Some(expected_top), "{case}: top");
                    }
                }
            }
        }
    }

    #[test]
    fn check_promotion_maps_soldier_and_ballista_for_every_bottom_top_combination() {
        let mut tops: Vec<Option<PieceType>> = vec![None];
        tops.extend(ALL_TYPES.iter().copied().map(Some));

        for color in COLORS {
            for bottom in ALL_TYPES {
                for top in tops.iter().copied() {
                    // `Piece::new` panics when a king carries a top piece, so
                    // that combination is not a representable position.
                    if bottom == PieceType::King && top.is_some() {
                        continue;
                    }

                    let piece = Piece::new(color, bottom, top);
                    let case = format!("{color:?} {top:?} on {bottom:?}");

                    let needs_promotion = promotes(bottom) || top.map(promotes).unwrap_or(false);
                    let expected = if needs_promotion {
                        Some(Piece::new(
                            color,
                            promote_type(bottom),
                            top.map(promote_type),
                        ))
                    } else {
                        None
                    };

                    assert_eq!(
                        check_promotion(&piece, &at(3, promotion_rank(color))),
                        expected,
                        "{case}: on the promotion rank"
                    );
                    assert_eq!(
                        check_promotion(&piece, &at(3, other_rank(color))),
                        None,
                        "{case}: off the promotion rank"
                    );
                }
            }
        }
    }

    // ── check_game_over: ongoing ─────────────────────────────────────────────

    #[test]
    fn the_starting_position_is_not_over() {
        let result = check_game_over(&Board::new(), 0);
        assert_ongoing(&result, "starting position");
    }

    // ── check_game_over: king capture ────────────────────────────────────────

    #[test]
    fn a_missing_white_king_is_a_win_for_black() {
        let board = board_of(&[
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Rook), at(0, 0)),
            (single(Color::White, PieceType::Rook), at(8, 8)),
        ]);
        assert_win(&check_game_over(&board, 0), false, "white king captured");
    }

    #[test]
    fn a_missing_black_king_is_a_win_for_white() {
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Rook), at(0, 8)),
            (single(Color::Black, PieceType::Rook), at(8, 0)),
        ]);
        assert_win(&check_game_over(&board, 0), true, "black king captured");
    }

    #[test]
    fn with_both_kings_missing_the_check_order_awards_the_win_to_black() {
        // `check_game_over` tests for the white king first, so a board with
        // neither king reports a black win. This position cannot arise in a
        // real game (the first king capture ends it) — the assertion pins the
        // order of the two checks, it is not a rule claim.
        let board = board_of(&[
            (single(Color::White, PieceType::Rook), at(0, 8)),
            (single(Color::Black, PieceType::Rook), at(8, 0)),
        ]);
        assert_win(&check_game_over(&board, 0), false, "both kings missing");
    }

    // ── check_game_over: capturing every non-king enemy piece ────────────────

    #[test]
    fn white_wins_by_capturing_every_black_piece_but_the_king_even_with_thin_material() {
        // White is left with a lone knight, which on its own counts as
        // insufficient material — the outright win takes precedence (see the
        // comment in `check_game_over`).
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Knight), at(0, 8)),
            (king(Color::Black), at(4, 0)),
        ]);
        assert_win(&check_game_over(&board, 0), true, "black reduced to a king");
    }

    #[test]
    fn black_wins_by_capturing_every_white_piece_but_the_king_even_with_thin_material() {
        let board = board_of(&[
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Knight), at(0, 0)),
            (king(Color::White), at(4, 8)),
        ]);
        assert_win(
            &check_game_over(&board, 0),
            false,
            "white reduced to a king",
        );
    }

    // ── check_game_over: 40-move rule ────────────────────────────────────────

    #[test]
    fn forty_moves_without_a_capture_is_a_draw() {
        let board = Board::new();

        assert_ongoing(&check_game_over(&board, 39), "39 moves without a capture");

        for counter in [40u8, 41, 100, u8::MAX] {
            assert_draw(
                &check_game_over(&board, counter),
                &format!("{counter} moves without a capture"),
            );
        }
    }

    #[test]
    fn the_forty_move_draw_is_only_checked_after_the_win_conditions() {
        let no_white_king = board_of(&[
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Rook), at(0, 0)),
            (single(Color::White, PieceType::Rook), at(8, 8)),
        ]);
        assert_win(
            &check_game_over(&no_white_king, 40),
            false,
            "white king captured on the 40th quiet move",
        );

        let no_black_king = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Rook), at(0, 8)),
            (single(Color::Black, PieceType::Rook), at(8, 0)),
        ]);
        assert_win(
            &check_game_over(&no_black_king, 40),
            true,
            "black king captured on the 40th quiet move",
        );

        let black_stripped = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Knight), at(0, 8)),
            (king(Color::Black), at(4, 0)),
        ]);
        assert_win(
            &check_game_over(&black_stripped, 40),
            true,
            "black stripped to a king on the 40th quiet move",
        );
    }

    // ── check_game_over: insufficient material ───────────────────────────────

    #[test]
    fn two_bare_kings_are_an_insufficient_material_draw() {
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (king(Color::Black), at(4, 0)),
        ]);
        assert_draw(&check_game_over(&board, 0), "bare kings");
    }

    #[test]
    fn a_lone_knight_on_each_side_is_an_insufficient_material_draw() {
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Knight), at(0, 8)),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Knight), at(0, 0)),
        ]);
        assert_draw(&check_game_over(&board, 0), "king and knight each");
    }

    #[test]
    fn bishops_and_guards_confined_to_one_square_color_are_an_insufficient_material_draw() {
        // `SideMaterial::add` calls a square "white" when
        // (x + y) % 2 == 0. White's two pieces sit on (x + y) even squares and
        // black's on (x + y) odd ones, so both parities of the same-color
        // branch are exercised here.
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Bishop), at(0, 8)),
            (single(Color::White, PieceType::Guard), at(2, 8)),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Bishop), at(1, 0)),
            (single(Color::Black, PieceType::Guard), at(3, 0)),
        ]);
        for pos in [at(0, 8), at(2, 8)] {
            assert_eq!((pos.x + pos.y) % 2, 0, "{pos} should be an even square");
        }
        for pos in [at(1, 0), at(3, 0)] {
            assert_eq!((pos.x + pos.y) % 2, 1, "{pos} should be an odd square");
        }
        assert_draw(&check_game_over(&board, 0), "bishops/guards on one color");
    }

    #[test]
    fn bishops_on_two_square_colors_are_not_an_insufficient_material_draw() {
        // (0, 8) is an even square and (1, 8) an odd one, so white's bishops
        // cover both square colors and white is not in a drawing state even
        // though black (a lone knight) is.
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Bishop), at(0, 8)),
            (single(Color::White, PieceType::Bishop), at(1, 8)),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Knight), at(0, 0)),
        ]);
        assert_ongoing(&check_game_over(&board, 0), "bishops on both colors");
    }

    #[test]
    fn a_stacked_lone_knight_is_not_an_insufficient_material_draw() {
        // The lone-knight exception requires an unstacked knight; a knight
        // carrying a second knight is not covered by it.
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (
                stacked(Color::White, PieceType::Knight, PieceType::Knight),
                at(0, 8),
            ),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Knight), at(0, 0)),
        ]);
        assert_ongoing(&check_game_over(&board, 0), "stacked lone knight");
    }

    #[test]
    fn two_knights_are_not_an_insufficient_material_draw() {
        let board = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Knight), at(0, 8)),
            (single(Color::White, PieceType::Knight), at(2, 8)),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Knight), at(0, 0)),
        ]);
        assert_ongoing(&check_game_over(&board, 0), "two knights against one");
    }

    #[test]
    fn insufficient_material_needs_both_sides_to_be_drawing() {
        // Black has a lone knight (drawing) but white still has a rook, so
        // there is no draw — and the mirror image is equally not a draw.
        let white_has_a_rook = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Rook), at(0, 8)),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Knight), at(0, 0)),
        ]);
        assert_ongoing(&check_game_over(&white_has_a_rook, 0), "white has a rook");

        let black_has_a_rook = board_of(&[
            (king(Color::White), at(4, 8)),
            (single(Color::White, PieceType::Knight), at(0, 8)),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Rook), at(0, 0)),
        ]);
        assert_ongoing(&check_game_over(&black_has_a_rook, 0), "black has a rook");
    }

    #[test]
    fn a_stack_is_colour_bound_only_when_both_halves_are() {
        // A soldier riding a bishop can unstack, advance and promote, so the
        // side still has mating material even though the bishop half is
        // colour-bound.
        let soldier_on_bishop = board_of(&[
            (king(Color::White), at(4, 8)),
            (
                stacked(Color::White, PieceType::Bishop, PieceType::Soldier),
                at(0, 8),
            ),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Bishop), at(1, 0)),
        ]);
        assert_ongoing(
            &check_game_over(&soldier_on_bishop, 0),
            "soldier stacked on a bishop",
        );

        // Both halves colour-bound and on one square colour: still a draw.
        let guard_on_bishop = board_of(&[
            (king(Color::White), at(4, 8)),
            (
                stacked(Color::White, PieceType::Bishop, PieceType::Guard),
                at(0, 8),
            ),
            (king(Color::Black), at(4, 0)),
            (single(Color::Black, PieceType::Bishop), at(1, 0)),
        ]);
        assert_draw(
            &check_game_over(&guard_on_bishop, 0),
            "guard stacked on a bishop",
        );
    }
}
