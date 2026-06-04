/**
 * Material difference calculation — Lichess-style, board-state-based.
 *
 * Two positions with an identical board state always produce an identical
 * material score, regardless of game history.
 */

import {Board} from './types';

/**
 * Piece values. King is excluded from scoring (not listed here).
 * Store here so they can be tuned without touching display logic.
 */
export const PIECE_VALUES: Record<string, number> = {
    soldier:  1,
    paladin:  2,
    guard:    2,
    knight:   4,
    ballista: 5,
    bishop:   5,
    rook:     6,
};

/** Display order: most valuable first */
const PIECE_ORDER: string[] = ['rook', 'bishop', 'ballista', 'knight', 'paladin', 'guard', 'soldier'];

export interface MaterialDiff {
    /** Icons to show above white's side (pieces white has more of, type → excess count) */
    whiteExcess: Record<string, number>;
    /** Icons to show above black's side (pieces black has more of, type → excess count) */
    blackExcess: Record<string, number>;
    /** Score delta: positive = white ahead, negative = black ahead */
    scoreDelta: number;
}

/**
 * Count every individual piece on the board per color.
 * Pieces in stacks are counted individually.
 * King is ignored for scoring purposes.
 */
function countPieces(board: Board): { white: Record<string, number>; black: Record<string, number> } {
    const white: Record<string, number> = {};
    const black: Record<string, number> = {};

    for (const cell of board.cells) {
        if (!cell) continue;
        const target = cell.color ? white : black;

        // bottom piece (always present in an occupied cell)
        if (cell.bottom !== 'king') {
            target[cell.bottom] = (target[cell.bottom] || 0) + 1;
        }

        // top piece (stacked)
        if (cell.top && cell.top !== 'king') {
            target[cell.top] = (target[cell.top] || 0) + 1;
        }
    }

    return {white, black};
}

/**
 * Compute material difference purely from the current board state.
 */
export function computeMaterialDiff(board: Board): MaterialDiff {
    const {white, black} = countPieces(board);

    const whiteExcess: Record<string, number> = {};
    const blackExcess: Record<string, number> = {};

    let whiteScore = 0;
    let blackScore = 0;

    for (const piece of PIECE_ORDER) {
        const value = PIECE_VALUES[piece];
        const wCount = white[piece] || 0;
        const bCount = black[piece] || 0;

        whiteScore += wCount * value;
        blackScore += bCount * value;

        // Per-type excess → icon display
        const excess = wCount - bCount;
        if (excess > 0) {
            whiteExcess[piece] = excess;
        } else if (excess < 0) {
            blackExcess[piece] = -excess;
        }
    }

    return {
        whiteExcess,
        blackExcess,
        scoreDelta: whiteScore - blackScore,
    };
}

/**
 * Render the material bar HTML for one side.
 *
 * @param excess      Piece types this side has more of (type → count)
 * @param advantage   Score advantage for this side (positive = this side is ahead)
 * @param colorClass  'p-w' for white pieces (black icon on white bg), 'p-b' for black pieces (white icon on black bg)
 */
export function renderMaterialHTML(excess: Record<string, number>, advantage: number, colorClass: 'p-w' | 'p-b'): string {
    let html = '';

    for (const piece of PIECE_ORDER) {
        const count = excess[piece] || 0;
        for (let i = 0; i < count; i++) {
            html += `<svg class="material-piece ${colorClass}" viewBox="-5 -15 100 85" xmlns="http://www.w3.org/2000/svg"><use href="#piece-${piece}"/></svg>`;
        }
    }

    if (advantage > 0) {
        html += `<span class="material-score">+${advantage}</span>`;
    }

    return html;
}
