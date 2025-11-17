pub const BOARD_DIMENSION: usize = 9; // 9x9 board
pub const BOARD_SIZE: usize = BOARD_DIMENSION * BOARD_DIMENSION; // Total number of squares

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    White,
    Black,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    Jester = 0b010,    // 2
    Commander = 0b011, // 3
    Paladin = 0b100,   // 4
    Guard = 0b101,     // 5
    Dragon = 0b110,    // 6
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
            0b010 => Some(PieceType::Jester),
            0b011 => Some(PieceType::Commander),
            0b100 => Some(PieceType::Paladin),
            0b101 => Some(PieceType::Guard),
            0b110 => Some(PieceType::Dragon),
            0b111 => Some(PieceType::Ballista),
            _ => None, // Covers 0b000 and any other invalid 3-bit patterns for non-King pieces
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    data: [Option<Piece>; BOARD_SIZE], // each cell is an optional piece
    white_to_move: bool,               // true if it's white's turn to move
}

impl Board {
    pub fn new() -> Self {
        let mut data = [None; BOARD_SIZE]; // Initialize all to empty

        // Single array for initial black piece setup: [row][col]
        const HALF_BOARD_SETUP: [Option<PieceType>; 27] = [
            // Row 0
            Some(PieceType::Ballista),
            Some(PieceType::Dragon),
            Some(PieceType::Paladin),
            Some(PieceType::Guard),
            Some(PieceType::King),
            Some(PieceType::Guard),
            Some(PieceType::Paladin),
            Some(PieceType::Dragon),
            Some(PieceType::Ballista),
            // Row 1
            None,
            None,
            Some(PieceType::Commander),
            None,
            None,
            None,
            Some(PieceType::Jester),
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
        // Scan the board for both kings
        let mut king_found = false;
        for piece_opt in self.data.iter() {
            if let Some(piece) = piece_opt {
                if piece.is_king() {
                    if king_found {
                        return false; // Both kings found, game is not over
                    }
                    king_found = true;
                }
            }
        }
        // If either king is missing, the game is over
        true
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

    pub fn to_binary(&self) -> [u8; BOARD_SIZE + 1] {
        let mut binary = [0; BOARD_SIZE + 1];
        for (i, piece_opt) in self.data.iter().enumerate() {
            if let Some(piece) = piece_opt {
                binary[i] = piece.to_u8();
            }
        }
        // Add a trailing byte to indicate the turn (1 for white, 0 for black)
        binary[BOARD_SIZE] = if self.white_to_move { 1 } else { 0 };

        binary
    }

    pub fn from_binary(binary: [u8; BOARD_SIZE + 1]) -> Result<Self, String> {
        let mut data = [None; BOARD_SIZE];

        for (i, &byte) in binary.iter().enumerate() {
            if i == BOARD_SIZE {
                // The last byte indicates whose turn it is
                continue; // Skip the last byte for piece data
            }
            data[i] = Piece::from_u8(byte);
        }

        Ok(Board {
            data,
            white_to_move: binary[BOARD_SIZE] == 1,
        })
    }
}
