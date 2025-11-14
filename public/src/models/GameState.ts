import { Piece, PIECE_CODE } from './types';

/**
 * Game state model
 */
export class GameState {
  private boardData: Uint8Array | null = null;
  private possibleMoves: number[] = [];
  private selectedPiece: { from: number; to: number[] } | null = null;
  private selectedMove: { from: number; to: number } | null = null;
  private boardFlipped = false;
  private hoveredPiece: number | null = null;
  private moveHistory: string[] = [];
  private gameHistory: Uint8Array[] = [];

  getBoardData(): Uint8Array | null {
    return this.boardData;
  }

  setBoardData(data: Uint8Array): void {
    this.boardData = data;
  }

  getPossibleMoves(): number[] {
    return this.possibleMoves;
  }

  setPossibleMoves(moves: number[]): void {
    this.possibleMoves = moves;
  }

  getSelectedPiece(): { from: number; to: number[] } | null {
    return this.selectedPiece;
  }

  setSelectedPiece(piece: { from: number; to: number[] } | null): void {
    this.selectedPiece = piece;
  }

  getSelectedMove(): { from: number; to: number } | null {
    return this.selectedMove;
  }

  setSelectedMove(move: { from: number; to: number } | null): void {
    this.selectedMove = move;
  }

  isBoardFlipped(): boolean {
    return this.boardFlipped;
  }

  flipBoard(): void {
    this.boardFlipped = !this.boardFlipped;
  }

  setBoardFlipped(flipped: boolean): void {
    this.boardFlipped = flipped;
  }

  getHoveredPiece(): number | null {
    return this.hoveredPiece;
  }

  setHoveredPiece(piece: number | null): void {
    this.hoveredPiece = piece;
  }

  getMoveHistory(): string[] {
    return this.moveHistory;
  }

  addMove(move: string): void {
    this.moveHistory.push(move);
  }

  popMove(): void {
    this.moveHistory.pop();
  }

  clearMoveHistory(): void {
    this.moveHistory = [];
  }

  getGameHistory(): Uint8Array[] {
    return this.gameHistory;
  }

  pushGameState(state: Uint8Array): void {
    this.gameHistory.push(state);
  }

  popGameState(): Uint8Array | undefined {
    return this.gameHistory.pop();
  }

  clearGameHistory(): void {
    this.gameHistory = [];
  }

  getCurrentTurn(): 'White' | 'Red' {
    if (!this.boardData) return 'White';
    return this.boardData[81] === 1 ? 'White' : 'Red';
  }

  /**
   * Decode a piece from board data
   */
  decodePiece(piece: number): Piece | null {
    if (piece === 0) return null;
    const color = (piece >> 6) & 0b1;
    const payload = piece & 0b00111111;

    if (payload === 0b111000) {
      return { top: 'king', bottom: null, color };
    }

    const topCode = (payload >> 3) & 0b111;
    const bottomCode = payload & 0b111;

    if (topCode === 0) {
      if (PIECE_CODE[bottomCode]) {
        return { top: PIECE_CODE[bottomCode], bottom: null, color };
      }
    } else {
      if (PIECE_CODE[topCode] && PIECE_CODE[bottomCode]) {
        return { top: PIECE_CODE[topCode], bottom: PIECE_CODE[bottomCode], color };
      }
    }
    return null;
  }

  /**
   * Get piece at position
   */
  getPieceAt(position: number): Piece | null {
    if (!this.boardData || position < 0 || position >= 81) return null;
    return this.decodePiece(this.boardData[position]);
  }
}
