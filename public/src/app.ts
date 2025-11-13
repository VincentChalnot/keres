import * as THREE from 'three';

// Type definitions
interface Config {
  backendUrl: string;
}

interface Piece {
  top: string;
  bottom: string | null;
  color: number;
}

interface SelectedPiece {
  from: number;
  to: number[];
}

interface SelectedMove {
  from: number;
  to: number;
}

interface TileOverlay extends THREE.Mesh {
  userData: {
    tileIndex: number;
  };
}

// DOM Elements with proper type assertions
const boardContainer = document.getElementById('board-container') as HTMLDivElement;
const statusDiv = document.getElementById('status') as HTMLDivElement;
const unstackModal = document.getElementById('unstack-modal') as HTMLDivElement;
const moveStackBtn = document.getElementById('move-stack') as HTMLButtonElement;
const moveUnstackBtn = document.getElementById('move-unstack') as HTMLButtonElement;
const switchSidesBtn = document.getElementById('switch-sides-btn') as HTMLButtonElement;
const moveHistoryTextarea = document.getElementById('move-history') as HTMLTextAreaElement;
const loadGameBtn = document.getElementById('load-game-btn') as HTMLButtonElement;
const undoBtn = document.getElementById('undo-btn') as HTMLButtonElement;
const askEngineBtn = document.getElementById('ask-engine-btn') as HTMLButtonElement;

// Game State
let config: Config | null = null;
let boardData: Uint8Array | null = null;
let possibleMoves: number[] = [];
let selectedPiece: SelectedPiece | null = null;
let selectedMove: SelectedMove | null = null;
let boardFlipped = false;
let hoveredPiece: number | null = null;
let moveHistory: string[] = [];
let gameHistory: Uint8Array[] = [];

// Three.js variables
let scene: THREE.Scene;
let camera: THREE.OrthographicCamera;
let renderer: THREE.WebGLRenderer;
let canvas: HTMLCanvasElement;
let boardSprite: THREE.Sprite | null = null;
let overlaySprites: TileOverlay[] = [];
let pieceSprites: THREE.Sprite[] = [];
let raycaster: THREE.Raycaster;
let mouse: THREE.Vector2;

// Constants
const BOARD_SIZE = 9;
const LAST_BOARD_INDEX = (BOARD_SIZE * BOARD_SIZE) - 1;
const BOARD_ASPECT_RATIO = 3860 / 3163;
const PIECE_OFFSET_Y = 0.08;

const PIECE_CODE: Record<number, string> = {
  0b001: 'soldier',
  0b010: 'jester',
  0b011: 'commander',
  0b100: 'paladin',
  0b101: 'guard',
  0b110: 'dragon',
  0b111: 'ballista',
};

const COLOR_NAME: Record<number, string> = {
  0: 'red',
  1: 'white'
};

function initThreeJS(): void {
  canvas = document.createElement('canvas');
  canvas.id = 'board-canvas';
  boardContainer.appendChild(canvas);
  
  scene = new THREE.Scene();
  
  const aspect = BOARD_ASPECT_RATIO;
  const viewSize = 10;
  camera = new THREE.OrthographicCamera(
    -viewSize * aspect / 2,
    viewSize * aspect / 2,
    viewSize / 2,
    -viewSize / 2,
    0.1,
    1000
  );
  camera.position.z = 10;
  
  renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
  renderer.setPixelRatio(window.devicePixelRatio);
  updateRendererSize();
  
  raycaster = new THREE.Raycaster();
  mouse = new THREE.Vector2();
  
  window.addEventListener('resize', onWindowResize);
  canvas.addEventListener('click', onCanvasClick);
  canvas.addEventListener('mousemove', onCanvasMouseMove);
  canvas.addEventListener('mouseleave', onCanvasMouseLeave);
}

function updateRendererSize(): void {
  const containerWidth = boardContainer.clientWidth;
  const height = containerWidth / BOARD_ASPECT_RATIO;
  renderer.setSize(containerWidth, height);
}

function onWindowResize(): void {
  updateRendererSize();
  
  const aspect = BOARD_ASPECT_RATIO;
  const viewSize = 10;
  camera.left = -viewSize * aspect / 2;
  camera.right = viewSize * aspect / 2;
  camera.top = viewSize / 2;
  camera.bottom = -viewSize / 2;
  camera.updateProjectionMatrix();
}

function loadTexture(path: string): Promise<THREE.Texture> {
  return new Promise((resolve, reject) => {
    const loader = new THREE.TextureLoader();
    loader.load(path, resolve, undefined, reject);
  });
}

async function createBoard(): Promise<void> {
  if (boardSprite) {
    scene.remove(boardSprite);
    boardSprite.geometry.dispose();
    boardSprite.material.dispose();
  }
  
  const texture = await loadTexture('images/board.jpg');
  texture.minFilter = THREE.LinearFilter;
  
  const material = new THREE.SpriteMaterial({ map: texture });
  boardSprite = new THREE.Sprite(material);
  
  const viewSize = 10;
  boardSprite.scale.set(viewSize * BOARD_ASPECT_RATIO, viewSize, 1);
  boardSprite.position.z = -1;
  
  scene.add(boardSprite);
}

function getTilePosition(index: number): { x: number; y: number } {
  const actualIndex = boardFlipped ? (LAST_BOARD_INDEX - index) : index;
  const col = actualIndex % 9;
  const row = Math.floor(actualIndex / 9);
  
  const viewSize = 10;
  const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
  const tileHeight = viewSize / 9;
  
  const x = -viewSize * BOARD_ASPECT_RATIO / 2 + tileWidth * (col + 0.5);
  const y = viewSize / 2 - tileHeight * (row + 0.5);
  
  return { x, y };
}

function createOverlays(): void {
  overlaySprites.forEach(sprite => {
    scene.remove(sprite);
    sprite.geometry.dispose();
    if (Array.isArray(sprite.material)) {
      sprite.material.forEach(m => m.dispose());
    } else {
      sprite.material.dispose();
    }
  });
  overlaySprites = [];
  
  const viewSize = 10;
  const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
  const tileHeight = viewSize / 9;
  
  for (let i = 0; i < 81; i++) {
    const pos = getTilePosition(i);
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
    
    scene.add(mesh);
    overlaySprites.push(mesh as unknown as TileOverlay);
  }
}

function updateOverlays(): void {
  if (!boardData) return;
  
  for (let i = 0; i < 81; i++) {
    const overlay = overlaySprites[i];
    const tileIndex = overlay.userData.tileIndex;
    const actualPos = boardFlipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
    
    let color = 0xffffff;
    let opacity = 0;
    
    if (selectedPiece && selectedPiece.from === actualPos) {
      color = 0x7fa0dd;
      opacity = 0.6;
    } else if (selectedPiece && selectedPiece.to.includes(actualPos)) {
      color = 0x55d157;
      opacity = 0.5;
    } else if (hoveredPiece !== null) {
      const hoveredMoves = getMovesForPiece(hoveredPiece);
      if (hoveredMoves.includes(actualPos) && (!selectedPiece || selectedPiece.from !== hoveredPiece)) {
        color = 0xe1ca58;
        opacity = 0.4;
      }
    }
    
    const material = overlay.material as THREE.MeshBasicMaterial;
    material.color.setHex(color);
    material.opacity = opacity;
  }
}

function decodePiece(piece: number): Piece | null {
  if (piece === 0) return null;
  const color = (piece >> 6) & 0b1;
  const payload = piece & 0b00111111;
  
  if (payload === 0b111000) {
    return { top: 'king', bottom: null, color };
  }
  
  const topCode = (payload >> 3) & 0b111;
  const bottomCode = payload & 0b111;
  
  if (topCode === 0) {
    if (PIECE_CODE[bottomCode]) {
      return { top: PIECE_CODE[bottomCode], bottom: null, color };
    }
  } else {
    if (PIECE_CODE[topCode] && PIECE_CODE[bottomCode]) {
      return { top: PIECE_CODE[topCode], bottom: PIECE_CODE[bottomCode], color };
    }
  }
  return null;
}

async function loadPieceSprite(pieceName: string, color: number, reversed = false): Promise<THREE.Texture> {
  const colorName = COLOR_NAME[color];
  const reversedSuffix = reversed ? '-reversed' : '';
  const path = `images/${pieceName}-${colorName}${reversedSuffix}.png`;
  return await loadTexture(path);
}

async function createPieceSprites(): Promise<void> {
  if (!boardData) return;
  
  pieceSprites.forEach(sprite => {
    scene.remove(sprite);
    sprite.geometry.dispose();
    sprite.material.dispose();
  });
  pieceSprites = [];
  
  const viewSize = 10;
  const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
  const tileHeight = viewSize / 9;
  const pieceSize = Math.max(tileWidth, tileHeight) * 1.2;
  
  for (let i = 0; i < 81; i++) {
    const pieceVal = boardData[i];
    const piece = decodePiece(pieceVal);
    
    if (!piece) continue;
    
    const pos = getTilePosition(i);
    const reversed = boardFlipped;
    
    if (piece.bottom) {
      const bottomTexture = await loadPieceSprite(piece.bottom, piece.color, reversed);
      const bottomMaterial = new THREE.SpriteMaterial({ map: bottomTexture });
      const bottomSprite = new THREE.Sprite(bottomMaterial);
      bottomSprite.scale.set(pieceSize, pieceSize, 1);
      bottomSprite.position.set(pos.x, pos.y + PIECE_OFFSET_Y, 1);
      scene.add(bottomSprite);
      pieceSprites.push(bottomSprite);
    }
    
    const topTexture = await loadPieceSprite(piece.top, piece.color, reversed);
    const topMaterial = new THREE.SpriteMaterial({ map: topTexture });
    const topSprite = new THREE.Sprite(topMaterial);
    topSprite.scale.set(pieceSize, pieceSize, 1);
    const zOffset = piece.bottom ? 2 : 1;
    topSprite.position.set(pos.x, pos.y + PIECE_OFFSET_Y, zOffset);
    scene.add(topSprite);
    pieceSprites.push(topSprite);
  }
}

function render(): void {
  renderer.render(scene, camera);
}

async function renderBoard(): Promise<void> {
  if (!boardData) return;
  
  const turn = boardData[81] === 1 ? "White" : "Red";
  statusDiv.innerText = `${turn}'s turn to play.`;
  
  await createPieceSprites();
  updateOverlays();
  render();
}

function onCanvasClick(event: MouseEvent): void {
  if (!boardData) return;
  
  const rect = canvas.getBoundingClientRect();
  mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
  mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
  
  raycaster.setFromCamera(mouse, camera);
  const intersects = raycaster.intersectObjects(overlaySprites);
  
  if (intersects.length > 0) {
    const tileOverlay = intersects[0].object as TileOverlay;
    const tileIndex = tileOverlay.userData.tileIndex;
    const pos = boardFlipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
    
    if (selectedPiece) {
      if (selectedPiece.to.includes(pos)) {
        selectedMove = { from: selectedPiece.from, to: pos };
        const potentialMove = getPotentialMove(selectedPiece.from, pos);
        if (potentialMove && potentialMove.unstackable) {
          unstackModal.classList.add('is-active');
        } else {
          playMove(selectedMove.from, selectedMove.to, false);
        }
      } else {
        selectedPiece = null;
        renderBoard();
      }
    } else {
      const moves = getMovesForPiece(pos);
      if (moves.length > 0) {
        selectedPiece = { from: pos, to: moves };
        renderBoard();
      }
    }
  }
}

function onCanvasMouseMove(event: MouseEvent): void {
  if (!boardData) return;
  
  const rect = canvas.getBoundingClientRect();
  mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
  mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
  
  raycaster.setFromCamera(mouse, camera);
  const intersects = raycaster.intersectObjects(overlaySprites);
  
  if (intersects.length > 0) {
    const tileOverlay = intersects[0].object as TileOverlay;
    const tileIndex = tileOverlay.userData.tileIndex;
    const pos = boardFlipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
    
    const pieceVal = boardData[pos];
    const piece = decodePiece(pieceVal);
    
    if (piece && piece.color === boardData[81] && (!selectedPiece || selectedPiece.from !== pos)) {
      if (hoveredPiece !== pos) {
        hoveredPiece = pos;
        updateOverlays();
        render();
      }
    } else {
      if (hoveredPiece !== null) {
        hoveredPiece = null;
        updateOverlays();
        render();
      }
    }
  } else {
    if (hoveredPiece !== null) {
      hoveredPiece = null;
      updateOverlays();
      render();
    }
  }
}

function onCanvasMouseLeave(): void {
  if (hoveredPiece !== null) {
    hoveredPiece = null;
    updateOverlays();
    render();
  }
}

async function getPossibleMoves(): Promise<void> {
  if (!config || !boardData) return;
  
  const response = await fetch(`${config.backendUrl}/moves`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/octet-stream' },
    body: boardData as BodyInit,
  });
  const buffer = await response.arrayBuffer();
  const moves = new Uint16Array(buffer);
  possibleMoves = Array.from(moves);
}

function getMovesForPiece(pos: number): number[] {
  const moves: number[] = [];
  for (const move of possibleMoves) {
    const from = move & 0x7F;
    const to = (move >> 7) & 0x7F;
    if (from === pos) {
      moves.push(to);
    }
  }
  return moves;
}

function getPotentialMove(fromPos: number, toPos: number) {
  for (const move of possibleMoves) {
    const from = move & 0x7F;
    const to = (move >> 7) & 0x7F;
    if (from === fromPos && to === toPos) {
      const unstackable = (move >> 14) & 0x1;
      const force_unstack = (move >> 15) & 0x1;
      return { from, to, unstackable: unstackable === 1, force_unstack: force_unstack === 1 };
    }
  }
  return null;
}

function posToAlgebraic(pos: number): string {
  const x = pos % 9;
  const y = Math.floor(pos / 9);
  const col = String.fromCharCode('A'.charCodeAt(0) + x);
  const row = 9 - y;
  return col + row;
}

function algebraicToPos(algebraic: string): number | null {
  if (!algebraic || algebraic.length < 2) return null;
  const col = algebraic[0].toUpperCase();
  const row = parseInt(algebraic.substring(1));
  if (col < 'A' || col > 'I' || row < 1 || row > 9) return null;
  const x = col.charCodeAt(0) - 'A'.charCodeAt(0);
  const y = 9 - row;
  return y * 9 + x;
}

function updateMoveHistoryDisplay(): void {
  let text = '';
  for (let i = 0; i < moveHistory.length; i += 2) {
    text += moveHistory[i];
    if (i + 1 < moveHistory.length) {
      text += ' ' + moveHistory[i + 1];
    }
    text += '\n';
  }
  moveHistoryTextarea.value = text;
}

async function playMove(from: number, to: number, unstack = false): Promise<void> {
  if (!config || !boardData) return;
  
  let moveBits = (from & 0x7F) | ((to & 0x7F) << 7);
  if (unstack) {
    moveBits |= (1 << 14);
  }
  const moveBuffer = new Uint16Array([moveBits]).buffer;
  const payload = new Uint8Array(boardData.length + 2);
  payload.set(new Uint8Array(boardData), 0);
  payload.set(new Uint8Array(moveBuffer), boardData.length);
  
  const response = await fetch(`${config.backendUrl}/play`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/octet-stream' },
    body: payload as BodyInit,
  });
  
  const newBoardBuffer = await response.arrayBuffer();
  gameHistory.push(new Uint8Array(boardData));
  boardData = new Uint8Array(newBoardBuffer);
  window.location.hash = btoa(String.fromCharCode.apply(null, Array.from(boardData)));
  
  const moveNotation = posToAlgebraic(from) + '-' + posToAlgebraic(to);
  moveHistory.push(moveNotation);
  updateMoveHistoryDisplay();
  
  selectedPiece = null;
  selectedMove = null;
  await getPossibleMoves();
  await renderBoard();
}

moveStackBtn.addEventListener('click', () => {
  unstackModal.classList.remove('is-active');
  if (selectedMove) {
    playMove(selectedMove.from, selectedMove.to, false);
  }
});

moveUnstackBtn.addEventListener('click', () => {
  unstackModal.classList.remove('is-active');
  if (selectedMove) {
    playMove(selectedMove.from, selectedMove.to, true);
  }
});

document.querySelector('#unstack-modal .modal-background')?.addEventListener('click', () => {
  unstackModal.classList.remove('is-active');
  selectedPiece = null;
  selectedMove = null;
  renderBoard();
});

switchSidesBtn.addEventListener('click', () => {
  boardFlipped = !boardFlipped;
  selectedPiece = null;
  selectedMove = null;
  renderBoard();
});

askEngineBtn.addEventListener('click', async () => {
  if (!config || !boardData) return;
  
  try {
    askEngineBtn.disabled = true;
    askEngineBtn.innerText = 'Thinking...';
    
    const response = await fetch(`${config.backendUrl}/engine-move`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body: boardData as BodyInit,
    });
    
    if (!response.ok) {
      throw new Error(`Server returned ${response.status}`);
    }
    
    const moveBuffer = await response.arrayBuffer();
    const moveArray = new Uint16Array(moveBuffer);
    const engineMove = moveArray[0];
    
    const from = engineMove & 0x7F;
    const to = (engineMove >> 7) & 0x7F;
    const unstack = (engineMove >> 14) & 0x1;
    
    await playMove(from, to, unstack === 1);
  } catch (error) {
    console.error('Error getting engine move:', error);
    statusDiv.innerText = `Error: ${(error as Error).message}. Engine may not be available.`;
  } finally {
    askEngineBtn.disabled = false;
    askEngineBtn.innerText = 'Ask Engine';
  }
});

undoBtn.addEventListener('click', async () => {
  if (gameHistory.length === 0) {
    alert('No moves to undo');
    return;
  }
  
  boardData = gameHistory.pop()!;
  moveHistory.pop();
  updateMoveHistoryDisplay();
  window.location.hash = btoa(String.fromCharCode.apply(null, Array.from(boardData)));
  
  selectedPiece = null;
  selectedMove = null;
  await getPossibleMoves();
  await renderBoard();
});

loadGameBtn.addEventListener('click', async () => {
  if (!config) return;
  
  const text = moveHistoryTextarea.value.trim();
  if (!text) {
    alert('Please enter moves to load');
    return;
  }
  
  const lines = text.split('\n');
  const moves: string[] = [];
  for (const line of lines) {
    const parts = line.trim().split(/\s+/);
    for (const part of parts) {
      if (part.includes('-')) {
        moves.push(part);
      }
    }
  }
  
  if (moves.length === 0) {
    alert('No valid moves found');
    return;
  }
  
  if (!config) return; // Extra check
  const response = await fetch(`${config.backendUrl}/new`);
  const buffer = await response.arrayBuffer();
  boardData = new Uint8Array(buffer);
  moveHistory = [];
  gameHistory = [];
  
  for (const moveNotation of moves) {
    const parts = moveNotation.split('-');
    if (parts.length !== 2) {
      alert(`Invalid move format: ${moveNotation}`);
      return;
    }
    
    const fromPos = algebraicToPos(parts[0]);
    const toPos = algebraicToPos(parts[1]);
    
    if (fromPos === null || toPos === null) {
      alert(`Invalid position in move: ${moveNotation}`);
      return;
    }
    
    await getPossibleMoves();
    const moves = getMovesForPiece(fromPos);
    if (!moves.includes(toPos)) {
      alert(`Illegal move: ${moveNotation}`);
      return;
    }
    
    await playMove(fromPos, toPos, false);
  }
  
  await renderBoard();
});

async function init(): Promise<void> {
  statusDiv.innerText = 'Loading...';
  
  initThreeJS();
  
  const response = await fetch(`/config.json`);
  config = await response.json();
  
  if (!config) return; // Safety check
  
  if (window.location.hash) {
    try {
      const base64Board = window.location.hash.substring(1);
      const binaryString = atob(base64Board);
      const len = binaryString.length;
      const bytes = new Uint8Array(len);
      for (let i = 0; i < len; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }
      boardData = bytes;
      moveHistory = [];
      gameHistory = [];
    } catch (e) {
      console.error("Failed to load board from URL, starting new game.", e);
      if (!config) return;
      const response = await fetch(`${config.backendUrl}/new`);
      const buffer = await response.arrayBuffer();
      boardData = new Uint8Array(buffer);
      moveHistory = [];
      gameHistory = [];
    }
  } else {
    if (!config) return;
    const response = await fetch(`${config.backendUrl}/new`);
    const buffer = await response.arrayBuffer();
    boardData = new Uint8Array(buffer);
    moveHistory = [];
    gameHistory = [];
  }
  
  await createBoard();
  createOverlays();
  await getPossibleMoves();
  await renderBoard();
  updateMoveHistoryDisplay();
}

init();
