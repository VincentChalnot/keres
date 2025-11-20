---
applyTo: '**'
---
# Piece, Board, Position, and Move Encoding Instructions

## Piece Encoding
Each board position is encoded using 7 bits, each line from the board is encoded on a 64bit unsigned integer. A whole board is encoded in 9 x u64.

- **Stacking rules:**
  - King+*, *+King: Not permitted in encoding (King has a special code).
  - Jester+Jester, Commander+Commander: Can be encoded but never appear in a game.
- When describing a stack, the first piece is always on top (e.g., Jester+Paladin = J+P: Jester is on top).

The 7 bits for a piece are interpreted as `C UUU LLL`:
- `C` (1 bit): Color. `0` for Black, `1` for White.
- `UUU` (3 bits): Top Piece Code (top of stack, or `0b000` for single piece).
- `LLL` (3 bits): Bottom Piece Code (bottom of stack, or type for single piece).

- Single piece (e.g., Guard): type code in `LLL`, `UUU` is `0b000`. (Black Guard: `0 000 101`)
- Stack (e.g., Jester+Paladin): Jester's code in `UUU`, Paladin's in `LLL`. (White Jester+Paladin: `1 010 100`)

### Special cases
- `0b0000000`: Empty square
- `0b_111000`: King (single piece, `UUU=111`, `LLL=000`, color bit `C`)

#### Base pieces
- `0b001`: Soldier
- `0b010`: Jester
- `0b011`: Commander
- `0b100`: Paladins
- `0b101`: Guards
- `0b110`: Dragons
- `0b111`: Ballista

### Examples
- White Soldier+Commander: `1 001 011`
- Black Guard: `0 000 101`
- Black King: `0 111000`
- White Soldier+Soldier: `1 001 001`

### Notes
- Codes of the form `0bUUU000` (where `UUU` is `0b001` to `0b110`) are invalid and must throw an exception.

---

## Board Encoding
- The board is a 9x9 grid (81 squares).
- Each square is encoded as 7 bits (see above), so a board can be encoded as 9 x u64 (each row fits in a 64-bit integer).
- Board state includes:
  - Array of 81 Option<Piece> (empty or piece/stack)
  - Flags:
    - `white_to_move`: whose turn it is (1 bit)
    - `game_over`, `white_wins`, `draw`: game state flags (1 bit each)
    - `moves_without_capture`: u8 counter (for 40-move draw rule)
- When serializing the board, include both the piece array and these flags.

---

## Position Encoding
- Positions are encoded as a single u8 (0-80), representing the absolute index in the board array.
- Conversion functions:
  - (x, y) → u8: `y * 9 + x`
  - u8 → (x, y): `x = value % 9`, `y = value / 9`

---

## Move and PotentialMove Encoding
- Moves are encoded as 16 bits (u16):
  - For Move:
    - 1 bit: `unstack` (bit 14)
    - 7 bits: `to` position (bits 7-13)
    - 7 bits: `from` position (bits 0-6)
  - For PotentialMove:
    - 1 bit: `force_unstack` (bit 15)
    - 1 bit: `unstackable` (bit 14)
    - 7 bits: `to` position (bits 7-13)
    - 7 bits: `from` position (bits 0-6)
- Conversion functions exist to go from struct to u16 and back.

---

## Move Generation and Encoding
- Moves are generated for each piece on the board, considering both top and bottom pieces in a stack.
- Only the player whose turn it is can move their pieces.
- PotentialMove is used for move generation, and Move is used for actual moves.
- All moves can be encoded as described above for efficient storage or transmission.

---

## Summary
- Piece encoding: 7 bits per square, as `C UUU LLL`.
- Board encoding: 9x9 grid, 9 x u64, plus flags.
- Position encoding: u8 (0-80).
- Move encoding: u16, with bit layout for Move and PotentialMove.
- All encoding and decoding must follow the rules above and throw exceptions for invalid codes.
