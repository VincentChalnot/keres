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

    pub fn from_u16(v: u16) -> Self {
        PotentialMove {
            force_unstack: (v & 0x8000) != 0,
            unstackable: (v & 0x4000) != 0,
            // Mask the shifted value to 7 bits to avoid including the flag bits
            to: Position::from_u8(((v >> 7) & 0x7F) as u8),
            from: Position::from_u8((v & 0x007F) as u8),
        }
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

    pub fn from_u16(v: u16) -> Self {
        Move {
            unstack: (v & 0x4000) != 0,
            // Mask the shifted value to 7 bits to avoid including the flag bits
            to: Position::from_u8(((v >> 7) & 0x7F) as u8),
            from: Position::from_u8((v & 0x007F) as u8),
        }
    }

    pub fn to_string(&self) -> String {
        format!(
            "{}-{}{}",
            self.from.to_string(),
            self.to.to_string(),
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
        self.compute_moves_for_piece_type(
            position,
            piece.color,
            piece.bottom,
            false,
            piece.top.is_some(),
        )
        .into_iter()
        .for_each(|m| moves.push(m));

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
        let target_piece = self.board.get_piece(&target_position);
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
