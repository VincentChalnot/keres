pub const BOARD_DIMENSION: usize = 9; // 9x9 board
pub const BOARD_SIZE: usize = BOARD_DIMENSION * BOARD_DIMENSION; // Total number of squares

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: usize, // 0-8 for columns
    pub y: usize, // 0-8 for rows
}

impl Position {
    pub const ORTHOGONAL_MOVES: [(isize, isize); 4] = [
        (1, 0),  // Right
        (0, 1),  // Down
        (-1, 0), // Left
        (0, -1), // Up
    ];

    pub const DIAGONAL_MOVES: [(isize, isize); 4] = [
        (1, 1),   // Down-Right
        (1, -1),  // Up-Right
        (-1, -1), // Up-Left
        (-1, 1),  // Down-Left
    ];

    pub const ALL_MOVES: [(isize, isize); 8] = [
        (1, 0),   // Right
        (0, 1),   // Down
        (-1, 0),  // Left
        (0, -1),  // Up
        (1, 1),   // Down-Right
        (1, -1),  // Up-Right
        (-1, -1), // Up-Left
        (-1, 1),  // Down-Left
    ];

    pub fn new(x: usize, y: usize) -> Self {
        if x >= BOARD_DIMENSION || y >= BOARD_DIMENSION {
            panic!("Position coordinates must be between 0 and 8 inclusive.");
        }
        Position { x, y }
    }

    pub fn validate(x: isize, y: isize) -> bool {
        x >= 0 && x < BOARD_DIMENSION as isize && y >= 0 && y < BOARD_DIMENSION as isize
    }

    pub fn to_absolute(&self) -> usize {
        self.y * BOARD_DIMENSION + self.x
    }

    pub fn to_u8(&self) -> u8 {
        // Number of the case in the board, from 0 to 80
        self.to_absolute() as u8
    }

    pub fn from_u8(value: u8) -> Self {
        let x = value as usize % BOARD_DIMENSION; // Column (0-8)
        let y = value as usize / BOARD_DIMENSION; // Row (0-8)

        Position::new(x, y)
    }

    pub fn get_new(&self, dx: isize, dy: isize) -> Option<Self> {
        let new_x = self.x as isize + dx;
        let new_y = self.y as isize + dy;

        if !Self::validate(new_x, new_y) {
            return None; // Out of bounds
        }

        Some(Position::new(new_x as usize, new_y as usize))
    }

    pub fn to_string(&self) -> String {
        format!("{}{}", (b'A' + self.x as u8) as char, 9 - self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceType {
    Soldier = 0b001,   // 1
    Bishop = 0b010,    // 2
    Rook = 0b011, // 3
    Paladin = 0b100,   // 4
    Guard = 0b101,     // 5
    Knight = 0b110,    // 6
    Ballista = 0b111,  // 7
    King,              // Handled specially, its discriminant (8) is not used in 3-bit piece codes
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub color: Color,
    pub bottom: PieceType,      // Base piece, always present
    pub top: Option<PieceType>, // Optional top piece
}

impl Piece {
    pub fn new(color: Color, bottom: PieceType, top: Option<PieceType>) -> Self {
        if bottom == PieceType::King && top.is_some() {
            panic!("Invalid piece configuration: King cannot have a piece on top of it.");
        }
        Piece { color, bottom, top }
    }

    pub fn is_stackable(&self) -> bool {
        // A piece is stackable if it has no top piece
        !self.is_king() && !self.is_stacked()
    }

    pub fn is_stacked(&self) -> bool {
        self.top.is_some()
    }

    pub fn is_king(&self) -> bool {
        self.bottom == PieceType::King
    }

    pub fn to_u8(&self) -> u8 {
        let color_bit = match self.color {
            Color::White => 0b1000000,
            Color::Black => 0b0000000,
        };

        if self.bottom == PieceType::King {
            return color_bit | 0b0111000; // Special King encoding: C_111000
        }

        let bottom_code = self.bottom as u8; // This is LLL

        match self.top {
            Some(top_type) => {
                // Stacked piece: C UUU LLL
                if top_type == PieceType::King {
                    panic!("Invalid piece configuration: King cannot be the top piece of a regular stack (it has a special encoding).");
                }
                let top_code = top_type as u8; // This is UUU
                color_bit | (top_code << 3) | bottom_code
            }
            None => {
                // Single piece (bottom piece is the actual piece type): C 000 LLL
                color_bit | bottom_code // UUU is implicitly 000
            }
        }
    }

    pub fn from_u8(value: u8) -> Option<Piece> {
        if value == 0b0000000 {
            // Empty case
            return None;
        }

        let color = if (value >> 6) == 1 {
            Color::White
        } else {
            Color::Black
        };
        let payload = value & 0b00111111; // Lower 6 bits for piece data

        if payload == 0b0111000 {
            // Check for King: C_111000
            return Some(Piece {
                color,
                bottom: PieceType::King,
                top: None, // King is always single in its encoding form
            });
        }

        let uuu = (payload >> 3) & 0b111; // Potential top piece code
        let lll = payload & 0b111; // Bottom/single piece code

        // LLL must be a valid piece code (001-111) because bottom piece is always present
        // and 000 is not a valid piece type code for LLL (unless it's King's payload).
        if lll == 0b000 {
            panic!(
                "Invalid piece encoding: LLL (bottom piece code) is 0b000 but not part of King's special payload. Value: 0b{:07b}",
                value
            );
        }
        // This also covers the instruction: "0bUUU000 where UUU is 0b001 through 0b110" is invalid.

        let bottom_piece_type = Self::code_to_piece_type(lll).unwrap_or_else(|| {
            panic!( // Should be caught by lll == 0b000 check if code_to_piece_type doesn't handle 000
                "Invalid piece encoding: bottom piece type code (LLL) 0b{:03b} is invalid for value 0b{:07b}",
                lll, value
            )
        });

        if uuu == 0b000 {
            // Single piece: C 000 LLL.
            Some(Piece {
                color,
                bottom: bottom_piece_type,
                top: None,
            })
        } else {
            // Stacked piece: C UUU LLL
            // UUU must be a valid piece code (001-111).
            let top_piece_type = Self::code_to_piece_type(uuu).unwrap_or_else(|| {
                panic!(
                    "Invalid piece encoding: top piece type code (UUU) 0b{:03b} is invalid for value 0b{:07b}",
                    uuu, value
                )
            });

            // King cannot be part of a regular stack (already checked for bottom_piece_type == King via special payload)
            if top_piece_type == PieceType::King {
                panic!("Invalid stack: King cannot be the top piece in a regular stack configuration. Value: 0b{:07b}", value);
            }

            Some(Piece {
                color,
                bottom: bottom_piece_type,
                top: Some(top_piece_type),
            })
        }
    }

    // Helper to convert 3-bit code to PieceType (excluding King)
    fn code_to_piece_type(code: u8) -> Option<PieceType> {
        match code {
            0b001 => Some(PieceType::Soldier),
            0b010 => Some(PieceType::Bishop),
            0b011 => Some(PieceType::Rook),
            0b100 => Some(PieceType::Paladin),
            0b101 => Some(PieceType::Guard),
            0b110 => Some(PieceType::Knight),
            0b111 => Some(PieceType::Ballista),
            _ => None, // Covers 0b000 and any other invalid 3-bit patterns for non-King pieces
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    data: [Option<Piece>; BOARD_SIZE], // each cell is an optional piece
    white_to_move: bool,               // true if it's white's turn to move
    game_over: bool,                   // true if the game has ended
    white_wins: bool,                  // true if white won (only meaningful if game_over)
    draw: bool,                        // true if the game ended in a draw (only meaningful if game_over)
    moves_without_capture: u8,         // counter for moves without capture (for 40-move draw rule)
}

impl Board {
    pub fn new() -> Self {
        let mut data = [None; BOARD_SIZE]; // Initialize all to empty

        // Single array for initial black piece setup: [row][col]
        const HALF_BOARD_SETUP: [Option<PieceType>; 27] = [
            // Row 0
            Some(PieceType::Ballista),
            Some(PieceType::Knight),
            Some(PieceType::Paladin),
            Some(PieceType::Guard),
            Some(PieceType::King),
            Some(PieceType::Guard),
            Some(PieceType::Paladin),
            Some(PieceType::Knight),
            Some(PieceType::Ballista),
            // Row 1
            None,
            None,
            Some(PieceType::Rook),
            None,
            None,
            None,
            Some(PieceType::Bishop),
            None,
            None,
            // Row 2
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
            Some(PieceType::Soldier),
        ];

        for (absolute_position, piece_type) in HALF_BOARD_SETUP.iter().enumerate() {
            if piece_type.is_none() {
                continue;
            }
            let position = Position::from_u8(absolute_position as u8);
            data[position.to_absolute()] = Some(Piece {
                color: Color::Black,
                bottom: piece_type.unwrap(),
                top: None,
            });
            data[BOARD_SIZE - position.to_absolute() - 1] = Some(Piece {
                color: Color::White,
                bottom: piece_type.unwrap(),
                top: None,
            });
        }

        Board {
            data,
            white_to_move: true,
            game_over: false,
            white_wins: false,
            draw: false,
            moves_without_capture: 0,
        }
    }

    pub fn is_white_to_move(&self) -> bool {
        self.white_to_move
    }

    pub fn set_white_to_move(&mut self, white_to_move: bool) {
        self.white_to_move = white_to_move;
    }

    pub fn color_to_move(&self) -> Color {
        if self.white_to_move {
            Color::White
        } else {
            Color::Black
        }
    }

    pub fn is_game_over(&self) -> bool {
        self.game_over
    }

    pub fn is_draw(&self) -> bool {
        self.draw
    }

    pub fn white_wins(&self) -> bool {
        self.white_wins
    }

    pub fn moves_without_capture(&self) -> u8 {
        self.moves_without_capture
    }

    pub fn set_game_over(&mut self, game_over: bool, white_wins: bool, draw: bool) {
        self.game_over = game_over;
        self.white_wins = white_wins;
        self.draw = draw;
    }

    pub fn increment_moves_without_capture(&mut self) {
        self.moves_without_capture = self.moves_without_capture.saturating_add(1);
    }

    pub fn reset_moves_without_capture(&mut self) {
        self.moves_without_capture = 0;
    }

    pub fn set_moves_without_capture(&mut self, count: u8) {
        self.moves_without_capture = count;
    }

    pub fn get_piece(&self, position: &Position) -> Option<&Piece> {
        self.data[position.to_absolute()].as_ref()
    }

    pub fn set_piece(&mut self, position: &Position, piece: Option<Piece>) {
        self.data[position.to_absolute()] = piece;
    }

    pub fn unstack_piece(&mut self, position: &Position) -> Result<Piece, String> {
        let piece = self.get_piece(position);
        if piece.is_none() {
            return Err("No piece at the specified position".to_string());
        }
        let piece = piece.unwrap();
        if piece.top.is_none() {
            return Err("No top piece to unstack".to_string());
        }
        let bottom_piece = Piece {
            color: piece.color,
            bottom: piece.bottom, // The bottom remains the same
            top: None,            // After unstacking, the top is now None
        };

        let new_piece = Piece {
            color: piece.color,
            bottom: piece.top.unwrap(), // The top piece becomes the new bottom
            top: None,                  // After unstacking, the top is now None
        };

        self.set_piece(position, Some(bottom_piece));

        Ok(new_piece) // Return the top piece that was unstacked
    }

    /// Stack a moving piece onto an existing piece at the given position
    /// Returns an error if stacking is not allowed
    pub fn stack_piece(&mut self, position: &Position, moving_piece: Piece) -> Result<(), String> {
        let existing_piece = self.get_piece(position);
        if existing_piece.is_none() {
            return Err("No piece at position to stack onto".to_string());
        }
        let existing_piece = existing_piece.unwrap();

        // Check if stacking is allowed
        if !existing_piece.is_stackable() {
            return Err("Cannot stack onto this piece (King or already stacked)".to_string());
        }

        // Check if pieces are same color
        if existing_piece.color != moving_piece.color {
            return Err("Cannot stack pieces of different colors".to_string());
        }

        // Check if moving piece is a single piece (not already stacked)
        if moving_piece.top.is_some() {
            return Err("Cannot stack an already stacked piece".to_string());
        }

        // Create new stacked piece: moving piece goes on top, existing piece becomes bottom
        let stacked_piece = Piece {
            color: existing_piece.color,
            bottom: existing_piece.bottom,
            top: Some(moving_piece.bottom),
        };

        self.set_piece(position, Some(stacked_piece));
        Ok(())
    }

    pub fn to_binary(&self) -> [u8; BOARD_SIZE + 2] {
        let mut binary = [0; BOARD_SIZE + 2];
        for (i, piece_opt) in self.data.iter().enumerate() {
            if let Some(piece) = piece_opt {
                binary[i] = piece.to_u8();
            }
        }
        // Pack all boolean flags into the last byte:
        // bit 8: white_to_move
        // bit 7: game_over
        // bit 6: white_wins
        // bit 5: draw
        // bits 4-1: unused
        let mut flags = 0u8;
        if self.white_to_move {
            flags |= 0b10000000; // bit 8
        }
        if self.game_over {
            flags |= 0b01000000; // bit 7
        }
        if self.white_wins {
            flags |= 0b00100000; // bit 6
        }
        if self.draw {
            flags |= 0b00010000; // bit 5
        }
        binary[BOARD_SIZE] = flags;
        
        // Add the moves_without_capture counter as the last byte
        binary[BOARD_SIZE + 1] = self.moves_without_capture;

        binary
    }

    pub fn from_binary(binary: [u8; BOARD_SIZE + 2]) -> Result<Self, String> {
        let mut data = [None; BOARD_SIZE];

        for (i, &byte) in binary.iter().enumerate() {
            if i >= BOARD_SIZE {
                // The last two bytes are for flags and counter
                break;
            }
            data[i] = Piece::from_u8(byte);
        }

        // Unpack flags from byte at BOARD_SIZE
        let flags = binary[BOARD_SIZE];
        let white_to_move = (flags & 0b10000000) != 0;
        let game_over = (flags & 0b01000000) != 0;
        let white_wins = (flags & 0b00100000) != 0;
        let draw = (flags & 0b00010000) != 0;
        
        // Get moves_without_capture counter from last byte
        let moves_without_capture = binary[BOARD_SIZE + 1];

        Ok(Board {
            data,
            white_to_move,
            game_over,
            white_wins,
            draw,
            moves_without_capture,
        })
    }
}

#[derive(Clone, Debug)]
pub struct UndoInfo {
    pub from_piece: Option<Piece>,
    pub to_piece: Option<Piece>,
    pub was_white_to_move: bool,
    pub prev_moves_without_capture: u8,
    pub was_game_over: bool,
    pub was_white_wins: bool,
    pub was_draw: bool,
}

fn check_promotion(piece: &Piece, position: &Position) -> Option<Piece> {
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

fn check_draw_condition_for_side(pieces: &[(Piece, Position)]) -> bool {
    let mut non_king_pieces = Vec::new();
    for (piece, pos) in pieces {
        if !piece.is_king() {
            non_king_pieces.push((*piece, *pos));
        }
    }
    if non_king_pieces.is_empty() {
        return true;
    }
    if non_king_pieces.len() == 1 {
        let (piece, _) = &non_king_pieces[0];
        if piece.bottom == PieceType::Knight && piece.top.is_none() {
            return true;
        }
    }
    let mut all_bishops_or_guards = true;
    let mut first_square_color: Option<bool> = None;
    for (piece, pos) in &non_king_pieces {
        let is_bishop_or_guard = piece.bottom == PieceType::Bishop
            || piece.bottom == PieceType::Guard
            || piece.top == Some(PieceType::Bishop)
            || piece.top == Some(PieceType::Guard);
        if !is_bishop_or_guard {
            all_bishops_or_guards = false;
            break;
        }
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

fn check_game_over(board: &mut Board) {
    let mut white_king_exists = false;
    let mut black_king_exists = false;
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
                if piece.color == Color::White {
                    white_pieces.push((*piece, pos));
                } else {
                    black_pieces.push((*piece, pos));
                }
            }
        }
    }
    if !white_king_exists {
        board.set_game_over(true, false, false);
        return;
    }
    if !black_king_exists {
        board.set_game_over(true, true, false);
        return;
    }
    if board.moves_without_capture() >= 40 {
        board.set_game_over(true, false, true);
        return;
    }
    let white_draw = check_draw_condition_for_side(&white_pieces);
    let black_draw = check_draw_condition_for_side(&black_pieces);
    if white_draw && black_draw {
        board.set_game_over(true, false, true);
    }
}

impl Board {
    /// Apply a move in-place and return undo information.
    /// Assumes the move is valid (generated by the move generator).
    pub fn make(&mut self, mv: &crate::game::Move) -> UndoInfo {
        let undo = UndoInfo {
            from_piece: self.data[mv.from.to_absolute()],
            to_piece: self.data[mv.to.to_absolute()],
            was_white_to_move: self.white_to_move,
            prev_moves_without_capture: self.moves_without_capture,
            was_game_over: self.game_over,
            was_white_wins: self.white_wins,
            was_draw: self.draw,
        };

        let piece = self.data[mv.from.to_absolute()].unwrap();

        let source_piece: Piece;
        if mv.unstack {
            // Remove top piece from source; top becomes the moving piece
            source_piece = Piece {
                color: piece.color,
                bottom: piece.top.unwrap(),
                top: None,
            };
            self.data[mv.from.to_absolute()] = Some(Piece {
                color: piece.color,
                bottom: piece.bottom,
                top: None,
            });
        } else {
            source_piece = piece;
            self.data[mv.from.to_absolute()] = None;
        }

        let dest = self.data[mv.to.to_absolute()];
        let mut final_piece = source_piece;
        let mut was_capture = false;

        match dest {
            None => {
                self.data[mv.to.to_absolute()] = Some(final_piece);
            }
            Some(dest_piece) => {
                if dest_piece.color != source_piece.color {
                    was_capture = true;
                    self.data[mv.to.to_absolute()] = Some(final_piece);
                } else {
                    // Stack: moving piece on top, existing piece on bottom
                    self.stack_piece(&mv.to, source_piece).unwrap();
                    final_piece = self.data[mv.to.to_absolute()].unwrap();
                }
            }
        }

        if let Some(promoted) = check_promotion(&final_piece, &mv.to) {
            self.data[mv.to.to_absolute()] = Some(promoted);
        }

        if was_capture {
            self.moves_without_capture = 0;
        } else {
            self.moves_without_capture = self.moves_without_capture.saturating_add(1);
        }

        self.white_to_move = !self.white_to_move;
        check_game_over(self);

        undo
    }

    /// Undo a move by restoring the saved state.
    pub fn unmake(&mut self, mv: &crate::game::Move, undo: UndoInfo) {
        self.data[mv.from.to_absolute()] = undo.from_piece;
        self.data[mv.to.to_absolute()] = undo.to_piece;
        self.white_to_move = undo.was_white_to_move;
        self.moves_without_capture = undo.prev_moves_without_capture;
        self.game_over = undo.was_game_over;
        self.white_wins = undo.was_white_wins;
        self.draw = undo.was_draw;
    }
}

#[cfg(test)]
mod make_unmake_tests {
    use super::*;
    use crate::game::Move;

    fn empty_board(white_to_move: bool) -> Board {
        Board {
            data: [None; BOARD_SIZE],
            white_to_move,
            game_over: false,
            white_wins: false,
            draw: false,
            moves_without_capture: 0,
        }
    }

    #[test]
    fn make_unmake_roundtrip_restores_state() {
        let mut board = Board::new();
        let original = board;

        // White soldier at (0, 6) moves to (0, 5) — simple pawn push
        let mv = Move {
            from: Position::new(0, 6),
            to: Position::new(0, 5),
            unstack: false,
        };

        let undo = board.make(&mv);
        // Board should have changed
        assert_ne!(board, original);
        // Unmake should restore it exactly
        board.unmake(&mv, undo);
        assert_eq!(board, original);
    }

    #[test]
    fn make_capture_and_unmake() {
        let mut board = empty_board(true);
        // Place white soldier at (4, 4), black soldier at (4, 3)
        let white_soldier = Piece::new(Color::White, PieceType::Soldier, None);
        let black_soldier = Piece::new(Color::Black, PieceType::Soldier, None);
        // Place kings so game-over check doesn't trigger
        let white_king = Piece::new(Color::White, PieceType::King, None);
        let black_king = Piece::new(Color::Black, PieceType::King, None);
        board.set_piece(&Position::new(0, 8), Some(white_king));
        board.set_piece(&Position::new(0, 0), Some(black_king));
        board.set_piece(&Position::new(4, 4), Some(white_soldier));
        board.set_piece(&Position::new(4, 3), Some(black_soldier));
        let original = board;

        let mv = Move {
            from: Position::new(4, 4),
            to: Position::new(4, 3),
            unstack: false,
        };

        let undo = board.make(&mv);
        // After capture, white soldier should be at (4, 3), nothing at (4, 4)
        assert!(board.get_piece(&Position::new(4, 4)).is_none());
        assert_eq!(
            board.get_piece(&Position::new(4, 3)).unwrap().color,
            Color::White
        );
        assert_eq!(board.moves_without_capture(), 0);

        board.unmake(&mv, undo);
        assert_eq!(board, original);
    }

    #[test]
    fn make_stacking_and_unmake() {
        let mut board = empty_board(true);
        let white_soldier = Piece::new(Color::White, PieceType::Soldier, None);
        let white_guard = Piece::new(Color::White, PieceType::Guard, None);
        let white_king = Piece::new(Color::White, PieceType::King, None);
        let black_king = Piece::new(Color::Black, PieceType::King, None);
        board.set_piece(&Position::new(0, 8), Some(white_king));
        board.set_piece(&Position::new(0, 0), Some(black_king));
        board.set_piece(&Position::new(3, 4), Some(white_soldier));
        board.set_piece(&Position::new(3, 3), Some(white_guard));
        let original = board;

        let mv = Move {
            from: Position::new(3, 4),
            to: Position::new(3, 3),
            unstack: false,
        };

        let undo = board.make(&mv);
        // After stacking, (3, 4) should be empty and (3, 3) should have a stacked piece
        assert!(board.get_piece(&Position::new(3, 4)).is_none());
        let stacked = board.get_piece(&Position::new(3, 3)).unwrap();
        assert_eq!(stacked.bottom, PieceType::Guard);
        assert_eq!(stacked.top, Some(PieceType::Soldier));

        board.unmake(&mv, undo);
        assert_eq!(board, original);
    }
}
