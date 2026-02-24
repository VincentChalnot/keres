use crate::board::{Board, Color, Piece, PieceType, Position, BOARD_DIMENSION, BOARD_SIZE};

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

#[derive(Clone, Debug)]
pub struct Game {
    pub board: Board,
}

impl Game {
    pub fn new() -> Self {
        Game {
            board: Board::new(),
        }
    }

    pub fn from_board(board: Board) -> Self {
        Game { board }
    }

    pub fn apply_move(&mut self, mv: Move) -> Result<(), String> {
        // Check if game is already over
        if self.board.is_game_over() {
            return Err("Game is over, no more moves allowed".to_string());
        }

        let new_board = self.apply_move_copy(mv)?;
        self.board = new_board;
        Ok(())
    }

    pub fn apply_move_copy(&self, mv: Move) -> Result<Board, String> {
        // Get the piece at the 'from' position
        let piece = self
            .board
            .get_piece(&mv.from)
            .ok_or("No piece at 'from' position")?;

        // Check if the piece can be moved (e.g., not empty)
        if piece.bottom == PieceType::King && mv.unstack {
            return Err("Cannot unstack King".to_string());
        }

        let source_piece: Piece;
        let mut new_board = self.board.clone();
        if mv.unstack {
            // Unstack the top piece if it exists
            if !piece.top.is_some() {
                return Err("No top piece to unstack".to_string());
            }

            let new_piece = new_board.unstack_piece(&mv.from);
            if let Err(e) = new_piece {
                return Err(e);
            }
            source_piece = new_piece?;
        } else {
            source_piece = piece.clone();
            // Remove the piece from the 'from' position
            new_board.set_piece(&mv.from, None);
        }

        // Check what's at the destination position
        let destination_piece_opt = new_board.get_piece(&mv.to).cloned();

        let mut final_piece = source_piece;
        let mut was_capture = false;

        if destination_piece_opt.is_none() {
            // Empty square: just place the piece
            new_board.set_piece(&mv.to, Some(final_piece));
        } else {
            let destination_piece = destination_piece_opt.unwrap();

            if destination_piece.color != source_piece.color {
                // Enemy piece: capture it (replace with our piece)
                was_capture = true;
                new_board.set_piece(&mv.to, Some(final_piece));
            } else {
                // Friendly piece: attempt to stack
                if let Err(e) = new_board.stack_piece(&mv.to, source_piece) {
                    return Err(format!("Cannot complete move: {}", e));
                }
                // Update final_piece to reflect the stacked piece for promotion check
                final_piece = *new_board.get_piece(&mv.to).unwrap();
            }
        }

        // Check for promotion: Soldier → Paladin, Ballista → Rook on opposite side
        let promote_piece = Self::check_promotion(&final_piece, &mv.to);
        if let Some(promoted) = promote_piece {
            new_board.set_piece(&mv.to, Some(promoted));
        }

        // Update moves_without_capture counter
        if was_capture {
            new_board.reset_moves_without_capture();
        } else {
            new_board.increment_moves_without_capture();
        }

        new_board.set_white_to_move(!new_board.is_white_to_move()); // Switch turn

        // Check for game over conditions
        Self::check_game_over(&mut new_board);

        Ok(new_board)
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
            let current_color = if self.board.is_white_to_move() { Color::White } else { Color::Black };
            dest_piece.color != current_color
        } else {
            false
        }
    }

    /// Get the material value of the piece at the destination square (if any).
    /// Returns 0 if the square is empty or occupied by a friendly piece.
    pub fn capture_value(&self, mv: &Move) -> u32 {
        if let Some(dest_piece) = self.board.get_piece(&mv.to) {
            let current_color = if self.board.is_white_to_move() { Color::White } else { Color::Black };
            if dest_piece.color != current_color {
                let mut value = Self::piece_material_value(&dest_piece.bottom);
                if let Some(ref top) = dest_piece.top {
                    value += Self::piece_material_value(top);
                }
                return value;
            }
        }
        0
    }

    fn piece_material_value(pt: &PieceType) -> u32 {
        match pt {
            PieceType::Soldier => 100,
            PieceType::Bishop => 300,
            PieceType::Rook => 500,
            PieceType::Paladin => 300,
            PieceType::Guard => 300,
            PieceType::Knight => 300,
            PieceType::Ballista => 500,
            PieceType::King => 10_000,
        }
    }

    pub fn get_moves(&self, position: &Position) -> Vec<PotentialMove> {
        let mut moves = Vec::new();

        let piece = self.board.get_piece(position);
        if piece.is_none() {
            return moves; // No piece at the position, no moves possible
        }
        let piece = piece.unwrap();
        if piece.color != self.board.color_to_move() {
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

    pub fn to_binary(&self) -> [u8; BOARD_SIZE + 2] {
        self.board.to_binary()
    }

    pub fn from_binary(binary: [u8; BOARD_SIZE + 2]) -> Result<Self, String> {
        let board = Board::from_binary(binary)?;
        Ok(Game::from_board(board))
    }

    /// Check if the game has ended and update the board state accordingly
    /// This checks for:
    /// 1. King capture (win condition)
    /// 2. Draw conditions:
    ///    - Both sides have only kings left
    ///    - Color lock: Bishop and/or guards all on the same color for both sides
    ///    - Single knight for both sides
    ///    - 40 moves without capture
    fn check_game_over(board: &mut Board) {
        // Check for king captures
        let mut white_king_exists = false;
        let mut black_king_exists = false;

        // Count pieces for draw detection
        let mut white_pieces = Vec::new();
        let mut black_pieces = Vec::new();

        for y in 0..BOARD_DIMENSION {
            for x in 0..BOARD_DIMENSION {
                let pos = Position::new(x, y);
                if let Some(piece) = board.get_piece(&pos) {
                    if piece.is_king() {
                        if piece.color == Color::White {
                            white_king_exists = true;
                        } else {
                            black_king_exists = true;
                        }
                    }

                    // Collect pieces for draw detection
                    if piece.color == Color::White {
                        white_pieces.push((piece.clone(), pos));
                    } else {
                        black_pieces.push((piece.clone(), pos));
                    }
                }
            }
        }

        // Check for king capture (win condition)
        if !white_king_exists {
            board.set_game_over(true, false, false); // Black wins
            return;
        }
        if !black_king_exists {
            board.set_game_over(true, true, false); // White wins
            return;
        }

        // Check for 40-move rule
        if board.moves_without_capture() >= 40 {
            board.set_game_over(true, false, true); // Draw
            return;
        }

        // Check for draw conditions (both sides must satisfy the condition)
        let white_draw_eligible = Self::check_draw_condition_for_side(&white_pieces);
        let black_draw_eligible = Self::check_draw_condition_for_side(&black_pieces);

        if white_draw_eligible && black_draw_eligible {
            board.set_game_over(true, false, true); // Draw
        }
    }

    /// Check if a side satisfies draw conditions:
    /// - Only king remaining
    /// - Only bishops/guards on same color square (color lock)
    /// - Single knight (plus king)
    fn check_draw_condition_for_side(pieces: &[(Piece, Position)]) -> bool {
        let mut non_king_pieces = Vec::new();

        for (piece, pos) in pieces {
            if !piece.is_king() {
                non_king_pieces.push((piece.clone(), *pos));
            }
        }

        // No pieces apart from King
        if non_king_pieces.is_empty() {
            return true;
        }

        // Single Knight
        if non_king_pieces.len() == 1 {
            let (piece, _) = &non_king_pieces[0];
            // Check if it's a single knight (not stacked)
            if piece.bottom == PieceType::Knight && piece.top.is_none() {
                return true;
            }
        }

        // Check for color lock: all bishops and/or guards on same color square
        let mut all_bishops_or_guards = true;
        let mut first_square_color: Option<bool> = None; // true for white square, false for black square

        for (piece, pos) in &non_king_pieces {
            // Check if piece is a bishop or guard (consider both bottom and top)
            let is_bishop_or_guard = piece.bottom == PieceType::Bishop
                || piece.bottom == PieceType::Guard
                || piece.top == Some(PieceType::Bishop)
                || piece.top == Some(PieceType::Guard);

            if !is_bishop_or_guard {
                all_bishops_or_guards = false;
                break;
            }

            // Calculate square color (white if (x + y) is even, black if odd)
            let square_is_white = (pos.x + pos.y) % 2 == 0;

            match first_square_color {
                None => first_square_color = Some(square_is_white),
                Some(color) => {
                    if color != square_is_white {
                        all_bishops_or_guards = false;
                        break;
                    }
                }
            }
        }

        if all_bishops_or_guards && first_square_color.is_some() {
            return true;
        }

        false
    }

    /// Check if a piece needs to be promoted when it reaches the opposite side
    /// Soldier → Paladin, Ballista → Rook
    /// Returns Some(promoted_piece) if promotion is needed, None otherwise
    fn check_promotion(piece: &Piece, position: &Position) -> Option<Piece> {
        // Check if the piece reached the opposite side
        let reached_opposite_side = match piece.color {
            Color::White => position.y == 0, // White pieces start at bottom (y=8) and move up (y=0)
            Color::Black => position.y == 8, // Black pieces start at top (y=0) and move down (y=8)
        };

        if !reached_opposite_side {
            return None;
        }

        // Helper function to promote a piece type if applicable
        let promote_piece_type = |piece_type: PieceType| -> PieceType {
            match piece_type {
                PieceType::Soldier => PieceType::Paladin,
                PieceType::Ballista => PieceType::Rook,
                _ => piece_type, // No promotion for other pieces
            }
        };

        // Check if any promotion is needed
        let bottom_needs_promotion =
            piece.bottom == PieceType::Soldier || piece.bottom == PieceType::Ballista;
        let top_needs_promotion = piece.top.is_some()
            && (piece.top == Some(PieceType::Soldier) || piece.top == Some(PieceType::Ballista));

        if !bottom_needs_promotion && !top_needs_promotion {
            return None;
        }

        // Perform the promotion for both bottom and top pieces
        let promoted_bottom = promote_piece_type(piece.bottom);
        let promoted_top = piece.top.map(promote_piece_type);

        Some(Piece::new(piece.color, promoted_bottom, promoted_top))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soldier_promotion_to_paladin_white() {
        let mut game = Game::new();

        // Place a white soldier at position (4, 1) - near the top
        let soldier_pos = Position::new(4, 1);
        game.board.set_piece(
            &soldier_pos,
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );

        // Move the soldier to (3, 0) - top row (opposite side for white)
        let mv = Move {
            from: soldier_pos,
            to: Position::new(3, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Paladin,
            "Soldier should be promoted to Paladin"
        );
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_soldier_promotion_to_paladin_black() {
        let mut game = Game::new();

        // Place a black soldier at position (4, 7) - near the bottom
        let soldier_pos = Position::new(4, 7);
        game.board.set_piece(
            &soldier_pos,
            Some(Piece::new(Color::Black, PieceType::Soldier, None)),
        );

        // Move the soldier to (3, 8) - bottom row (opposite side for black)
        let mv = Move {
            from: soldier_pos,
            to: Position::new(3, 8),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 8));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::Black);
        assert_eq!(
            piece.bottom,
            PieceType::Paladin,
            "Soldier should be promoted to Paladin"
        );
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_ballista_promotion_to_rook_white() {
        let mut game = Game::new();

        // Place a white ballista at position (4, 1) - near the top
        let ballista_pos = Position::new(4, 1);
        game.board.set_piece(
            &ballista_pos,
            Some(Piece::new(Color::White, PieceType::Ballista, None)),
        );

        // Move the ballista to (4, 0) - top row (opposite side for white)
        let mv = Move {
            from: ballista_pos,
            to: Position::new(4, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(4, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Rook,
            "Ballista should be promoted to Rook"
        );
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_ballista_promotion_to_rook_black() {
        let mut game = Game::new();

        // Place a black ballista at position (4, 7) - near the bottom
        let ballista_pos = Position::new(4, 7);
        game.board.set_piece(
            &ballista_pos,
            Some(Piece::new(Color::Black, PieceType::Ballista, None)),
        );

        // Move the ballista to (4, 8) - bottom row (opposite side for black)
        let mv = Move {
            from: ballista_pos,
            to: Position::new(4, 8),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(4, 8));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::Black);
        assert_eq!(
            piece.bottom,
            PieceType::Rook,
            "Ballista should be promoted to Rook"
        );
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_soldier_no_promotion_middle_board() {
        let mut game = Game::new();

        // Place a white soldier at position (4, 5)
        let soldier_pos = Position::new(4, 5);
        game.board.set_piece(
            &soldier_pos,
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );

        // Move the soldier to (3, 4) - not on opposite side
        let mv = Move {
            from: soldier_pos,
            to: Position::new(3, 4),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 4));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Soldier,
            "Soldier should NOT be promoted in middle of board"
        );
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_stacked_soldier_promotion() {
        let mut game = Game::new();

        // Place a stacked piece: Soldier on top of Guard at position (4, 1)
        let stack_pos = Position::new(4, 1);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::White,
                PieceType::Guard,
                Some(PieceType::Soldier),
            )),
        );

        // Unstack and move the soldier to (3, 0) - top row
        let mv = Move {
            from: stack_pos,
            to: Position::new(3, 0),
            unstack: true,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Paladin,
            "Unstacked Soldier should be promoted to Paladin"
        );
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_other_pieces_no_promotion() {
        let mut game = Game::new();

        // Test that other pieces (like Guard) don't get promoted
        let guard_pos = Position::new(4, 1);
        game.board.set_piece(
            &guard_pos,
            Some(Piece::new(Color::White, PieceType::Guard, None)),
        );

        let mv = Move {
            from: guard_pos,
            to: Position::new(3, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Guard,
            "Guard should NOT be promoted"
        );
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_stacked_soldier_on_soldier_promotion_white() {
        let mut game = Game::new();

        // Place a stacked piece: Soldier on top of Soldier at position (4, 1)
        let stack_pos = Position::new(4, 1);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::White,
                PieceType::Soldier,
                Some(PieceType::Soldier),
            )),
        );

        // Move the stack (without unstacking) to (3, 0) - top row
        let mv = Move {
            from: stack_pos,
            to: Position::new(3, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Paladin,
            "Bottom Soldier should be promoted to Paladin"
        );
        assert_eq!(
            piece.top,
            Some(PieceType::Paladin),
            "Top Soldier should be promoted to Paladin"
        );
    }

    #[test]
    fn test_stacked_soldier_on_soldier_promotion_black() {
        let mut game = Game::new();

        // Place a stacked piece: Soldier on top of Soldier at position (4, 7)
        let stack_pos = Position::new(4, 7);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::Black,
                PieceType::Soldier,
                Some(PieceType::Soldier),
            )),
        );

        // Move the stack to (3, 8) - bottom row (opposite side for black)
        let mv = Move {
            from: stack_pos,
            to: Position::new(3, 8),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 8));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::Black);
        assert_eq!(
            piece.bottom,
            PieceType::Paladin,
            "Bottom Soldier should be promoted to Paladin"
        );
        assert_eq!(
            piece.top,
            Some(PieceType::Paladin),
            "Top Soldier should be promoted to Paladin"
        );
    }

    #[test]
    fn test_stacked_ballista_on_ballista_promotion_white() {
        let mut game = Game::new();

        // Place a stacked piece: Ballista on top of Ballista at position (4, 1)
        let stack_pos = Position::new(4, 1);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::White,
                PieceType::Ballista,
                Some(PieceType::Ballista),
            )),
        );

        // Move the stack to (4, 0) - top row
        let mv = Move {
            from: stack_pos,
            to: Position::new(4, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(4, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Rook,
            "Bottom Ballista should be promoted to Rook"
        );
        assert_eq!(
            piece.top,
            Some(PieceType::Rook),
            "Top Ballista should be promoted to Rook"
        );
    }

    #[test]
    fn test_stacked_ballista_on_ballista_promotion_black() {
        let mut game = Game::new();

        // Place a stacked piece: Ballista on top of Ballista at position (4, 7)
        let stack_pos = Position::new(4, 7);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::Black,
                PieceType::Ballista,
                Some(PieceType::Ballista),
            )),
        );

        // Move the stack to (4, 8) - bottom row (opposite side for black)
        let mv = Move {
            from: stack_pos,
            to: Position::new(4, 8),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(4, 8));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::Black);
        assert_eq!(
            piece.bottom,
            PieceType::Rook,
            "Bottom Ballista should be promoted to Rook"
        );
        assert_eq!(
            piece.top,
            Some(PieceType::Rook),
            "Top Ballista should be promoted to Rook"
        );
    }

    #[test]
    fn test_stacked_soldier_on_guard_promotion() {
        let mut game = Game::new();

        // Place a stacked piece: Soldier on top of Guard at position (4, 1)
        let stack_pos = Position::new(4, 1);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::White,
                PieceType::Guard,
                Some(PieceType::Soldier),
            )),
        );

        // Move the stack to (3, 0) - top row
        let mv = Move {
            from: stack_pos,
            to: Position::new(3, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Guard,
            "Guard should NOT be promoted"
        );
        assert_eq!(
            piece.top,
            Some(PieceType::Paladin),
            "Soldier should be promoted to Paladin"
        );
    }

    #[test]
    fn test_stacked_guard_on_soldier_promotion() {
        let mut game = Game::new();

        // Place a stacked piece: Guard on top of Soldier at position (4, 1)
        let stack_pos = Position::new(4, 1);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::White,
                PieceType::Soldier,
                Some(PieceType::Guard),
            )),
        );

        // Move the stack to (3, 0) - top row
        let mv = Move {
            from: stack_pos,
            to: Position::new(3, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(3, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Paladin,
            "Soldier should be promoted to Paladin"
        );
        assert_eq!(
            piece.top,
            Some(PieceType::Guard),
            "Guard should NOT be promoted"
        );
    }

    #[test]
    fn test_stacked_ballista_on_soldier_promotion() {
        let mut game = Game::new();

        // Place a stacked piece: Ballista on top of Soldier at position (4, 1)
        let stack_pos = Position::new(4, 1);
        game.board.set_piece(
            &stack_pos,
            Some(Piece::new(
                Color::White,
                PieceType::Soldier,
                Some(PieceType::Ballista),
            )),
        );

        // Move the stack to (4, 0) - top row
        let mv = Move {
            from: stack_pos,
            to: Position::new(4, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        let piece = new_board.get_piece(&Position::new(4, 0));
        assert!(piece.is_some(), "Piece should be at destination");

        let piece = piece.unwrap();
        assert_eq!(piece.color, Color::White);
        assert_eq!(
            piece.bottom,
            PieceType::Paladin,
            "Soldier should be promoted to Paladin"
        );
        assert_eq!(
            piece.top,
            Some(PieceType::Rook),
            "Ballista should be promoted to Rook"
        );
    }

    #[test]
    fn test_king_capture_white_wins() {
        let mut game = Game::new();

        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                game.board.set_piece(&pos, None);
            }
        }

        // Place white king and a white soldier
        game.board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(3, 1),
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );

        // Place black king at position where white soldier can capture it
        game.board.set_piece(
            &Position::new(4, 0),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );

        // White soldier captures black king
        let mv = Move {
            from: Position::new(3, 1),
            to: Position::new(4, 0),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        assert!(new_board.is_game_over(), "Game should be over");
        assert!(new_board.white_wins(), "White should win");
        assert!(!new_board.is_draw(), "Should not be a draw");
    }

    #[test]
    fn test_king_capture_black_wins() {
        let mut game = Game::new();

        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                game.board.set_piece(&pos, None);
            }
        }

        // Place black king and a black soldier
        game.board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(3, 7),
            Some(Piece::new(Color::Black, PieceType::Soldier, None)),
        );

        // Place white king at position where black soldier can capture it
        game.board.set_piece(
            &Position::new(4, 8),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );

        // Set black to move
        game.board.set_white_to_move(false);

        // Black soldier captures white king
        let mv = Move {
            from: Position::new(3, 7),
            to: Position::new(4, 8),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        assert!(new_board.is_game_over(), "Game should be over");
        assert!(!new_board.white_wins(), "White should not win");
        assert!(!new_board.is_draw(), "Should not be a draw");
    }

    #[test]
    fn test_draw_only_kings_remaining() {
        let mut game = Game::new();

        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                game.board.set_piece(&pos, None);
            }
        }

        // Place only kings
        game.board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(4, 5),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );

        // Place a black soldier that will be captured (in middle of board, not promotion zone)
        game.board.set_piece(
            &Position::new(3, 3),
            Some(Piece::new(Color::Black, PieceType::Soldier, None)),
        );

        // Place a white soldier to capture the black soldier
        game.board.set_piece(
            &Position::new(4, 2),
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );

        // White soldier captures black soldier, leaving only kings and the white soldier
        let mv = Move {
            from: Position::new(4, 2),
            to: Position::new(3, 3),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let board_after_first_move = result.unwrap();
        // Game not over yet - there's still a white soldier
        assert!(
            !board_after_first_move.is_game_over(),
            "Game should not be over yet"
        );

        // Now have black king capture the white soldier
        let game2 = Game::from_board(board_after_first_move);
        let mv2 = Move {
            from: Position::new(4, 5),
            to: Position::new(3, 3),
            unstack: false,
        };

        let result2 = game2.apply_move_copy(mv2);
        assert!(result2.is_ok(), "Move should be valid");

        let new_board = result2.unwrap();
        assert!(new_board.is_game_over(), "Game should be over");
        assert!(new_board.is_draw(), "Should be a draw");
    }

    #[test]
    fn test_draw_single_knight() {
        let mut game = Game::new();

        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                game.board.set_piece(&pos, None);
            }
        }

        // Place kings and single knights
        game.board.set_piece(
            &Position::new(4, 0),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(3, 0),
            Some(Piece::new(Color::White, PieceType::Knight, None)),
        );

        game.board.set_piece(
            &Position::new(4, 8),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(5, 8),
            Some(Piece::new(Color::Black, PieceType::Knight, None)),
        );

        // Place a white soldier to be captured
        game.board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );

        game.board.set_white_to_move(false);

        // Black knight captures soldier
        let mv = Move {
            from: Position::new(5, 8),
            to: Position::new(4, 4),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        assert!(
            new_board.is_game_over(),
            "Game should be over (single knight rule)"
        );
        assert!(new_board.is_draw(), "Should be a draw");
    }

    #[test]
    fn test_draw_40_move_rule() {
        let mut game = Game::new();

        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                game.board.set_piece(&pos, None);
            }
        }

        // Place kings
        game.board.set_piece(
            &Position::new(0, 0),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(8, 8),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );

        // Place some pieces
        game.board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );
        game.board.set_piece(
            &Position::new(5, 5),
            Some(Piece::new(Color::Black, PieceType::Soldier, None)),
        );

        // Simulate 39 moves without capture
        game.board.set_moves_without_capture(39);

        // Make a non-capturing move (40th move)
        let mv = Move {
            from: Position::new(4, 4),
            to: Position::new(5, 3),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        assert!(
            new_board.is_game_over(),
            "Game should be over after 40 moves without capture"
        );
        assert!(new_board.is_draw(), "Should be a draw");
        assert_eq!(new_board.moves_without_capture(), 40);
    }

    #[test]
    fn test_capture_resets_move_counter() {
        let mut game = Game::new();

        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                game.board.set_piece(&pos, None);
            }
        }

        // Place kings
        game.board.set_piece(
            &Position::new(0, 0),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(8, 8),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );

        // Place pieces
        game.board.set_piece(
            &Position::new(4, 5),
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );
        game.board.set_piece(
            &Position::new(5, 4),
            Some(Piece::new(Color::Black, PieceType::Soldier, None)),
        );

        // Set counter to some value
        game.board.set_moves_without_capture(15);

        // Make a capturing move
        let mv = Move {
            from: Position::new(4, 5),
            to: Position::new(5, 4),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        assert_eq!(
            new_board.moves_without_capture(),
            0,
            "Counter should be reset after capture"
        );
    }

    #[test]
    fn test_cannot_move_when_game_over() {
        let mut game = Game::new();

        // Set up a game over state
        game.board.set_game_over(true, true, false);

        // Try to make a move
        let mv = Move {
            from: Position::new(0, 6),
            to: Position::new(1, 5),
            unstack: false,
        };

        let result = game.apply_move(mv);
        assert!(
            result.is_err(),
            "Should not be able to move when game is over"
        );
        assert!(result.unwrap_err().contains("Game is over"));
    }

    #[test]
    fn test_binary_encoding_with_game_state() {
        let mut game = Game::new();

        // Set game state
        game.board.set_game_over(true, false, true);
        game.board.set_moves_without_capture(25);

        // Encode and decode
        let binary = game.to_binary();
        let decoded_game = Game::from_binary(binary).unwrap();

        assert_eq!(decoded_game.board.is_game_over(), true);
        assert_eq!(decoded_game.board.white_wins(), false);
        assert_eq!(decoded_game.board.is_draw(), true);
        assert_eq!(decoded_game.board.moves_without_capture(), 25);
    }

    #[test]
    fn test_color_lock_draw() {
        let mut game = Game::new();

        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                game.board.set_piece(&pos, None);
            }
        }

        // Place kings
        game.board.set_piece(
            &Position::new(4, 4),
            Some(Piece::new(Color::White, PieceType::King, None)),
        );
        game.board.set_piece(
            &Position::new(4, 5),
            Some(Piece::new(Color::Black, PieceType::King, None)),
        );

        // Place white bishops on white squares (color lock)
        // White squares: (x + y) % 2 == 0
        game.board.set_piece(
            &Position::new(0, 0),
            Some(Piece::new(Color::White, PieceType::Bishop, None)),
        );
        game.board.set_piece(
            &Position::new(2, 0),
            Some(Piece::new(Color::White, PieceType::Bishop, None)),
        );

        // Place black guards on black squares (color lock)
        // Black squares: (x + y) % 2 == 1
        game.board.set_piece(
            &Position::new(0, 1),
            Some(Piece::new(Color::Black, PieceType::Guard, None)),
        );
        game.board.set_piece(
            &Position::new(2, 1),
            Some(Piece::new(Color::Black, PieceType::Guard, None)),
        );

        // Place a white soldier on a black square to be captured
        // Black square: (1 + 2) = 3 (odd)
        game.board.set_piece(
            &Position::new(1, 2),
            Some(Piece::new(Color::White, PieceType::Soldier, None)),
        );

        game.board.set_white_to_move(false);

        // Black guard captures white soldier, staying on a black square and triggering color lock check
        let mv = Move {
            from: Position::new(0, 1),
            to: Position::new(1, 2),
            unstack: false,
        };

        let result = game.apply_move_copy(mv);

        let result = game.apply_move_copy(mv);
        assert!(result.is_ok(), "Move should be valid");

        let new_board = result.unwrap();
        assert!(new_board.is_game_over(), "Game should be over (color lock)");
        assert!(new_board.is_draw(), "Should be a draw");
    }
}
