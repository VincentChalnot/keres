import {GameState} from '../models/GameState';
import {GameAPI} from '../network/GameAPI';
import {IBoardView, TileHighlight} from '../views/IBoardView';
import {
    posToAlgebraic,
    algebraicToPos,
    encodeBoardToHash,
    decodeBoardFromHash,
    decodeBoardFromBinary,
    encodeBoardToBinary
} from '../utils/boardUtils';
import {Board} from '../models/types';

/**
 * Main game controller - handles game logic and coordinates between model, view, and network
 */
export class GameController {
    private gameState: GameState;
    private api: GameAPI;
    private view: IBoardView;
    private updatingHashProgrammatically = false;

    constructor(gameState: GameState, api: GameAPI, view: IBoardView) {
        this.gameState = gameState;
        this.api = api;
        this.view = view;

        // Set up view event handlers
        this.view.onTileClick((pos) => this.handleTileClick(pos));
        this.view.onTileHover((pos) => this.handleTileHover(pos));

        // Set up hash change listener for browser history navigation
        window.addEventListener('hashchange', () => this.handleHashChange());
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

        await this.updatePotentialMoves();
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
     * Update potential moves from server
     */
    async updatePotentialMoves(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        this.gameState.setPotentialMoves(await this.api.getPotentialMoves(board));
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

        // Update URL hash (set flag to prevent hashchange handler from triggering)
        this.updatingHashProgrammatically = true;
        window.location.hash = encodeBoardToHash(encodeBoardToBinary(newBoard));
        this.updatingHashProgrammatically = false;

        // Record move in algebraic notation
        const moveNotation = posToAlgebraic(from) + '-' + posToAlgebraic(to);
        this.gameState.addMove(moveNotation);

        // Reset selection
        this.gameState.setSelectedPosition(null);

        // Update game state
        await this.updatePotentialMoves();
        await this.renderBoard();
    }

    /**
     * Request engine move (MCTS)
     */
    async requestEngineMove(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        const move = await this.api.getEngineMove(board);
        await this.playMove(move.from, move.to, move.unstack);
    }

    /**
     * Request minimax engine move
     */
    async requestMinimaxMove(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        const move = await this.api.getMinimaxMove(board);
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
        
        // Update URL hash (set flag to prevent hashchange handler from triggering)
        this.updatingHashProgrammatically = true;
        window.location.hash = encodeBoardToHash(encodeBoardToBinary(previousState));
        this.updatingHashProgrammatically = false;

        this.gameState.setSelectedPosition(null);
        await this.updatePotentialMoves();
        await this.renderBoard();
    }

    /**
     * Flip board view
     */
    async flipBoard(): Promise<void> {
        this.gameState.flipBoard();
        this.gameState.setSelectedPosition(null);
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

        const selectedPosition = this.gameState.getSelectedPosition();

        // No selected position: select piece if potential
        if (!selectedPosition) {
            const moves = this.gameState.getPotentialMovesForPosition(pos);
            if (moves.length > 0) { // Only select if there are potential moves
                this.gameState.setSelectedPosition(pos);
                this.updateOverlays();
            }
            return;
        }

        // Deselect if clicking the same piece
        if (selectedPosition === pos) {
            this.gameState.setSelectedPosition(null);
            this.updateOverlays();
            return;
        }

        // Look for a move from selectedPosition to pos
        const moves = this.gameState.getPotentialMovesForPosition(selectedPosition);
        for (const move of moves) {
            if (move.to !== pos) continue;
            // This is a move
            if (move.unstackable && !move.force_unstack) {
                // Trigger unstack modal (handled by UI layer)
                this.gameState.setClickedDestination(pos);
                window.dispatchEvent(new CustomEvent('showUnstackModal'));
            } else {
                this.playMove(selectedPosition, pos, move.force_unstack);
            }
            return;
        }

        // If we reach here, clicked on a different piece - change selection if potential
        const newMoves = this.gameState.getPotentialMovesForPosition(pos);
        if (newMoves.length > 0) {
            this.gameState.setSelectedPosition(pos);
            this.updateOverlays();
        } else {
            // Invalid selection - clear selection
            this.gameState.setSelectedPosition(null);
            this.updateOverlays();
        }
    }

    /**
     * Handle tile hover
     */
    private handleTileHover(pos: number | null): void {
        if (pos === null) {
            this.gameState.setHoveredPosition(null);
            this.updateOverlays();
            return;
        }

        const board = this.gameState.getBoard();
        if (!board) return;

        const selectedPosition = this.gameState.getSelectedPosition();

        // Show potential moves for hovered piece if no piece is selected
        if (!selectedPosition) {
            this.gameState.setHoveredPosition(pos);
            this.updateOverlays();
        }
    }

    /**
     * Update overlay highlights based on game state
     */
    private updateOverlays(): void {
        const highlights: TileHighlight[] = [];
        const selectedPosition = this.gameState.getSelectedPosition();

        // Selected piece
        if (selectedPosition) {
            highlights.push({position: selectedPosition, type: 'selected'});

            // Potential moves
            for (const move of this.gameState.getPotentialMovesForPosition(selectedPosition)) {
                highlights.push({position: move.to, type: 'potential'});
            }
            this.view.updateOverlays(highlights);
            return;
        }

        const hoveredPosition = this.gameState.getHoveredPosition();
        if (hoveredPosition) {
            for (const move of this.gameState.getPotentialMovesForPosition(hoveredPosition)) {
                highlights.push({position: move.to, type: 'hovered'});
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
     * Handle hash change event (browser back/forward navigation)
     */
    private async handleHashChange(): Promise<void> {
        // Ignore hash changes that we triggered programmatically
        if (this.updatingHashProgrammatically) {
            return;
        }

        // Load board state from the new hash
        if (window.location.hash) {
            const boardBinary = decodeBoardFromHash(window.location.hash);
            if (boardBinary) {
                const board = decodeBoardFromBinary(boardBinary);
                this.gameState.setBoard(board);
                
                // Clear move history since we're loading a specific board state
                // without knowing the move sequence that led to it
                this.gameState.clearMoveHistory();
                this.gameState.clearGameHistory();
                
                // Clear selection and update the view
                this.gameState.setSelectedPosition(null);
                await this.updatePotentialMoves();
                await this.renderBoard();
                
                // Dispatch event to notify UI to update status and move history display
                window.dispatchEvent(new CustomEvent('boardStateChanged'));
            }
        } else {
            // No hash - start a new game
            await this.startNewGame();
            await this.updatePotentialMoves();
            await this.renderBoard();
            
            // Dispatch event to notify UI to update
            window.dispatchEvent(new CustomEvent('boardStateChanged'));
        }
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
     * Clear selected move
     */
    clearSelectedMove(): void {
        this.gameState.setSelectedPosition(null);
        this.updateOverlays();
    }
}
