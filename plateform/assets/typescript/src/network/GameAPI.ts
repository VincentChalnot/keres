import {Board, PotentialMove, Move} from '../models/types';
import {decodeBoardFromBinary, encodeBoardToBinary, decodePotentialMove, encodeMove} from '../utils/boardUtils';

/**
 * API client for game backend
 * Handles binary communication with server and converts to/from objects
 */
export class GameAPI {
    private readonly backendUrl: string;
    private gameUuid: string | null = null;

    constructor() {
        // Set backend URL from current location
        this.backendUrl = `${window.location.protocol}//${window.location.hostname}`;
        if (window.location.port) {
            this.backendUrl += `:${window.location.port}`;
        }
        this.backendUrl += '/api';
        
        // Get game UUID from board container data attribute
        const boardContainer = document.getElementById('board-container');
        if (boardContainer) {
            this.gameUuid = boardContainer.getAttribute('data-game-uuid');
        }
    }

    /**
     * Get potential moves for current board state
     */
    async getPotentialMoves(board: Board): Promise<PotentialMove[]> {
        return this.fetchMoves(board);
    }

    /**
     * Get opponent threats by fetching moves with inverted turn
     */
    async getOpponentThreats(board: Board): Promise<PotentialMove[]> {
        // Create a copy of the board with inverted turn
        const invertedBoard = new Board(
            [...board.cells],
            !board.whiteToMove,  // Invert the turn
            board.gameOver,
            board.whiteWins,
            board.draw,
            board.movesWithoutCapture
        );
        
        return this.fetchMoves(invertedBoard);
    }

    /**
     * Private helper to fetch moves for a given board state
     */
    private async fetchMoves(board: Board): Promise<PotentialMove[]> {
        const binary = encodeBoardToBinary(board);
        const response = await fetch(`${this.backendUrl}/moves`, {
            method: 'POST',
            headers: {'Content-Type': 'application/octet-stream'},
            body: binary as BodyInit,
        });
        const buffer = await response.arrayBuffer();
        const movesU16 = new Uint16Array(buffer);

        return Array.from(movesU16).map(decodePotentialMove);
    }

    /**
     * Submit a move to the game and get the new board state
     * This submits to Symfony which validates and may add an AI response
     */
    async submitMove(move: Move): Promise<Board> {
        if (!this.gameUuid) {
            throw new Error('No game UUID available');
        }

        const moveU16 = encodeMove(move);
        const moveBuffer = new Uint16Array([moveU16]).buffer;

        const response = await fetch(`/play/${this.gameUuid}/move`, {
            method: 'POST',
            headers: {'Content-Type': 'application/octet-stream'},
            body: moveBuffer as BodyInit,
        });

        if (!response.ok) {
            const errorData = await response.json();
            throw new Error(errorData.error || 'Failed to submit move');
        }

        const data = await response.json();
        
        // Decode board from base64
        const boardBase64 = data.board;
        const binaryString = atob(boardBase64);
        const bytes = new Uint8Array(binaryString.length);
        for (let i = 0; i < binaryString.length; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        
        return decodeBoardFromBinary(bytes);
    }

    /**
     * Get engine move for current board state (MCTS engine)
     */
    async getEngineMove(board: Board): Promise<Move> {
        const binary = encodeBoardToBinary(board);
        const response = await fetch(`${this.backendUrl}/engine-move`, {
            method: 'POST',
            headers: {'Content-Type': 'application/octet-stream'},
            body: binary as BodyInit,
        });

        if (!response.ok) {
            throw new Error(`Server returned ${response.status}`);
        }

        const moveBuffer = await response.arrayBuffer();
        const moveArray = new Uint16Array(moveBuffer);
        const engineMoveU16 = moveArray[0];

        const from = engineMoveU16 & 0x7F;
        const to = (engineMoveU16 >> 7) & 0x7F;
        const unstack = ((engineMoveU16 >> 14) & 0x1) === 1;

        return {from, to, unstack};
    }

    /**
     * Get minimax engine move for current board state
     */
    async getMinimaxMove(board: Board): Promise<Move> {
        const binary = encodeBoardToBinary(board);
        const response = await fetch(`${this.backendUrl}/minimax-move`, {
            method: 'POST',
            headers: {'Content-Type': 'application/octet-stream'},
            body: binary as BodyInit,
        });

        if (!response.ok) {
            throw new Error(`Server returned ${response.status}`);
        }

        const moveBuffer = await response.arrayBuffer();
        const moveArray = new Uint16Array(moveBuffer);
        const engineMoveU16 = moveArray[0];

        const from = engineMoveU16 & 0x7F;
        const to = (engineMoveU16 >> 7) & 0x7F;
        const unstack = ((engineMoveU16 >> 14) & 0x1) === 1;

        return {from, to, unstack};
    }

    /**
     * Replay a list of moves and get the final board state
     */
    async replayMoves(moves: Move[]): Promise<Board> {
        // Import the encoding function
        const {encodeMoveListToBinary} = await import('../utils/boardUtils');
        const binary = encodeMoveListToBinary(moves);
        
        const response = await fetch(`${this.backendUrl}/replay-moves`, {
            method: 'POST',
            headers: {'Content-Type': 'application/octet-stream'},
            body: binary as BodyInit,
        });

        if (!response.ok) {
            throw new Error(`Server returned ${response.status}`);
        }

        const boardBuffer = await response.arrayBuffer();
        return decodeBoardFromBinary(new Uint8Array(boardBuffer));
    }
}
