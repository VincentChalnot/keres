import * as THREE from 'three';
import { IBoardView, TileHighlight } from './IBoardView';
import { Piece, BOARD_ASPECT_RATIO, LAST_BOARD_INDEX, PIECE_OFFSET_Y, COLOR_NAME } from '../models/types';
import { GameState } from '../models/GameState';

interface TileOverlay {
  geometry: THREE.PlaneGeometry;
  material: THREE.MeshBasicMaterial;
  userData: {
    tileIndex: number;
  };
  position: THREE.Vector3;
}

/**
 * Three.js sprite-based board renderer
 */
export class ThreeJSBoardView implements IBoardView {
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

  constructor(gameState: GameState) {
    this.gameState = gameState;
  }

  initialize(container: HTMLElement): void {
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
    const viewSize = 10;
    this.camera = new THREE.OrthographicCamera(
      -viewSize * aspect / 2,
      viewSize * aspect / 2,
      viewSize / 2,
      -viewSize / 2,
      0.1,
      1000
    );
    this.camera.position.z = 10;

    // Create renderer
    this.renderer = new THREE.WebGLRenderer({ canvas: this.canvas, alpha: true, antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.updateRendererSize();

    // Raycaster for mouse picking
    this.raycaster = new THREE.Raycaster();
    this.mouse = new THREE.Vector2();

    // Event listeners
    window.addEventListener('resize', () => this.onResize());
    this.canvas.addEventListener('click', (e) => this.handleClick(e));
    this.canvas.addEventListener('mousemove', (e) => this.handleMouseMove(e));
    this.canvas.addEventListener('mouseleave', () => this.handleMouseLeave());

    // Create board and overlays
    this.createBoard();
    this.createOverlays();
  }

  private updateRendererSize(): void {
    const containerWidth = this.container.clientWidth;
    const height = containerWidth / BOARD_ASPECT_RATIO;
    this.renderer.setSize(containerWidth, height);
  }

  onResize(): void {
    this.updateRendererSize();

    const aspect = BOARD_ASPECT_RATIO;
    const viewSize = 10;
    this.camera.left = -viewSize * aspect / 2;
    this.camera.right = viewSize * aspect / 2;
    this.camera.top = viewSize / 2;
    this.camera.bottom = -viewSize / 2;
    this.camera.updateProjectionMatrix();
  }

  private async createBoard(): Promise<void> {
    if (this.boardSprite) {
      this.scene.remove(this.boardSprite);
      this.boardSprite.geometry.dispose();
      this.boardSprite.material.dispose();
    }

    const texture = await this.loadTexture('images/board.jpg');
    texture.minFilter = THREE.LinearFilter;

    const material = new THREE.SpriteMaterial({ map: texture });
    this.boardSprite = new THREE.Sprite(material);

    const viewSize = 10;
    this.boardSprite.scale.set(viewSize * BOARD_ASPECT_RATIO, viewSize, 1);
    this.boardSprite.position.z = -1;

    this.scene.add(this.boardSprite);
  }

  private createOverlays(): void {
    this.overlaySprites.forEach(sprite => {
      this.scene.remove(sprite);
      sprite.geometry.dispose();
      sprite.material.dispose();
    });
    this.overlaySprites = [];

    const viewSize = 10;
    const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
    const tileHeight = viewSize / 9;

    for (let i = 0; i < 81; i++) {
      const pos = this.getTilePosition(i);
      const geometry = new THREE.PlaneGeometry(tileWidth * 0.9, tileHeight * 0.9);
      const material = new THREE.MeshBasicMaterial({
        color: 0xffffff,
        transparent: true,
        opacity: 0,
        side: THREE.DoubleSide
      });
      const mesh = new THREE.Mesh<THREE.PlaneGeometry, THREE.MeshBasicMaterial>(geometry, material);
      mesh.position.set(pos.x, pos.y, 0);
      (mesh.userData as { tileIndex: number }).tileIndex = i;

      this.scene.add(mesh);
      this.overlaySprites.push(mesh as any);
    }
  }

  private getTilePosition(index: number): { x: number; y: number } {
    const flipped = this.gameState.isBoardFlipped();
    const actualIndex = flipped ? (LAST_BOARD_INDEX - index) : index;
    const col = actualIndex % 9;
    const row = Math.floor(actualIndex / 9);

    const viewSize = 10;
    const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
    const tileHeight = viewSize / 9;

    const x = -viewSize * BOARD_ASPECT_RATIO / 2 + tileWidth * (col + 0.5);
    const y = viewSize / 2 - tileHeight * (row + 0.5);

    return { x, y };
  }

  updateOverlays(highlights: TileHighlight[]): void {
    const flipped = this.gameState.isBoardFlipped();
    
    // Reset all overlays
    for (let i = 0; i < 81; i++) {
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
        case 'possible':
          overlay.material.color.setHex(0x55d157);
          overlay.material.opacity = 0.5;
          break;
        case 'hovered':
          overlay.material.color.setHex(0xe1ca58);
          overlay.material.opacity = 0.4;
          break;
      }
    }

    this.renderScene();
  }

  async render(boardData: Uint8Array, flipped: boolean): Promise<void> {
    await this.createPieceSprites(boardData, flipped);
    this.renderScene();
  }

  private async createPieceSprites(boardData: Uint8Array, flipped: boolean): Promise<void> {
    // Clear existing pieces
    this.pieceSprites.forEach(sprite => {
      this.scene.remove(sprite);
      sprite.geometry.dispose();
      sprite.material.dispose();
    });
    this.pieceSprites = [];

    const viewSize = 10;
    const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
    const tileHeight = viewSize / 9;
    const pieceSize = Math.max(tileWidth, tileHeight) * 1.2;

    for (let i = 0; i < 81; i++) {
      const pieceVal = boardData[i];
      const piece = this.gameState.decodePiece(pieceVal);

      if (!piece) continue;

      const pos = this.getTilePosition(i);

      // Load bottom piece if stacked
      if (piece.bottom) {
        const bottomTexture = await this.loadPieceSprite(piece.bottom, piece.color, flipped);
        const bottomMaterial = new THREE.SpriteMaterial({ map: bottomTexture });
        const bottomSprite = new THREE.Sprite(bottomMaterial);
        bottomSprite.scale.set(pieceSize, pieceSize, 1);
        bottomSprite.position.set(pos.x, pos.y + PIECE_OFFSET_Y, 1);
        this.scene.add(bottomSprite);
        this.pieceSprites.push(bottomSprite);
      }

      // Load top piece
      const topTexture = await this.loadPieceSprite(piece.top, piece.color, flipped);
      const topMaterial = new THREE.SpriteMaterial({ map: topTexture });
      const topSprite = new THREE.Sprite(topMaterial);
      topSprite.scale.set(pieceSize, pieceSize, 1);
      const zOffset = piece.bottom ? 2 : 1;
      topSprite.position.set(pos.x, pos.y + PIECE_OFFSET_Y, zOffset);
      this.scene.add(topSprite);
      this.pieceSprites.push(topSprite);
    }
  }

  private async loadPieceSprite(pieceName: string, color: number, reversed: boolean): Promise<THREE.Texture> {
    const colorName = COLOR_NAME[color];
    const reversedSuffix = reversed ? '-reversed' : '';
    const path = `images/${pieceName}-${colorName}${reversedSuffix}.png`;
    return await this.loadTexture(path);
  }

  private loadTexture(path: string): Promise<THREE.Texture> {
    return new Promise((resolve, reject) => {
      const loader = new THREE.TextureLoader();
      loader.load(path, resolve, undefined, reject);
    });
  }

  private renderScene(): void {
    this.renderer.render(this.scene, this.camera);
  }

  private handleClick(event: MouseEvent): void {
    if (!this.clickHandler) return;

    const rect = this.canvas.getBoundingClientRect();
    this.mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    this.mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;

    this.raycaster.setFromCamera(this.mouse, this.camera);
    const intersects = this.raycaster.intersectObjects(this.overlaySprites);

    if (intersects.length > 0) {
      const tileOverlay = intersects[0].object as any;
      const tileIndex = tileOverlay.userData.tileIndex;
      const flipped = this.gameState.isBoardFlipped();
      const pos = flipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
      this.clickHandler(pos);
    }
  }

  private handleMouseMove(event: MouseEvent): void {
    if (!this.hoverHandler) return;

    const rect = this.canvas.getBoundingClientRect();
    this.mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    this.mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;

    this.raycaster.setFromCamera(this.mouse, this.camera);
    const intersects = this.raycaster.intersectObjects(this.overlaySprites);

    if (intersects.length > 0) {
      const tileOverlay = intersects[0].object as any;
      const tileIndex = tileOverlay.userData.tileIndex;
      const flipped = this.gameState.isBoardFlipped();
      const pos = flipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
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

    this.renderer.dispose();
  }
}
