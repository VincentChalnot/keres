import { GameState } from '../models/GameState';
import { GameAPI } from '../network/GameAPI';
import { IBoardView, TileHighlight } from '../views/IBoardView';
import { posToAlgebraic, algebraicToPos, encodeBoardToHash, decodeBoardFromHash } from '../utils/boardUtils';
import { PotentialMove } from '../models/types';

/**
 * Main game controller - handles game logic and coordinates between model, view, and network
 */
export class GameController {
  private gameState: GameState;
  private api: GameAPI;
  private view: IBoardView;

  constructor(gameState: GameState, api: GameAPI, view: IBoardView) {
    this.gameState = gameState;
    this.api = api;
    this.view = view;

    // Set up view event handlers
    this.view.onTileClick((pos) => this.handleTileClick(pos));
    this.view.onTileHover((pos) => this.handleTileHover(pos));
  }

  /**
   * Initialize game from URL or start new game
   */
  async initialize(): Promise<void> {
    if (window.location.hash) {
      const boardData = decodeBoardFromHash(window.location.hash);
      if (boardData) {
        this.gameState.setBoardData(boardData);
        this.gameState.clearMoveHistory();
        this.gameState.clearGameHistory();
      } else {
        await this.startNewGame();
      }
    } else {
      await this.startNewGame();
    }

    await this.updatePossibleMoves();
    await this.renderBoard();
  }

  /**
   * Start a new game
   */
  async startNewGame(): Promise<void> {
    const boardData = await this.api.getNewGame();
    this.gameState.setBoardData(boardData);
    this.gameState.clearMoveHistory();
    this.gameState.clearGameHistory();
  }

  /**
   * Update possible moves from server
   */
  async updatePossibleMoves(): Promise<void> {
    const boardData = this.gameState.getBoardData();
    if (!boardData) return;

    const moves = await this.api.getPossibleMoves(boardData);
    this.gameState.setPossibleMoves(moves);
  }

  /**
   * Get moves for a specific piece
   */
  getMovesForPiece(pos: number): number[] {
    const moves: number[] = [];
    for (const move of this.gameState.getPossibleMoves()) {
      const from = move & 0x7F;
      const to = (move >> 7) & 0x7F;
      if (from === pos) {
        moves.push(to);
      }
    }
    return moves;
  }

  /**
   * Get potential move details
   */
  getPotentialMove(fromPos: number, toPos: number): PotentialMove | null {
    for (const move of this.gameState.getPossibleMoves()) {
      const from = move & 0x7F;
      const to = (move >> 7) & 0x7F;
      if (from === fromPos && to === toPos) {
        const unstackable = (move >> 14) & 0x1;
        const force_unstack = (move >> 15) & 0x1;
        return { from, to, unstackable: unstackable === 1, force_unstack: force_unstack === 1 };
      }
    }
    return null;
  }

  /**
   * Play a move
   */
  async playMove(from: number, to: number, unstack = false): Promise<void> {
    const boardData = this.gameState.getBoardData();
    if (!boardData) return;

    // Save current state to history
    this.gameState.pushGameState(new Uint8Array(boardData));

    // Play move on server
    const newBoardData = await this.api.playMove(boardData, from, to, unstack);
    this.gameState.setBoardData(newBoardData);

    // Update URL hash
    window.location.hash = encodeBoardToHash(newBoardData);

    // Record move in algebraic notation
    const moveNotation = posToAlgebraic(from) + '-' + posToAlgebraic(to);
    this.gameState.addMove(moveNotation);

    // Reset selection
    this.gameState.setSelectedPiece(null);
    this.gameState.setSelectedMove(null);

    // Update game state
    await this.updatePossibleMoves();
    await this.renderBoard();
  }

  /**
   * Request engine move
   */
  async requestEngineMove(): Promise<void> {
    const boardData = this.gameState.getBoardData();
    if (!boardData) return;

    const { from, to, unstack } = await this.api.getEngineMove(boardData);
    await this.playMove(from, to, unstack);
  }

  /**
   * Undo last move
   */
  async undoMove(): Promise<void> {
    const previousState = this.gameState.popGameState();
    if (!previousState) {
      alert('No moves to undo');
      return;
    }

    this.gameState.setBoardData(previousState);
    this.gameState.popMove();
    window.location.hash = encodeBoardToHash(previousState);

    this.gameState.setSelectedPiece(null);
    this.gameState.setSelectedMove(null);
    await this.updatePossibleMoves();
    await this.renderBoard();
  }

  /**
   * Flip board view
   */
  async flipBoard(): Promise<void> {
    this.gameState.flipBoard();
    this.gameState.setSelectedPiece(null);
    this.gameState.setSelectedMove(null);
    await this.renderBoard();
  }

  /**
   * Load game from move history
   */
  async loadGameFromMoves(moves: string[]): Promise<void> {
    await this.startNewGame();

    for (const moveNotation of moves) {
      const parts = moveNotation.split('-');
      if (parts.length !== 2) {
        throw new Error(`Invalid move format: ${moveNotation}`);
      }

      const fromPos = algebraicToPos(parts[0]);
      const toPos = algebraicToPos(parts[1]);

      if (fromPos === null || toPos === null) {
        throw new Error(`Invalid position in move: ${moveNotation}`);
      }

      await this.updatePossibleMoves();
      const moves = this.getMovesForPiece(fromPos);
      if (!moves.includes(toPos)) {
        throw new Error(`Illegal move: ${moveNotation}`);
      }

      await this.playMove(fromPos, toPos, false);
    }

    await this.renderBoard();
  }

  /**
   * Handle tile click
   */
  private handleTileClick(pos: number): void {
    const boardData = this.gameState.getBoardData();
    if (!boardData) return;

    const selectedPiece = this.gameState.getSelectedPiece();

    if (selectedPiece) {
      if (selectedPiece.to.includes(pos)) {
        // This is a move
        this.gameState.setSelectedMove({ from: selectedPiece.from, to: pos });
        const potentialMove = this.getPotentialMove(selectedPiece.from, pos);
        
        if (potentialMove && potentialMove.unstackable) {
          // Trigger unstack modal (handled by UI layer)
          const event = new CustomEvent('showUnstackModal', { detail: { from: selectedPiece.from, to: pos } });
          window.dispatchEvent(event);
        } else {
          this.playMove(selectedPiece.from, pos, false);
        }
      } else {
        // Clicked somewhere else, deselect
        this.gameState.setSelectedPiece(null);
        this.updateOverlays();
      }
    } else {
      const moves = this.getMovesForPiece(pos);
      if (moves.length > 0) {
        this.gameState.setSelectedPiece({ from: pos, to: moves });
        this.updateOverlays();
      }
    }
  }

  /**
   * Handle tile hover
   */
  private handleTileHover(pos: number | null): void {
    const boardData = this.gameState.getBoardData();
    if (!boardData) return;

    const selectedPiece = this.gameState.getSelectedPiece();
    
    if (pos !== null) {
      const piece = this.gameState.getPieceAt(pos);
      const currentTurn = boardData[81];
      
      if (piece && piece.color === currentTurn && (!selectedPiece || selectedPiece.from !== pos)) {
        if (this.gameState.getHoveredPiece() !== pos) {
          this.gameState.setHoveredPiece(pos);
          this.updateOverlays();
        }
      } else {
        if (this.gameState.getHoveredPiece() !== null) {
          this.gameState.setHoveredPiece(null);
          this.updateOverlays();
        }
      }
    } else {
      if (this.gameState.getHoveredPiece() !== null) {
        this.gameState.setHoveredPiece(null);
        this.updateOverlays();
      }
    }
  }

  /**
   * Update overlay highlights based on game state
   */
  private updateOverlays(): void {
    const highlights: TileHighlight[] = [];
    const selectedPiece = this.gameState.getSelectedPiece();
    const hoveredPiece = this.gameState.getHoveredPiece();

    // Selected piece
    if (selectedPiece) {
      highlights.push({ position: selectedPiece.from, type: 'selected' });
      
      // Possible moves
      for (const to of selectedPiece.to) {
        highlights.push({ position: to, type: 'possible' });
      }
    }

    // Hovered piece moves
    if (hoveredPiece !== null && (!selectedPiece || selectedPiece.from !== hoveredPiece)) {
      const hoveredMoves = this.getMovesForPiece(hoveredPiece);
      for (const to of hoveredMoves) {
        highlights.push({ position: to, type: 'hovered' });
      }
    }

    this.view.updateOverlays(highlights);
  }

  /**
   * Render the board
   */
  private async renderBoard(): Promise<void> {
    const boardData = this.gameState.getBoardData();
    if (!boardData) return;

    const flipped = this.gameState.isBoardFlipped();
    await this.view.render(boardData, flipped);
    this.updateOverlays();
  }

  /**
   * Get current turn
   */
  getCurrentTurn(): string {
    return this.gameState.getCurrentTurn();
  }

  /**
   * Get move history
   */
  getMoveHistory(): string[] {
    return this.gameState.getMoveHistory();
  }

  /**
   * Get selected move (for unstack modal)
   */
  getSelectedMove(): { from: number; to: number } | null {
    return this.gameState.getSelectedMove();
  }

  /**
   * Clear selected move
   */
  clearSelectedMove(): void {
    this.gameState.setSelectedMove(null);
    this.gameState.setSelectedPiece(null);
    this.updateOverlays();
  }
}
