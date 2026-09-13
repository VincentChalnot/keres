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

    /// Build a position from in-range coordinates.
    ///
    /// Panics on out-of-range input: this is the constructor for coordinates
    /// the caller already knows to be valid (literals, loop indices, values
    /// derived from an existing `Position`). Anything decoded from untrusted
    /// bytes must go through [`Position::try_new`] / [`Position::try_from_u8`]
    /// instead — a panic reachable from the wire format is a denial of
    /// service, not a bug report.
    pub fn new(x: usize, y: usize) -> Self {
        Self::try_new(x, y).expect("Position coordinates must be between 0 and 8 inclusive.")
    }

    /// Fallible counterpart of [`Position::new`]: returns `None` when either
    /// coordinate is off the board.
    pub fn try_new(x: usize, y: usize) -> Option<Self> {
        if x >= BOARD_DIMENSION || y >= BOARD_DIMENSION {
            return None;
        }
        Some(Position { x, y })
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

    /// Decode a square index (`0..=80`).
    ///
    /// Panics on `value > 80`; see [`Position::new`] for when that is
    /// acceptable and [`Position::try_from_u8`] for the wire-decoding path.
    pub fn from_u8(value: u8) -> Self {
        Self::try_from_u8(value).expect("Position index must be between 0 and 80 inclusive.")
    }

    /// Fallible counterpart of [`Position::from_u8`]: returns `None` for any
    /// index outside `0..=80`. Every `Position` that originates in a caller-
    /// supplied byte stream must be decoded with this.
    pub fn try_from_u8(value: u8) -> Option<Self> {
        let x = value as usize % BOARD_DIMENSION; // Column (0-8)
        let y = value as usize / BOARD_DIMENSION; // Row (0-8)

        Position::try_new(x, y)
    }

    pub fn get_new(&self, dx: isize, dy: isize) -> Option<Self> {
        let new_x = self.x as isize + dx;
        let new_y = self.y as isize + dy;

        if !Self::validate(new_x, new_y) {
            return None; // Out of bounds
        }

        Some(Position::new(new_x as usize, new_y as usize))
    }
}

/// Algebraic notation: column `A`-`I` left to right, row `9`-`1` top to
/// bottom (so `A1` is the bottom-left square, i.e. `(0, 8)`).
impl std::fmt::Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", (b'A' + self.x as u8) as char, 9 - self.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceType {
    Soldier = 0b001,  // 1
    Bishop = 0b010,   // 2
    Rook = 0b011,     // 3
    Paladin = 0b100,  // 4
    Guard = 0b101,    // 5
    Knight = 0b110,   // 6
    Ballista = 0b111, // 7
    King,             // Handled specially, its discriminant (8) is not used in 3-bit piece codes
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

    /// Decode a square byte, panicking on an invalid encoding.
    ///
    /// Only for bytes known to come from [`Piece::to_u8`]. Anything read off
    /// the wire must use [`Piece::try_from_u8`], which reports the same
    /// conditions as an error instead of aborting the caller.
    pub fn from_u8(value: u8) -> Option<Piece> {
        Self::try_from_u8(value).unwrap_or_else(|e| panic!("{e}"))
    }

    /// Strictly decode a square byte (see `docs/PROTOCOL.md`).
    ///
    /// `Ok(None)` is an empty square. Every bit pattern that the encoder can
    /// never produce is rejected rather than silently reinterpreted:
    ///
    /// - bit 7 set (reserved, always `0` in the format),
    /// - `LLL == 000` outside the king's dedicated `C 111 000` payload
    ///   (a stack has to have a bottom piece).
    pub fn try_from_u8(value: u8) -> Result<Option<Piece>, String> {
        if value == 0b0000_0000 {
            // Empty case
            return Ok(None);
        }

        if value & 0b1000_0000 != 0 {
            return Err(format!(
                "invalid piece encoding 0b{value:08b}: bit 7 is reserved and must be 0"
            ));
        }

        let color = if (value >> 6) == 1 {
            Color::White
        } else {
            Color::Black
        };
        let payload = value & 0b0011_1111; // Lower 6 bits for piece data

        if payload == 0b011_1000 {
            // Check for King: C_111000
            return Ok(Some(Piece {
                color,
                bottom: PieceType::King,
                top: None, // King is always single in its encoding form
            }));
        }

        let uuu = (payload >> 3) & 0b111; // Potential top piece code
        let lll = payload & 0b111; // Bottom/single piece code

        // LLL must be a valid piece code (001-111) because the bottom piece is
        // always present, and 000 is only valid as part of the King payload
        // handled above. This also rejects `0bUUU000` for UUU in 001..=110.
        let bottom = Self::code_to_piece_type(lll).ok_or_else(|| {
            format!(
                "invalid piece encoding 0b{value:08b}: bottom piece code (LLL) 0b{lll:03b} is not a piece"
            )
        })?;

        if uuu == 0b000 {
            // Single piece: C 000 LLL.
            return Ok(Some(Piece {
                color,
                bottom,
                top: None,
            }));
        }

        // Stacked piece: C UUU LLL. UUU must be a valid piece code; the king
        // has no ordinary discriminant, so `code_to_piece_type` can never
        // yield one here — a stacked king is unrepresentable by construction.
        let top = Self::code_to_piece_type(uuu).ok_or_else(|| {
            format!(
                "invalid piece encoding 0b{value:08b}: top piece code (UUU) 0b{uuu:03b} is not a piece"
            )
        })?;

        Ok(Some(Piece {
            color,
            bottom,
            top: Some(top),
        }))
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
    data: [Option<Piece>; BOARD_SIZE],
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

        Board { data }
    }

    pub fn empty() -> Self {
        Board {
            data: [None; BOARD_SIZE],
        }
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

        // Check if moving piece is a single, non-king piece. A king on top of
        // a stack has no representation in the wire format (`Piece::to_u8`
        // would panic), so it must be rejected here rather than produced.
        if moving_piece.is_king() {
            return Err("Cannot stack a King onto another piece".to_string());
        }
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

    pub fn to_binary(&self) -> [u8; BOARD_SIZE] {
        let mut binary = [0; BOARD_SIZE];
        for (i, piece_opt) in self.data.iter().enumerate() {
            if let Some(piece) = piece_opt {
                binary[i] = piece.to_u8();
            }
        }
        binary
    }

    /// Decode 81 square bytes. Every byte is validated strictly (see
    /// [`Piece::try_from_u8`]), so a malformed payload is an error the caller
    /// can turn into a `400`, never a panic.
    pub fn from_binary(binary: &[u8; BOARD_SIZE]) -> Result<Self, String> {
        let mut data = [None; BOARD_SIZE];
        for (i, &byte) in binary.iter().enumerate() {
            data[i] = Piece::try_from_u8(byte).map_err(|e| format!("square {i}: {e}"))?;
        }
        Ok(Board { data })
    }

    /// Reject boards that no sequence of legal moves could ever produce.
    ///
    /// Only structural impossibilities are checked — a caller-supplied board
    /// is otherwise free to be any arbitrary arrangement (the engine is
    /// routinely asked about hand-built study positions). What cannot be
    /// tolerated is more than one king per side: the search treats a king
    /// capture as terminal and the evaluation counts `KING_VALUE` per king,
    /// so duplicated kings produce nonsense scores.
    pub fn validate(&self) -> Result<(), String> {
        let mut white_kings = 0usize;
        let mut black_kings = 0usize;
        for (_, piece) in self.pieces() {
            if piece.is_king() {
                match piece.color {
                    Color::White => white_kings += 1,
                    Color::Black => black_kings += 1,
                }
            }
        }
        if white_kings > 1 {
            return Err(format!(
                "board has {white_kings} white kings, at most 1 is legal"
            ));
        }
        if black_kings > 1 {
            return Err(format!(
                "board has {black_kings} black kings, at most 1 is legal"
            ));
        }
        Ok(())
    }

    /// Iterate over all occupied squares, yielding (Position, &Piece).
    pub fn pieces(&self) -> impl Iterator<Item = (Position, &Piece)> {
        self.data.iter().enumerate().filter_map(|(i, piece_opt)| {
            piece_opt
                .as_ref()
                .map(|piece| (Position::from_u8(i as u8), piece))
        })
    }
}

impl Default for Board {
    /// The standard starting position (see [`Board::new`]).
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `PieceType` with an ordinary 3-bit discriminant, i.e. everything
    /// the encoder can put in LLL or UUU (see `docs/PROTOCOL.md`).
    const NON_KING_TYPES: [PieceType; 7] = [
        PieceType::Soldier,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Paladin,
        PieceType::Guard,
        PieceType::Knight,
        PieceType::Ballista,
    ];

    const COLORS: [Color; 2] = [Color::White, Color::Black];

    /// The documented initial back rank, left to right.
    const BACK_RANK: [PieceType; BOARD_DIMENSION] = [
        PieceType::Ballista,
        PieceType::Knight,
        PieceType::Paladin,
        PieceType::Guard,
        PieceType::King,
        PieceType::Guard,
        PieceType::Paladin,
        PieceType::Knight,
        PieceType::Ballista,
    ];

    fn single(color: Color, bottom: PieceType) -> Piece {
        Piece::new(color, bottom, None)
    }

    fn stacked(color: Color, bottom: PieceType, top: PieceType) -> Piece {
        Piece::new(color, bottom, Some(top))
    }

    fn expect_piece(board: &Board, x: usize, y: usize) -> Piece {
        let position = Position::new(x, y);
        match board.get_piece(&position) {
            Some(piece) => *piece,
            None => panic!("expected a piece at {position}"),
        }
    }

    fn assert_empty(board: &Board, x: usize, y: usize) {
        let position = Position::new(x, y);
        assert!(
            board.get_piece(&position).is_none(),
            "expected {position} to be empty"
        );
    }

    // --- Position ---------------------------------------------------------

    #[test]
    fn position_new_accepts_every_on_board_coordinate() {
        for y in 0..BOARD_DIMENSION {
            for x in 0..BOARD_DIMENSION {
                let position = Position::new(x, y);
                assert_eq!((position.x, position.y), (x, y));
            }
        }
    }

    #[test]
    fn position_try_new_returns_none_off_the_board_instead_of_panicking() {
        assert_eq!(Position::try_new(8, 8), Some(Position { x: 8, y: 8 }));
        for (x, y) in [(9, 0), (0, 9), (9, 9), (usize::MAX, 0), (0, usize::MAX)] {
            assert_eq!(Position::try_new(x, y), None, "({x}, {y})");
        }
    }

    #[test]
    #[should_panic(expected = "Position coordinates must be between 0 and 8 inclusive.")]
    fn position_new_panics_off_the_board() {
        Position::new(9, 0);
    }

    #[test]
    fn position_to_absolute_is_row_major_y_times_nine_plus_x() {
        assert_eq!(Position::new(0, 0).to_absolute(), 0);
        assert_eq!(Position::new(8, 0).to_absolute(), 8);
        assert_eq!(Position::new(0, 1).to_absolute(), 9);
        assert_eq!(Position::new(4, 4).to_absolute(), 40);
        assert_eq!(Position::new(8, 8).to_absolute(), BOARD_SIZE - 1);
    }

    #[test]
    fn position_u8_round_trips_over_all_81_squares() {
        for index in 0..BOARD_SIZE as u8 {
            let position = Position::from_u8(index);
            assert_eq!(position.to_u8(), index, "{position}");
            assert_eq!(position.to_absolute(), index as usize, "{position}");
            assert_eq!(Position::try_from_u8(index), Some(position), "{position}");
            // ...and the other direction: coordinates -> index -> coordinates.
            assert_eq!(
                Position::from_u8(Position::new(position.x, position.y).to_u8()),
                position
            );
        }
    }

    #[test]
    fn position_try_from_u8_returns_none_for_every_index_past_the_board() {
        for index in BOARD_SIZE as u8..=u8::MAX {
            assert_eq!(Position::try_from_u8(index), None, "index {index}");
        }
    }

    #[test]
    #[should_panic(expected = "Position index must be between 0 and 80 inclusive.")]
    fn position_from_u8_panics_past_the_board() {
        Position::from_u8(81);
    }

    #[test]
    fn position_validate_rejects_negative_and_too_large_coordinates() {
        assert!(Position::validate(0, 0));
        assert!(Position::validate(8, 8));
        for (x, y) in [
            (-1, 0),
            (0, -1),
            (-1, -1),
            (9, 0),
            (0, 9),
            (9, 9),
            (100, 100),
        ] {
            assert!(!Position::validate(x, y), "({x}, {y})");
        }
    }

    #[test]
    fn position_get_new_applies_the_delta_inside_the_board() {
        let center = Position::new(4, 4);
        assert_eq!(center.get_new(1, 0), Some(Position::new(5, 4)));
        assert_eq!(center.get_new(0, -1), Some(Position::new(4, 3)));
        assert_eq!(center.get_new(-1, 1), Some(Position::new(3, 5)));
        assert_eq!(center.get_new(4, 4), Some(Position::new(8, 8)));
        assert_eq!(center.get_new(-4, -4), Some(Position::new(0, 0)));
        assert_eq!(center.get_new(0, 0), Some(center));
    }

    #[test]
    fn position_get_new_returns_none_off_the_board_for_every_square_and_delta() {
        for index in 0..BOARD_SIZE as u8 {
            let position = Position::from_u8(index);
            for (dx, dy) in Position::ALL_MOVES {
                let (x, y) = (position.x as isize + dx, position.y as isize + dy);
                let expected = if Position::validate(x, y) {
                    Some(Position::new(x as usize, y as usize))
                } else {
                    None
                };
                assert_eq!(
                    position.get_new(dx, dy),
                    expected,
                    "{position} + ({dx}, {dy})"
                );
            }
        }
    }

    #[test]
    fn position_get_new_returns_none_at_the_four_corners() {
        let top_left = Position::new(0, 0);
        assert_eq!(top_left.get_new(-1, 0), None);
        assert_eq!(top_left.get_new(0, -1), None);
        assert_eq!(top_left.get_new(-1, -1), None);

        let top_right = Position::new(8, 0);
        assert_eq!(top_right.get_new(1, 0), None);
        assert_eq!(top_right.get_new(1, -1), None);

        let bottom_left = Position::new(0, 8);
        assert_eq!(bottom_left.get_new(-1, 0), None);
        assert_eq!(bottom_left.get_new(-1, 1), None);

        let bottom_right = Position::new(8, 8);
        assert_eq!(bottom_right.get_new(1, 0), None);
        assert_eq!(bottom_right.get_new(0, 1), None);
        assert_eq!(bottom_right.get_new(1, 1), None);
    }

    #[test]
    fn position_display_uses_algebraic_notation_with_a1_bottom_left() {
        assert_eq!(Position::new(0, 8).to_string(), "A1");
        assert_eq!(Position::new(8, 0).to_string(), "I9");
        assert_eq!(Position::new(0, 0).to_string(), "A9");
        assert_eq!(Position::new(8, 8).to_string(), "I1");
        assert_eq!(Position::new(4, 4).to_string(), "E5");
    }

    #[test]
    fn position_display_agrees_with_from_u8_for_every_square() {
        for index in 0..BOARD_SIZE as u8 {
            let position = Position::from_u8(index);
            let expected = format!(
                "{}{}",
                ["A", "B", "C", "D", "E", "F", "G", "H", "I"][index as usize % BOARD_DIMENSION],
                BOARD_DIMENSION - index as usize / BOARD_DIMENSION
            );
            assert_eq!(position.to_string(), expected, "index {index}");
        }
    }

    // --- Piece encoding ---------------------------------------------------

    #[test]
    fn piece_to_u8_matches_the_documented_bit_layout() {
        // 0 C UUU LLL, with C = 1 for White (docs/PROTOCOL.md).
        assert_eq!(
            single(Color::Black, PieceType::Soldier).to_u8(),
            0b0_000_001
        );
        assert_eq!(
            single(Color::White, PieceType::Soldier).to_u8(),
            0b1_000_001
        );
        assert_eq!(
            single(Color::Black, PieceType::Ballista).to_u8(),
            0b0_000_111
        );
        assert_eq!(
            stacked(Color::Black, PieceType::Soldier, PieceType::Rook).to_u8(),
            0b0_011_001
        );
        assert_eq!(
            stacked(Color::White, PieceType::Ballista, PieceType::Ballista).to_u8(),
            0b1_111_111
        );
        assert_eq!(single(Color::Black, PieceType::King).to_u8(), 0b0_111_000);
        assert_eq!(single(Color::White, PieceType::King).to_u8(), 0b1_111_000);
    }

    #[test]
    fn piece_u8_round_trips_for_every_single_piece() {
        for color in COLORS {
            for bottom in NON_KING_TYPES {
                let piece = single(color, bottom);
                let byte = piece.to_u8();
                assert_eq!(byte & 0b1000_0000, 0, "{piece:?} must leave bit 7 clear");
                assert_eq!(byte >> 3 & 0b111, 0b000, "{piece:?} must encode UUU = 000");
                assert_eq!(
                    Piece::try_from_u8(byte),
                    Ok(Some(piece)),
                    "{piece:?} (0b{byte:08b})"
                );
                assert_eq!(Piece::from_u8(byte), Some(piece), "{piece:?}");
                // byte -> piece -> byte must be lossless too.
                assert_eq!(
                    Piece::from_u8(byte).unwrap().to_u8(),
                    byte,
                    "{piece:?} (0b{byte:08b})"
                );
            }
        }
    }

    #[test]
    fn piece_u8_round_trips_for_all_49_stacked_combinations_of_both_colors() {
        for color in COLORS {
            for bottom in NON_KING_TYPES {
                for top in NON_KING_TYPES {
                    let piece = stacked(color, bottom, top);
                    let byte = piece.to_u8();
                    assert_eq!(byte & 0b1000_0000, 0, "{piece:?} must leave bit 7 clear");
                    assert_eq!(
                        Piece::try_from_u8(byte),
                        Ok(Some(piece)),
                        "{piece:?} (0b{byte:08b})"
                    );
                    assert_eq!(
                        Piece::from_u8(byte).unwrap().to_u8(),
                        byte,
                        "{piece:?} (0b{byte:08b})"
                    );
                }
            }
        }
    }

    #[test]
    fn piece_u8_round_trips_for_the_king_of_both_colors() {
        for color in COLORS {
            let king = single(color, PieceType::King);
            let byte = king.to_u8();
            let decoded = Piece::try_from_u8(byte).unwrap().unwrap();
            assert_eq!(decoded, king, "0b{byte:08b}");
            assert!(decoded.is_king());
            assert_eq!(decoded.top, None, "a king is never stacked");
            assert_eq!(decoded.to_u8(), byte);
        }
    }

    #[test]
    fn piece_try_from_u8_reads_zero_as_an_empty_square() {
        assert_eq!(Piece::try_from_u8(0), Ok(None));
        assert_eq!(Piece::from_u8(0), None);
    }

    #[test]
    fn piece_try_from_u8_rejects_every_byte_with_bit_7_set() {
        for byte in 0b1000_0000..=u8::MAX {
            let error = Piece::try_from_u8(byte).expect_err("bit 7 must be rejected");
            assert!(error.contains("bit 7"), "0b{byte:08b}: {error}");
        }
    }

    #[test]
    fn piece_try_from_u8_rejects_a_stack_with_no_bottom_piece() {
        // UUU != 000 with LLL == 000 is unreachable for the encoder: every
        // stack needs a bottom. The one exception is the king's C 111 000.
        for color_bit in [0b0_000_000, 0b1_000_000] {
            for uuu in 1u8..=6 {
                let byte = color_bit | (uuu << 3);
                let error = Piece::try_from_u8(byte).expect_err("a bottomless stack is invalid");
                assert!(error.contains("(LLL)"), "0b{byte:08b}: {error}");
            }
        }
    }

    #[test]
    fn piece_try_from_u8_reads_the_kings_own_payload_as_a_king_not_a_bottomless_stack() {
        // C 111 000 has LLL == 000 like the rejected payloads above, but it is
        // the king's dedicated encoding and must decode, not error.
        assert_eq!(
            Piece::try_from_u8(0b0_111_000),
            Ok(Some(single(Color::Black, PieceType::King)))
        );
        assert_eq!(
            Piece::try_from_u8(0b1_111_000),
            Ok(Some(single(Color::White, PieceType::King)))
        );
    }

    #[test]
    #[should_panic(expected = "bit 7 is reserved")]
    fn piece_from_u8_panics_on_an_invalid_encoding() {
        Piece::from_u8(0b1000_0001);
    }

    #[test]
    fn every_one_of_the_256_bytes_either_errors_or_round_trips_identically() {
        // The property that actually protects the wire format: no byte from an
        // untrusted peer may panic, and no accepted byte may decode to a piece
        // that re-encodes differently (that would silently rewrite a position).
        let mut accepted = 0usize;
        for byte in u8::MIN..=u8::MAX {
            match Piece::try_from_u8(byte) {
                Ok(None) => {
                    accepted += 1;
                    assert_eq!(byte, 0, "only 0 may decode to an empty square");
                }
                Ok(Some(piece)) => {
                    accepted += 1;
                    assert_eq!(piece.to_u8(), byte, "0b{byte:08b} -> {piece:?}");
                }
                Err(error) => assert!(
                    error.contains(&format!("0b{byte:08b}")),
                    "error should name the offending byte: {error}"
                ),
            }
        }
        // 1 empty square + per color (7 singles + 49 stacks + 1 king).
        assert_eq!(accepted, 1 + 2 * (7 + 49 + 1));
    }

    // --- Piece helpers ----------------------------------------------------

    #[test]
    fn is_stackable_only_for_unstacked_non_king_pieces() {
        for color in COLORS {
            assert!(!single(color, PieceType::King).is_stackable());
            for bottom in NON_KING_TYPES {
                let piece = single(color, bottom);
                assert!(piece.is_stackable(), "{piece:?}");
                let piece = stacked(color, bottom, PieceType::Soldier);
                assert!(!piece.is_stackable(), "{piece:?}");
            }
        }
    }

    #[test]
    fn is_stacked_reflects_the_presence_of_a_top_piece() {
        assert!(!single(Color::White, PieceType::Rook).is_stacked());
        assert!(!single(Color::White, PieceType::King).is_stacked());
        assert!(stacked(Color::White, PieceType::Rook, PieceType::Soldier).is_stacked());
    }

    #[test]
    fn is_king_only_for_a_piece_whose_bottom_is_the_king() {
        for color in COLORS {
            assert!(single(color, PieceType::King).is_king());
            for bottom in NON_KING_TYPES {
                assert!(!single(color, bottom).is_king(), "{bottom:?}");
                assert!(
                    !stacked(color, bottom, PieceType::Guard).is_king(),
                    "{bottom:?}"
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "King cannot have a piece on top of it")]
    fn piece_new_panics_when_stacking_onto_a_king() {
        Piece::new(Color::White, PieceType::King, Some(PieceType::Soldier));
    }

    // --- Board: starting position -----------------------------------------

    #[test]
    fn new_board_has_the_documented_starting_layout() {
        let board = Board::new();

        // Back ranks: black on y = 0, white on y = 8, kings on (4, 0)/(4, 8).
        for (x, &piece_type) in BACK_RANK.iter().enumerate() {
            assert_eq!(expect_piece(&board, x, 0), single(Color::Black, piece_type));
            assert_eq!(expect_piece(&board, x, 8), single(Color::White, piece_type));
        }
        assert!(expect_piece(&board, 4, 0).is_king());
        assert!(expect_piece(&board, 4, 8).is_king());

        // Rook and bishop on the second rank; the white half is rotated, not
        // mirrored, so its rook and bishop swap columns.
        assert_eq!(
            expect_piece(&board, 2, 1),
            single(Color::Black, PieceType::Rook)
        );
        assert_eq!(
            expect_piece(&board, 6, 1),
            single(Color::Black, PieceType::Bishop)
        );
        assert_eq!(
            expect_piece(&board, 6, 7),
            single(Color::White, PieceType::Rook)
        );
        assert_eq!(
            expect_piece(&board, 2, 7),
            single(Color::White, PieceType::Bishop)
        );
        for x in [0, 1, 3, 4, 5, 7, 8] {
            assert_empty(&board, x, 1);
            assert_empty(&board, x, 7);
        }

        // Full ranks of soldiers, and an empty middle.
        for x in 0..BOARD_DIMENSION {
            assert_eq!(
                expect_piece(&board, x, 2),
                single(Color::Black, PieceType::Soldier)
            );
            assert_eq!(
                expect_piece(&board, x, 6),
                single(Color::White, PieceType::Soldier)
            );
            for y in 3..=5 {
                assert_empty(&board, x, y);
            }
        }
    }

    #[test]
    fn new_board_has_forty_pieces_and_exactly_one_king_per_color() {
        let board = Board::new();
        let white = board
            .pieces()
            .filter(|(_, piece)| piece.color == Color::White)
            .count();
        let black = board
            .pieces()
            .filter(|(_, piece)| piece.color == Color::Black)
            .count();
        // 9 back-rank + 2 second-rank + 9 soldiers per side.
        assert_eq!((white, black), (20, 20));
        assert_eq!(board.pieces().count(), 40);

        for color in COLORS {
            let kings = board
                .pieces()
                .filter(|(_, piece)| piece.is_king() && piece.color == color)
                .count();
            assert_eq!(kings, 1, "{color:?}");
        }
        assert!(board.pieces().all(|(_, piece)| !piece.is_stacked()));
    }

    #[test]
    fn new_board_is_point_symmetric_between_the_two_colors() {
        let board = Board::new();
        for (position, piece) in board.pieces() {
            let opposite = Position::from_u8((BOARD_SIZE - 1 - position.to_absolute()) as u8);
            let counterpart = expect_piece(&board, opposite.x, opposite.y);
            assert_eq!(counterpart.bottom, piece.bottom, "{position} vs {opposite}");
            assert_ne!(counterpart.color, piece.color, "{position} vs {opposite}");
        }
    }

    #[test]
    fn default_board_is_the_starting_position() {
        assert_eq!(Board::default(), Board::new());
    }

    #[test]
    fn empty_board_has_no_pieces() {
        let board = Board::empty();
        assert_eq!(board.pieces().count(), 0);
        for index in 0..BOARD_SIZE as u8 {
            let position = Position::from_u8(index);
            assert!(board.get_piece(&position).is_none(), "{position}");
        }
        assert_eq!(board.to_binary(), [0u8; BOARD_SIZE]);
        assert_ne!(board, Board::new());
    }

    // --- Board: accessors -------------------------------------------------

    #[test]
    fn set_piece_and_get_piece_round_trip_on_every_square() {
        let piece = stacked(Color::White, PieceType::Guard, PieceType::Soldier);
        for index in 0..BOARD_SIZE as u8 {
            let position = Position::from_u8(index);
            let mut board = Board::empty();
            board.set_piece(&position, Some(piece));
            assert_eq!(board.get_piece(&position), Some(&piece), "{position}");
            assert_eq!(board.pieces().count(), 1, "{position}");
            board.set_piece(&position, None);
            assert_eq!(board.get_piece(&position), None, "{position}");
        }
    }

    #[test]
    fn pieces_iterator_yields_exactly_the_occupied_squares_with_their_positions() {
        let mut board = Board::empty();
        let placed = [
            (Position::new(0, 0), single(Color::Black, PieceType::Rook)),
            (Position::new(4, 4), single(Color::White, PieceType::King)),
            (
                Position::new(8, 8),
                stacked(Color::White, PieceType::Soldier, PieceType::Knight),
            ),
        ];
        for (position, piece) in placed {
            board.set_piece(&position, Some(piece));
        }

        let yielded: Vec<(Position, Piece)> =
            board.pieces().map(|(pos, piece)| (pos, *piece)).collect();
        assert_eq!(yielded, placed.to_vec());
    }

    // --- Board: binary encoding -------------------------------------------

    #[test]
    fn board_binary_round_trips_for_the_starting_position() {
        let board = Board::new();
        let binary = board.to_binary();
        assert_eq!(binary.len(), BOARD_SIZE);
        assert_eq!(Board::from_binary(&binary), Ok(board));
        // Empty squares stay 0, occupied ones carry the piece byte.
        for (index, &byte) in binary.iter().enumerate() {
            let position = Position::from_u8(index as u8);
            let expected = board.get_piece(&position).map_or(0, Piece::to_u8);
            assert_eq!(byte, expected, "{position}");
        }
    }

    #[test]
    fn board_binary_round_trips_for_a_hand_built_stacked_position() {
        let mut board = Board::empty();
        board.set_piece(
            &Position::new(0, 0),
            Some(stacked(Color::Black, PieceType::Soldier, PieceType::Rook)),
        );
        board.set_piece(
            &Position::new(3, 5),
            Some(stacked(
                Color::White,
                PieceType::Ballista,
                PieceType::Ballista,
            )),
        );
        board.set_piece(
            &Position::new(8, 8),
            Some(single(Color::White, PieceType::King)),
        );
        board.set_piece(
            &Position::new(4, 0),
            Some(single(Color::Black, PieceType::King)),
        );

        assert_eq!(Board::from_binary(&board.to_binary()), Ok(board));
    }

    #[test]
    fn board_from_binary_errors_and_names_the_offending_square() {
        let mut binary = Board::new().to_binary();
        binary[42] = 0b1111_1111;
        let error = Board::from_binary(&binary).expect_err("malformed byte must be rejected");
        assert!(error.starts_with("square 42:"), "{error}");

        // A bottomless stack is rejected the same way.
        let mut binary = [0u8; BOARD_SIZE];
        binary[0] = 0b0_010_000;
        let error = Board::from_binary(&binary).expect_err("malformed byte must be rejected");
        assert!(error.starts_with("square 0:"), "{error}");
    }

    #[test]
    fn board_from_binary_accepts_an_all_zero_payload_as_an_empty_board() {
        assert_eq!(Board::from_binary(&[0u8; BOARD_SIZE]), Ok(Board::empty()));
    }

    // --- Board: validate --------------------------------------------------

    #[test]
    fn validate_accepts_the_starting_board() {
        assert_eq!(Board::new().validate(), Ok(()));
    }

    #[test]
    fn validate_accepts_zero_or_one_king_per_side() {
        assert_eq!(Board::empty().validate(), Ok(()));

        let mut board = Board::empty();
        board.set_piece(
            &Position::new(0, 0),
            Some(single(Color::White, PieceType::King)),
        );
        assert_eq!(board.validate(), Ok(()), "one white king, no black king");

        board.set_piece(
            &Position::new(8, 8),
            Some(single(Color::Black, PieceType::King)),
        );
        assert_eq!(board.validate(), Ok(()), "one king each");
    }

    #[test]
    fn validate_rejects_two_kings_of_the_same_color() {
        for color in COLORS {
            let mut board = Board::empty();
            board.set_piece(&Position::new(0, 0), Some(single(color, PieceType::King)));
            board.set_piece(&Position::new(1, 0), Some(single(color, PieceType::King)));
            let error = board.validate().expect_err("two kings must be rejected");
            assert!(error.contains("2"), "{color:?}: {error}");
            match color {
                Color::White => assert!(error.contains("white kings"), "{error}"),
                Color::Black => assert!(error.contains("black kings"), "{error}"),
            }
        }
    }

    // --- Board: unstack_piece / stack_piece -------------------------------

    #[test]
    fn unstack_piece_returns_the_top_piece_and_leaves_the_bottom_behind() {
        let position = Position::new(3, 4);
        for color in COLORS {
            for bottom in NON_KING_TYPES {
                for top in NON_KING_TYPES {
                    let mut board = Board::empty();
                    let piece = stacked(color, bottom, top);
                    board.set_piece(&position, Some(piece));

                    let unstacked = board.unstack_piece(&position).expect("stack must split");
                    // The former top becomes a single piece of the same color.
                    assert_eq!(unstacked, single(color, top), "{piece:?}");
                    // The square keeps the bottom piece, now unstacked.
                    assert_eq!(
                        board.get_piece(&position),
                        Some(&single(color, bottom)),
                        "{piece:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn unstack_piece_fails_on_an_empty_square() {
        let mut board = Board::empty();
        assert_eq!(
            board.unstack_piece(&Position::new(0, 0)),
            Err("No piece at the specified position".to_string())
        );
    }

    #[test]
    fn unstack_piece_fails_on_a_single_piece_and_on_a_king() {
        let position = Position::new(2, 2);
        for piece in [
            single(Color::White, PieceType::Rook),
            single(Color::White, PieceType::King),
        ] {
            let mut board = Board::empty();
            board.set_piece(&position, Some(piece));
            assert_eq!(
                board.unstack_piece(&position),
                Err("No top piece to unstack".to_string()),
                "{piece:?}"
            );
            // The failed call must leave the square untouched.
            assert_eq!(board.get_piece(&position), Some(&piece), "{piece:?}");
        }
    }

    #[test]
    fn stack_piece_puts_the_moving_piece_on_top_of_the_resident_one() {
        let position = Position::new(5, 1);
        for color in COLORS {
            for bottom in NON_KING_TYPES {
                for moving in NON_KING_TYPES {
                    let mut board = Board::empty();
                    board.set_piece(&position, Some(single(color, bottom)));
                    assert_eq!(
                        board.stack_piece(&position, single(color, moving)),
                        Ok(()),
                        "{bottom:?} + {moving:?}"
                    );
                    assert_eq!(
                        board.get_piece(&position),
                        Some(&stacked(color, bottom, moving)),
                        "{bottom:?} + {moving:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn stack_piece_fails_on_an_empty_square() {
        let mut board = Board::empty();
        assert_eq!(
            board.stack_piece(
                &Position::new(0, 0),
                single(Color::White, PieceType::Soldier)
            ),
            Err("No piece at position to stack onto".to_string())
        );
    }

    #[test]
    fn stack_piece_fails_onto_a_king() {
        let position = Position::new(4, 0);
        let mut board = Board::empty();
        let king = single(Color::White, PieceType::King);
        board.set_piece(&position, Some(king));
        assert_eq!(
            board.stack_piece(&position, single(Color::White, PieceType::Soldier)),
            Err("Cannot stack onto this piece (King or already stacked)".to_string())
        );
        assert_eq!(board.get_piece(&position), Some(&king));
    }

    #[test]
    fn stack_piece_fails_onto_an_already_stacked_piece() {
        let position = Position::new(4, 4);
        let mut board = Board::empty();
        let resident = stacked(Color::Black, PieceType::Rook, PieceType::Soldier);
        board.set_piece(&position, Some(resident));
        assert_eq!(
            board.stack_piece(&position, single(Color::Black, PieceType::Guard)),
            Err("Cannot stack onto this piece (King or already stacked)".to_string())
        );
        assert_eq!(board.get_piece(&position), Some(&resident));
    }

    #[test]
    fn stack_piece_fails_across_colors() {
        let position = Position::new(1, 1);
        let mut board = Board::empty();
        let resident = single(Color::Black, PieceType::Rook);
        board.set_piece(&position, Some(resident));
        assert_eq!(
            board.stack_piece(&position, single(Color::White, PieceType::Soldier)),
            Err("Cannot stack pieces of different colors".to_string())
        );
        assert_eq!(board.get_piece(&position), Some(&resident));
    }

    #[test]
    fn stack_piece_fails_when_the_moving_piece_is_itself_stacked() {
        let position = Position::new(7, 3);
        let mut board = Board::empty();
        let resident = single(Color::White, PieceType::Guard);
        board.set_piece(&position, Some(resident));
        assert_eq!(
            board.stack_piece(
                &position,
                stacked(Color::White, PieceType::Soldier, PieceType::Knight)
            ),
            Err("Cannot stack an already stacked piece".to_string())
        );
        assert_eq!(board.get_piece(&position), Some(&resident));
    }

    #[test]
    fn stack_piece_fails_when_the_moving_piece_is_a_king() {
        // A king on top of a stack has no encoding: `Piece::to_u8` panics on
        // `top == Some(King)`, so the board must never hold one.
        let position = Position::new(7, 3);
        let mut board = Board::empty();
        let resident = single(Color::White, PieceType::Guard);
        board.set_piece(&position, Some(resident));
        assert_eq!(
            board.stack_piece(&position, single(Color::White, PieceType::King)),
            Err("Cannot stack a King onto another piece".to_string())
        );
        assert_eq!(board.get_piece(&position), Some(&resident));
    }

    #[test]
    fn a_stack_survives_a_binary_round_trip_after_stack_then_unstack() {
        // stack_piece / unstack_piece must stay consistent with the wire
        // format: both intermediate boards have to re-encode identically.
        let position = Position::new(6, 2);
        let mut board = Board::empty();
        board.set_piece(&position, Some(single(Color::Black, PieceType::Rook)));
        board
            .stack_piece(&position, single(Color::Black, PieceType::Soldier))
            .expect("stacking onto a single piece is legal");
        assert_eq!(Board::from_binary(&board.to_binary()), Ok(board));

        let unstacked = board.unstack_piece(&position).expect("stack must split");
        assert_eq!(unstacked, single(Color::Black, PieceType::Soldier));
        assert_eq!(Board::from_binary(&board.to_binary()), Ok(board));
        assert_eq!(
            board.get_piece(&position),
            Some(&single(Color::Black, PieceType::Rook))
        );
    }
}
