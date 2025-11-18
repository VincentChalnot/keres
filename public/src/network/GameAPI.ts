import {Config, Board, PotentialMove, Move} from '../models/types';
import {decodeBoardFromBinary, encodeBoardToBinary, decodePotentialMove, encodeMove} from '../utils/boardUtils';

/**
 * API client for game backend
 * Handles binary communication with server and converts to/from objects
 */
export class GameAPI {
    constructor(private config: Config) {
    }

    /**
     * Get a new game board
     */
    async getNewGame(): Promise<Board> {
        const response = await fetch(`${this.config.backendUrl}/new`);
        const buffer = await response.arrayBuffer();
        const binary = new Uint8Array(buffer);
        return decodeBoardFromBinary(binary);
    }

    /**
     * Get possible moves for current board state
     */
    async getPossibleMoves(board: Board): Promise<PotentialMove[]> {
        const binary = encodeBoardToBinary(board);
        const response = await fetch(`${this.config.backendUrl}/moves`, {
            method: 'POST',
            headers: {'Content-Type': 'application/octet-stream'},
            body: binary as BodyInit,
        });
        const buffer = await response.arrayBuffer();
        const movesU16 = new Uint16Array(buffer);
        
        return Array.from(movesU16).map(decodePotentialMove);
    }

    /**
     * Play a move and get the new board state
     */
    async playMove(board: Board, move: Move): Promise<Board> {
        const boardBinary = encodeBoardToBinary(board);
        const moveU16 = encodeMove(move);
        const moveBuffer = new Uint16Array([moveU16]).buffer;
        
        const payload = new Uint8Array(boardBinary.length + 2);
        payload.set(boardBinary, 0);
        payload.set(new Uint8Array(moveBuffer), boardBinary.length);

        const response = await fetch(`${this.config.backendUrl}/play`, {
            method: 'POST',
            headers: {'Content-Type': 'application/octet-stream'},
            body: payload as BodyInit,
        });

        const newBoardBuffer = await response.arrayBuffer();
        return decodeBoardFromBinary(new Uint8Array(newBoardBuffer));
    }

    /**
     * Get engine move for current board state
     */
    async getEngineMove(board: Board): Promise<Move> {
        const binary = encodeBoardToBinary(board);
        const response = await fetch(`${this.config.backendUrl}/engine-move`, {
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
     * Load configuration
     */
    static async loadConfig(): Promise<Config> {
        const response = await fetch('/config.json');
        return await response.json();
    }
}
