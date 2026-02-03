import * as THREE from 'three';
import {IBoardView, TileHighlight} from './IBoardView';
import {BOARD_SIZE, LAST_BOARD_INDEX} from '../models/types';
import {GameState} from '../models/GameState';
import {decodePiece} from "../utils/boardUtils";

interface TileOverlay {
    geometry: THREE.PlaneGeometry;
    material: THREE.MeshBasicMaterial;
    userData: {
        tileIndex: number;
    };
    position: THREE.Vector3;
}

// Board margin constants for aligning with physical board image
const BOARD_ASPECT_RATIO = 1.285;
const BOARD_MARGIN_TOP = 0.106
const BOARD_MARGIN_SIDES = 0.095; // Left and right margins
const BOARD_MARGIN_BOTTOM = 0.136;
const PIECE_SCALE_FACTOR = 0.75; // Scale factor for piece size relative to tile size
const PIECE_OFFSET_Y = 0.008;
const PIECE_TOP_OFFSET_FACTOR = 0.2; // Relative to tile height
const OVERLAY_SCALE_FACTOR = 1;

class Tile {
    index: number;
    col: number;
    row: number;
    x: number;
    y: number;
    width: number;
    height: number;

    constructor(index: number, col: number, row: number, x: number, y: number, width: number, height: number) {
        this.index = index;
        this.col = col;
        this.row = row;
        this.x = x;
        this.y = y;
        this.width = width;
        this.height = height;
    }
}

/**
 * Three.js sprite-based board renderer
 */
export default class ThreeJSBoardView implements IBoardView {
    private scene!: THREE.Scene;
    private camera!: THREE.OrthographicCamera;
    private renderer!: THREE.WebGLRenderer;
    private canvas!: HTMLCanvasElement;
    private boardSprite: THREE.Sprite | null = null;
    private overlaySprites: (THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial> & TileOverlay)[] = [];
    private pieceSprites: THREE.Sprite[] = [];
    private raycaster!: THREE.Raycaster;
    private mouse!: THREE.Vector2;
    private container!: HTMLElement;
    private gameState: GameState;

    private clickHandler: ((tileIndex: number) => void) | null = null;
    private hoverHandler: ((tileIndex: number | null) => void) | null = null;

    // Texture cache to avoid repeated network requests
    private textureCache: Map<string, THREE.Texture> = new Map();

    // Track current board state to enable differential updates
    private currentBoardData: Uint8Array | null = null;
    private currentFlipped: boolean = false;

    constructor(gameState: GameState) {
        this.gameState = gameState;
    }

    initialize(container: HTMLElement): Promise<void> {
        this.container = container;

        // Create canvas
        this.canvas = document.createElement('canvas');
        this.canvas.id = 'board-canvas';
        this.canvas.style.cursor = 'pointer';
        container.appendChild(this.canvas);

        // Create scene
        this.scene = new THREE.Scene();

        // Create orthographic camera
        const aspect = BOARD_ASPECT_RATIO;
        this.camera = new THREE.OrthographicCamera(
            -aspect / 2,
            aspect / 2,
            1 / 2,
            -1 / 2,
            0.1,
            1000
        );
        this.camera.position.z = 10;

        // Create renderer
        this.renderer = new THREE.WebGLRenderer({canvas: this.canvas, alpha: true, antialias: true});
        this.renderer.setPixelRatio(window.devicePixelRatio);
        this.renderer.toneMapping = THREE.NoToneMapping;
        this.updateRendererSize();

        // Raycaster for mouse picking
        this.raycaster = new THREE.Raycaster();
        this.mouse = new THREE.Vector2();

        // Event listeners
        window.addEventListener('resize', () => {
            try {
                this.onResize();
            } catch (err) {
                console.error('Resize event error:', err);
            }
        });
        this.canvas.addEventListener('click', (e) => {
            try {
                this.handleClick(e);
            } catch (err) {
                console.error('Click event error:', err);
            }
        });
        this.canvas.addEventListener('mousemove', (e) => {
            try {
                this.handleMouseMove(e);
            } catch (err) {
                console.error('MouseMove event error:', err);
            }
        });
        this.canvas.addEventListener('mouseleave', () => {
            try {
                this.handleMouseLeave();
            } catch (err) {
                console.error('MouseLeave event error:', err);
            }
        });

        // Create board and overlays
        this.createOverlays();

        return Promise.resolve();
    }

    private updateRendererSize(): void {
        const containerWidth = this.container.clientWidth;
        const height = containerWidth / BOARD_ASPECT_RATIO;
        this.renderer.setSize(containerWidth, height);
    }

    onResize(): void {
        this.updateRendererSize();

        const aspect = BOARD_ASPECT_RATIO;
        this.camera.left = -1 * aspect / 2;
        this.camera.right = aspect / 2;
        this.camera.top = 1 / 2;
        this.camera.bottom = -1 / 2;
        this.camera.updateProjectionMatrix();
        this.renderScene();
    }

    private async updateBoard(): Promise<void> {
        if (!this.boardSprite) {
            const texture = await this.loadTexture('/images/board-v2.webp');
            texture.colorSpace = THREE.SRGBColorSpace;
            const material = new THREE.SpriteMaterial({map: texture});
            this.boardSprite = new THREE.Sprite(material);
            this.boardSprite.scale.set(BOARD_ASPECT_RATIO, 1, 1);
            this.boardSprite.position.z = -1;
            this.scene.add(this.boardSprite);
        }
    }

    private createOverlays(): void {
        this.overlaySprites.forEach(sprite => {
            this.scene.remove(sprite);
            sprite.geometry.dispose();
            sprite.material.dispose();
        });
        this.overlaySprites = [];

        for (let i = 0; i <= LAST_BOARD_INDEX; i++) {
            const tile = this.getTile(i);
            const geometry = new THREE.PlaneGeometry(tile.width * OVERLAY_SCALE_FACTOR, tile.height * OVERLAY_SCALE_FACTOR);
            const material = new THREE.MeshBasicMaterial({
                color: 0xffffff,
                transparent: true,
                opacity: 0,
                side: THREE.DoubleSide
            });
            const mesh = new THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>(geometry, material);
            mesh.position.set(tile.x, tile.y, 0);
            (mesh.userData as { tileIndex: number }).tileIndex = i;

            this.scene.add(mesh);
            this.overlaySprites.push(mesh as any);
        }
    }

    private getTile(index: number): Tile {
        const actualIndex = this.gameState.isBoardFlipped() ? (LAST_BOARD_INDEX - index) : index;
        const col = actualIndex % BOARD_SIZE;
        const row = Math.floor(actualIndex / BOARD_SIZE);

        const tileWidth = BOARD_ASPECT_RATIO * (1 - 2 * BOARD_MARGIN_SIDES) / BOARD_SIZE;
        const tileHeight = (1 - BOARD_MARGIN_TOP - BOARD_MARGIN_BOTTOM) / BOARD_SIZE;

        const x = -BOARD_ASPECT_RATIO / 2 + tileWidth * (col + 0.5) + BOARD_MARGIN_SIDES * BOARD_ASPECT_RATIO;
        const y = 1 / 2 - tileHeight * (row + 0.5) - BOARD_MARGIN_TOP;

        return new Tile(index, col, row, x, y, tileWidth, tileHeight);
    }

    updateOverlays(highlights: TileHighlight[]): void {
        const flipped = this.gameState.isBoardFlipped();

        // Reset all overlays
        for (let i = 0; i <= LAST_BOARD_INDEX; i++) {
            const overlay = this.overlaySprites[i];
            overlay.material.opacity = 0;
        }

        // Apply highlights
        for (const highlight of highlights) {
            const tileIndex = highlight.position;
            const visualIndex = flipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
            const overlay = this.overlaySprites[visualIndex];

            switch (highlight.type) {
                case 'selected':
                    overlay.material.color.setHex(0x7fa0dd);
                    overlay.material.opacity = 0.6;
                    break;
                case 'potential':
                    overlay.material.color.setHex(0x55d157);
                    overlay.material.opacity = 0.5;
                    break;
                case 'hovered':
                    overlay.material.color.setHex(0xe1ca58);
                    overlay.material.opacity = 0.4;
                    break;
                case 'threat':
                    overlay.material.color.setHex(0xff4444);
                    overlay.material.opacity = 0.5;
                    break;
            }
        }

        this.renderScene();
    }

    async render(boardData: Uint8Array, flipped: boolean): Promise<void> {
        try {
            await this.updateBoard();
            await this.updatePieceSprites(boardData, flipped);
            this.renderScene();
        } catch (err) {
            console.error('Error in render:', err);
        }
    }

    private async updatePieceSprites(boardData: Uint8Array, flipped: boolean): Promise<void> {
        // Check if board orientation changed
        const orientationChanged = this.currentFlipped !== flipped;

        // If this is the first render or orientation changed, recreate all sprites
        if (!this.currentBoardData || orientationChanged) {
            await this.recreateAllPieceSprites(boardData, flipped);
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
            await this.updatePieceSpriteAtIndex(index, boardData[index], flipped);
        }

        // Update cached board state
        this.currentBoardData = new Uint8Array(boardData);
    }

    private async updatePieceSpriteAtIndex(index: number, pieceVal: number, flipped: boolean): Promise<void> {
        // Remove existing sprites at this position
        this.removePieceSpritesAtIndex(index);

        // Add new sprites if there's a piece
        const piece = decodePiece(pieceVal);
        if (!piece) return;

        const tile = this.getTile(index);
        const pieceSize = tile.width * PIECE_SCALE_FACTOR;

        // Load bottom piece
        const bottomTexture = await this.loadPieceSprite(piece.bottom, piece.color, flipped);
        const bottomMaterial = new THREE.SpriteMaterial({map: bottomTexture});
        const bottomSprite = new THREE.Sprite(bottomMaterial);
        bottomSprite.scale.set(pieceSize, pieceSize, 1);
        bottomSprite.position.set(tile.x, tile.y + PIECE_OFFSET_Y, 1 - (0.1 * tile.row));
        bottomSprite.userData.tileIndex = index;
        bottomSprite.userData.isTopPiece = false;
        this.scene.add(bottomSprite);
        this.pieceSprites.push(bottomSprite);

        // Load top piece if stacked
        if (!piece.top) return;

        const topTexture = await this.loadPieceSprite(piece.top, piece.color, flipped);
        const topMaterial = new THREE.SpriteMaterial({map: topTexture});
        const topSprite = new THREE.Sprite(topMaterial);
        topSprite.scale.set(pieceSize, pieceSize, 1);
        topSprite.position.set(tile.x, tile.y + PIECE_OFFSET_Y + pieceSize * PIECE_TOP_OFFSET_FACTOR, 2 - (0.1 * tile.row));
        topSprite.userData.tileIndex = index;
        topSprite.userData.isTopPiece = true;
        this.scene.add(topSprite);
        this.pieceSprites.push(topSprite);
    }

    private removePieceSpritesAtIndex(index: number): void {
        // Find and remove all sprites at this tile index
        const spritesToRemove = this.pieceSprites.filter(
            sprite => sprite.userData.tileIndex === index
        );

        for (const sprite of spritesToRemove) {
            this.scene.remove(sprite);
            sprite.geometry.dispose();
            sprite.material.dispose();
        }

        // Update pieceSprites array
        this.pieceSprites = this.pieceSprites.filter(
            sprite => sprite.userData.tileIndex !== index
        );
    }

    private async recreateAllPieceSprites(boardData: Uint8Array, flipped: boolean): Promise<void> {
        // Clear existing pieces
        this.pieceSprites.forEach(sprite => {
            this.scene.remove(sprite);
            sprite.geometry.dispose();
            sprite.material.dispose();
        });
        this.pieceSprites = [];

        for (let i = 0; i <= LAST_BOARD_INDEX; i++) {
            try {
                const pieceVal = boardData[i];
                const piece = decodePiece(pieceVal);
                if (!piece) continue;

                const tile = this.getTile(i);
                const pieceSize = tile.width * PIECE_SCALE_FACTOR;

                // Load bottom piece
                const bottomTexture = await this.loadPieceSprite(piece.bottom, piece.color, flipped);
                const bottomMaterial = new THREE.SpriteMaterial({map: bottomTexture});
                const bottomSprite = new THREE.Sprite(bottomMaterial);
                bottomSprite.scale.set(pieceSize, pieceSize, 1);
                bottomSprite.position.set(tile.x, tile.y + PIECE_OFFSET_Y, 1);
                bottomSprite.userData.tileIndex = i;
                bottomSprite.userData.isTopPiece = false;
                this.scene.add(bottomSprite);
                this.pieceSprites.push(bottomSprite);

                // Load top piece if stacked
                if (!piece.top) continue;

                const topTexture = await this.loadPieceSprite(piece.top, piece.color, flipped);
                const topMaterial = new THREE.SpriteMaterial({map: topTexture});
                const topSprite = new THREE.Sprite(topMaterial);
                topSprite.scale.set(pieceSize, pieceSize, 1);
                topSprite.position.set(tile.x, tile.y + PIECE_OFFSET_Y + pieceSize * PIECE_TOP_OFFSET_FACTOR, 2);
                topSprite.userData.tileIndex = i;
                topSprite.userData.isTopPiece = true;
                this.scene.add(topSprite);
                this.pieceSprites.push(topSprite);
            } catch (err) {
                console.error(`Error creating piece sprite at index ${i}:`, err);
            }
        }
    }

    private async loadPieceSprite(pieceName: string, color: boolean, reversed: boolean): Promise<THREE.Texture> {
        try {
            const colorName = color ? 'white' : 'black';
            const reversedSuffix = color === reversed ? '-reversed' : '';
            const path = `/images/${pieceName}-${colorName}${reversedSuffix}.png`;
            const texture = await this.loadTexture(path);
            texture.colorSpace = THREE.SRGBColorSpace;
            return texture;
        } catch (err) {
            console.error(`Error loading piece sprite: ${pieceName}, color: ${color}, reversed: ${reversed}`, err);
            throw err;
        }
    }

    private loadTexture(path: string): Promise<THREE.Texture> {
        // Return cached texture if available
        const cached = this.textureCache.get(path);
        if (cached) {
            return Promise.resolve(cached);
        }

        return new Promise((resolve, reject) => {
            const loader = new THREE.TextureLoader();
            loader.load(path, (texture) => {
                // Cache the loaded texture
                this.textureCache.set(path, texture);
                resolve(texture);
            }, undefined, (err) => {
                console.error(`Error loading texture: ${path}`, err);
                reject(err);
            });
        });
    }

    private renderScene(): void {
        this.renderer.render(this.scene, this.camera);
    }

    private handleClick(event: MouseEvent): void {
        if (!this.clickHandler) return;

        const pos = this.getPosFromMouseEvent(event);
        if (pos != null) {
            this.clickHandler(pos);
        }
    }

    private handleMouseMove(event: MouseEvent): void {
        if (!this.hoverHandler) return;

        const pos = this.getPosFromMouseEvent(event);
        if (pos != null) {
            this.hoverHandler(pos);
        } else {
            this.hoverHandler(null);
        }
    }

    private getPosFromMouseEvent(event: MouseEvent): number | null {
        const rect = this.canvas.getBoundingClientRect();
        this.mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
        this.mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;

        this.raycaster.setFromCamera(this.mouse, this.camera);
        const intersects = this.raycaster.intersectObjects(this.overlaySprites);

        if (intersects.length === 0) {
            return null;
        }

        const tileOverlay = intersects[0].object as any;
        const tileIndex = tileOverlay.userData.tileIndex;
        const flipped = this.gameState.isBoardFlipped();
        return flipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
    }

    private handleMouseLeave(): void {
        if (this.hoverHandler) {
            this.hoverHandler(null);
        }
    }

    onTileClick(handler: (tileIndex: number) => void): void {
        this.clickHandler = handler;
    }

    onTileHover(handler: (tileIndex: number | null) => void): void {
        this.hoverHandler = handler;
    }

    dispose(): void {
        window.removeEventListener('resize', () => this.onResize());

        this.pieceSprites.forEach(sprite => {
            sprite.geometry.dispose();
            sprite.material.dispose();
        });

        this.overlaySprites.forEach(sprite => {
            sprite.geometry.dispose();
            sprite.material.dispose();
        });

        if (this.boardSprite) {
            this.boardSprite.geometry.dispose();
            this.boardSprite.material.dispose();
        }

        // Clean up texture cache
        this.textureCache.forEach(texture => {
            texture.dispose();
        });
        this.textureCache.clear();

        this.renderer.dispose();
    }
}
