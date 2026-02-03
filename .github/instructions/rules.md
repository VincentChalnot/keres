---
applyTo: '**'
---
# Game of Keres
Abstract strategy board game inspired by chess.

## Game Rules Summary
- The board is 9x9.
- Each player controls a set of unique pieces with specific movement patterns (see below).
- Most pieces can be stacked (combined) on the same tile to merge their movement abilities.

### Stacking
- Stacking is only allowed under specific conditions:
    - The target piece must be accessible using the moving piece’s movement.
    - The target piece must not already be stacked (i.e., has no top piece).
    - The King can never be stacked or be the top piece in a stack.
    - Stacking is only allowed for pieces of the same color.
    - The moving piece must not already be stacked.
- A stacked piece can be separated (unstacked), moving the top piece according to its movement. Unstacking is only possible if there is a top piece.

### Movement and Capture
- Players cannot move through other pieces (except Knights, which can jump over other pieces).
- A piece captures enemy pieces by moving onto their tile.
- Capturing a stacked piece removes the entire stack.
- Only the player whose turn it is can move their pieces.
- The game ends immediately if a King is captured.
- If 40 moves occur without any capture, the game is declared a draw.

### Piece Movement
- Soldiers: move 1 tile forward diagonally
- Bishop: move any number of tiles diagonally
- Rook: move any number of tiles orthogonally
- Paladins: move 1 or 2 tiles orthogonally
- Guards: move 1 or 2 tiles diagonally
- Knights: move in an L-shape like chess knights; **can jump over other pieces**
- Ballista: move any number of tiles forward
- King: move 1 tile in any direction (orthogonally or diagonally); **cannot be stacked with other pieces or be the top piece in a stack**

### Promotions
- When a Soldier reaches the opponent's back rank, it is promoted to a Paladin.
- When a Ballista reaches the opponent's back rank, it is promoted to a Rook.
- If a stacked piece reaches the back rank, both the bottom and top pieces are promoted if eligible.

### Initial Board Positions
```
|B|D|P|G|K|G|P|D|B|
| | |C| | | |J| | |
|S|S|S|S|S|S|S|S|S|
| | | | | | | | | |
| | | | | | | | | |
| | | | | | | | | |
|S|S|S|S|S|S|S|S|S|
| | |J| | | |C| | |
|B|D|P|G|K|G|P|D|B|
```

---

## Additional Rules
- All rules above are enforced by the game engine. Invalid moves, stacking, or promotions will result in errors or be disallowed.
- The board tracks whose turn it is, game over state, winner, draw, and moves without capture.
