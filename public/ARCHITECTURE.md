# Frontend Architecture

This document describes the architecture of the Arx game frontend.

## Architecture Overview

The frontend follows a **Model-View-Controller (MVC)** pattern with clear separation of concerns:

```
src/
├── models/          # Game state and data models
├── views/           # Rendering implementations (abstracted via interfaces)
├── controllers/     # Game logic and coordination
├── network/         # API communication
├── utils/           # Utility functions
└── app.ts           # Main application entry point
```

## Design Principles

### 1. **Separation of Concerns**
- **Model**: Game state management (board data, move history, piece decoding)
- **View**: Rendering (completely swappable via IBoardView interface)
- **Controller**: Game logic (move validation, user interaction)
- **Network**: Backend communication (API calls)

### 2. **Interface-Based View System**
The rendering engine is abstracted behind the `IBoardView` interface, making it easy to swap implementations:

```typescript
interface IBoardView {
  initialize(container: HTMLElement): void;
  render(boardData: Uint8Array, flipped: boolean): Promise<void>;
  updateOverlays(highlights: TileHighlight[]): void;
  onResize(): void;
  dispose(): void;
  onTileClick(handler: (tileIndex: number) => void): void;
  onTileHover(handler: (tileIndex: number | null) => void): void;
}
```

Current implementation: `ThreeJSBoardView` (2D sprites with Three.js)
Future implementation: 3D board with 3D pieces

### 3. **Dependency Injection**
The controller accepts any view implementation that conforms to `IBoardView`:

```typescript
const view = new ThreeJSBoardView(gameState);
const controller = new GameController(gameState, api, view);
```

To switch to a 3D renderer, simply create a new implementation and inject it.

## Components

### Models (`models/`)

#### `GameState.ts`
Manages the game state:
- Board data (Uint8Array)
- Move history and game history
- Selected pieces and moves
- Board flip state
- Piece decoding logic

#### `types.ts`
TypeScript interfaces and constants:
- Core game types (Config, Piece, SelectedPiece, etc.)
- Game constants (BOARD_SIZE, PIECE_CODE, COLOR_NAME)

### Views (`views/`)

#### `IBoardView.ts`
Interface that all rendering implementations must follow.

#### `ThreeJSBoardView.ts`
Current implementation using Three.js with sprite-based rendering:
- Orthographic camera
- Board sprite (wooden texture)
- Piece sprites (with stacking support)
- Overlay system for tile highlights
- Raycasting for mouse interaction

**To create a 3D view**: Implement `IBoardView` with 3D meshes instead of sprites.

### Controllers (`controllers/`)

#### `GameController.ts`
Main game logic coordinator:
- Handles tile clicks and hover
- Manages move validation
- Coordinates between model, view, and network
- Handles game actions (undo, flip, load, engine moves)

### Network (`network/`)

#### `GameAPI.ts`
Backend communication:
- `getNewGame()`: Start new game
- `getPossibleMoves()`: Get legal moves
- `playMove()`: Execute a move
- `getEngineMove()`: Request AI move
- `loadConfig()`: Load configuration

### Utils (`utils/`)

#### `boardUtils.ts`
Utility functions:
- `posToAlgebraic()`: Convert position to algebraic notation
- `algebraicToPos()`: Convert algebraic to position
- `encodeBoardToHash()`: Encode board for URL
- `decodeBoardFromHash()`: Decode board from URL

## Data Flow

1. **User clicks a tile**
   → View captures click
   → View calls `onTileClick` handler
   → Controller processes click
   → Controller updates game state
   → Controller calls network API if needed
   → Controller updates view

2. **Board rendering**
   → Controller has board data
   → Controller calls `view.render(boardData, flipped)`
   → View creates/updates Three.js objects
   → View renders scene

3. **Network operations**
   → Controller calls GameAPI methods
   → API makes HTTP request to backend
   → API returns typed data
   → Controller updates game state
   → Controller triggers view update

## Adding a New View Implementation

To create a new rendering engine (e.g., 3D board):

1. **Create new view class**:
```typescript
export class ThreeJS3DBoardView implements IBoardView {
  // Implement all IBoardView methods
  initialize(container: HTMLElement): void { }
  render(boardData: Uint8Array, flipped: boolean): Promise<void> { }
  updateOverlays(highlights: TileHighlight[]): void { }
  // ...
}
```

2. **Update main app.ts**:
```typescript
// Replace this:
this.view = new ThreeJSBoardView(this.gameState);

// With this:
this.view = new ThreeJS3DBoardView(this.gameState);
```

That's it! No other changes needed. The controller will work with any view implementation.

## Benefits of This Architecture

1. **Maintainability**: Each component has a single responsibility
2. **Testability**: Components can be tested in isolation
3. **Flexibility**: Easy to swap implementations (especially the view)
4. **Scalability**: Clear structure for adding new features
5. **Type Safety**: Full TypeScript type checking throughout

## Future Enhancements

- **3D Board View**: Create `ThreeJS3DBoardView` implementing `IBoardView`
- **Animation System**: Add piece movement animations
- **Sound Effects**: Add audio feedback
- **Multiplayer**: Add WebSocket support in network layer
- **AI Visualization**: Show engine thinking process
