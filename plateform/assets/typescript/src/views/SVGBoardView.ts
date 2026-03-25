import {IBoardView, TileHighlight} from './IBoardView';
import {BOARD_SIZE, LAST_BOARD_INDEX} from '../models/types';
import {GameState} from '../models/GameState';
import {decodePiece} from "../utils/boardUtils";
import {PIECE_RULES} from '../models/pieceRules';

// Constants for the SVG board
const SQUARE_WIDTH = 100; // px
const SQUARE_HEIGHT = 80; // px
const BOARD_WIDTH = BOARD_SIZE * SQUARE_WIDTH; // 900px
const BOARD_HEIGHT = BOARD_SIZE * SQUARE_HEIGHT; // 720px
const STACKED_OFFSET = 23;

// Coordinate label dimensions (left margin for row labels, bottom margin for column labels)
const COORD_WIDTH = 25; // px left margin for row numbers (1-9)
const COORD_HEIGHT = 25; // px bottom margin for column letters (A-I)

// Piece card constants (2× tile size for the piece preview)
const CARD_PIECE_SCALE = 2;
const CARD_PIECE_W = SQUARE_WIDTH * CARD_PIECE_SCALE;  // 200
const CARD_PIECE_H = SQUARE_HEIGHT * CARD_PIECE_SCALE; // 160
const CARD_PADDING = 12;
const CARD_TEXT_LINE_H = 20;
const CARD_WIDTH = CARD_PIECE_W + CARD_PADDING * 2;    // 224
// Height is computed dynamically based on text lines

const SPRITE_URL = '/build/pieces-sprite.svg';
// Use Vite's asset handling to get the correct URL for board.css
const BOARD_CSS_URL = new URL('/assets/board.css', import.meta.url).href;

/**
 * SVG-based board renderer
 */
export default class SVGBoardView implements IBoardView {
    private container!: HTMLElement;
    private svg!: SVGSVGElement;
    private boardGroup!: SVGGElement;
    private piecesGroup!: SVGGElement;
    private overlaysGroup!: SVGGElement;
    private coordsGroup!: SVGGElement;
    private cardGroup!: SVGGElement;
    private gameState: GameState;

    private clickHandler: ((tileIndex: number, shiftKey?: boolean) => void) | null = null;
    private hoverHandler: ((tileIndex: number | null) => void) | null = null;
    private dragMoveHandler: ((from: number, to: number, shiftKey?: boolean) => void) | null = null;
    private pieceLongHoverHandler: ((tileIndex: number, clientX: number, clientY: number) => void) | null = null;

    // Track current board state to enable differential updates
    private currentBoardData: Uint8Array | null = null;
    private currentFlipped: boolean = false;

    // Cache for overlay rectangles
    private overlayElements: Map<number, SVGRectElement> = new Map();

    // Drag & drop state
    private dragState: {
        active: boolean;
        from: number;
        startX: number;
        startY: number;
        ghost: SVGGElement | null;
    } = { active: false, from: -1, startX: 0, startY: 0, ghost: null };
    private preventNextClick: boolean = false;
    private static readonly DRAG_THRESHOLD = 5; // pixels

    // Long press state — only triggered by sustained mousedown/touchstart, NOT hover
    private longPressTimer: ReturnType<typeof setTimeout> | null = null;
    private static readonly LONG_PRESS_DELAY = 800; // ms

    // Whether the piece card is currently visible
    private cardVisible: boolean = false;

    // Bound event handler references for proper cleanup
    private boundHandleClick: ((e: MouseEvent) => void) | null = null;
    private boundHandleMouseMove: ((e: MouseEvent) => void) | null = null;
    private boundHandleMouseLeave: (() => void) | null = null;
    private boundHandleMouseDown: ((e: MouseEvent) => void) | null = null;
    private boundHandleMouseUp: ((e: MouseEvent) => void) | null = null;
    private boundHandleTouchStart: ((e: TouchEvent) => void) | null = null;
    private boundHandleTouchMove: ((e: TouchEvent) => void) | null = null;
    private boundHandleTouchEnd: ((e: TouchEvent) => void) | null = null;
    private boundOnResize: (() => void) | null = null;
    private boundHandleDocumentClick: ((e: MouseEvent) => void) | null = null;

    constructor(gameState: GameState) {
        this.gameState = gameState;
    }

    async initialize(container: HTMLElement): Promise<void> {
        this.container = container;
        await this.injectBoardCSS();
        this.svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
        this.svg.setAttribute('viewBox', `${-COORD_WIDTH} 0 ${BOARD_WIDTH + COORD_WIDTH} ${BOARD_HEIGHT + COORD_HEIGHT}`);
        this.svg.style.cursor = 'pointer';

        // Inline the sprite sheet symbols/defs directly into the SVG
        await this.inlineSpriteDefs(this.svg);

        // Create groups for layering as in board.svg
        this.boardGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.boardGroup.setAttribute('id', 'board-layer');
        this.svg.appendChild(this.boardGroup);

        this.coordsGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.coordsGroup.setAttribute('id', 'coords-layer');
        this.svg.appendChild(this.coordsGroup);

        this.overlaysGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.overlaysGroup.setAttribute('id', 'overlays-layer');
        this.svg.appendChild(this.overlaysGroup);

        this.piecesGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.piecesGroup.setAttribute('id', 'pieces-layer');
        this.svg.appendChild(this.piecesGroup);

        // Card group on top of everything
        this.cardGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        this.cardGroup.setAttribute('id', 'card-layer');
        this.cardGroup.style.display = 'none';
        this.svg.appendChild(this.cardGroup);

        container.appendChild(this.svg);

        // Create the board background
        this.createBoard();
        this.createCoordinates(false);
        this.createOverlays();

        // Event listeners - bind and store references for proper cleanup
        this.boundHandleClick = (e: MouseEvent) => this.handleClick(e);
        this.boundHandleMouseMove = (e: MouseEvent) => this.handleMouseMove(e);
        this.boundHandleMouseLeave = () => this.handleMouseLeave();
        this.boundHandleMouseDown = (e: MouseEvent) => this.handleMouseDown(e);
        this.boundHandleMouseUp = (e: MouseEvent) => this.handleMouseUp(e);
        this.boundHandleTouchStart = (e: TouchEvent) => this.handleTouchStart(e);
        this.boundHandleTouchMove = (e: TouchEvent) => this.handleTouchMove(e);
        this.boundHandleTouchEnd = (e: TouchEvent) => this.handleTouchEnd(e);
        this.boundOnResize = () => this.onResize();
        this.boundHandleDocumentClick = (e: MouseEvent) => this.handleDocumentClick(e);

        this.svg.addEventListener('click', this.boundHandleClick);
        this.svg.addEventListener('mousemove', this.boundHandleMouseMove);
        this.svg.addEventListener('mouseleave', this.boundHandleMouseLeave);
        this.svg.addEventListener('mousedown', this.boundHandleMouseDown);
        this.svg.addEventListener('mouseup', this.boundHandleMouseUp);
        this.svg.addEventListener('touchstart', this.boundHandleTouchStart, { passive: false });
        this.svg.addEventListener('touchmove', this.boundHandleTouchMove, { passive: false });
        this.svg.addEventListener('touchend', this.boundHandleTouchEnd);

        window.addEventListener('resize', this.boundOnResize);
        document.addEventListener('click', this.boundHandleDocumentClick, true);
        this.onResize();
    }

    private async injectBoardCSS() {
        if (!document.querySelector(`link[data-board-css]`)) {
            const link = document.createElement('link');
            link.rel = 'stylesheet';
            link.href = BOARD_CSS_URL;
            link.setAttribute('data-board-css', 'true');
            document.head.appendChild(link);
        }
    }

    private async inlineSpriteDefs(targetSvg: SVGSVGElement) {
        // Fetch the sprite SVG and insert its <defs> or <symbol> content into the main SVG
        const response = await fetch(SPRITE_URL);
        const svgText = await response.text();
        const parser = new DOMParser();
        const spriteDoc = parser.parseFromString(svgText, 'image/svg+xml');
        // Move all <defs> and <symbol> children into our SVG
        const spriteSvg = spriteDoc.documentElement;
        for (const child of Array.from(spriteSvg.children)) {
            if (child.tagName === 'defs' || child.tagName === 'symbol') {
                targetSvg.appendChild(targetSvg.ownerDocument.importNode(child, true));
            }
        }
    }

    private createBoard(): void {
        // Use <use> elements to build the board as in board.svg
        // 9 rows, alternating odd/even
        for (let row = 0; row < BOARD_SIZE; row++) {
            const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
            use.setAttribute('href', row % 2 === 0 ? '#board-row-odd' : '#board-row-even');
            use.setAttribute('y', String(row * SQUARE_HEIGHT));
            this.boardGroup.appendChild(use);
        }
    }

    private createCoordinates(flipped: boolean): void {
        // Clear existing coordinate labels
        while (this.coordsGroup.firstChild) {
            this.coordsGroup.removeChild(this.coordsGroup.firstChild);
        }

        const columns = flipped
            ? ['I', 'H', 'G', 'F', 'E', 'D', 'C', 'B', 'A']
            : ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I'];

        // Row labels (1-9) on the left side
        for (let row = 0; row < BOARD_SIZE; row++) {
            const rowNumber = flipped ? (row + 1) : (BOARD_SIZE - row);
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('x', String(-COORD_WIDTH / 2));
            text.setAttribute('y', String(row * SQUARE_HEIGHT + SQUARE_HEIGHT / 2));
            text.setAttribute('text-anchor', 'middle');
            text.setAttribute('dominant-baseline', 'central');
            text.setAttribute('class', 'coord-label');
            text.textContent = String(rowNumber);
            this.coordsGroup.appendChild(text);
        }

        // Column labels (A-I) at the bottom
        for (let col = 0; col < BOARD_SIZE; col++) {
            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('x', String(col * SQUARE_WIDTH + SQUARE_WIDTH / 2));
            text.setAttribute('y', String(BOARD_HEIGHT + COORD_HEIGHT / 2));
            text.setAttribute('text-anchor', 'middle');
            text.setAttribute('dominant-baseline', 'central');
            text.setAttribute('class', 'coord-label');
            text.textContent = columns[col];
            this.coordsGroup.appendChild(text);
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

    private getTileIndex(x: number, y: number): number | null {
        // Given board coordinates (x, y), return the tile index, respecting board flip
        const col = Math.floor(x / SQUARE_WIDTH);
        const row = Math.floor(y / SQUARE_HEIGHT);
        if (col < 0 || col >= BOARD_SIZE || row < 0 || row >= BOARD_SIZE) {
            return null;
        }
        const visualIndex = row * BOARD_SIZE + col;
        const flipped = this.gameState.isBoardFlipped();
        return flipped ? (LAST_BOARD_INDEX - visualIndex) : visualIndex;
    }

    async render(boardData: Uint8Array, flipped: boolean): Promise<void> {
        // Check if board orientation changed
        const orientationChanged = this.currentFlipped !== flipped;

        // If this is the first render or orientation changed, recreate all pieces
        if (!this.currentBoardData || orientationChanged) {
            await this.recreateAllPieces(boardData, flipped);
            if (orientationChanged) {
                this.createCoordinates(flipped);
            }
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

    private async createPieceUse(x: number, y: number, pieceType: string, color: boolean, reversed: boolean, tileIndex: number, isTopPiece: boolean): Promise<void> {
        // Use <use> referencing the inlined sprite symbol
        const colorClass = color ? 'p-w' : 'p-b';
        const reversedClass = color !== reversed ? '' : 'p-r';
        const use = document.createElementNS('http://www.w3.org/2000/svg', 'use');
        use.setAttribute('href', `#piece-${pieceType}`);
        use.setAttribute('class', `piece ${colorClass} ${reversedClass}`);
        use.setAttribute('x', x.toString());
        use.setAttribute('y', y.toString());
        use.dataset.tileIndex = tileIndex.toString();
        use.dataset.isTopPiece = isTopPiece.toString();
        this.piecesGroup.appendChild(use);
    }

    private async updatePieceAtIndex(index: number, pieceVal: number, flipped: boolean): Promise<void> {
        // Remove existing pieces at this position
        this.removePiecesAtIndex(index);

        // Add new piece if there's a piece
        const piece = decodePiece(pieceVal);
        if (!piece) return;

        const {x, y} = this.getTilePosition(index);
        // Use raw x/y, no PIECE_X_OFFSET or PIECE_Y_OFFSET
        // Render bottom piece
        await this.createPieceUse(
            x,
            y,
            piece.bottom,
            piece.color,
            flipped,
            index,
            false
        );

        // Render top piece if stacked
        if (piece.top) {
            await this.createPieceUse(
                x,
                y - STACKED_OFFSET,
                piece.top,
                piece.color,
                flipped,
                index,
                true
            );
        }
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
                case 'last_move':
                    color = '#e89038';
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
        // If the card is visible, clicking on the SVG closes it and swallows the click
        if (this.cardVisible) {
            this.hidePieceCard();
            this.preventNextClick = false;
            return;
        }

        if (this.preventNextClick) {
            this.preventNextClick = false;
            return;
        }
        if (!this.clickHandler) return;

        const pos = this.getPosFromMouseEvent(event);
        if (pos !== null) {
            this.clickHandler(pos, event.shiftKey);
        }
    }

    private handleMouseDown(event: MouseEvent): void {
        if (event.button !== 0) return;
        const pos = this.getPosFromMouseEvent(event);
        if (pos === null) return;
        this.dragState.from = pos;
        this.dragState.startX = event.clientX;
        this.dragState.startY = event.clientY;
        this.dragState.active = false;
        // Start long press timer on mouse down
        this.startLongPressTimer(pos, event.clientX, event.clientY);
    }

    private handleMouseMove(event: MouseEvent): void {
        // Handle drag in progress
        if (this.dragState.from >= 0) {
            const dx = event.clientX - this.dragState.startX;
            const dy = event.clientY - this.dragState.startY;
            if (!this.dragState.active && (dx * dx + dy * dy) > SVGBoardView.DRAG_THRESHOLD * SVGBoardView.DRAG_THRESHOLD) {
                this.cancelLongPressTimer();
                this.startDrag(this.dragState.from);
            }
            if (this.dragState.active && this.dragState.ghost) {
                this.updateDragGhost(event.clientX, event.clientY);
            }
            return;
        }

        // Normal hover
        if (!this.hoverHandler) return;
        const pos = this.getPosFromMouseEvent(event);
        if (pos !== null) {
            this.hoverHandler(pos);
        } else {
            this.hoverHandler(null);
        }
    }

    private handleMouseUp(event: MouseEvent): void {
        this.cancelLongPressTimer();
        if (!this.dragState.active) {
            this.dragState.from = -1;
            return;
        }
        const to = this.getPosFromMouseEvent(event);
        this.endDrag(to, event.shiftKey);
    }

    private handleTouchStart(event: TouchEvent): void {
        if (event.touches.length !== 1) return;
        const touch = event.touches[0];
        const pos = this.getPosFromTouch(touch);
        if (pos === null) return;
        this.dragState.from = pos;
        this.dragState.startX = touch.clientX;
        this.dragState.startY = touch.clientY;
        this.dragState.active = false;
        // Start long press timer for touch
        this.startLongPressTimer(pos, touch.clientX, touch.clientY);
    }

    private handleTouchMove(event: TouchEvent): void {
        if (this.dragState.from < 0 || event.touches.length !== 1) return;
        const touch = event.touches[0];
        const dx = touch.clientX - this.dragState.startX;
        const dy = touch.clientY - this.dragState.startY;
        if (!this.dragState.active && (dx * dx + dy * dy) > SVGBoardView.DRAG_THRESHOLD * SVGBoardView.DRAG_THRESHOLD) {
            this.cancelLongPressTimer();
            this.startDrag(this.dragState.from);
        }
        if (this.dragState.active && this.dragState.ghost) {
            event.preventDefault();
            this.updateDragGhost(touch.clientX, touch.clientY);
        }
    }

    private handleTouchEnd(event: TouchEvent): void {
        this.cancelLongPressTimer();
        if (!this.dragState.active) {
            // If card is visible, a tap anywhere closes it
            if (this.cardVisible) {
                this.hidePieceCard();
                return;
            }
            this.dragState.from = -1;
            return;
        }
        // Use the last known touch position from changedTouches
        const touch = event.changedTouches[0];
        const to = this.getPosFromTouch(touch);
        this.endDrag(to, event.shiftKey);
    }

    private startDrag(from: number): void {
        this.dragState.active = true;

        // Hide the source piece(s) so only the ghost is visible
        const sourcePieces = this.piecesGroup.querySelectorAll(`[data-tile-index="${from}"]`);
        sourcePieces.forEach(el => (el as SVGElement).style.visibility = 'hidden');

        // Select the source piece to show potential move highlights
        if (this.clickHandler) {
            this.clickHandler(from);
        }

        // Create a ghost piece for visual feedback
        this.createDragGhost(from);
    }

    private endDrag(to: number | null, shiftKey: boolean = false): void {
        // Remove ghost
        if (this.dragState.ghost) {
            this.dragState.ghost.remove();
            this.dragState.ghost = null;
        }

        const from = this.dragState.from;
        this.dragState.active = false;
        this.dragState.from = -1;
        this.preventNextClick = true;

        // Restore hidden source pieces
        const sourcePieces = this.piecesGroup.querySelectorAll(`[data-tile-index="${from}"]`);
        sourcePieces.forEach(el => (el as SVGElement).style.visibility = '');

        if (to !== null && to !== from && this.dragMoveHandler) {
            this.dragMoveHandler(from, to, shiftKey);
        } else if (this.clickHandler) {
            // Cancel: deselect by clicking the same position
            this.clickHandler(from);
        }
    }

    private createDragGhost(from: number): void {
        if (!this.currentBoardData) return;
        const pieceVal = this.currentBoardData[from];
        if (pieceVal === 0) return;

        const piece = decodePiece(pieceVal);
        if (!piece) return;

        const ghost = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        ghost.setAttribute('pointer-events', 'none');
        ghost.style.opacity = '0.7';

        const flipped = this.gameState.isBoardFlipped();
        const colorClass = piece.color ? 'p-w' : 'p-b';
        const reversedClass = (piece.color !== flipped) ? '' : 'p-r';

        // Render bottom piece
        const useBottom = document.createElementNS('http://www.w3.org/2000/svg', 'use');
        useBottom.setAttribute('href', `#piece-${piece.bottom}`);
        useBottom.setAttribute('class', `piece ${colorClass} ${reversedClass}`);
        useBottom.setAttribute('x', '0');
        useBottom.setAttribute('y', '0');
        ghost.appendChild(useBottom);

        // Render top piece if stacked
        if (piece.top) {
            const useTop = document.createElementNS('http://www.w3.org/2000/svg', 'use');
            useTop.setAttribute('href', `#piece-${piece.top}`);
            useTop.setAttribute('class', `piece ${colorClass} ${reversedClass}`);
            useTop.setAttribute('x', '0');
            useTop.setAttribute('y', (-STACKED_OFFSET).toString());
            ghost.appendChild(useTop);
        }

        this.svg.appendChild(ghost);
        this.dragState.ghost = ghost;
    }

    private updateDragGhost(clientX: number, clientY: number): void {
        if (!this.dragState.ghost) return;
        const rect = this.svg.getBoundingClientRect();
        const totalSvgWidth = BOARD_WIDTH + COORD_WIDTH;
        const totalSvgHeight = BOARD_HEIGHT + COORD_HEIGHT;
        const svgX = ((clientX - rect.left) / rect.width) * totalSvgWidth - COORD_WIDTH - SQUARE_WIDTH / 2;
        const svgY = ((clientY - rect.top) / rect.height) * totalSvgHeight - SQUARE_HEIGHT / 2;
        this.dragState.ghost.setAttribute('transform', `translate(${svgX}, ${svgY})`);
    }

    private getPosFromTouch(touch: Touch): number | null {
        const rect = this.svg.getBoundingClientRect();
        const x = touch.clientX - rect.left;
        const y = touch.clientY - rect.top;
        const totalSvgWidth = BOARD_WIDTH + COORD_WIDTH;
        const totalSvgHeight = BOARD_HEIGHT + COORD_HEIGHT;
        const svgX = (x / rect.width) * totalSvgWidth - COORD_WIDTH;
        const svgY = (y / rect.height) * totalSvgHeight;
        return this.getTileIndex(svgX, svgY);
    }

    private handleMouseLeave(): void {
        this.cancelLongPressTimer();
        if (this.hoverHandler) {
            this.hoverHandler(null);
        }
    }

    private handleDocumentClick(event: MouseEvent): void {
        if (!this.cardVisible) return;
        // Close card on any click outside the SVG
        if (!this.svg.contains(event.target as Node)) {
            this.hidePieceCard();
        }
    }

    private getPosFromMouseEvent(event: MouseEvent): number | null {
        const rect = this.svg.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;

        // Convert from screen coordinates to SVG coordinates
        // ViewBox is: -COORD_WIDTH 0 (BOARD_WIDTH + COORD_WIDTH) (BOARD_HEIGHT + COORD_HEIGHT)
        const totalSvgWidth = BOARD_WIDTH + COORD_WIDTH;
        const totalSvgHeight = BOARD_HEIGHT + COORD_HEIGHT;
        const svgX = (x / rect.width) * totalSvgWidth - COORD_WIDTH;
        const svgY = (y / rect.height) * totalSvgHeight;

        return this.getTileIndex(svgX, svgY);
    }

    onTileClick(handler: (tileIndex: number, shiftKey?: boolean) => void): void {
        this.clickHandler = handler;
    }

    onTileHover(handler: (tileIndex: number | null) => void): void {
        this.hoverHandler = handler;
    }

    onDragMove(handler: (from: number, to: number, shiftKey?: boolean) => void): void {
        this.dragMoveHandler = handler;
    }

    onPieceLongHover(handler: (tileIndex: number, clientX: number, clientY: number) => void): void {
        this.pieceLongHoverHandler = handler;
    }

    /**
     * Start the long-press timer. The timer fires only if the button/finger
     * is held down without moving beyond the drag threshold.
     */
    private startLongPressTimer(pos: number, clientX: number, clientY: number): void {
        this.cancelLongPressTimer();
        if (!this.currentBoardData || this.currentBoardData[pos] === 0) return;
        this.longPressTimer = setTimeout(() => {
            this.longPressTimer = null;
            // Neutralise drag state so a subsequent mousemove/touchmove can't start a drag
            this.dragState.from = -1;
            this.dragState.active = false;
            this.showPieceCard(pos);
            if (this.pieceLongHoverHandler) {
                this.pieceLongHoverHandler(pos, clientX, clientY);
            }
        }, SVGBoardView.LONG_PRESS_DELAY);
    }

    private cancelLongPressTimer(): void {
        if (this.longPressTimer !== null) {
            clearTimeout(this.longPressTimer);
            this.longPressTimer = null;
        }
    }

    // ─── Piece Info Card (SVG-native) ────────────────────────────────────────

    /**
     * Show an inline SVG card next to the tile at `tileIndex`.
     * The card contains:
     *  - A 2× visual of the piece/stack
     *  - The piece name(s)
     *  - The movement description in French
     */
    private showPieceCard(tileIndex: number): void {
        if (!this.currentBoardData) return;
        const pieceVal = this.currentBoardData[tileIndex];
        if (pieceVal === 0) return;

        const piece = decodePiece(pieceVal);
        if (!piece) return;

        // Build text lines with type info (header = bold name, body = movement line, spacer = empty gap)
        type TextLine = { text: string; bold: boolean; spacer?: boolean };
        const lines: TextLine[] = [];

        const addPieceLines = (type: string) => {
            const rule = PIECE_RULES[type];
            if (!rule) return;
            lines.push({ text: rule.name, bold: true });
            rule.movement.split('\n').forEach(l => lines.push({ text: l, bold: false }));
        };

        if (piece.top) {
            addPieceLines(piece.top);
            lines.push({ text: '', bold: false, spacer: true });
            addPieceLines(piece.bottom);
        } else {
            addPieceLines(piece.bottom);
        }

        // Card height: padding top + piece preview + gap + text area + padding bottom
        const textAreaH = lines.reduce((h, l) => h + (l.spacer ? CARD_TEXT_LINE_H * 0.5 : CARD_TEXT_LINE_H), 0);
        const cardHeight = CARD_PADDING + CARD_PIECE_H + CARD_PADDING + textAreaH + CARD_PADDING;

        // Tile position in SVG coordinates
        const {x: tileX, y: tileY} = this.getTilePosition(tileIndex);

        // Determine which visual column the tile is in (0–8)
        const visualCol = this.gameState.isBoardFlipped()
            ? (BOARD_SIZE - 1 - (tileIndex % BOARD_SIZE))
            : (tileIndex % BOARD_SIZE);

        // Place card to the left if piece is on right half, to the right otherwise
        const onRightHalf = visualCol >= Math.floor(BOARD_SIZE / 2);
        let cardX: number;
        if (onRightHalf) {
            // Card is to the left of the tile
            cardX = tileX - CARD_WIDTH - 4;
        } else {
            // Card is to the right of the tile
            cardX = tileX + SQUARE_WIDTH + 4;
        }

        // Vertically center card on the tile, clamped within board
        let cardY = tileY + SQUARE_HEIGHT / 2 - cardHeight / 2;
        cardY = Math.max(0, Math.min(BOARD_HEIGHT - cardHeight, cardY));

        // Clear previous card content
        while (this.cardGroup.firstChild) {
            this.cardGroup.removeChild(this.cardGroup.firstChild);
        }

        // Background rect with drop shadow effect
        const shadow = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
        shadow.setAttribute('x', (cardX + 3).toString());
        shadow.setAttribute('y', (cardY + 3).toString());
        shadow.setAttribute('width', CARD_WIDTH.toString());
        shadow.setAttribute('height', cardHeight.toString());
        shadow.setAttribute('rx', '6');
        shadow.setAttribute('fill', 'rgba(0,0,0,0.25)');
        this.cardGroup.appendChild(shadow);

        const bg = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
        bg.setAttribute('x', cardX.toString());
        bg.setAttribute('y', cardY.toString());
        bg.setAttribute('width', CARD_WIDTH.toString());
        bg.setAttribute('height', cardHeight.toString());
        bg.setAttribute('rx', '6');
        bg.setAttribute('fill', '#fffdf8');
        bg.setAttribute('stroke', '#55442d');
        bg.setAttribute('stroke-width', '1.5');
        this.cardGroup.appendChild(bg);

        // Piece preview — scaled 2× using a nested SVG (foreignObject would not work in all browsers)
        // We use a <g transform="scale(2)"> centred inside the card.
        const flipped = this.gameState.isBoardFlipped();
        const colorClass = piece.color ? 'p-w' : 'p-b';
        const reversedClass = (piece.color !== flipped) ? '' : 'p-r';

        // The piece symbols are drawn in a 100×80 viewport. We scale by 2 and translate to center.
        const scaleX = cardX + CARD_PADDING;
        const scaleY = cardY + CARD_PADDING;

        const pieceGroup = document.createElementNS('http://www.w3.org/2000/svg', 'g');
        pieceGroup.setAttribute('transform', `translate(${scaleX}, ${scaleY}) scale(${CARD_PIECE_SCALE})`);
        pieceGroup.setAttribute('pointer-events', 'none');

        // Tile background for the preview
        const tileBg = document.createElementNS('http://www.w3.org/2000/svg', 'rect');
        tileBg.setAttribute('x', '0');
        tileBg.setAttribute('y', '0');
        tileBg.setAttribute('width', SQUARE_WIDTH.toString());
        tileBg.setAttribute('height', SQUARE_HEIGHT.toString());
        tileBg.setAttribute('fill', '#d2b48c');
        tileBg.setAttribute('rx', '3');
        pieceGroup.appendChild(tileBg);

        // Bottom piece
        const useBottom = document.createElementNS('http://www.w3.org/2000/svg', 'use');
        useBottom.setAttribute('href', `#piece-${piece.bottom}`);
        useBottom.setAttribute('class', `piece ${colorClass} ${reversedClass}`);
        useBottom.setAttribute('x', '0');
        useBottom.setAttribute('y', '0');
        pieceGroup.appendChild(useBottom);

        // Top piece if stacked
        if (piece.top) {
            const useTop = document.createElementNS('http://www.w3.org/2000/svg', 'use');
            useTop.setAttribute('href', `#piece-${piece.top}`);
            useTop.setAttribute('class', `piece ${colorClass} ${reversedClass}`);
            useTop.setAttribute('x', '0');
            useTop.setAttribute('y', (-STACKED_OFFSET).toString());
            pieceGroup.appendChild(useTop);
        }

        this.cardGroup.appendChild(pieceGroup);

        // Text lines
        const textStartX = cardX + CARD_PADDING;
        const textStartY = cardY + CARD_PADDING + CARD_PIECE_H + CARD_PADDING;

        // Track vertical offset separately to skip spacer rows visually
        let yOffset = 0;
        lines.forEach((line) => {
            if (line.spacer) {
                yOffset += CARD_TEXT_LINE_H * 0.5; // half-height gap between stacked pieces
                return;
            }

            const text = document.createElementNS('http://www.w3.org/2000/svg', 'text');
            text.setAttribute('x', textStartX.toString());
            text.setAttribute('y', (textStartY + yOffset + CARD_TEXT_LINE_H * 0.75).toString());
            text.setAttribute('pointer-events', 'none');
            text.setAttribute('font-family', 'sans-serif');
            text.setAttribute('font-size', '13');

            if (line.bold) {
                text.setAttribute('font-weight', 'bold');
                text.setAttribute('fill', '#3a2c1a');
            } else {
                text.setAttribute('fill', '#5a4a3a');
            }
            text.textContent = line.text;
            this.cardGroup.appendChild(text);
            yOffset += CARD_TEXT_LINE_H;
        });

        this.cardGroup.style.display = '';
        this.cardVisible = true;
    }

    private hidePieceCard(): void {
        this.cardGroup.style.display = 'none';
        this.cardVisible = false;
    }

    dispose(): void {
        // Remove event listeners using stored references
        if (this.boundOnResize) {
            window.removeEventListener('resize', this.boundOnResize);
        }
        if (this.boundHandleDocumentClick) {
            document.removeEventListener('click', this.boundHandleDocumentClick, true);
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
            if (this.boundHandleMouseDown) {
                this.svg.removeEventListener('mousedown', this.boundHandleMouseDown);
            }
            if (this.boundHandleMouseUp) {
                this.svg.removeEventListener('mouseup', this.boundHandleMouseUp);
            }
            if (this.boundHandleTouchStart) {
                this.svg.removeEventListener('touchstart', this.boundHandleTouchStart);
            }
            if (this.boundHandleTouchMove) {
                this.svg.removeEventListener('touchmove', this.boundHandleTouchMove);
            }
            if (this.boundHandleTouchEnd) {
                this.svg.removeEventListener('touchend', this.boundHandleTouchEnd);
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
