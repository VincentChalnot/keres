# Game of Keres
Abstract strategy board game inspired by chess where the main difference is the ability to stack friendly pieces to combine their movements.

## Game Rules Summary:
- The board is a 9x9 checkerboard.
- Each player controls a set of unique pieces with specific movement rules (see below).
- At each turn, a player can only move a single piece or stack of it's own pieces.
- Most pieces can be stacked (combined) on the same tile to merge their movement abilities.
- Stacking is only allowed under specific conditions:
    - The target piece must be accessible using the moving piece’s movement.
    - The target piece must not already be stacked.
    - The king can never be stacked.
- A stacked piece can be separated, moving only the top piece according to its movement.
- Players cannot move through other pieces (except specific ones like the dragon).
- A piece captures enemy pieces by moving onto their tile.
- Capturing a stacked piece removes the entire stack.
- The game ends immediately if a king is captured.

## Moves
The pieces move and capture (always same movement) as follow :
- Soldiers: move 1 tile forward diagonally
- Jester: move any number of tiles diagonally
- Commander: move any number of tiles orthogonally
- Paladins: move 1 or 2 tiles orthogonally
- Guards: move 1 or 2 tiles diagonally
- Dragons: move in an L-shape like chess knights; **can jump over other pieces**
- Ballista: move any number of tiles forward
- King: move 1 tile in any direction (orthogonally or diagonally); **cannot be stacked with other pieces**

## Board
Initial board positions are as follow:
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
