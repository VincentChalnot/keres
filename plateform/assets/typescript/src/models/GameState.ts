import {Board, Move, PotentialMove} from './types';

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
    private moveList: Move[] = []; // List of all moves played
    private currentMoveIndex: number = -1; // -1 means at initial position, 0+ is after that move
    private boardLocked: boolean = false; // True when viewing history, not at latest move
    private forceFullStack: boolean = false; // True when shift-clicking to move full stack

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

    getCurrentTurn(): 'White' | 'Black' {
        if (!this.board) return 'White';
        return this.board.getCurrentTurn();
    }

    getLastMove(): Move | null {
        if (this.moveList.length === 0 || this.currentMoveIndex < 0) {
            return null;
        }

        return this.moveList[this.currentMoveIndex];
    }

    // Move list management
    getMoveList(): Move[] {
        return this.moveList;
    }

    setMoveList(moves: Move[]): void {
        this.moveList = moves;
    }

    addMoveToList(move: Move): void {
        this.moveList.push(move);
    }

    clearMoveList(): void {
        this.moveList = [];
    }

    getCurrentMoveIndex(): number {
        return this.currentMoveIndex;
    }

    setCurrentMoveIndex(index: number): void {
        this.currentMoveIndex = index;
    }

    isBoardLocked(): boolean {
        return this.boardLocked;
    }

    setBoardLocked(locked: boolean): void {
        this.boardLocked = locked;
    }

    isAtLatestMove(): boolean {
        return this.currentMoveIndex === this.moveList.length - 1;
    }

    isForceFullStack(): boolean {
        return this.forceFullStack;
    }

    setForceFullStack(force: boolean): void {
        this.forceFullStack = force;
    }
}
