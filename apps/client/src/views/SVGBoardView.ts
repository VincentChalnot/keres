import {IBoardView, TileHighlight} from './IBoardView';
import {BOARD_SIZE, LAST_BOARD_INDEX} from '../models/types';
import {GameState} from '../models/GameState';
import {decodePiece} from "../utils/boardUtils";

// Constants for the SVG board
const SQUARE_WIDTH = 100; // px
const SQUARE_HEIGHT = 80; // px
const BOARD_WIDTH = BOARD_SIZE * SQUARE_WIDTH; // 900px
const BOARD_HEIGHT = BOARD_SIZE * SQUARE_HEIGHT; // 800px
const PIECE_SIZE = 90; // px (centered in 100px square)
const PIECE_X_OFFSET = (SQUARE_WIDTH - PIECE_SIZE) / 2;
const PIECE_Y_OFFSET = (SQUARE_HEIGHT - PIECE_SIZE) / 2;
const STACKED_OFFSET = 23;

// Colors
const BLACK_SQUARE_COLOR = '#e1c499';
const WHITE_SQUARE_COLOR = '#f8f0e6';
const BORDER_COLOR = '#55442d';

// Z-index layers
const Z_BOARD = 1;
const Z_PIECES = 10;
const Z_OVERLAY = 20;

/**
 * SVG-based board renderer
 */
export default class SVGBoardView implements IBoardView {
    private container!: HTMLElement;
    private svg!: SVGSVGElement;
    private boardGroup!: SVGGElement;
    private piecesGroup!: SVGGElement;
    private overlaysGroup!: SVGGElement;
    private gameState: GameState;

    private clickHandler: ((tileIndex: number) => void) | null = null;
    private hoverHandler: ((tileIndex: number | null) => void) | null = null;

    // Track current board state to enable differential updates
    private currentBoardData: Uint8Array | null = null;
    private currentFlipped: boolean = false;

    // Cache for overlay rectangles
    private overlayElements: Map<number, SVGRectElement> = new Map();

    // Bound event handler references for proper cleanup
    private boundHandleClick: ((e: MouseEvent) => void) | null = null;
    private boundHandleMouseMove: ((e: MouseEvent) => void) | null = null;
    private boundHandleMouseLeave: (() => void) | null = null;
    private boundOnResize: (() => void) | null = null;

    constructor(gameState: GameState) {
        this.gameState = gameState;
    }

    initialize(container: HTMLElement): void {
        this.container = container;

        // Create SVG element
        this.svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        this.svg.setAttribute('viewBox', `0 0 ${BOARD_WIDTH} ${BOARD_HEIGHT}`);
        this.svg.style.display = 'block';
        this.svg.style.margin = '0 auto';
        this.svg.style.cursor = 'pointer';
        
        // Create groups for layering
        this.boardGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.boardGroup.setAttribute('id', 'board-layer');
        
        this.overlaysGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.overlaysGroup.setAttribute('id', 'overlays-layer');

        this.piecesGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.piecesGroup.setAttribute('id', 'pieces-layer');

        this.svg.appendChild(this.boardGroup);
        this.svg.appendChild(this.overlaysGroup);
        this.svg.appendChild(this.piecesGroup);

        container.appendChild(this.svg);

        // Create the board background
        this.createBoard();
        this.createOverlays();

        // Event listeners - bind and store references for proper cleanup
        this.boundHandleClick = (e: MouseEvent) => this.handleClick(e);
        this.boundHandleMouseMove = (e: MouseEvent) => this.handleMouseMove(e);
        this.boundHandleMouseLeave = () => this.handleMouseLeave();
        this.boundOnResize = () => this.onResize();

        this.svg.addEventListener('click', this.boundHandleClick);
        this.svg.addEventListener('mousemove', this.boundHandleMouseMove);
        this.svg.addEventListener('mouseleave', this.boundHandleMouseLeave);
        
        window.addEventListener('resize', this.boundOnResize);
        this.onResize();
    }

    private createBoard(): void {
        // Create checkerboard pattern
        for (let row = 0; row < BOARD_SIZE; row++) {
            for (let col = 0; col < BOARD_SIZE; col++) {
                const x = col * SQUARE_WIDTH;
                const y = row * SQUARE_HEIGHT;
                
                // Determine square color (checkerboard pattern)
                const isBlackSquare = (row + col) % 2 === 1;
                const fillColor = isBlackSquare ? BLACK_SQUARE_COLOR : WHITE_SQUARE_COLOR;
                
                // Create square
                const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
                rect.setAttribute('x', x.toString());
                rect.setAttribute('y', y.toString());
                rect.setAttribute('width', SQUARE_WIDTH.toString());
                rect.setAttribute('height', SQUARE_HEIGHT.toString());
                rect.setAttribute('fill', fillColor);
                rect.setAttribute('stroke', BORDER_COLOR);
                rect.setAttribute('stroke-width', '1');
                
                this.boardGroup.appendChild(rect);
            }
        }
    }

    private createOverlays(): void {
        // Create invisible overlay rectangles for each square
        for (let i = 0; i <= LAST_BOARD_INDEX; i++) {
            const {x, y} = this.getTilePosition(i);
            
            const rect = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
            rect.setAttribute('x', x.toString());
            rect.setAttribute('y', y.toString());
            rect.setAttribute('width', SQUARE_WIDTH.toString());
            rect.setAttribute('height', SQUARE_HEIGHT.toString());
            rect.setAttribute('fill', 'transparent');
            rect.setAttribute('opacity', '0');
            rect.setAttribute('pointer-events', 'all');
            rect.dataset.tileIndex = i.toString();
            
            this.overlaysGroup.appendChild(rect);
            this.overlayElements.set(i, rect);
        }
    }

    private getTilePosition(index: number): {x: number, y: number} {
        const actualIndex = this.gameState.isBoardFlipped() ? (LAST_BOARD_INDEX - index) : index;
        const col = actualIndex % BOARD_SIZE;
        const row = Math.floor(actualIndex / BOARD_SIZE);
        
        return {
            x: col * SQUARE_WIDTH,
            y: row * SQUARE_HEIGHT
        };
    }

    async render(boardData: Uint8Array, flipped: boolean): Promise<void> {
        // Check if board orientation changed
        const orientationChanged = this.currentFlipped !== flipped;

        // If this is the first render or orientation changed, recreate all pieces
        if (!this.currentBoardData || orientationChanged) {
            await this.recreateAllPieces(boardData, flipped);
            this.currentBoardData = new Uint8Array(boardData);
            this.currentFlipped = flipped;
            return;
        }

        // Perform differential update
        const changes: number[] = [];
        for (let i = 0; i <= LAST_BOARD_INDEX; i++) {
            if (boardData[i] !== this.currentBoardData[i]) {
                changes.push(i);
            }
        }

        // Update only changed positions
        for (const index of changes) {
            await this.updatePieceAtIndex(index, boardData[index], flipped);
        }

        // Update cached board state
        this.currentBoardData = new Uint8Array(boardData);
    }

    private async recreateAllPieces(boardData: Uint8Array, flipped: boolean): Promise<void> {
        // Clear existing pieces
        while (this.piecesGroup.firstChild) {
            this.piecesGroup.removeChild(this.piecesGroup.firstChild);
        }

        // Create all pieces
        for (let i = 0; i <= LAST_BOARD_INDEX; i++) {
            const pieceVal = boardData[i];
            if (pieceVal === 0) continue;
            
            await this.updatePieceAtIndex(i, pieceVal, flipped);
        }
    }

    private async updatePieceAtIndex(index: number, pieceVal: number, flipped: boolean): Promise<void> {
        // Remove existing pieces at this position
        this.removePiecesAtIndex(index);

        // Add new piece if there's a piece
        const piece = decodePiece(pieceVal);
        if (!piece) return;

        const {x, y} = this.getTilePosition(index);
        const pieceX = x + PIECE_X_OFFSET;
        const pieceY = y + PIECE_Y_OFFSET;

        // Load bottom piece
        const bottomPath = this.getPiecePath(piece.bottom, piece.color, flipped);
        await this.createPieceImage(pieceX, pieceY, bottomPath, index, false);

        // Load top piece if stacked
        if (piece.top) {
            const topPath = this.getPiecePath(piece.top, piece.color, flipped);
            // Offset the top piece slightly
            const topOffset = STACKED_OFFSET; // pixels
            await this.createPieceImage(pieceX, pieceY - topOffset, topPath, index, true);
        }
    }

    private async createPieceImage(x: number, y: number, svgPath: string, tileIndex: number, isTopPiece: boolean): Promise<void> {
        try {
            // Load the SVG content
            const response = await fetch(svgPath);
            if (!response.ok) {
                console.error(`Failed to load SVG: ${svgPath}, status: ${response.status}`);
                return;
            }
            const svgText = await response.text();
        
            // Parse the SVG
            const parser = new DOMParser();
            const svgDoc = parser.parseFromString(svgText, 'image/svg+xml');
            const svgElement = svgDoc.documentElement;
            
            // Create a group to contain the embedded SVG
            const group = document.createElementNS('http://www.w3.org/2000/svg', 'g');
            group.setAttribute('transform', `translate(${x}, ${y})`);
            group.dataset.tileIndex = tileIndex.toString();
            group.dataset.isTopPiece = isTopPiece.toString();
            
            // Set the viewBox and dimensions on the embedded SVG
            svgElement.setAttribute('width', PIECE_SIZE.toString());
            svgElement.setAttribute('height', PIECE_SIZE.toString());
            svgElement.setAttribute('viewBox', '0 0 90 90');
            
            // Import the SVG element into our document and add to group
            const importedSvg = document.importNode(svgElement, true);
            group.appendChild(importedSvg);
            
            this.piecesGroup.appendChild(group);
        } catch (error) {
            console.error(`Error loading piece image from ${svgPath}:`, error);
        }
    }

    private getPiecePath(pieceName: string, color: boolean, reversed: boolean): string {
        const colorName = color ? 'white' : 'black';
        const reversedSuffix = color === reversed ? '-reversed' : '';
        return `images/vector/${pieceName}-${colorName}${reversedSuffix}.svg`;
    }

    private removePiecesAtIndex(index: number): void {
        // Find and remove all piece elements at this tile index
        const pieces = this.piecesGroup.querySelectorAll(`[data-tile-index="${index}"]`);
        pieces.forEach(piece => piece.remove());
    }

    updateOverlays(highlights: TileHighlight[]): void {
        const flipped = this.gameState.isBoardFlipped();

        // Reset all overlays
        for (let i = 0; i <= LAST_BOARD_INDEX; i++) {
            const overlay = this.overlayElements.get(i);
            if (overlay) {
                overlay.setAttribute('opacity', '0');
                overlay.setAttribute('fill', 'transparent');
            }
        }

        // Apply highlights
        for (const highlight of highlights) {
            const tileIndex = highlight.position;
            const visualIndex = flipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
            const overlay = this.overlayElements.get(visualIndex);
            if (!overlay) continue;

            let color: string;
            let opacity: string;

            switch (highlight.type) {
                case 'selected':
                    color = '#7fa0dd';
                    opacity = '0.6';
                    break;
                case 'potential':
                    color = '#55d157';
                    opacity = '0.5';
                    break;
                case 'hovered':
                    color = '#e1ca58';
                    opacity = '0.4';
                    break;
                case 'threat':
                    color = '#ff4444';
                    opacity = '0.5';
                    break;
            }

            overlay.setAttribute('fill', color);
            overlay.setAttribute('opacity', opacity);
        }
    }

    onResize(): void {
        // The SVG scales automatically with CSS, but we can adjust container if needed
        // For now, the viewBox handles scaling
    }

    private handleClick(event: MouseEvent): void {
        if (!this.clickHandler) return;

        const pos = this.getPosFromMouseEvent(event);
        if (pos !== null) {
            this.clickHandler(pos);
        }
    }

    private handleMouseMove(event: MouseEvent): void {
        if (!this.hoverHandler) return;

        const pos = this.getPosFromMouseEvent(event);
        if (pos !== null) {
            this.hoverHandler(pos);
        } else {
            this.hoverHandler(null);
        }
    }

    private handleMouseLeave(): void {
        if (this.hoverHandler) {
            this.hoverHandler(null);
        }
    }

    private getPosFromMouseEvent(event: MouseEvent): number | null {
        const rect = this.svg.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;

        // Convert from screen coordinates to SVG coordinates
        const scaleX = BOARD_WIDTH / rect.width;
        const scaleY = BOARD_HEIGHT / rect.height;
        
        const svgX = x * scaleX;
        const svgY = y * scaleY;

        // Calculate which square was clicked
        const col = Math.floor(svgX / SQUARE_WIDTH);
        const row = Math.floor(svgY / SQUARE_HEIGHT);

        if (col < 0 || col >= BOARD_SIZE || row < 0 || row >= BOARD_SIZE) {
            return null;
        }

        const visualIndex = row * BOARD_SIZE + col;
        const flipped = this.gameState.isBoardFlipped();
        return flipped ? (LAST_BOARD_INDEX - visualIndex) : visualIndex;
    }

    onTileClick(handler: (tileIndex: number) => void): void {
        this.clickHandler = handler;
    }

    onTileHover(handler: (tileIndex: number | null) => void): void {
        this.hoverHandler = handler;
    }

    dispose(): void {
        // Remove event listeners using stored references
        if (this.boundOnResize) {
            window.removeEventListener('resize', this.boundOnResize);
        }
        
        if (this.svg) {
            if (this.boundHandleClick) {
                this.svg.removeEventListener('click', this.boundHandleClick);
            }
            if (this.boundHandleMouseMove) {
                this.svg.removeEventListener('mousemove', this.boundHandleMouseMove);
            }
            if (this.boundHandleMouseLeave) {
                this.svg.removeEventListener('mouseleave', this.boundHandleMouseLeave);
            }
        }
        
        // Remove SVG element
        if (this.svg && this.svg.parentNode) {
            this.svg.parentNode.removeChild(this.svg);
        }

        // Clear caches
        this.overlayElements.clear();
        this.currentBoardData = null;
    }
}
