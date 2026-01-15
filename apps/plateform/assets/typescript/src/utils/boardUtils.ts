/**
 * Utility functions for board coordinate conversions and binary encoding/decoding
 */

import {BOARD_SIZE, Piece, PIECE_CODE, Board, PotentialMove, Move} from "../models/types";

/**
 * Convert position index (0-80) to algebraic notation (A1-I9)
 */
export function posToAlgebraic(pos: number): string {
    const x = pos % BOARD_SIZE;
    const y = Math.floor(pos / BOARD_SIZE);
    const col = String.fromCharCode('A'.charCodeAt(0) + x);
    const row = BOARD_SIZE - y;
    return col + row;
}

/**
 * Convert algebraic notation (A1-I9) to position index (0-80)
 */
export function algebraicToPos(algebraic: string): number | null {
    if (!algebraic || algebraic.length < 2) return null;
    const col = algebraic[0].toUpperCase();
    const row = parseInt(algebraic.substring(1));
    if (col < 'A' || col > 'I' || row < 1 || row > BOARD_SIZE) return null;
    const x = col.charCodeAt(0) - 'A'.charCodeAt(0);
    const y = BOARD_SIZE - row;
    return y * BOARD_SIZE + x;
}

/**
 * Encode board state to base64 for URL hash
 */
export function encodeBoardToHash(boardData: Uint8Array): string {
    return btoa(String.fromCharCode.apply(null, Array.from(boardData)));
}

/**
 * Decode board state from base64 URL hash
 */
export function decodeBoardFromHash(hash: string): Uint8Array | null {
    try {
        const base64Board = hash.substring(1);
        const binaryString = atob(base64Board);
        const len = binaryString.length;
        const bytes = new Uint8Array(len);
        for (let i = 0; i < len; i++) {
            bytes[i] = binaryString.charCodeAt(i);
        }
        return bytes;
    } catch (e) {
        console.error("Failed to decode board from hash", e);
        return null;
    }
}

/**
 * Decode a piece from its byte representation
 */
export function decodePiece(byte: number): Piece | null {
    if (byte === 0) return null;
    
    const color = !!((byte >> 6) & 0b1);
    const payload = byte & 0b00111111;

    // Check for King: payload = 0b111000
    if (payload === 0b111000) {
        return {color, bottom: 'king', top: null};
    }

    const topCode = (payload >> 3) & 0b111;
    const bottomCode = payload & 0b111;

    if (topCode === 0) {
        // Single piece
        if (PIECE_CODE[bottomCode]) {
            return {color, bottom: PIECE_CODE[bottomCode], top: null};
        }
    } else {
        // Stacked piece
        if (PIECE_CODE[topCode] && PIECE_CODE[bottomCode]) {
            return {color, bottom: PIECE_CODE[bottomCode], top: PIECE_CODE[topCode]};
        }
    }
    
    return null;
}

/**
 * Encode a piece to its byte representation
 */
export function encodePiece(piece: Piece): number {
    const colorBit = piece.color ? 0b1000000 : 0b0000000;

    if (piece.bottom === 'king') {
        return colorBit | 0b0111000;
    }

    // Find the code for the bottom piece
    const bottomCode = Object.entries(PIECE_CODE).find(([_, name]) => name === piece.bottom)?.[0];
    if (!bottomCode) {
        throw new Error(`Invalid bottom piece: ${piece.bottom}`);
    }
    const bottomBits = parseInt(bottomCode);

    if (piece.top === null) {
        // Single piece
        return colorBit | bottomBits;
    } else {
        // Stacked piece
        const topCode = Object.entries(PIECE_CODE).find(([_, name]) => name === piece.top)?.[0];
        if (!topCode) {
            throw new Error(`Invalid top piece: ${piece.top}`);
        }
        const topBits = parseInt(topCode);
        return colorBit | (topBits << 3) | bottomBits;
    }
}

/**
 * Decode Board from binary representation (Uint8Array of 83 bytes)
 */
export function decodeBoardFromBinary(binary: Uint8Array): Board {
    if (binary.length !== 83) {
        throw new Error('Binary board data must be 83 bytes');
    }

    const cells: (Piece | null)[] = [];
    for (let i = 0; i < 81; i++) {
        cells.push(decodePiece(binary[i]));
    }

    // Decode flags from byte 81
    const flags = binary[81];
    const whiteToMove = (flags & 0b10000000) !== 0; // bit 8
    const gameOver = (flags & 0b01000000) !== 0;    // bit 7
    const whiteWins = (flags & 0b00100000) !== 0;   // bit 6
    const draw = (flags & 0b00010000) !== 0;        // bit 5
    
    // Get moves_without_capture counter from byte 82
    const movesWithoutCapture = binary[82];
    
    return new Board(cells, whiteToMove, gameOver, whiteWins, draw, movesWithoutCapture);
}

/**
 * Encode Board to binary representation (Uint8Array of 83 bytes)
 */
export function encodeBoardToBinary(board: Board): Uint8Array {
    const binary = new Uint8Array(83);
    
    for (let i = 0; i < 81; i++) {
        const piece = board.cells[i];
        binary[i] = piece ? encodePiece(piece) : 0;
    }
    
    // Pack flags into byte 81
    let flags = 0;
    if (board.whiteToMove) flags |= 0b10000000; // bit 8
    if (board.gameOver) flags |= 0b01000000;    // bit 7
    if (board.whiteWins) flags |= 0b00100000;   // bit 6
    if (board.draw) flags |= 0b00010000;        // bit 5
    binary[81] = flags;
    
    // Set moves_without_capture counter in byte 82
    binary[82] = board.movesWithoutCapture;
    
    return binary;
}

/**
 * Decode PotentialMove from u16 representation
 */
export function decodePotentialMove(moveU16: number): PotentialMove {
    const from = moveU16 & 0x7F;
    const to = (moveU16 >> 7) & 0x7F;
    const unstackable = ((moveU16 >> 14) & 0x1) === 1;
    const force_unstack = ((moveU16 >> 15) & 0x1) === 1;
    
    return {from, to, unstackable, force_unstack};
}

/**
 * Encode Move to u16 representation
 */
export function encodeMove(move: Move): number {
    let moveBits = (move.from & 0x7F) | ((move.to & 0x7F) << 7);
    if (move.unstack) {
        moveBits |= (1 << 14);
    }
    return moveBits;
}

/**
 * Encode a list of moves to base64 for URL hash
 */
export function encodeMoveListToHash(moves: Move[]): string {
    const bytes = new Uint8Array(moves.length * 2);
    for (let i = 0; i < moves.length; i++) {
        const moveU16 = encodeMove(moves[i]);
        bytes[i * 2] = moveU16 & 0xFF;
        bytes[i * 2 + 1] = (moveU16 >> 8) & 0xFF;
    }
    return btoa(String.fromCharCode.apply(null, Array.from(bytes)));
}

/**
 * Decode a list of moves from base64 URL hash
 */
export function decodeMoveListFromHash(hash: string): Move[] | null {
    try {
        const base64Moves = hash.substring(1); // Remove '#'
        const binaryString = atob(base64Moves);
        const len = binaryString.length;
        
        if (len % 2 !== 0) {
            console.error("Invalid move list: length must be even");
            return null;
        }
        
        const moves: Move[] = [];
        for (let i = 0; i < len; i += 2) {
            const moveU16 = binaryString.charCodeAt(i) | (binaryString.charCodeAt(i + 1) << 8);
            const from = moveU16 & 0x7F;
            const to = (moveU16 >> 7) & 0x7F;
            const unstack = ((moveU16 >> 14) & 0x1) === 1;
            moves.push({from, to, unstack});
        }
        
        return moves;
    } catch (e) {
        console.error("Failed to decode move list from hash", e);
        return null;
    }
}

/**
 * Encode a list of moves to binary format for API
 */
export function encodeMoveListToBinary(moves: Move[]): Uint8Array {
    const bytes = new Uint8Array(moves.length * 2);
    for (let i = 0; i < moves.length; i++) {
        const moveU16 = encodeMove(moves[i]);
        bytes[i * 2] = moveU16 & 0xFF;
        bytes[i * 2 + 1] = (moveU16 >> 8) & 0xFF;
    }
    return bytes;
}
