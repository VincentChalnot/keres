## Piece Encoding Strategy

- **Total piece variations:** 104 possible states (including empty square)

- **Encoding scheme:**
    - `u8` (8 bits) per square
    - `0` represents an empty square
    - Values `1..=49`: White pieces
    - Values `50..=104`: Black pieces
    - All possible valid stack combinations are encoded, even if some are technically illegal, to simplify bitwise decoding

## Board Representation

- **Memory layout:**
    - 9 rows, each with 9 squares = `9x9 = 81` cells
    - Each cell encoded as a `u8`
    - Entire board stored as `[[u8; 9]; 9]`

- **Alternative formats:**
    - For compact state hashing or GPU batching: use `9 * u64` to store board state (each row = 63 bits)

## Move Encoding
- **13 bits total (optional 14th bit for future use):**
    - `7 bits`: From position `(x*9 + y)`
    - `7 bits`: To position `(x*9 + y)`
    - `Optional`: 1 bit for color or stack-unstack intent (not strictly necessary)
- **Move validation is handled after generation**, so encoding does not need to include legality flags