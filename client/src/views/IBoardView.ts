/**
 * Interface for board rendering
 * This abstraction allows switching between different rendering engines (2D sprites, 3D, etc.)
 */
export interface IBoardView {
    /**
     * Initialize the rendering system
     */
    initialize(container: HTMLElement): void;

    /**
     * Render the board with all pieces
     */
    render(boardData: Uint8Array, flipped: boolean): Promise<void>;

    /**
     * Update tile overlays (selected, hovered, potential moves)
     */
    updateOverlays(highlights: TileHighlight[]): void;

    /**
     * Handle window resize
     */
    onResize(): void;

    /**
     * Clean up resources
     */
    dispose(): void;

    /**
     * Set click handler
     */
    onTileClick(handler: (tileIndex: number) => void): void;

    /**
     * Set hover handler
     */
    onTileHover(handler: (tileIndex: number | null) => void): void;
}

export interface TileHighlight {
    position: number;
    type: 'selected' | 'potential' | 'hovered';
}

/**
 * Interface for piece sprite loading
 */
export interface IPieceSpriteLoader {
    loadPieceSprite(pieceName: string, color: number, reversed: boolean): Promise<any>;
}
