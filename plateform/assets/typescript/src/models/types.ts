// Core game types

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
    0b010: 'bishop',
    0b011: 'rook',
    0b100: 'paladin',
    0b101: 'guard',
    0b110: 'knight',
    0b111: 'ballista',
};

/**
 * Board class representing the game state
 * Stores 81 cells (9x9 board) and the current turn color
 */
export class Board {
    cells: (Piece | null)[];
    whiteToMove: boolean;
    gameOver: boolean;
    whiteWins: boolean;
    draw: boolean;
    movesWithoutCapture: number;

    constructor(
        cells: (Piece | null)[],
        whiteToMove: boolean,
        gameOver: boolean = false,
        whiteWins: boolean = false,
        draw: boolean = false,
        movesWithoutCapture: number = 0
    ) {
        if (cells.length !== 81) {
            throw new Error('Board must have exactly 81 cells');
        }
        this.cells = cells;
        this.whiteToMove = whiteToMove;
        this.gameOver = gameOver;
        this.whiteWins = whiteWins;
        this.draw = draw;
        this.movesWithoutCapture = movesWithoutCapture;
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
    getCurrentTurn(): 'White' | 'Black' {
        return this.whiteToMove ? 'White' : 'Black';
    }

    /**
     * Check if the game is over
     */
    isGameOver(): boolean {
        return this.gameOver;
    }

    /**
     * Get the game result message
     */
    getGameResult(): string {
        if (!this.gameOver) {
            return '';
        }
        if (this.draw) {
            return 'Game Over - Draw!';
        }
        if (this.whiteWins) {
            return 'Game Over - White Wins!';
        }
        return 'Game Over - Black Wins!';
    }

    /**
     * Apply a move to the board locally (no validation)
     * This is a simple implementation that only updates the board state
     * without checking move validity. Validation is done server-side.
     */
    applyMoveLocally(move: Move): void {
        const fromPiece = this.cells[move.from];
        if (!fromPiece) {
            // No piece to move - this shouldn't happen in a valid game
            // but we don't validate here
            return;
        }

        let movingPiece: Piece;
        
        if (move.unstack && fromPiece.top) {
            // Unstack: move only the top piece
            movingPiece = {
                color: fromPiece.color,
                bottom: fromPiece.top,
                top: null
            };
            // Update the source to remove the top piece
            this.cells[move.from] = {
                color: fromPiece.color,
                bottom: fromPiece.bottom,
                top: null
            };
        } else {
            // Move the whole piece/stack
            movingPiece = fromPiece;
            this.cells[move.from] = null;
        }

        const toPiece = this.cells[move.to];
        
        if (!toPiece) {
            // Empty destination: just place the piece
            this.cells[move.to] = movingPiece;
        } else if (toPiece.color !== movingPiece.color) {
            // Enemy piece: capture (replace)
            this.cells[move.to] = movingPiece;
        } else {
            // Friendly piece: stack (if not already stacked)
            if (!toPiece.top && !movingPiece.top) {
                // Both are single pieces, can stack
                this.cells[move.to] = {
                    color: toPiece.color,
                    bottom: toPiece.bottom,
                    top: movingPiece.bottom
                };
            } else {
                // Can't stack (one or both already stacked)
                // This shouldn't happen in a valid move, but we don't validate
                this.cells[move.to] = movingPiece;
            }
        }

        // Handle promotion: Soldier → Paladin on opposite end
        const destY = Math.floor(move.to / BOARD_SIZE);
        const piece = this.cells[move.to];
        if (piece) {
            const isWhite = piece.color;
            const isOppositeEnd = isWhite ? destY === 0 : destY === 8;
            
            if (isOppositeEnd) {
                // Promote soldier to paladin
                if (piece.bottom === 'soldier') {
                    piece.bottom = 'paladin';
                }
                // Promote ballista to rook
                if (piece.bottom === 'ballista') {
                    piece.bottom = 'rook';
                }
            }
        }

        // Toggle turn
        this.whiteToMove = !this.whiteToMove;
    }
}
