import {GameState} from '../models/GameState';
import {GameAPI} from '../network/GameAPI';
import {MercureClient, GameUpdate} from '../network/MercureClient';
import {IBoardView, TileHighlight} from '../views/IBoardView';
import {Move} from '../models/types';
import {decodeMoveListFromBase64, posToAlgebraic, encodeBoardToBinary} from '../utils/boardUtils';

/**
 * Main game controller - handles game logic and coordinates between model, view, and network
 */
export class GameController {
    private gameState: GameState;
    private api: GameAPI;
    private view: IBoardView;
    private mercureClient: MercureClient | null = null;

    constructor(gameState: GameState, api: GameAPI, view: IBoardView) {
        this.gameState = gameState;
        this.api = api;
        this.view = view;

        // Set up view event handlers
        this.view.onTileClick((pos, shiftKey) => this.handleTileClick(pos, shiftKey));
        this.view.onTileHover((pos) => this.handleTileHover(pos));
        if (this.view.onDragMove) {
            this.view.onDragMove((from, to, shiftKey) => this.handleDragMove(from, to, shiftKey));
        }
    }

    /**
     * Initialize Mercure connection for real-time updates
     */
    initializeMercure(gameUuid: string): void {
        this.mercureClient = new MercureClient();
        this.mercureClient.subscribe(gameUuid, (update: GameUpdate) => {
            this.handleMercureUpdate(update);
        });
    }

    /**
     * Handle incoming Mercure update
     */
    private async handleMercureUpdate(update: GameUpdate): Promise<void> {
        console.log('Received Mercure update:', update);

        // Update the board state
        this.gameState.setBoard(update.board);

        // Decode moves from the update
        const moves: Move[] = [];
        for (let i = 0; i < update.moves.length; i++) {
            const moveU16 = update.moves[i];
            const from = moveU16 & 0x7F;
            const to = (moveU16 >> 7) & 0x7F;
            const unstack = ((moveU16 >> 14) & 0x1) === 1;
            moves.push({from, to, unstack});
        }

        // Update move list
            this.gameState.setMoveList(moves);
            this.gameState.setCurrentMoveIndex(moves.length - 1);

            // Update move history
            this.gameState.clearMoveHistory();
            for (const move of moves) {
            const fromPos = posToAlgebraic(move.from);
            const toPos = posToAlgebraic(move.to);
            const notation = `${fromPos}-${toPos}${move.unstack ? '*' : ''}`;
            this.gameState.addMove(notation);
        }

        // Unlock board if it was locked waiting for AI
        this.gameState.setBoardLocked(false);

        // Update view
        await this.updatePotentialMoves();
        await this.renderBoard();
        window.dispatchEvent(new CustomEvent('boardStateChanged'));
    }

    /**
     * Disconnect from Mercure
     */
    disconnectMercure(): void {
        if (this.mercureClient) {
            this.mercureClient.disconnect();
            this.mercureClient = null;
        }
    }

    /**
     * Set the move list and update the board from the backend
     */
    async setMoves(moves: Move[]): Promise<void> {
        this.gameState.setMoveList(moves);
        this.gameState.clearMoveHistory();
        this.gameState.clearGameHistory();
        
        // Build move history from moves using algebraic notation
        for (const move of moves) {
            const fromPos = posToAlgebraic(move.from);
            const toPos = posToAlgebraic(move.to);
            const notation = `${fromPos}-${toPos}${move.unstack ? '*' : ''}`;
            this.gameState.addMove(notation);
        }
        
        const board = await this.api.replayMoves(moves);
        this.gameState.setBoard(board);
        this.gameState.setCurrentMoveIndex(moves.length - 1);
        this.gameState.setBoardLocked(false);
        await this.updatePotentialMoves();
        await this.renderBoard();
    }

    /**
     * Update potential moves from server
     */
    async updatePotentialMoves(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;
        this.gameState.setPotentialMoves(await this.api.getPotentialMoves(board));
        this.gameState.setOpponentThreats(await this.api.getOpponentThreats(board));
    }

    /**
     * Play a move
     */
    async playMove(from: number, to: number, unstack = false): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;
        if (this.gameState.isBoardLocked()) return;
        if (board.isGameOver()) return;
        
        // Lock board while waiting for server response (especially for AI moves)
        this.gameState.setBoardLocked(true);
        
        // Submit move to server
        const move: Move = {from, to, unstack};
        try {
            const result = await this.api.submitMove(move);
            this.gameState.setBoard(result.board);

            // Update move list with all moves (including AI response if any)
            this.gameState.addMoveToList(move);
            this.gameState.setCurrentMoveIndex(this.gameState.getMoveList().length - 1);

            // Update move history
            this.gameState.clearMoveHistory();
   
            for (const mv of this.gameState.getMoveList()) {
                const fromPos = posToAlgebraic(mv.from);
                const toPos = posToAlgebraic(mv.to);
                const notation = `${fromPos}-${toPos}${mv.unstack ? '*' : ''}`;
                this.gameState.addMove(notation);
            }

            // Unlock board after successful move
            this.gameState.setBoardLocked(false);

            await this.updatePotentialMoves();
            await this.renderBoard();
            window.dispatchEvent(new CustomEvent('boardStateChanged'));
            window.dispatchEvent(new CustomEvent('moveSubmitted'));
        } catch (error) {
            // Unlock board on error
            this.gameState.setBoardLocked(false);
            console.error('Failed to play move:', error);
            alert('Failed to play move: ' + (error as Error).message);
        }
    }

    async requestEngineMove(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;
        const move = await this.api.getEngineMove(board);
        await this.playMove(move.from, move.to, move.unstack);
    }

    async undoMove(): Promise<void> {
        try {
            const movesBase64 = await this.api.undoMove();
            let moves: Move[];
            try {
                moves = decodeMoveListFromBase64(movesBase64);
            } catch (error) {
                console.error('Failed to decode move stack:', error);
                alert('Failed to decode move stack: ' + (error as Error).message);
                moves = [];
            }
            await this.setMoves(moves);
            window.dispatchEvent(new CustomEvent('boardStateChanged'));
        } catch (error) {
            console.error('Failed to undo move:', error);
            alert('Failed to undo move: ' + (error as Error).message);
        }
    }

    async navigateToPreviousMove(): Promise<void> {
        const currentIndex = this.gameState.getCurrentMoveIndex();
        if (currentIndex < 0) return;
        await this.navigateToMoveIndex(currentIndex - 1);
    }

    async navigateToNextMove(): Promise<void> {
        const currentIndex = this.gameState.getCurrentMoveIndex();
        const moveList = this.gameState.getMoveList();
        if (currentIndex >= moveList.length - 1) return;
        await this.navigateToMoveIndex(currentIndex + 1);
    }

    private async navigateToMoveIndex(targetIndex: number): Promise<void> {
        const fullMoveList = [...this.gameState.getMoveList()];
        const movesToReplay = fullMoveList.slice(0, targetIndex + 1);

        // Replay board to target position
        const board = await this.api.replayMoves(movesToReplay);
        this.gameState.setBoard(board);
        this.gameState.setCurrentMoveIndex(targetIndex);

        // Restore the full move list (setMoves would have replaced it)
        this.gameState.setMoveList(fullMoveList);

        // Lock board if not at latest move
        this.gameState.setBoardLocked(targetIndex < fullMoveList.length - 1);

        // Rebuild move history display from full list
        this.gameState.clearMoveHistory();
        for (const mv of fullMoveList) {
            const fromPos = posToAlgebraic(mv.from);
            const toPos = posToAlgebraic(mv.to);
            const notation = `${fromPos}-${toPos}${mv.unstack ? '*' : ''}`;
            this.gameState.addMove(notation);
        }

        await this.updatePotentialMoves();
        await this.renderBoard();
    }

    async flipBoard(): Promise<void> {
        this.gameState.flipBoard();
        this.gameState.setSelectedPosition(null);
        await this.renderBoard();
    }

    private handleTileClick(pos: number, shiftKey?: boolean): void {
        const board = this.gameState.getBoard();
        if (!board) return;
        if (this.gameState.isBoardLocked()) return;
        if (board.isGameOver()) return;
        const selectedPosition = this.gameState.getSelectedPosition();
        if (selectedPosition === null) {
            const moves = this.gameState.getPotentialMovesForPosition(pos);
            if (moves.length > 0) {
                this.gameState.setSelectedPosition(pos);
                this.updateOverlays();
            }
            return;
        }
        if (selectedPosition === pos) {
            this.gameState.setSelectedPosition(null);
            this.updateOverlays();
            return;
        }
        const moves = this.gameState.getPotentialMovesForPosition(selectedPosition);
        for (const move of moves) {
            if (move.to !== pos) continue;
            if (move.unstackable && !move.force_unstack && !shiftKey) {
                this.gameState.setClickedDestination(pos);
                window.dispatchEvent(new CustomEvent('showUnstackModal'));
            } else {
                this.gameState.setSelectedPosition(null);
                this.playMove(selectedPosition, pos, move.force_unstack);
            }
            return;
        }
        const newMoves = this.gameState.getPotentialMovesForPosition(pos);
        if (newMoves.length > 0) {
            this.gameState.setSelectedPosition(pos);
            this.updateOverlays();
        } else {
            this.gameState.setSelectedPosition(null);
            this.updateOverlays();
        }
    }

    private handleTileHover(pos: number | null): void {
        if (pos === null) {
            this.gameState.setHoveredPosition(null);
            this.updateOverlays();
            return;
        }
        const board = this.gameState.getBoard();
        if (!board) return;
        const selectedPosition = this.gameState.getSelectedPosition();
        if (selectedPosition === null) {
            this.gameState.setHoveredPosition(pos);
            this.updateOverlays();
        }
    }

    private handleDragMove(from: number, to: number, shiftKey?: boolean): void {
        const board = this.gameState.getBoard();
        if (!board) return;
        if (this.gameState.isBoardLocked()) return;
        if (board.isGameOver()) return;

        const moves = this.gameState.getPotentialMovesForPosition(from);
        for (const move of moves) {
            if (move.to !== to) continue;
            if (move.unstackable && !move.force_unstack && !shiftKey) {
                this.gameState.setSelectedPosition(from);
                this.gameState.setClickedDestination(to);
                window.dispatchEvent(new CustomEvent('showUnstackModal'));
            } else {
                this.gameState.setSelectedPosition(null);
                this.updateOverlays();
                this.playMove(from, to, move.force_unstack);
            }
            return;
        }

        // Invalid destination: deselect
        this.gameState.setSelectedPosition(null);
        this.updateOverlays();
    }

    private updateOverlays(): void {
        const highlights: TileHighlight[] = [];
        const selectedPosition = this.gameState.getSelectedPosition();
        const lastMove = this.gameState.getLastMove();
        if (lastMove) {
            highlights.push({position: lastMove.from, type: 'last_move'});
            highlights.push({position: lastMove.to, type: 'last_move'});
        }
        if (selectedPosition != null) {
            highlights.push({position: selectedPosition, type: 'selected'});
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
                if (piece && piece.color !== board.whiteToMove && this.gameState.isShowThreats()) {
                    for (const threat of this.gameState.getOpponentThreatsForPosition(hoveredPosition)) {
                        highlights.push({position: threat.to, type: 'threat'});
                    }
                } else {
                    for (const move of this.gameState.getPotentialMovesForPosition(hoveredPosition)) {
                        highlights.push({position: move.to, type: 'hovered'});
                    }
                }
            }
        }
        this.view.updateOverlays(highlights);
    }

    private async renderBoard(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;
        const flipped = this.gameState.isBoardFlipped();
        const boardBinary = encodeBoardToBinary(board);
        await this.view.render(boardBinary, flipped);
        this.updateOverlays();
    }

    getCurrentTurn(): string {
        return this.gameState.getCurrentTurn();
    }
    getMoveHistory(): string[] {
        return this.gameState.getMoveHistory();
    }
    clearSelectedMove(): void {
        this.gameState.setSelectedPosition(null);
        this.updateOverlays();
    }
    toggleShowThreats(): void {
        this.gameState.setShowThreats(!this.gameState.isShowThreats());
        this.updateOverlays();
    }
    isShowThreats(): boolean {
        return this.gameState.isShowThreats();
    }
    async previousMove(): Promise<void> {
        await this.navigateToPreviousMove();
        window.dispatchEvent(new CustomEvent('boardStateChanged'));
    }
    async nextMove(): Promise<void> {
        await this.navigateToNextMove();
        window.dispatchEvent(new CustomEvent('boardStateChanged'));
    }
    isBoardLocked(): boolean {
        return this.gameState.isBoardLocked();
    }
    canNavigateToPrevious(): boolean {
        return this.gameState.getCurrentMoveIndex() >= 0;
    }
    canNavigateToNext(): boolean {
        const moveList = this.gameState.getMoveList();
        return this.gameState.getCurrentMoveIndex() < moveList.length - 1;
    }
}
