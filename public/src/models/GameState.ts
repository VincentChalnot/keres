import {Board} from './types';

/**
 * Game state model
 */
export class GameState {
    private board: Board | null = null;
    private selectedPiece: { from: number; to: number[] } | null = null;
    private selectedMove: { from: number; to: number } | null = null;
    private boardFlipped = false;
    private hoveredPiece: number | null = null;
    private moveHistory: string[] = [];
    private gameHistory: Board[] = [];

    getBoard(): Board | null {
        return this.board;
    }

    setBoard(board: Board): void {
        this.board = board;
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

    getGameHistory(): Board[] {
        return this.gameHistory;
    }

    pushGameState(state: Board): void {
        this.gameHistory.push(state);
    }

    popGameState(): Board | undefined {
        return this.gameHistory.pop();
    }

    clearGameHistory(): void {
        this.gameHistory = [];
    }

    getCurrentTurn(): 'White' | 'Red' {
        if (!this.board) return 'White';
        return this.board.getCurrentTurn();
    }
}
