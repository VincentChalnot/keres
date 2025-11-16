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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
        
        if destination_piece_opt.is_none() {
            // Empty square: just place the piece
            new_board.set_piece(&mv.to, Some(final_piece));
        } else {
            let destination_piece = destination_piece_opt.unwrap();
            
            if destination_piece.color != source_piece.color {
                // Enemy piece: capture it (replace with our piece)
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
        
        // Check for promotion: Soldier → Paladin, Ballista → Commander on opposite side
        let promote_piece = Self::check_promotion(&final_piece, &mv.to);
        if let Some(promoted) = promote_piece {
            new_board.set_piece(&mv.to, Some(promoted));
        }
        
        new_board.set_white_to_move(!new_board.is_white_to_move()); // Switch turn

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
            PieceType::Soldier => self.compute_soldier_moves(position, color, is_top, has_top, &mut moves),
            PieceType::Jester => self.compute_generic_moves(position, color, is_top, has_top, &mut moves, &Position::DIAGONAL_MOVES, BOARD_DIMENSION as isize),
            PieceType::Commander => self.compute_generic_moves(position, color, is_top, has_top, &mut moves, &Position::ORTHOGONAL_MOVES, BOARD_DIMENSION as isize),
            PieceType::Paladin => self.compute_generic_moves(position, color, is_top, has_top, &mut moves, &Position::ORTHOGONAL_MOVES, 2),
            PieceType::Guard => self.compute_generic_moves(position, color, is_top, has_top, &mut moves, &Position::DIAGONAL_MOVES, 2),
            PieceType::Dragon => self.compute_dragon_moves(position, color, is_top, has_top, &mut moves),
            PieceType::Ballista => self.compute_ballista_moves(position, color, is_top, has_top, &mut moves),
            PieceType::King => self.compute_generic_moves(position, color, false, true, &mut moves, &Position::ALL_MOVES, 1), // King cannot be stacked so we do this trick with is_top and has_top
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

    pub fn to_binary(&self) -> [u8; BOARD_SIZE + 1] {
        self.board.to_binary()
    }

    pub fn from_binary(binary: [u8; BOARD_SIZE + 1]) -> Result<Self, String> {
        let board = Board::from_binary(binary)?;
        Ok(Game::from_board(board))
    }

    /// Check if a piece needs to be promoted when it reaches the opposite side
    /// Soldier → Paladin, Ballista → Commander
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
        
        // Check if promotion is needed based on piece type
        let needs_promotion = if piece.top.is_some() {
            // For stacked pieces, check if the top piece needs promotion
            piece.top == Some(PieceType::Soldier) || piece.top == Some(PieceType::Ballista)
        } else {
            // For single pieces, check if the bottom piece needs promotion
            piece.bottom == PieceType::Soldier || piece.bottom == PieceType::Ballista
        };
        
        if !needs_promotion {
            return None;
        }
        
        // Perform the promotion
        if piece.top.is_some() {
            // Promote the top piece
            let promoted_top = match piece.top.unwrap() {
                PieceType::Soldier => PieceType::Paladin,
                PieceType::Ballista => PieceType::Commander,
                _ => return None, // Should not happen based on needs_promotion check
            };
            Some(Piece::new(piece.color, piece.bottom, Some(promoted_top)))
        } else {
            // Promote the single piece (bottom)
            let promoted_bottom = match piece.bottom {
                PieceType::Soldier => PieceType::Paladin,
                PieceType::Ballista => PieceType::Commander,
                _ => return None, // Should not happen based on needs_promotion check
            };
            Some(Piece::new(piece.color, promoted_bottom, None))
        }
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
        self.compute_generic_moves(position, color, is_top, has_top, moves, &directions, BOARD_DIMENSION as isize);
    }

    fn compute_dragon_moves(
        &self,
        position: &Position,
        color: Color,
        is_top: bool,
        has_top: bool,
        moves: &mut Vec<PotentialMove>,
    ) {
        // Dragon move like a knight in chess
        let directions = [
            (2, 1), (2, -1), (-2, 1), (-2, -1),
            (1, 2), (1, -2), (-1, 2), (-1, -2),
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
        game.board.set_piece(&soldier_pos, Some(Piece::new(Color::White, PieceType::Soldier, None)));
        
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
        assert_eq!(piece.bottom, PieceType::Paladin, "Soldier should be promoted to Paladin");
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_soldier_promotion_to_paladin_black() {
        let mut game = Game::new();
        
        // Place a black soldier at position (4, 7) - near the bottom
        let soldier_pos = Position::new(4, 7);
        game.board.set_piece(&soldier_pos, Some(Piece::new(Color::Black, PieceType::Soldier, None)));
        
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
        assert_eq!(piece.bottom, PieceType::Paladin, "Soldier should be promoted to Paladin");
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_ballista_promotion_to_commander_white() {
        let mut game = Game::new();
        
        // Place a white ballista at position (4, 1) - near the top
        let ballista_pos = Position::new(4, 1);
        game.board.set_piece(&ballista_pos, Some(Piece::new(Color::White, PieceType::Ballista, None)));
        
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
        assert_eq!(piece.bottom, PieceType::Commander, "Ballista should be promoted to Commander");
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_ballista_promotion_to_commander_black() {
        let mut game = Game::new();
        
        // Place a black ballista at position (4, 7) - near the bottom
        let ballista_pos = Position::new(4, 7);
        game.board.set_piece(&ballista_pos, Some(Piece::new(Color::Black, PieceType::Ballista, None)));
        
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
        assert_eq!(piece.bottom, PieceType::Commander, "Ballista should be promoted to Commander");
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_soldier_no_promotion_middle_board() {
        let mut game = Game::new();
        
        // Place a white soldier at position (4, 5)
        let soldier_pos = Position::new(4, 5);
        game.board.set_piece(&soldier_pos, Some(Piece::new(Color::White, PieceType::Soldier, None)));
        
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
        assert_eq!(piece.bottom, PieceType::Soldier, "Soldier should NOT be promoted in middle of board");
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_stacked_soldier_promotion() {
        let mut game = Game::new();
        
        // Place a stacked piece: Soldier on top of Guard at position (4, 1)
        let stack_pos = Position::new(4, 1);
        game.board.set_piece(&stack_pos, Some(Piece::new(Color::White, PieceType::Guard, Some(PieceType::Soldier))));
        
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
        assert_eq!(piece.bottom, PieceType::Paladin, "Unstacked Soldier should be promoted to Paladin");
        assert_eq!(piece.top, None);
    }

    #[test]
    fn test_other_pieces_no_promotion() {
        let mut game = Game::new();
        
        // Test that other pieces (like Guard) don't get promoted
        let guard_pos = Position::new(4, 1);
        game.board.set_piece(&guard_pos, Some(Piece::new(Color::White, PieceType::Guard, None)));
        
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
        assert_eq!(piece.bottom, PieceType::Guard, "Guard should NOT be promoted");
        assert_eq!(piece.top, None);
    }
}
