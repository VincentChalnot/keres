import {Board, PotentialMove} from './types.ts';

/**
 * Game state model
 */
export class GameState {
    private board: Board | null = null;
    private potentialMoves: PotentialMove[] = [];
    private opponentThreats: PotentialMove[] = [];
    private showThreats = true;
    private selectedPosition: number | null = null;
    private clickedDestination: number | null = null;
    private boardFlipped = false;
    private hoveredPosition: number | null = null;
    private moveHistory: string[] = [];
    private gameHistory: Board[] = [];

    getBoard(): Board | null {
        return this.board;
    }

    setBoard(board: Board): void {
        this.board = board;
    }

    getPotentialMoves(): PotentialMove[] {
        return this.potentialMoves;
    }

    setPotentialMoves(moves: PotentialMove[]): void {
        this.potentialMoves = moves;
    }

    getOpponentThreats(): PotentialMove[] {
        return this.opponentThreats;
    }

    setOpponentThreats(threats: PotentialMove[]): void {
        this.opponentThreats = threats;
    }

    getOpponentThreatsForPosition(pos: number): PotentialMove[] {
        const threats: PotentialMove[] = [];
        for (const threat of this.opponentThreats) {
            if (threat.from === pos) {
                threats.push(threat);
            }
        }
        return threats;
    }

    isShowThreats(): boolean {
        return this.showThreats;
    }

    setShowThreats(show: boolean): void {
        this.showThreats = show;
    }

    getPotentialMovesForPosition(pos: number): PotentialMove[] {
        const moves: PotentialMove[] = [];
        for (const move of this.potentialMoves) {
            if (move.from === pos) {
                moves.push(move);
            }
        }
        return moves;
    }

    getPotentialMove(fromPos: number, toPos: number): PotentialMove | null {
        for (const move of this.potentialMoves) {
            if (move.from === fromPos && move.to === toPos) {
                return move;
            }
        }
        return null;
    }

    getSelectedPosition(): number | null {
        return this.selectedPosition;
    }

    setSelectedPosition(position: number | null): void {
        if (position === null) {
            this.clickedDestination = null;
        }
        this.selectedPosition = position;
    }

    getClickedDestination(): number | null {
        return this.clickedDestination;
    }

    setClickedDestination(position: number | null): void {
        this.clickedDestination = position;
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

    getHoveredPosition(): number | null {
        return this.hoveredPosition;
    }

    setHoveredPosition(position: number | null): void {
        this.hoveredPosition = position;
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
