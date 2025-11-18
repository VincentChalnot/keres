/**
 * Utility functions for board coordinate conversions
 */

import {BOARD_SIZE} from "../models/types";

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
