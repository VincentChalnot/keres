/**
 * Hardcoded piece movement rules for display in UI
 */

export interface PieceRule {
    name: string;
    description: string;
    movement: string;
}

export const PIECE_RULES: Record<string, PieceRule> = {
    soldier: {
        name: 'Soldier',
        description: 'Basic infantry unit. Moves one square forward or diagonally forward. Captures diagonally forward. Promotes to Paladin upon reaching the opposite end of the board.',
        movement: '1 square forward or diagonally forward; captures diagonally forward',
    },
    bishop: {
        name: 'Bishop',
        description: 'Moves diagonally any number of squares. Cannot jump over other pieces.',
        movement: 'Any number of squares diagonally',
    },
    rook: {
        name: 'Rook',
        description: 'Moves horizontally or vertically any number of squares. Cannot jump over other pieces.',
        movement: 'Any number of squares horizontally or vertically',
    },
    paladin: {
        name: 'Paladin',
        description: 'Promoted from Soldier. Moves one square in any direction (horizontally, vertically, or diagonally).',
        movement: '1 square in any direction',
    },
    guard: {
        name: 'Guard',
        description: 'Protects the King. Moves one square diagonally.',
        movement: '1 square diagonally',
    },
    knight: {
        name: 'Knight',
        description: 'Moves in an L-shape: two squares in one direction and one square perpendicular. Can jump over other pieces.',
        movement: 'L-shape (2+1 squares); can jump over pieces',
    },
    ballista: {
        name: 'Ballista',
        description: 'Long-range artillery. Moves horizontally or vertically any number of squares. Promotes to Rook upon reaching the opposite end of the board.',
        movement: 'Any number of squares horizontally or vertically; promotes to Rook',
    },
    king: {
        name: 'King',
        description: 'The most important piece. Moves one square in any direction. The game is lost if the King is captured.',
        movement: '1 square in any direction',
    },
};
