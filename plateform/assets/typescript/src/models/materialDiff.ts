/**
 * Material difference calculation for Lichess-style display
 */

import {Board} from './types';

/** Piece values for material difference scoring */
const PIECE_VALUES: Record<string, number> = {
    soldier: 1,
    bishop: 3,
    rook: 5,
    paladin: 3,
    guard: 2,
    knight: 3,
    ballista: 4,
    king: 0, // king is not counted in material
};

/** Standard starting piece counts per side */
const STARTING_COUNTS: Record<string, number> = {
    soldier: 9,
    bishop: 2,
    rook: 2,
    paladin: 0,
    guard: 2,
    knight: 2,
    ballista: 2,
    king: 1,
};

/** Piece display order (most valuable first) */
const PIECE_ORDER: string[] = ['rook', 'ballista', 'bishop', 'knight', 'paladin', 'guard', 'soldier'];

export interface MaterialInfo {
    /** Piece types captured from white, with counts */
    whiteCaptured: Record<string, number>;
    /** Piece types captured from black, with counts */
    blackCaptured: Record<string, number>;
    /** Material score advantage (positive = white ahead) */
    scoreDelta: number;
}

/**
 * Count pieces on the board per color
 */
function countPieces(board: Board): { white: Record<string, number>; black: Record<string, number> } {
    const white: Record<string, number> = {};
    const black: Record<string, number> = {};

    for (const cell of board.cells) {
        if (!cell) continue;
        const target = cell.color ? white : black;
        target[cell.bottom] = (target[cell.bottom] || 0) + 1;
        if (cell.top) {
            target[cell.top] = (target[cell.top] || 0) + 1;
        }
    }

    return { white, black };
}

/**
 * Compute material difference between players
 */
export function computeMaterialDiff(board: Board): MaterialInfo {
    const counts = countPieces(board);

    // Captured = starting count - current count (clamped to 0)
    const whiteCaptured: Record<string, number> = {};
    const blackCaptured: Record<string, number> = {};

    let whiteScore = 0;
    let blackScore = 0;

    for (const piece of PIECE_ORDER) {
        const startCount = STARTING_COUNTS[piece] || 0;
        const whiteOnBoard = counts.white[piece] || 0;
        const blackOnBoard = counts.black[piece] || 0;

        const whiteLost = Math.max(0, startCount - whiteOnBoard);
        const blackLost = Math.max(0, startCount - blackOnBoard);

        if (whiteLost > 0) whiteCaptured[piece] = whiteLost;
        if (blackLost > 0) blackCaptured[piece] = blackLost;

        whiteScore += whiteOnBoard * (PIECE_VALUES[piece] || 0);
        blackScore += blackOnBoard * (PIECE_VALUES[piece] || 0);
    }

    return {
        whiteCaptured,
        blackCaptured,
        scoreDelta: whiteScore - blackScore,
    };
}

/**
 * Render material difference as HTML for a given side
 * @param captured Pieces captured FROM this side (shown on opponent's side)
 * @param advantage Score advantage for the display side (positive means this side is ahead)
 */
export function renderMaterialHTML(captured: Record<string, number>, advantage: number): string {
    let html = '';
    for (const piece of PIECE_ORDER) {
        const count = captured[piece] || 0;
        for (let i = 0; i < count; i++) {
            html += `<span class="material-piece piece-icon ${piece}"></span>`;
        }
    }
    if (advantage > 0) {
        html += `<span class="material-score">+${advantage}</span>`;
    }
    return html;
}
