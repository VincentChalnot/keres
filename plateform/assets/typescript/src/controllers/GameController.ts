import {GameState} from '../models/GameState';
import {GameAPI} from '../network/GameAPI';
import {IBoardView, TileHighlight} from '../views/IBoardView';
import {Move} from '../models/types';

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
     * Set the move list and update the board from the backend
     */
    async setMoves(moves: Move[]): Promise<void> {
        this.gameState.setMoveList(moves);
        this.gameState.clearMoveHistory();
        this.gameState.clearGameHistory();
        const board = await this.api.replayMoves(moves);
        this.gameState.setBoard(board);
        this.gameState.setCurrentMoveIndex(moves.length - 1);
        this.gameState.setBoardLocked(false);
        if (moves.length > 0) {
            const lastMove = moves[moves.length - 1];
            this.gameState.setLastMove({from: lastMove.from, to: lastMove.to});
        } else {
            this.gameState.setLastMove(null);
        }
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
        // Play move on server
        const move: Move = {from, to, unstack};
        const moves = [...this.gameState.getMoveList(), move];
        await this.setMoves(moves);
        window.dispatchEvent(new CustomEvent('boardStateChanged'));
    }

    async requestEngineMove(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;
        const move = await this.api.getEngineMove(board);
        await this.playMove(move.from, move.to, move.unstack);
    }

    async requestMinimaxMove(): Promise<void> {
        const board = this.gameState.getBoard();
        if (!board) return;
        const move = await this.api.getMinimaxMove(board);
        await this.playMove(move.from, move.to, move.unstack);
    }

    async undoMove(): Promise<void> {
        const moveList = this.gameState.getMoveList();
        if (moveList.length === 0) return;
        const newMoves = moveList.slice(0, -1);
        await this.setMoves(newMoves);
        window.dispatchEvent(new CustomEvent('boardStateChanged'));
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
        const moveList = this.gameState.getMoveList();
        const moves = moveList.slice(0, targetIndex + 1);
        await this.setMoves(moves);
    }

    async flipBoard(): Promise<void> {
        this.gameState.flipBoard();
        this.gameState.setSelectedPosition(null);
        await this.renderBoard();
    }

    private handleTileClick(pos: number): void {
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
            if (move.unstackable && !move.force_unstack) {
                this.gameState.setClickedDestination(pos);
                window.dispatchEvent(new CustomEvent('showUnstackModal'));
            } else {
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
        const {encodeBoardToBinary} = await import('../utils/boardUtils');
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
