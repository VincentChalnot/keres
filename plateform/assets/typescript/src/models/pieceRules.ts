/**
 * Piece movement rules for display in UI (in French)
 */

export interface PieceRule {
    name: string;
    movement: string;
}

export const PIECE_RULES: Record<string, PieceRule> = {
    soldier: {
        name: 'Soldat (S)',
        movement: '1 case en diagonale avant\n→ Promotion en Paladin',
    },
    bishop: {
        name: 'Fou (F)',
        movement: 'Illimité en diagonale',
    },
    rook: {
        name: 'Tour (T)',
        movement: 'Illimité orthogonalement',
    },
    paladin: {
        name: 'Paladin (P)',
        movement: '1 ou 2 cases orthogonalement',
    },
    guard: {
        name: 'Garde (G)',
        movement: '1 ou 2 cases en diagonale',
    },
    knight: {
        name: 'Cavalier (C)',
        movement: 'Mouvement en L\nPeut sauter par-dessus les pièces',
    },
    ballista: {
        name: 'Baliste (B)',
        movement: 'Illimité vers l\'avant uniquement\n→ Promotion en Tour',
    },
    king: {
        name: 'Roi (R)',
        movement: '1 case dans toutes les directions\nNe peut pas être empilé',
    },
};
