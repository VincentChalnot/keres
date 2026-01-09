import {GameState} from '../models/GameState';
import {GameAPI} from '../network/GameAPI';
import {IBoardView, TileHighlight} from '../views/IBoardView';
import {
    posToAlgebraic,
    algebraicToPos,
    encodeMoveListToHash,
    decodeMoveListFromHash,
    decodeBoardFromBinary,
    encodeBoardToBinary
} from '../utils/boardUtils';
import {Board, Move} from '../models/types';

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
            const moves = decodeMoveListFromHash(window.location.hash);
            if (moves && moves.length > 0) {
                // Load from move list
                await this.loadFromMoveList(moves);
            } else {
                // Invalid or empty hash, start new game
                await this.startNewGame();
            }
        } else {
            await this.startNewGame();
        }

        await this.updatePotentialMoves();
        await this.renderBoard();
    }

    /**
     * Load game from a move list
     */
    private async loadFromMoveList(moves: Move[]): Promise<void> {
        // Start with a new board
        const initialBoard = await this.api.getNewGame();
        
        // Store the move list
        this.gameState.setMoveList(moves);
        this.gameState.clearMoveHistory();
        this.gameState.clearGameHistory();
        
        // Replay moves locally
        const localBoard = new Board(
            [...initialBoard.cells],
            initialBoard.whiteToMove,
            initialBoard.gameOver,
            initialBoard.whiteWins,
            initialBoard.draw,
            initialBoard.movesWithoutCapture
        );
        
        for (const move of moves) {
            localBoard.applyMoveLocally(move);
            // Add to move history in algebraic notation
            const moveNotation = posToAlgebraic(move.from) + '-' + posToAlgebraic(move.to);
            this.gameState.addMove(moveNotation);
        }
        
        // Set the board from local replay
        this.gameState.setBoard(localBoard);
        this.gameState.setCurrentMoveIndex(moves.length - 1);
        this.gameState.setBoardLocked(false);
        
        // Set last move
        if (moves.length > 0) {
            const lastMove = moves[moves.length - 1];
            this.gameState.setLastMove({from: lastMove.from, to: lastMove.to});
        }
        
        // Asynchronously verify with server
        this.api.replayMoves(moves).then(serverBoard => {
            // Compare local board with server board
            const localBinary = encodeBoardToBinary(localBoard);
            const serverBinary = encodeBoardToBinary(serverBoard);
            
            // Check if they match
            let match = true;
            if (localBinary.length !== serverBinary.length) {
                match = false;
            } else {
                for (let i = 0; i < localBinary.length; i++) {
                    if (localBinary[i] !== serverBinary[i]) {
                        match = false;
                        break;
                    }
                }
            }
            
            if (!match) {
                console.error("Board state mismatch between client and server!");
                // Use server board as source of truth
                this.gameState.setBoard(serverBoard);
                this.renderBoard();
            } else {
                console.log("Board state verified with server ✓");
            }
        }).catch(error => {
            console.error("Failed to verify board state with server:", error);
        });
    }

    /**
     * Start a new game
     */
    async startNewGame(): Promise<void> {
        const board = await this.api.getNewGame();
        this.gameState.setBoard(board);
        this.gameState.clearMoveHistory();
        this.gameState.clearGameHistory();
        this.gameState.clearMoveList();
        this.gameState.setCurrentMoveIndex(-1);
        this.gameState.setBoardLocked(false);
        this.gameState.setLastMove(null);
        
        // Clear URL hash
        this.updatingHashProgrammatically = true;
        window.location.hash = '';
        setTimeout(() => {
            this.updatingHashProgrammatically = false;
        }, 0);
    }

    /**
     * Update potential moves from server
     */
    async updatePotentialMoves(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        this.gameState.setPotentialMoves(await this.api.getPotentialMoves(board));
        
        // Also fetch opponent threats
        this.gameState.setOpponentThreats(await this.api.getOpponentThreats(board));
    }

    /**
     * Play a move
     */
    async playMove(from: number, to: number, unstack = false): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;

        // Don't allow moves if board is locked (viewing history)
        if (this.gameState.isBoardLocked()) {
            console.log('Board is locked - navigate to latest move first');
            return;
        }

        // Check if game is over
        if (board.isGameOver()) {
            console.log('Game is over - no more moves allowed');
            return;
        }

        // Save current state to history (clone the board with all properties)
        const boardCopy = new Board(
            [...board.cells],
            board.whiteToMove,
            board.gameOver,
            board.whiteWins,
            board.draw,
            board.movesWithoutCapture
        );
        this.gameState.pushGameState(boardCopy);

        // Play move on server
        const newBoard = await this.api.playMove(board, {from, to, unstack});
        this.gameState.setBoard(newBoard);
        
        // Record last move IMMEDIATELY after setting board
        this.gameState.setLastMove({from, to});

        // Add move to move list
        const move: Move = {from, to, unstack};
        this.gameState.addMoveToList(move);
        this.gameState.setCurrentMoveIndex(this.gameState.getMoveList().length - 1);

        // Update URL hash with move list (use replaceState to avoid history pollution)
        this.updatingHashProgrammatically = true;
        const newHash = encodeMoveListToHash(this.gameState.getMoveList());
        window.history.replaceState(null, '', '#' + newHash);
        // setTimeout with 0ms ensures hashchange event handler runs before flag is cleared
        setTimeout(() => {
            this.updatingHashProgrammatically = false;
        }, 0);

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
        const moveList = this.gameState.getMoveList();
        if (moveList.length === 0) {
            alert('No moves to undo');
            return;
        }

        // Remove the last move from the move list
        moveList.pop();
        this.gameState.setMoveList(moveList);
        this.gameState.popMove();

        // Replay all remaining moves from the start
        const initialBoard = await this.api.getNewGame();
        const board = new Board(
            [...initialBoard.cells],
            initialBoard.whiteToMove,
            initialBoard.gameOver,
            initialBoard.whiteWins,
            initialBoard.draw,
            initialBoard.movesWithoutCapture
        );

        let lastMove = null;
        for (const move of moveList) {
            board.applyMoveLocally(move);
            lastMove = move;
        }

        this.gameState.setBoard(board);
        this.gameState.setCurrentMoveIndex(moveList.length - 1);
        this.gameState.setBoardLocked(false);
        this.gameState.setLastMove(lastMove ? {from: lastMove.from, to: lastMove.to} : null);

        // Update URL hash (use replaceState)
        this.updatingHashProgrammatically = true;
        const newHash = moveList.length > 0 ? encodeMoveListToHash(moveList) : '';
        window.history.replaceState(null, '', newHash ? '#' + newHash : window.location.pathname);
        setTimeout(() => {
            this.updatingHashProgrammatically = false;
        }, 0);

        // Clear game history since we're using move list now
        this.gameState.clearGameHistory();
        this.gameState.setSelectedPosition(null);
        await this.updatePotentialMoves();
        await this.renderBoard();
    }

    /**
     * Navigate to previous move
     */
    async navigateToPreviousMove(): Promise<void> {
        const currentIndex = this.gameState.getCurrentMoveIndex();
        if (currentIndex < 0) {
            return; // Already at start
        }

        // Replay moves up to the previous position
        const targetIndex = currentIndex - 1;
        await this.navigateToMoveIndex(targetIndex);
    }

    /**
     * Navigate to next move
     */
    async navigateToNextMove(): Promise<void> {
        const currentIndex = this.gameState.getCurrentMoveIndex();
        const moveList = this.gameState.getMoveList();
        
        if (currentIndex >= moveList.length - 1) {
            return; // Already at end
        }

        // Replay moves up to the next position
        const targetIndex = currentIndex + 1;
        await this.navigateToMoveIndex(targetIndex);
    }

    /**
     * Navigate to a specific move index
     */
    private async navigateToMoveIndex(targetIndex: number): Promise<void> {
        const moveList = this.gameState.getMoveList();
        
        // Start with initial board
        const initialBoard = await this.api.getNewGame();
        const board = new Board(
            [...initialBoard.cells],
            initialBoard.whiteToMove,
            initialBoard.gameOver,
            initialBoard.whiteWins,
            initialBoard.draw,
            initialBoard.movesWithoutCapture
        );

        // Replay moves up to targetIndex
        let lastMove = null;
        for (let i = 0; i <= targetIndex; i++) {
            const move = moveList[i];
            board.applyMoveLocally(move);
            lastMove = move;
        }

        this.gameState.setBoard(board);
        this.gameState.setCurrentMoveIndex(targetIndex);
        this.gameState.setLastMove(lastMove ? {from: lastMove.from, to: lastMove.to} : null);
        
        // Lock board if not at latest move
        const isAtLatest = targetIndex === moveList.length - 1;
        this.gameState.setBoardLocked(!isAtLatest);

        this.gameState.setSelectedPosition(null);
        if (isAtLatest) {
            await this.updatePotentialMoves();
        } else {
            // Clear potential moves when viewing history
            this.gameState.setPotentialMoves([]);
            this.gameState.setOpponentThreats([]);
        }
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

        // Don't allow clicks if board is locked
        if (this.gameState.isBoardLocked()) {
            console.log('Board is locked - navigate to latest move to make moves');
            return;
        }

        // Check if game is over - don't allow any clicks
        if (board.isGameOver()) {
            console.log('Game is over - no more moves allowed');
            return;
        }

        const selectedPosition = this.gameState.getSelectedPosition();

        // No selected position: select piece if potential
        if (selectedPosition === null) {
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
        if (selectedPosition === null) {
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

        // Add last move highlights (show these even when a piece is selected)
        const lastMove = this.gameState.getLastMove();
        if (lastMove) {
            highlights.push({position: lastMove.from, type: 'last_move'});
            highlights.push({position: lastMove.to, type: 'last_move'});
        }

        // Selected piece
        if (selectedPosition != null) {
            highlights.push({position: selectedPosition, type: 'selected'});

            // Potential moves
            for (const move of this.gameState.getPotentialMovesForPosition(selectedPosition)) {
                highlights.push({position: move.to, type: 'potential'});
            }
            this.view.updateOverlays(highlights);
            return;
        }

        const hoveredPosition = this.gameState.getHoveredPosition();
        if (hoveredPosition != null) {
            const board = this.gameState.getBoard();
            if (board) {
                const piece = board.getPieceAt(hoveredPosition);
                
                // If hovering over an enemy piece and show threats is enabled, show threats in red
                if (piece && piece.color !== board.whiteToMove && this.gameState.isShowThreats()) {
                    for (const threat of this.gameState.getOpponentThreatsForPosition(hoveredPosition)) {
                        highlights.push({position: threat.to, type: 'threat'});
                    }
                } else {
                    // Otherwise show potential moves for friendly pieces
                    for (const move of this.gameState.getPotentialMovesForPosition(hoveredPosition)) {
                        highlights.push({position: move.to, type: 'hovered'});
                    }
                }
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

        // Load board state from the new hash (now contains move list)
        if (window.location.hash) {
            const moves = decodeMoveListFromHash(window.location.hash);
            if (moves && moves.length > 0) {
                await this.loadFromMoveList(moves);
                await this.updatePotentialMoves();
                await this.renderBoard();
                window.dispatchEvent(new CustomEvent('boardStateChanged'));
            }
        } else {
            // No hash - start a new game
            await this.startNewGame();
            await this.updatePotentialMoves();
            await this.renderBoard();
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

    /**
     * Toggle showing threats
     */
    toggleShowThreats(): void {
        this.gameState.setShowThreats(!this.gameState.isShowThreats());
        this.updateOverlays();
    }

    /**
     * Get show threats state
     */
    isShowThreats(): boolean {
        return this.gameState.isShowThreats();
    }

    /**
     * Navigate to previous move in history
     */
    async previousMove(): Promise<void> {
        await this.navigateToPreviousMove();
        window.dispatchEvent(new CustomEvent('boardStateChanged'));
    }

    /**
     * Navigate to next move in history
     */
    async nextMove(): Promise<void> {
        await this.navigateToNextMove();
        window.dispatchEvent(new CustomEvent('boardStateChanged'));
    }

    /**
     * Check if board is locked (viewing history)
     */
    isBoardLocked(): boolean {
        return this.gameState.isBoardLocked();
    }

    /**
     * Check if can navigate to previous move
     */
    canNavigateToPrevious(): boolean {
        return this.gameState.getCurrentMoveIndex() >= 0;
    }

    /**
     * Check if can navigate to next move
     */
    canNavigateToNext(): boolean {
        const moveList = this.gameState.getMoveList();
        return this.gameState.getCurrentMoveIndex() < moveList.length - 1;
    }
}
