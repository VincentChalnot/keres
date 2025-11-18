// Core game types

export interface Config {
    backendUrl: string;
}

export interface Piece {
    color: boolean;
    bottom: string;
    top: string | null;
}

export interface SelectedPiece {
    from: number;
    to: number[];
}

export interface SelectedMove {
    from: number;
    to: number;
}

export interface PotentialMove {
    from: number;
    to: number;
    unstackable: boolean;
    force_unstack: boolean;
}

export interface Move {
    from: number;
    to: number;
    unstack: boolean;
}

export interface TileState {
    position: number;
    highlighted: boolean;
    highlightColor?: 'selected' | 'potential' | 'hovered';
}

// Constants
export const BOARD_SIZE = 9;
export const LAST_BOARD_INDEX = (BOARD_SIZE * BOARD_SIZE) - 1;

export const PIECE_CODE: Record<number, string> = {
    0b001: 'soldier',
    0b010: 'jester',
    0b011: 'commander',
    0b100: 'paladin',
    0b101: 'guard',
    0b110: 'dragon',
    0b111: 'ballista',
};

/**
 * Board class representing the game state
 * Stores 81 cells (9x9 board) and the current turn color
 */
export class Board {
    cells: (Piece | null)[];
    whiteToMove: boolean;

    constructor(cells: (Piece | null)[], whiteToMove: boolean) {
        if (cells.length !== 81) {
            throw new Error('Board must have exactly 81 cells');
        }
        this.cells = cells;
        this.whiteToMove = whiteToMove;
    }

    /**
     * Get piece at position (0-80)
     */
    getPieceAt(position: number): Piece | null {
        if (position < 0 || position >= 81) {
            return null;
        }
        return this.cells[position];
    }

    /**
     * Get the current turn color
     */
    getCurrentTurn(): 'White' | 'Red' {
        return this.whiteToMove ? 'White' : 'Red';
    }
}
