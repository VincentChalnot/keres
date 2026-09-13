use crate::board::{Board, Color, PieceType, Position, BOARD_DIMENSION};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PotentialMove {
    pub from: Position,
    pub to: Position,
    pub unstackable: bool,
    pub force_unstack: bool,
}

impl PotentialMove {
    pub fn to_u16(self) -> u16 {
        ((self.force_unstack as u16) << 15)
            | ((self.unstackable as u16) << 14)
            | ((self.to.to_u8() as u16) << 7)
            | (self.from.to_u8() as u16)
    }

    /// Decode a wire `u16`, panicking if it is not a valid `PotentialMove`.
    /// Use [`PotentialMove::try_from_u16`] for caller-supplied bytes.
    pub fn from_u16(v: u16) -> Self {
        Self::try_from_u16(v).expect("invalid PotentialMove encoding")
    }

    /// Fallible counterpart of [`PotentialMove::from_u16`]. The `from`/`to`
    /// fields are 7 bits wide, so 81..=127 are representable but not valid
    /// squares; those are rejected here instead of panicking deep inside
    /// `Position`. `force_unstack` without `unstackable` is also rejected:
    /// the generator never produces that combination and [`Self::to_moves`]
    /// would panic on it.
    pub fn try_from_u16(v: u16) -> Option<Self> {
        let force_unstack = (v & 0x8000) != 0;
        let unstackable = (v & 0x4000) != 0;
        if force_unstack && !unstackable {
            return None;
        }
        Some(PotentialMove {
            force_unstack,
            unstackable,
            // Mask the shifted value to 7 bits to avoid including the flag bits
            to: Position::try_from_u8(((v >> 7) & 0x7F) as u8)?,
            from: Position::try_from_u8((v & 0x007F) as u8)?,
        })
    }

    pub fn to_move(&self, unstack: bool) -> Move {
        if unstack && !self.unstackable {
            panic!("Cannot unstack a piece that is not unstackable.");
        }
        if !unstack && self.force_unstack {
            panic!("Trying to move a piece that must be unstacked, but unstack is false.");
        }
        Move {
            from: self.from,
            to: self.to,
            unstack,
        }
    }

    /// Convert this PotentialMove into all valid Move variants.
    /// If unstackable is true, produces two moves (unstack=true and unstack=false).
    /// If force_unstack is true, produces only the unstack=true move.
    /// Otherwise produces a single move with unstack=false.
    pub fn to_moves(&self) -> Vec<Move> {
        if self.force_unstack {
            vec![self.to_move(true)]
        } else if self.unstackable {
            vec![self.to_move(false), self.to_move(true)]
        } else {
            vec![self.to_move(false)]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Move {
    pub from: Position,
    pub to: Position,
    pub unstack: bool,
}

impl Move {
    pub fn to_u16(self) -> u16 {
        ((self.unstack as u16) << 14) | ((self.to.to_u8() as u16) << 7) | (self.from.to_u8() as u16)
    }

    /// Decode a wire `u16`, panicking if either square index is off the
    /// board. Use [`Move::try_from_u16`] for caller-supplied bytes.
    pub fn from_u16(v: u16) -> Self {
        Self::try_from_u16(v).expect("Move square index must be between 0 and 80")
    }

    /// Fallible counterpart of [`Move::from_u16`], for decoding moves that
    /// arrive from outside the process (HTTP payloads, save files).
    ///
    /// Rejects off-board square indices — `from`/`to` are 7-bit fields, so
    /// 81..=127 are representable but meaningless — and `from == to`, which
    /// no generated move can ever be. Bit 15 is not part of the `Move`
    /// layout (it is `PotentialMove::force_unstack`) and must be zero, so a
    /// `PotentialMove` accidentally sent where a `Move` is expected is
    /// caught rather than silently truncated.
    pub fn try_from_u16(v: u16) -> Option<Self> {
        if v & 0x8000 != 0 {
            return None;
        }
        let mv = Move {
            unstack: (v & 0x4000) != 0,
            // Mask the shifted value to 7 bits to avoid including the flag bits
            to: Position::try_from_u8(((v >> 7) & 0x7F) as u8)?,
            from: Position::try_from_u8((v & 0x007F) as u8)?,
        };
        if mv.from == mv.to {
            return None;
        }
        Some(mv)
    }
}

/// `A1-B2` for a plain move, `A1-B2-` when the top of a stack moves alone.
impl std::fmt::Display for Move {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}-{}{}",
            self.from,
            self.to,
            if self.unstack { "-" } else { "" },
        )
    }
}

pub struct MoveGenerator<'a> {
    board: &'a Board,
    white_to_move: bool,
}

impl<'a> MoveGenerator<'a> {
    pub fn new(board: &'a Board, white_to_move: bool) -> Self {
        MoveGenerator {
            board,
            white_to_move,
        }
    }

    fn color_to_move(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    pub fn get_all_moves(&self) -> Vec<PotentialMove> {
        let mut all_moves = Vec::new();

        for y in 0..BOARD_DIMENSION {
            for x in 0..BOARD_DIMENSION {
                let position = Position::new(x, y);
                let moves = self.get_moves(&position);
                all_moves.extend(moves);
            }
        }

        all_moves
    }

    /// Check if a move captures an enemy piece by looking at the destination square.
    /// Returns true if there is an enemy piece at the move's destination square.
    pub fn is_capture(&self, mv: &Move) -> bool {
        if let Some(dest_piece) = self.board.get_piece(&mv.to) {
            let current_color = self.color_to_move();
            dest_piece.color != current_color
        } else {
            false
        }
    }

    pub fn get_moves(&self, position: &Position) -> Vec<PotentialMove> {
        let mut moves = Vec::new();

        let piece = self.board.get_piece(position);
        if piece.is_none() {
            return moves; // No piece at the position, no moves possible
        }
        let piece = piece.unwrap();
        if piece.color != self.color_to_move() {
            return moves; // Not the player's turn
        }

        if let Some(top_piece_type) = piece.top {
            self.compute_moves_for_piece_type(position, piece.color, top_piece_type, true, true)
                .into_iter()
                .for_each(|m| moves.push(m));
        }
        // Every move produced above is `unstackable`, so it expands into both
        // the "top piece leaves alone" and the "whole stack moves" variants.
        // The bottom half therefore only adds destinations the top half
        // cannot reach; without this filter the overlap (e.g. a paladin
        // riding a rook) yields the same whole-stack `Move` twice, which the
        // search would explore twice and the root move picker would weight
        // twice.
        let top_move_count = moves.len();
        for m in self.compute_moves_for_piece_type(
            position,
            piece.color,
            piece.bottom,
            false,
            piece.top.is_some(),
        ) {
            if moves[..top_move_count].iter().any(|t| t.to == m.to) {
                continue;
            }
            moves.push(m);
        }

        moves
    }

    fn compute_moves_for_piece_type(
        &self,
        position: &Position,
        color: Color,
        piece_type: PieceType,
        is_top: bool,
        has_top: bool,
    ) -> Vec<PotentialMove> {
        let mut moves = Vec::new();

        match piece_type {
            PieceType::Soldier => {
                self.compute_soldier_moves(position, color, is_top, has_top, &mut moves)
            }
            PieceType::Bishop => self.compute_generic_moves(
                position,
                color,
                is_top,
                has_top,
                &mut moves,
                &Position::DIAGONAL_MOVES,
                BOARD_DIMENSION as isize,
            ),
            PieceType::Rook => self.compute_generic_moves(
                position,
                color,
                is_top,
                has_top,
                &mut moves,
                &Position::ORTHOGONAL_MOVES,
                BOARD_DIMENSION as isize,
            ),
            PieceType::Paladin => self.compute_generic_moves(
                position,
                color,
                is_top,
                has_top,
                &mut moves,
                &Position::ORTHOGONAL_MOVES,
                2,
            ),
            PieceType::Guard => self.compute_generic_moves(
                position,
                color,
                is_top,
                has_top,
                &mut moves,
                &Position::DIAGONAL_MOVES,
                2,
            ),
            PieceType::Knight => {
                self.compute_knight_moves(position, color, is_top, has_top, &mut moves)
            }
            PieceType::Ballista => {
                self.compute_ballista_moves(position, color, is_top, has_top, &mut moves)
            }
            PieceType::King => self.compute_generic_moves(
                position,
                color,
                false,
                true,
                &mut moves,
                &Position::ALL_MOVES,
                1,
            ), // King cannot be stacked so we do this trick with is_top and has_top
        }

        moves
    }

    /// Explore a potential move from the current position to the target position.
    /// Return true if the move is not blocking, false if it's blocked.
    fn explore_position(
        &self,
        position: &Position,
        color: Color,
        target_position: &Position,
        is_top: bool,
        has_top: bool,
        moves: &mut Vec<PotentialMove>,
    ) -> bool {
        let target_piece = self.board.get_piece(target_position);
        // Empty case: OK can move
        if target_piece.is_none() {
            moves.push(PotentialMove {
                from: *position,
                to: *target_position,
                unstackable: is_top,
                force_unstack: false,
            });
            return true;
        }
        let target_piece = target_piece.unwrap();

        // Opposite color piece: OK can capture
        if target_piece.color != color {
            moves.push(PotentialMove {
                from: *position,
                to: *target_position,
                unstackable: is_top,
                force_unstack: false,
            });
            return false;
        }

        if !is_top && has_top {
            // Current piece cannot move to be stacked on top of a friendly piece because it's locked by a top piece
            return false;
        }

        // Cannot stack with the King or a piece that is already stacked
        if !target_piece.is_stackable() {
            return false;
        }

        moves.push(PotentialMove {
            from: *position,
            to: *target_position,
            unstackable: is_top,
            force_unstack: is_top, // Force unstacking the top piece
        });

        false
    }

    fn compute_soldier_moves(
        &self,
        position: &Position,
        color: Color,
        is_top: bool,
        has_top: bool,
        moves: &mut Vec<PotentialMove>,
    ) {
        // Soldier can move forward diagonally one step
        let dy: isize = if color == Color::White { -1 } else { 1 };
        if let Some(target_position) = position.get_new(1, dy) {
            self.explore_position(position, color, &target_position, is_top, has_top, moves);
        }
        if let Some(target_position) = position.get_new(-1, dy) {
            self.explore_position(position, color, &target_position, is_top, has_top, moves);
        }
    }

    fn compute_ballista_moves(
        &self,
        position: &Position,
        color: Color,
        is_top: bool,
        has_top: bool,
        moves: &mut Vec<PotentialMove>,
    ) {
        // Ballista can move forward any number of steps in a straight line
        let dy: isize = if color == Color::White { -1 } else { 1 };
        let directions = [(0isize, dy)];
        self.compute_generic_moves(
            position,
            color,
            is_top,
            has_top,
            moves,
            &directions,
            BOARD_DIMENSION as isize,
        );
    }

    fn compute_knight_moves(
        &self,
        position: &Position,
        color: Color,
        is_top: bool,
        has_top: bool,
        moves: &mut Vec<PotentialMove>,
    ) {
        // Knight move like a knight in chess
        let directions = [
            (2, 1),
            (2, -1),
            (-2, 1),
            (-2, -1),
            (1, 2),
            (1, -2),
            (-1, 2),
            (-1, -2),
        ];
        self.compute_generic_moves(position, color, is_top, has_top, moves, &directions, 1);
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_generic_moves(
        &self,
        position: &Position,
        color: Color,
        is_top: bool,
        has_top: bool,
        moves: &mut Vec<PotentialMove>,
        directions: &[(isize, isize)],
        max_distance: isize,
    ) {
        for &(dx, dy) in directions {
            for mult in 1..=max_distance {
                if let Some(target_position) = position.get_new(dx * mult, dy * mult) {
                    if !self.explore_position(
                        position,
                        color,
                        &target_position,
                        is_top,
                        has_top,
                        moves,
                    ) {
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Piece;
    use std::collections::HashSet;

    // ---------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------

    fn p(x: usize, y: usize) -> Position {
        Position::new(x, y)
    }

    /// An otherwise empty board carrying the given pieces.
    fn board_with(pieces: &[(Position, Piece)]) -> Board {
        let mut board = Board::empty();
        for (position, piece) in pieces {
            board.set_piece(position, Some(*piece));
        }
        board
    }

    fn single(color: Color, piece_type: PieceType) -> Piece {
        Piece::new(color, piece_type, None)
    }

    /// Sorted `(x, y)` destination list for the piece standing on `from`, so a
    /// test can compare the generator's output against an exact expected set.
    fn dests(board: &Board, white_to_move: bool, from: Position) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = MoveGenerator::new(board, white_to_move)
            .get_moves(&from)
            .iter()
            .map(|m| (m.to.x, m.to.y))
            .collect();
        out.sort_unstable();
        out
    }

    fn sorted(squares: &[(usize, usize)]) -> Vec<(usize, usize)> {
        let mut out = squares.to_vec();
        out.sort_unstable();
        out
    }

    /// Destinations of a lone piece of `color` standing on `from`, on an
    /// otherwise empty board, with that color to move.
    fn lone_dests(color: Color, piece_type: PieceType, from: Position) -> Vec<(usize, usize)> {
        let board = board_with(&[(from, single(color, piece_type))]);
        dests(&board, color == Color::White, from)
    }

    // ---------------------------------------------------------------------
    // PotentialMove <-> u16
    // ---------------------------------------------------------------------

    #[test]
    fn potential_move_u16_round_trips_every_valid_flag_combination() {
        let squares = [p(0, 0), p(8, 8), p(0, 8), p(8, 0), p(4, 4), p(3, 7)];
        // `force_unstack` implies `unstackable`; the fourth combination is
        // unrepresentable by construction and covered by its own test.
        let flags = [(false, false), (true, false), (true, true)];
        for &from in &squares {
            for &to in &squares {
                for (unstackable, force_unstack) in flags {
                    let pm = PotentialMove {
                        from,
                        to,
                        unstackable,
                        force_unstack,
                    };
                    assert_eq!(PotentialMove::from_u16(pm.to_u16()), pm);
                    assert_eq!(PotentialMove::try_from_u16(pm.to_u16()), Some(pm));
                }
            }
        }
    }

    #[test]
    fn potential_move_try_from_u16_rejects_a_forced_unstack_of_a_non_unstackable_move() {
        // `to_moves` would panic on this combination, so the decoder refuses
        // it up front. Bit 15 set, bit 14 clear, squares both on-board.
        assert_eq!(PotentialMove::try_from_u16(0x8000 | (1 << 7)), None);
        // The same payload with `unstackable` set decodes fine.
        assert!(PotentialMove::try_from_u16(0xC000 | (1 << 7)).is_some());
    }

    #[test]
    fn potential_move_flags_live_in_the_two_high_bits() {
        let pm = PotentialMove {
            from: p(0, 0),
            to: p(1, 0),
            unstackable: true,
            force_unstack: true,
        };
        assert_eq!(pm.to_u16(), 0xC000 | (1 << 7));
    }

    #[test]
    fn potential_move_try_from_u16_rejects_off_board_square_indices() {
        for index in 81u16..=127 {
            // Off-board `from`, on-board `to`.
            assert_eq!(PotentialMove::try_from_u16(index), None, "from = {index}");
            // On-board `from`, off-board `to`.
            assert_eq!(
                PotentialMove::try_from_u16(index << 7),
                None,
                "to = {index}"
            );
        }
    }

    #[test]
    fn potential_move_try_from_u16_accepts_exactly_the_valid_encodings() {
        let mut accepted = 0usize;
        for raw in 0..=u32::from(u16::MAX) {
            let v = raw as u16;
            let from_index = (v & 0x7F) as u8;
            let to_index = ((v >> 7) & 0x7F) as u8;
            let flags_ok = (v & 0x8000) == 0 || (v & 0x4000) != 0;
            let decoded = PotentialMove::try_from_u16(v);
            assert_eq!(
                decoded.is_some(),
                from_index <= 80 && to_index <= 80 && flags_ok,
                "unexpected verdict for {v:#06x}"
            );
            if let Some(pm) = decoded {
                // Every bit of the u16 belongs to a field, so the encoding is
                // canonical: re-encoding must reproduce the exact input.
                assert_eq!(pm.to_u16(), v);
                accepted += 1;
            }
        }
        // 81 `from` x 81 `to` x 3 valid flag combinations.
        assert_eq!(accepted, 81 * 81 * 3);
    }

    #[test]
    #[should_panic(expected = "invalid PotentialMove encoding")]
    fn potential_move_from_u16_panics_on_an_off_board_index() {
        PotentialMove::from_u16(81);
    }

    // ---------------------------------------------------------------------
    // Move <-> u16
    // ---------------------------------------------------------------------

    #[test]
    fn move_u16_round_trips_both_unstack_flags() {
        let squares = [p(0, 0), p(8, 8), p(0, 8), p(8, 0), p(4, 4), p(3, 7)];
        for &from in &squares {
            for &to in &squares {
                if from == to {
                    continue; // Null moves are rejected by design, see below.
                }
                for unstack in [false, true] {
                    let mv = Move { from, to, unstack };
                    assert_eq!(Move::from_u16(mv.to_u16()), mv);
                    assert_eq!(Move::try_from_u16(mv.to_u16()), Some(mv));
                }
            }
        }
    }

    #[test]
    fn move_to_u16_never_sets_the_force_unstack_bit() {
        let mv = Move {
            from: p(0, 0),
            to: p(8, 8),
            unstack: true,
        };
        assert_eq!(mv.to_u16() & 0x8000, 0);
        assert_eq!(mv.to_u16(), 0x4000 | (80 << 7));
    }

    #[test]
    fn move_try_from_u16_rejects_off_board_square_indices() {
        for index in 81u16..=127 {
            assert_eq!(Move::try_from_u16(index), None, "from = {index}");
            assert_eq!(Move::try_from_u16(index << 7), None, "to = {index}");
        }
    }

    #[test]
    fn move_try_from_u16_rejects_a_potential_move_with_force_unstack_set() {
        let pm = PotentialMove {
            from: p(0, 0),
            to: p(1, 1),
            unstackable: true,
            force_unstack: true,
        };
        // The same payload without bit 15 decodes fine, so the rejection is
        // really about `force_unstack` leaking into the `Move` layout.
        assert!(Move::try_from_u16(pm.to_u16()).is_none());
        assert!(Move::try_from_u16(pm.to_u16() & 0x7FFF).is_some());
    }

    #[test]
    fn move_try_from_u16_rejects_a_null_move() {
        for index in 0u16..=80 {
            let null = (index << 7) | index;
            assert_eq!(Move::try_from_u16(null), None, "square = {index}");
            assert_eq!(Move::try_from_u16(null | 0x4000), None, "square = {index}");
        }
    }

    /// Property test over the whole 16-bit input space: `try_from_u16` is the
    /// wire-decoding entry point, so it must never panic on hostile bytes, and
    /// whatever it accepts must survive a re-encode unchanged.
    #[test]
    fn move_try_from_u16_is_total_and_canonical_over_all_u16_values() {
        let mut accepted = 0usize;
        for raw in 0..=u32::from(u16::MAX) {
            let v = raw as u16;
            let from_index = (v & 0x7F) as u8;
            let to_index = ((v >> 7) & 0x7F) as u8;
            let expected_valid =
                v & 0x8000 == 0 && from_index <= 80 && to_index <= 80 && from_index != to_index;

            let decoded = Move::try_from_u16(v);
            assert_eq!(
                decoded.is_some(),
                expected_valid,
                "unexpected verdict for {v:#06x}"
            );

            if let Some(mv) = decoded {
                assert_eq!(mv.to_u16(), v, "non-canonical encoding for {v:#06x}");
                assert_eq!(Move::try_from_u16(mv.to_u16()), Some(mv));
                accepted += 1;
            }
        }
        // 81 `from` x 80 distinct `to` x 2 unstack flags.
        assert_eq!(accepted, 81 * 80 * 2);
    }

    #[test]
    #[should_panic(expected = "Move square index must be between 0 and 80")]
    fn move_from_u16_panics_on_an_off_board_index() {
        Move::from_u16(81 << 7);
    }

    #[test]
    fn move_display_uses_algebraic_notation_and_marks_unstacking() {
        let plain = Move {
            from: p(0, 8),
            to: p(1, 7),
            unstack: false,
        };
        assert_eq!(plain.to_string(), "A1-B2");

        let unstacked = Move {
            from: p(0, 8),
            to: p(1, 7),
            unstack: true,
        };
        assert_eq!(unstacked.to_string(), "A1-B2-");

        let corner = Move {
            from: p(8, 0),
            to: p(4, 4),
            unstack: false,
        };
        assert_eq!(corner.to_string(), "I9-E5");
    }

    // ---------------------------------------------------------------------
    // PotentialMove -> Move expansion
    // ---------------------------------------------------------------------

    fn potential(unstackable: bool, force_unstack: bool) -> PotentialMove {
        PotentialMove {
            from: p(4, 4),
            to: p(4, 2),
            unstackable,
            force_unstack,
        }
    }

    #[test]
    fn plain_potential_move_expands_to_a_single_non_unstacking_move() {
        let pm = potential(false, false);
        assert_eq!(
            pm.to_moves(),
            vec![Move {
                from: p(4, 4),
                to: p(4, 2),
                unstack: false,
            }]
        );
        assert!(!pm.to_move(false).unstack);
    }

    #[test]
    fn unstackable_potential_move_expands_to_both_variants() {
        let pm = potential(true, false);
        let moves = pm.to_moves();
        assert_eq!(moves.len(), 2);
        assert_eq!(
            moves,
            vec![
                Move {
                    from: p(4, 4),
                    to: p(4, 2),
                    unstack: false,
                },
                Move {
                    from: p(4, 4),
                    to: p(4, 2),
                    unstack: true,
                },
            ]
        );
    }

    #[test]
    fn force_unstack_potential_move_expands_to_the_unstacked_move_only() {
        let pm = potential(true, true);
        assert_eq!(
            pm.to_moves(),
            vec![Move {
                from: p(4, 4),
                to: p(4, 2),
                unstack: true,
            }]
        );
    }

    #[test]
    #[should_panic(expected = "Cannot unstack a piece that is not unstackable.")]
    fn to_move_panics_when_unstacking_a_move_that_is_not_unstackable() {
        potential(false, false).to_move(true);
    }

    #[test]
    #[should_panic(
        expected = "Trying to move a piece that must be unstacked, but unstack is false."
    )]
    fn to_move_panics_when_not_unstacking_a_forced_unstack_move() {
        potential(true, true).to_move(false);
    }

    // ---------------------------------------------------------------------
    // Movement of a lone piece on an empty board.
    //
    // y = 0 is Black's back rank and y = 8 is White's, so White moves towards
    // decreasing y ("forward" for White is up the screen).
    // ---------------------------------------------------------------------

    #[test]
    fn soldier_steps_one_square_diagonally_forward() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Soldier, p(4, 4)),
            sorted(&[(3, 3), (5, 3)])
        );
        assert_eq!(
            lone_dests(Color::Black, PieceType::Soldier, p(4, 4)),
            sorted(&[(3, 5), (5, 5)])
        );
    }

    #[test]
    fn soldier_moves_are_clipped_by_the_board_edges() {
        // White soldier in its own corner: only the inward diagonal exists.
        assert_eq!(
            lone_dests(Color::White, PieceType::Soldier, p(0, 8)),
            sorted(&[(1, 7)])
        );
        // A White soldier already on the far rank has nowhere to go.
        assert!(lone_dests(Color::White, PieceType::Soldier, p(4, 0)).is_empty());
        // Mirror image for Black.
        assert_eq!(
            lone_dests(Color::Black, PieceType::Soldier, p(0, 0)),
            sorted(&[(1, 1)])
        );
        assert!(lone_dests(Color::Black, PieceType::Soldier, p(4, 8)).is_empty());
    }

    #[test]
    fn bishop_slides_any_distance_on_all_four_diagonals() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Bishop, p(4, 4)),
            sorted(&[
                (5, 5),
                (6, 6),
                (7, 7),
                (8, 8),
                (5, 3),
                (6, 2),
                (7, 1),
                (8, 0),
                (3, 3),
                (2, 2),
                (1, 1),
                (0, 0),
                (3, 5),
                (2, 6),
                (1, 7),
                (0, 8),
            ])
        );
    }

    #[test]
    fn bishop_in_a_corner_has_a_single_diagonal() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Bishop, p(0, 0)),
            sorted(&[
                (1, 1),
                (2, 2),
                (3, 3),
                (4, 4),
                (5, 5),
                (6, 6),
                (7, 7),
                (8, 8)
            ])
        );
    }

    #[test]
    fn rook_slides_any_distance_on_both_orthogonals() {
        let expected: Vec<(usize, usize)> = sorted(
            &(0..9)
                .filter(|&x| x != 4)
                .map(|x| (x, 4))
                .chain((0..9).filter(|&y| y != 4).map(|y| (4, y)))
                .collect::<Vec<_>>(),
        );
        assert_eq!(lone_dests(Color::White, PieceType::Rook, p(4, 4)), expected);
        assert_eq!(expected.len(), 16);
    }

    #[test]
    fn rook_in_a_corner_still_reaches_both_edges() {
        let expected: Vec<(usize, usize)> = sorted(
            &(1..9)
                .map(|x| (x, 0))
                .chain((1..9).map(|y| (0, y)))
                .collect::<Vec<_>>(),
        );
        assert_eq!(lone_dests(Color::White, PieceType::Rook, p(0, 0)), expected);
    }

    #[test]
    fn paladin_reaches_up_to_two_squares_orthogonally() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Paladin, p(4, 4)),
            sorted(&[
                (5, 4),
                (6, 4),
                (3, 4),
                (2, 4),
                (4, 5),
                (4, 6),
                (4, 3),
                (4, 2)
            ])
        );
    }

    #[test]
    fn paladin_moves_are_clipped_by_the_board_edges() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Paladin, p(0, 0)),
            sorted(&[(1, 0), (2, 0), (0, 1), (0, 2)])
        );
    }

    #[test]
    fn guard_reaches_up_to_two_squares_diagonally() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Guard, p(4, 4)),
            sorted(&[
                (5, 5),
                (6, 6),
                (5, 3),
                (6, 2),
                (3, 3),
                (2, 2),
                (3, 5),
                (2, 6)
            ])
        );
    }

    #[test]
    fn guard_moves_are_clipped_by_the_board_edges() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Guard, p(8, 8)),
            sorted(&[(7, 7), (6, 6)])
        );
    }

    #[test]
    fn knight_leaps_like_a_chess_knight() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Knight, p(4, 4)),
            sorted(&[
                (6, 5),
                (6, 3),
                (2, 5),
                (2, 3),
                (5, 6),
                (5, 2),
                (3, 6),
                (3, 2)
            ])
        );
        // Direction-independent: Black gets the same eight squares.
        assert_eq!(
            lone_dests(Color::Black, PieceType::Knight, p(4, 4)),
            lone_dests(Color::White, PieceType::Knight, p(4, 4))
        );
    }

    #[test]
    fn knight_moves_are_clipped_by_the_board_edges() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Knight, p(0, 0)),
            sorted(&[(2, 1), (1, 2)])
        );
    }

    #[test]
    fn ballista_slides_straight_forward_only() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Ballista, p(4, 4)),
            sorted(&[(4, 3), (4, 2), (4, 1), (4, 0)])
        );
        assert_eq!(
            lone_dests(Color::Black, PieceType::Ballista, p(4, 4)),
            sorted(&[(4, 5), (4, 6), (4, 7), (4, 8)])
        );
    }

    #[test]
    fn ballista_moves_are_clipped_by_the_board_edges() {
        assert_eq!(
            lone_dests(Color::White, PieceType::Ballista, p(0, 8)),
            sorted(&[
                (0, 7),
                (0, 6),
                (0, 5),
                (0, 4),
                (0, 3),
                (0, 2),
                (0, 1),
                (0, 0)
            ])
        );
        assert!(lone_dests(Color::White, PieceType::Ballista, p(4, 0)).is_empty());
        assert!(lone_dests(Color::Black, PieceType::Ballista, p(4, 8)).is_empty());
    }

    #[test]
    fn king_steps_one_square_in_every_direction() {
        assert_eq!(
            lone_dests(Color::White, PieceType::King, p(4, 4)),
            sorted(&[
                (3, 3),
                (4, 3),
                (5, 3),
                (3, 4),
                (5, 4),
                (3, 5),
                (4, 5),
                (5, 5)
            ])
        );
    }

    #[test]
    fn king_moves_are_clipped_in_a_corner() {
        assert_eq!(
            lone_dests(Color::White, PieceType::King, p(0, 0)),
            sorted(&[(1, 0), (0, 1), (1, 1)])
        );
    }

    #[test]
    fn a_lone_piece_never_produces_an_unstacking_or_forced_move() {
        for piece_type in [
            PieceType::Soldier,
            PieceType::Bishop,
            PieceType::Rook,
            PieceType::Paladin,
            PieceType::Guard,
            PieceType::Knight,
            PieceType::Ballista,
            PieceType::King,
        ] {
            let from = p(4, 4);
            let board = board_with(&[(from, single(Color::White, piece_type))]);
            for mv in MoveGenerator::new(&board, true).get_moves(&from) {
                assert!(
                    !mv.unstackable,
                    "{piece_type:?} produced an unstackable move"
                );
                assert!(!mv.force_unstack, "{piece_type:?} forced an unstack");
                assert_eq!(mv.from, from);
            }
        }
    }

    // ---------------------------------------------------------------------
    // Blocking and capture
    // ---------------------------------------------------------------------

    #[test]
    fn rook_captures_the_first_enemy_piece_and_stops_there() {
        let from = p(4, 4);
        let board = board_with(&[
            (from, single(Color::White, PieceType::Rook)),
            (p(4, 2), single(Color::Black, PieceType::Soldier)),
        ]);
        let reachable = dests(&board, true, from);
        assert!(reachable.contains(&(4, 3)));
        assert!(reachable.contains(&(4, 2)), "the enemy piece is capturable");
        assert!(!reachable.contains(&(4, 1)), "nothing beyond the capture");
        assert!(!reachable.contains(&(4, 0)));
        assert_eq!(reachable.len(), 14);
    }

    #[test]
    fn rook_stops_before_a_friendly_non_stackable_piece() {
        let from = p(4, 4);
        let board = board_with(&[
            (from, single(Color::White, PieceType::Rook)),
            // The King is never stackable, so this square is simply blocked.
            (p(4, 2), single(Color::White, PieceType::King)),
        ]);
        let reachable = dests(&board, true, from);
        assert!(reachable.contains(&(4, 3)));
        assert!(!reachable.contains(&(4, 2)));
        assert!(!reachable.contains(&(4, 1)));
        assert_eq!(reachable.len(), 13);
    }

    #[test]
    fn rook_stacks_onto_a_friendly_stackable_piece_and_stops_there() {
        let from = p(4, 4);
        let board = board_with(&[
            (from, single(Color::White, PieceType::Rook)),
            (p(4, 2), single(Color::White, PieceType::Soldier)),
        ]);
        let moves = MoveGenerator::new(&board, true).get_moves(&from);
        let stacking: Vec<&PotentialMove> = moves.iter().filter(|m| m.to == p(4, 2)).collect();
        assert_eq!(stacking.len(), 1, "exactly one stacking move");
        // The rook is not itself a stack, so no unstacking is involved.
        assert!(!stacking[0].unstackable);
        assert!(!stacking[0].force_unstack);

        let reachable = dests(&board, true, from);
        assert!(reachable.contains(&(4, 3)));
        assert!(
            !reachable.contains(&(4, 1)),
            "the stacked-on piece still blocks the ray"
        );
        assert_eq!(reachable.len(), 14);
    }

    #[test]
    fn bishop_captures_the_first_enemy_piece_and_stops_there() {
        let from = p(4, 4);
        let board = board_with(&[
            (from, single(Color::White, PieceType::Bishop)),
            (p(6, 6), single(Color::Black, PieceType::Rook)),
        ]);
        let reachable = dests(&board, true, from);
        assert!(reachable.contains(&(5, 5)));
        assert!(reachable.contains(&(6, 6)));
        assert!(!reachable.contains(&(7, 7)));
        assert!(!reachable.contains(&(8, 8)));
        assert_eq!(reachable.len(), 14);
    }

    #[test]
    fn ballista_captures_the_first_enemy_piece_and_stops_there() {
        let from = p(4, 8);
        let board = board_with(&[
            (from, single(Color::White, PieceType::Ballista)),
            (p(4, 5), single(Color::Black, PieceType::Soldier)),
        ]);
        assert_eq!(dests(&board, true, from), sorted(&[(4, 7), (4, 6), (4, 5)]));
    }

    #[test]
    fn ballista_stops_before_a_friendly_non_stackable_piece() {
        let from = p(4, 8);
        let board = board_with(&[
            (from, single(Color::White, PieceType::Ballista)),
            (p(4, 5), single(Color::White, PieceType::King)),
        ]);
        assert_eq!(dests(&board, true, from), sorted(&[(4, 7), (4, 6)]));
    }

    #[test]
    fn knight_leaps_over_surrounding_pieces() {
        let from = p(4, 4);
        let mut pieces = vec![(from, single(Color::White, PieceType::Knight))];
        for (i, (dx, dy)) in Position::ALL_MOVES.iter().enumerate() {
            let blocker = from.get_new(*dx, *dy).unwrap();
            let color = if i % 2 == 0 {
                Color::White
            } else {
                Color::Black
            };
            pieces.push((blocker, single(color, PieceType::Soldier)));
        }
        let board = board_with(&pieces);
        assert_eq!(
            dests(&board, true, from),
            sorted(&[
                (6, 5),
                (6, 3),
                (2, 5),
                (2, 3),
                (5, 6),
                (5, 2),
                (3, 6),
                (3, 2)
            ])
        );
    }

    // ---------------------------------------------------------------------
    // Turn ownership
    // ---------------------------------------------------------------------

    #[test]
    fn get_moves_is_empty_for_an_empty_square() {
        let board = Board::new();
        // The three middle ranks are empty in the starting position.
        assert!(MoveGenerator::new(&board, true)
            .get_moves(&p(4, 4))
            .is_empty());
        assert!(MoveGenerator::new(&board, false)
            .get_moves(&p(4, 4))
            .is_empty());
    }

    #[test]
    fn get_moves_is_empty_for_a_piece_of_the_side_not_to_move() {
        let board = Board::new();
        let black_soldier = p(4, 2);
        assert_eq!(
            board.get_piece(&black_soldier).map(|piece| piece.color),
            Some(Color::Black)
        );
        assert!(MoveGenerator::new(&board, true)
            .get_moves(&black_soldier)
            .is_empty());
        assert!(!MoveGenerator::new(&board, false)
            .get_moves(&black_soldier)
            .is_empty());
    }

    #[test]
    fn every_generated_move_originates_from_a_piece_of_the_side_to_move() {
        let board = Board::new();
        for (white_to_move, expected) in [(true, Color::White), (false, Color::Black)] {
            let generator = MoveGenerator::new(&board, white_to_move);
            let moves = generator.get_all_moves();
            assert!(!moves.is_empty());
            for mv in &moves {
                let piece = board
                    .get_piece(&mv.from)
                    .expect("a move must start from an occupied square");
                assert_eq!(piece.color, expected, "{mv:?}");
            }
        }
    }

    #[test]
    fn no_generated_move_lands_on_a_friendly_piece_that_cannot_be_stacked_on() {
        let board = Board::new();
        for white_to_move in [true, false] {
            let generator = MoveGenerator::new(&board, white_to_move);
            for mv in generator.get_all_moves() {
                let mover = board.get_piece(&mv.from).unwrap();
                if let Some(target) = board.get_piece(&mv.to) {
                    if target.color == mover.color {
                        assert!(
                            target.is_stackable(),
                            "{mv:?} lands on a friendly piece that cannot be stacked on"
                        );
                    }
                }
            }
        }
    }

    /// Regression guard on the generator as a whole: 53 potential moves per
    /// side in the starting position. The number itself is not interesting —
    /// it is the observed output of the current generator, recorded so that an
    /// accidental change in reach, blocking or stacking rules shows up here.
    #[test]
    fn starting_position_offers_the_same_number_of_moves_to_both_sides() {
        let board = Board::new();
        let white = MoveGenerator::new(&board, true).get_all_moves();
        let black = MoveGenerator::new(&board, false).get_all_moves();
        assert_eq!(white.len(), 53);
        assert_eq!(black.len(), 53);
    }

    #[test]
    fn starting_position_has_no_stacks_so_no_move_is_unstackable() {
        let board = Board::new();
        for white_to_move in [true, false] {
            for mv in MoveGenerator::new(&board, white_to_move).get_all_moves() {
                assert!(!mv.unstackable, "{mv:?}");
                assert!(!mv.force_unstack, "{mv:?}");
            }
        }
    }

    // ---------------------------------------------------------------------
    // Stacks
    // ---------------------------------------------------------------------

    #[test]
    fn a_stack_offers_both_the_top_piece_moves_and_the_whole_stack_moves() {
        let from = p(4, 4);
        let board = board_with(&[(
            from,
            Piece::new(Color::White, PieceType::Rook, Some(PieceType::Soldier)),
        )]);
        let moves = MoveGenerator::new(&board, true).get_moves(&from);

        // The top soldier can leave the stack behind: its two forward
        // diagonals, all flagged unstackable.
        let top: Vec<(usize, usize)> = sorted(
            &moves
                .iter()
                .filter(|m| m.unstackable)
                .map(|m| (m.to.x, m.to.y))
                .collect::<Vec<_>>(),
        );
        assert_eq!(top, sorted(&[(3, 3), (5, 3)]));

        // The whole stack moves with the bottom rook's reach.
        let whole: Vec<(usize, usize)> = sorted(
            &moves
                .iter()
                .filter(|m| !m.unstackable)
                .map(|m| (m.to.x, m.to.y))
                .collect::<Vec<_>>(),
        );
        assert_eq!(whole, lone_dests(Color::White, PieceType::Rook, from));

        // Nothing is forced: every destination here is an empty square.
        assert!(moves.iter().all(|m| !m.force_unstack));
        assert_eq!(moves.len(), 18);
    }

    #[test]
    fn the_bottom_of_a_stack_cannot_stack_onto_a_friendly_piece() {
        let from = p(4, 4);
        let board = board_with(&[
            (
                from,
                Piece::new(Color::White, PieceType::Rook, Some(PieceType::Soldier)),
            ),
            (p(4, 2), single(Color::White, PieceType::Soldier)),
        ]);
        let moves = MoveGenerator::new(&board, true).get_moves(&from);

        assert!(
            !moves.iter().any(|m| m.to == p(4, 2)),
            "a stack may not climb onto a friendly piece"
        );
        // It still stops there, and the square in front of it is reachable.
        assert!(moves.iter().any(|m| m.to == p(4, 3) && !m.unstackable));
        assert!(!moves.iter().any(|m| m.to == p(4, 1)));
        assert!(!moves.iter().any(|m| m.to == p(4, 0)));
    }

    #[test]
    fn the_bottom_of_a_stack_can_still_capture() {
        let from = p(4, 4);
        let board = board_with(&[
            (
                from,
                Piece::new(Color::White, PieceType::Rook, Some(PieceType::Soldier)),
            ),
            (p(4, 2), single(Color::Black, PieceType::Soldier)),
        ]);
        let moves = MoveGenerator::new(&board, true).get_moves(&from);
        let capture: Vec<&PotentialMove> = moves.iter().filter(|m| m.to == p(4, 2)).collect();
        assert_eq!(capture.len(), 1);
        assert!(!capture[0].unstackable, "the whole stack makes the capture");
        assert!(!capture[0].force_unstack);
    }

    #[test]
    fn force_unstack_is_set_only_when_the_top_piece_stacks_onto_a_friendly_piece() {
        let from = p(4, 4);
        let board = board_with(&[
            (
                from,
                Piece::new(Color::White, PieceType::Rook, Some(PieceType::Soldier)),
            ),
            // On the top soldier's forward diagonal, off every rook ray.
            (p(3, 3), single(Color::White, PieceType::Guard)),
        ]);
        let moves = MoveGenerator::new(&board, true).get_moves(&from);

        let forced: Vec<&PotentialMove> = moves.iter().filter(|m| m.force_unstack).collect();
        assert_eq!(
            forced,
            vec![&PotentialMove {
                from,
                to: p(3, 3),
                unstackable: true,
                force_unstack: true,
            }]
        );
        // The soldier's other diagonal is an ordinary unstackable move.
        assert!(moves
            .iter()
            .any(|m| m.to == p(5, 3) && m.unstackable && !m.force_unstack));
    }

    #[test]
    fn a_top_piece_capturing_is_not_force_unstacked() {
        let from = p(4, 4);
        let board = board_with(&[
            (
                from,
                Piece::new(Color::White, PieceType::Rook, Some(PieceType::Soldier)),
            ),
            (p(3, 3), single(Color::Black, PieceType::Guard)),
        ]);
        let moves = MoveGenerator::new(&board, true).get_moves(&from);
        let capture: Vec<&PotentialMove> = moves.iter().filter(|m| m.to == p(3, 3)).collect();
        assert_eq!(capture.len(), 1);
        assert!(capture[0].unstackable);
        assert!(
            !capture[0].force_unstack,
            "the whole stack may capture too, so neither variant is forced"
        );
        assert_eq!(capture[0].to_moves().len(), 2);
    }

    #[test]
    fn force_unstack_never_appears_without_unstackable() {
        // `to_moves` would panic on that combination, so the generator must
        // never produce it. Sweep a stack across every square, with friendly
        // and enemy neighbours in reach.
        for y in 0..BOARD_DIMENSION {
            for x in 0..BOARD_DIMENSION {
                let from = p(x, y);
                let mut pieces = vec![(
                    from,
                    Piece::new(Color::White, PieceType::Guard, Some(PieceType::Knight)),
                )];
                for (dx, dy) in Position::ALL_MOVES {
                    if let Some(neighbour) = from.get_new(dx, dy) {
                        let color = if dx > 0 { Color::White } else { Color::Black };
                        pieces.push((neighbour, single(color, PieceType::Soldier)));
                    }
                }
                let board = board_with(&pieces);
                for mv in MoveGenerator::new(&board, true).get_moves(&from) {
                    if mv.force_unstack {
                        assert!(mv.unstackable, "{mv:?} is forced but not unstackable");
                    }
                    // Neither expansion may panic.
                    assert!(!mv.to_moves().is_empty());
                }
            }
        }
    }

    /// When the top piece's reach overlaps the bottom piece's, the bottom
    /// half must not re-emit a destination the top half already covers: the
    /// top half's `PotentialMove` is `unstackable`, so it already expands
    /// into the whole-stack move. Duplicates here would be searched twice
    /// and weighted twice by the root move picker.
    #[test]
    fn overlapping_stack_reach_produces_no_duplicate_moves() {
        let from = p(4, 4);
        let board = board_with(&[(
            from,
            // A paladin's 8 squares are all on the rook's rays.
            Piece::new(Color::White, PieceType::Rook, Some(PieceType::Paladin)),
        )]);
        let expanded: Vec<Move> = MoveGenerator::new(&board, true)
            .get_moves(&from)
            .iter()
            .flat_map(|m| m.to_moves())
            .collect();
        let distinct: HashSet<Move> = expanded.iter().copied().collect();

        // 16 whole-stack moves + 8 unstacking moves, each produced once.
        assert_eq!(distinct.len(), 24);
        assert_eq!(expanded.len(), 24);
    }

    #[test]
    fn no_position_ever_generates_the_same_move_twice() {
        // Sweep a stack whose halves have disjoint, overlapping and nested
        // reach across the board, with friendly and enemy neighbours.
        let combos = [
            (PieceType::Rook, PieceType::Paladin),
            (PieceType::Bishop, PieceType::Guard),
            (PieceType::Rook, PieceType::Bishop),
            (PieceType::Knight, PieceType::Soldier),
            (PieceType::Ballista, PieceType::Rook),
        ];
        for (bottom, top) in combos {
            for y in 0..9 {
                for x in 0..9 {
                    let from = p(x, y);
                    let mut squares = vec![(from, Piece::new(Color::White, bottom, Some(top)))];
                    if let Some(friend) = from.get_new(1, 1) {
                        squares.push((friend, single(Color::White, PieceType::Soldier)));
                    }
                    if let Some(enemy) = from.get_new(-1, -1) {
                        squares.push((enemy, single(Color::Black, PieceType::Soldier)));
                    }
                    let board = board_with(&squares);
                    let expanded: Vec<Move> = MoveGenerator::new(&board, true)
                        .get_moves(&from)
                        .iter()
                        .flat_map(|m| m.to_moves())
                        .collect();
                    let distinct: HashSet<Move> = expanded.iter().copied().collect();
                    assert_eq!(
                        expanded.len(),
                        distinct.len(),
                        "{bottom:?}/{top:?} at {from} produced duplicates"
                    );
                }
            }
        }
    }

    // ---------------------------------------------------------------------
    // is_capture
    // ---------------------------------------------------------------------

    #[test]
    fn is_capture_is_true_only_for_an_enemy_occupied_destination() {
        let from = p(4, 4);
        let board = board_with(&[
            (from, single(Color::White, PieceType::Rook)),
            (p(4, 2), single(Color::Black, PieceType::Soldier)),
            (p(4, 6), single(Color::White, PieceType::Soldier)),
        ]);
        let generator = MoveGenerator::new(&board, true);

        let to = |target: Position| Move {
            from,
            to: target,
            unstack: false,
        };

        assert!(generator.is_capture(&to(p(4, 2))), "enemy piece");
        assert!(!generator.is_capture(&to(p(4, 3))), "empty square");
        assert!(!generator.is_capture(&to(p(4, 6))), "friendly piece");
    }

    #[test]
    fn is_capture_follows_the_side_to_move_not_the_moving_piece() {
        let board = board_with(&[
            (p(4, 4), single(Color::White, PieceType::Rook)),
            (p(4, 2), single(Color::Black, PieceType::Soldier)),
        ]);
        let mv = Move {
            from: p(4, 4),
            to: p(4, 2),
            unstack: false,
        };
        assert!(MoveGenerator::new(&board, true).is_capture(&mv));
        assert!(!MoveGenerator::new(&board, false).is_capture(&mv));
    }
}
