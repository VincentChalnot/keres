import {GameState} from '../models/GameState';
import {GameAPI} from '../network/GameAPI';
import {IBoardView, TileHighlight} from '../views/IBoardView';
import {posToAlgebraic, algebraicToPos, encodeBoardToHash, decodeBoardFromHash, decodeBoardFromBinary, encodeBoardToBinary} from '../utils/boardUtils';
import {PotentialMove, Board} from '../models/types';

/**
 * Main game controller - handles game logic and coordinates between model, view, and network
 */
export class GameController {
    private gameState: GameState;
    private api: GameAPI;
    private view: IBoardView;
    private possibleMoves: PotentialMove[] = [];

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
            const boardBinary = decodeBoardFromHash(window.location.hash);
            if (boardBinary) {
                const board = decodeBoardFromBinary(boardBinary);
                this.gameState.setBoard(board);
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
        const board = await this.api.getNewGame();
        this.gameState.setBoard(board);
        this.gameState.clearMoveHistory();
        this.gameState.clearGameHistory();
    }

    /**
     * Update possible moves from server
     */
    async updatePossibleMoves(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        this.possibleMoves = await this.api.getPossibleMoves(board);
    }

    /**
     * Get moves for a specific piece
     */
    getMovesForPiece(pos: number): number[] {
        const moves: number[] = [];
        for (const move of this.possibleMoves) {
            if (move.from === pos) {
                moves.push(move.to);
            }
        }
        return moves;
    }

    /**
     * Get potential move details
     */
    getPotentialMove(fromPos: number, toPos: number): PotentialMove | null {
        for (const move of this.possibleMoves) {
            if (move.from === fromPos && move.to === toPos) {
                return move;
            }
        }
        return null;
    }

    /**
     * Play a move
     */
    async playMove(from: number, to: number, unstack = false): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        // Save current state to history (clone the board)
        const boardCopy = new Board([...board.cells], board.whiteToMove);
        this.gameState.pushGameState(boardCopy);

        // Play move on server
        const newBoard = await this.api.playMove(board, {from, to, unstack});
        this.gameState.setBoard(newBoard);

        // Update URL hash
        window.location.hash = encodeBoardToHash(encodeBoardToBinary(newBoard));

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
        const board = this.gameState.getBoard();
        if (!board) return;

        const move = await this.api.getEngineMove(board);
        await this.playMove(move.from, move.to, move.unstack);
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

        this.gameState.setBoard(previousState);
        this.gameState.popMove();
        window.location.hash = encodeBoardToHash(encodeBoardToBinary(previousState));

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
        const board = this.gameState.getBoard();
        if (!board) return;

        const selectedPiece = this.gameState.getSelectedPiece();

        if (selectedPiece) {
            if (selectedPiece.to.includes(pos)) {
                // This is a move
                this.gameState.setSelectedMove({from: selectedPiece.from, to: pos});
                const potentialMove = this.getPotentialMove(selectedPiece.from, pos);

                if (potentialMove && potentialMove.unstackable) {
                    // Trigger unstack modal (handled by UI layer)
                    const event = new CustomEvent('showUnstackModal', {detail: {from: selectedPiece.from, to: pos}});
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
                this.gameState.setSelectedPiece({from: pos, to: moves});
                this.updateOverlays();
            }
        }
    }

    /**
     * Handle tile hover
     */
    private handleTileHover(pos: number | null): void {
        const board = this.gameState.getBoard();
        if (!board) return;

        const selectedPiece = this.gameState.getSelectedPiece();

        if (pos !== null) {
            const piece = board.getPieceAt(pos);
            const currentTurn = board.whiteToMove;

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
            highlights.push({position: selectedPiece.from, type: 'selected'});

            // Possible moves
            for (const to of selectedPiece.to) {
                highlights.push({position: to, type: 'possible'});
            }
        }

        // Hovered piece moves
        if (hoveredPiece !== null && (!selectedPiece || selectedPiece.from !== hoveredPiece)) {
            const hoveredMoves = this.getMovesForPiece(hoveredPiece);
            for (const to of hoveredMoves) {
                highlights.push({position: to, type: 'hovered'});
            }
        }

        this.view.updateOverlays(highlights);
    }

    /**
     * Render the board
     */
    private async renderBoard(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        const flipped = this.gameState.isBoardFlipped();
        const boardBinary = encodeBoardToBinary(board);
        await this.view.render(boardBinary, flipped);
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
