import * as THREE from 'three';

// DOM Elements
const boardContainer = document.getElementById('board-container');
const statusDiv = document.getElementById('status');
const unstackModal = document.getElementById('unstack-modal');
const moveStackBtn = document.getElementById('move-stack');
const moveUnstackBtn = document.getElementById('move-unstack');
const switchSidesBtn = document.getElementById('switch-sides-btn');
const moveHistoryTextarea = document.getElementById('move-history');
const loadGameBtn = document.getElementById('load-game-btn');
const undoBtn = document.getElementById('undo-btn');
const askEngineBtn = document.getElementById('ask-engine-btn');

// Game State
let config = null;
let boardData = null;
let possibleMoves = [];
let selectedPiece = null; // { from: int, to: int[] }
let selectedMove = null; // { from: int, to: int }
let boardFlipped = false;
let hoveredPiece = null;
let moveHistory = [];
let gameHistory = [];

// Three.js variables
let scene, camera, renderer, canvas;
let boardSprite, overlaySprites = [], pieceSprites = [];
let raycaster, mouse;

// Constants
const BOARD_SIZE = 9;
const LAST_BOARD_INDEX = (BOARD_SIZE * BOARD_SIZE) - 1;
const BOARD_ASPECT_RATIO = 3860 / 3163; // board.jpg dimensions
const PIECE_OFFSET_Y = 0.08; // Vertical offset for pieces above tiles

const PIECE_CODE = {
    0b001: 'soldier',
    0b010: 'jester',
    0b011: 'commander',
    0b100: 'paladin',
    0b101: 'guard',
    0b110: 'dragon',
    0b111: 'ballista',
};

// Color mapping: 0=black(red), 1=white
const COLOR_NAME = {
    0: 'red',
    1: 'white'
};

// Initialize Three.js scene
function initThreeJS() {
    // Create canvas
    canvas = document.createElement('canvas');
    canvas.id = 'board-canvas';
    boardContainer.appendChild(canvas);
    
    // Create scene
    scene = new THREE.Scene();
    
    // Create orthographic camera
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
    
    // Create renderer
    renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    updateRendererSize();
    
    // Raycaster for mouse picking
    raycaster = new THREE.Raycaster();
    mouse = new THREE.Vector2();
    
    // Handle window resize
    window.addEventListener('resize', onWindowResize);
    
    // Handle mouse events
    canvas.addEventListener('click', onCanvasClick);
    canvas.addEventListener('mousemove', onCanvasMouseMove);
    canvas.addEventListener('mouseleave', onCanvasMouseLeave);
}

function updateRendererSize() {
    const containerWidth = boardContainer.clientWidth;
    const height = containerWidth / BOARD_ASPECT_RATIO;
    renderer.setSize(containerWidth, height);
}

function onWindowResize() {
    updateRendererSize();
    
    const containerWidth = boardContainer.clientWidth;
    const height = containerWidth / BOARD_ASPECT_RATIO;
    
    const aspect = BOARD_ASPECT_RATIO;
    const viewSize = 10;
    camera.left = -viewSize * aspect / 2;
    camera.right = viewSize * aspect / 2;
    camera.top = viewSize / 2;
    camera.bottom = -viewSize / 2;
    camera.updateProjectionMatrix();
}

// Load texture helper
function loadTexture(path) {
    return new Promise((resolve, reject) => {
        const loader = new THREE.TextureLoader();
        loader.load(
            path,
            texture => resolve(texture),
            undefined,
            error => reject(error)
        );
    });
}

// Create board sprite
async function createBoard() {
    // Remove existing board if any
    if (boardSprite) {
        scene.remove(boardSprite);
        boardSprite.geometry.dispose();
        boardSprite.material.dispose();
    }
    
    // Load board texture
    const texture = await loadTexture('images/board.jpg');
    texture.minFilter = THREE.LinearFilter;
    
    // Create sprite material
    const material = new THREE.SpriteMaterial({ map: texture });
    boardSprite = new THREE.Sprite(material);
    
    // Scale to fit the view
    const viewSize = 10;
    boardSprite.scale.set(viewSize * BOARD_ASPECT_RATIO, viewSize, 1);
    boardSprite.position.z = -1; // Behind overlays and pieces
    
    scene.add(boardSprite);
}

// Get board position for a tile index (0-80)
function getTilePosition(index) {
    // Flip index if board is flipped
    const actualIndex = boardFlipped ? (LAST_BOARD_INDEX - index) : index;
    
    const col = actualIndex % 9;
    const row = Math.floor(actualIndex / 9);
    
    // Map tile to board coordinates
    // Board goes from -BOARD_ASPECT_RATIO*5 to BOARD_ASPECT_RATIO*5 horizontally
    // and from -5 to 5 vertically
    const viewSize = 10;
    const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
    const tileHeight = viewSize / 9;
    
    const x = -viewSize * BOARD_ASPECT_RATIO / 2 + tileWidth * (col + 0.5);
    const y = viewSize / 2 - tileHeight * (row + 0.5);
    
    return { x, y };
}

// Create overlay sprites for tile states
function createOverlays() {
    // Clear existing overlays
    overlaySprites.forEach(sprite => {
        scene.remove(sprite);
        sprite.geometry.dispose();
        sprite.material.dispose();
    });
    overlaySprites = [];
    
    const viewSize = 10;
    const tileWidth = (viewSize * BOARD_ASPECT_RATIO) / 9;
    const tileHeight = viewSize / 9;
    
    // Create overlays for each tile
    for (let i = 0; i < 81; i++) {
        const pos = getTilePosition(i);
        
        // Create a colored plane for overlay
        const geometry = new THREE.PlaneGeometry(tileWidth * 0.9, tileHeight * 0.9);
        const material = new THREE.MeshBasicMaterial({
            color: 0xffffff,
            transparent: true,
            opacity: 0,
            side: THREE.DoubleSide
        });
        const mesh = new THREE.Mesh(geometry, material);
        mesh.position.set(pos.x, pos.y, 0);
        mesh.userData = { tileIndex: i };
        
        scene.add(mesh);
        overlaySprites.push(mesh);
    }
}

// Update overlay colors based on game state
function updateOverlays() {
    for (let i = 0; i < 81; i++) {
        const overlay = overlaySprites[i];
        const tileIndex = overlay.userData.tileIndex;
        
        // Map visual index to actual board position
        const actualPos = boardFlipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
        
        let color = 0xffffff;
        let opacity = 0;
        
        // Selected piece
        if (selectedPiece && selectedPiece.from === actualPos) {
            color = 0x7fa0dd; // Cornflower blue
            opacity = 0.6;
        }
        // Possible move
        else if (selectedPiece && selectedPiece.to.includes(actualPos)) {
            color = 0x55d157; // Light green
            opacity = 0.5;
        }
        // Hovered moves
        else if (hoveredPiece !== null) {
            const hoveredMoves = getMovesForPiece(hoveredPiece);
            if (hoveredMoves.includes(actualPos) && (!selectedPiece || selectedPiece.from !== hoveredPiece)) {
                color = 0xe1ca58; // Soft yellow
                opacity = 0.4;
            }
        }
        
        overlay.material.color.setHex(color);
        overlay.material.opacity = opacity;
    }
}

// Decode piece helper
function decodePiece(piece) {
    if (piece === 0) return null;
    const color = (piece >> 6) & 0b1;
    const payload = piece & 0b00111111;

    if (payload === 0b111000) {
        return { top: 'king', bottom: null, color: color };
    }

    const topCode = (payload >> 3) & 0b111;
    const bottomCode = payload & 0b111;

    if (topCode === 0) { // Single piece
        if (PIECE_CODE[bottomCode]) {
            return { top: PIECE_CODE[bottomCode], bottom: null, color: color };
        }
    } else { // Stacked piece
        if (PIECE_CODE[topCode] && PIECE_CODE[bottomCode]) {
            return { top: PIECE_CODE[topCode], bottom: PIECE_CODE[bottomCode], color: color };
        }
    }
    return null;
}

// Load piece sprite
async function loadPieceSprite(pieceName, color, reversed = false) {
    const colorName = COLOR_NAME[color];
    const reversedSuffix = reversed ? '-reversed' : '';
    const path = `images/${pieceName}-${colorName}${reversedSuffix}.png`;
    return await loadTexture(path);
}

// Create piece sprites
async function createPieceSprites() {
    // Clear existing pieces
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
    
    // Load all piece sprites
    for (let i = 0; i < 81; i++) {
        const pieceVal = boardData[i];
        const piece = decodePiece(pieceVal);
        
        if (!piece) continue;
        
        const pos = getTilePosition(i);
        
        // Determine if piece should be reversed (opponent view)
        const reversed = boardFlipped;
        
        // Load bottom piece if stacked
        if (piece.bottom) {
            const bottomTexture = await loadPieceSprite(piece.bottom, piece.color, reversed);
            const bottomMaterial = new THREE.SpriteMaterial({ map: bottomTexture });
            const bottomSprite = new THREE.Sprite(bottomMaterial);
            bottomSprite.scale.set(pieceSize, pieceSize, 1);
            bottomSprite.position.set(pos.x, pos.y + PIECE_OFFSET_Y, 1);
            scene.add(bottomSprite);
            pieceSprites.push(bottomSprite);
        }
        
        // Load top piece
        const topTexture = await loadPieceSprite(piece.top, piece.color, reversed);
        const topMaterial = new THREE.SpriteMaterial({ map: topTexture });
        const topSprite = new THREE.Sprite(topMaterial);
        topSprite.scale.set(pieceSize, pieceSize, 1);
        const zOffset = piece.bottom ? 2 : 1; // Stack pieces slightly higher
        topSprite.position.set(pos.x, pos.y + PIECE_OFFSET_Y, zOffset);
        scene.add(topSprite);
        pieceSprites.push(topSprite);
    }
}

// Render the scene
function render() {
    renderer.render(scene, camera);
}

// Update the board
async function renderBoard() {
    const turn = boardData[81] === 1 ? "White" : "Red";
    statusDiv.innerText = `${turn}'s turn to play.`;
    
    await createPieceSprites();
    updateOverlays();
    render();
}

// Mouse event handlers
function onCanvasClick(event) {
    const rect = canvas.getBoundingClientRect();
    mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
    
    raycaster.setFromCamera(mouse, camera);
    const intersects = raycaster.intersectObjects(overlaySprites);
    
    if (intersects.length > 0) {
        const tileIndex = intersects[0].object.userData.tileIndex;
        const pos = boardFlipped ? (LAST_BOARD_INDEX - tileIndex) : tileIndex;
        
        if (selectedPiece) {
            if (selectedPiece.to.includes(pos)) {
                // This is a move
                selectedMove = { from: selectedPiece.from, to: pos };
                const potentialMove = getPotentialMove(selectedPiece.from, pos);
                if (potentialMove && potentialMove.unstackable) {
                    unstackModal.classList.add('is-active');
                } else {
                    playMove(selectedMove.from, selectedMove.to, false);
                }
            } else {
                // Clicked somewhere else, deselect
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

function onCanvasMouseMove(event) {
    const rect = canvas.getBoundingClientRect();
    mouse.x = ((event.clientX - rect.left) / rect.width) * 2 - 1;
    mouse.y = -((event.clientY - rect.top) / rect.height) * 2 + 1;
    
    raycaster.setFromCamera(mouse, camera);
    const intersects = raycaster.intersectObjects(overlaySprites);
    
    if (intersects.length > 0) {
        const tileIndex = intersects[0].object.userData.tileIndex;
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

function onCanvasMouseLeave() {
    if (hoveredPiece !== null) {
        hoveredPiece = null;
        updateOverlays();
        render();
    }
}

// Game logic functions (same as before)
async function getPossibleMoves() {
    const response = await fetch(`${config.backendUrl}/moves`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/octet-stream' },
        body: boardData,
    });
    const buffer = await response.arrayBuffer();
    const moves = new Uint16Array(buffer);
    possibleMoves = Array.from(moves);
}

function getMovesForPiece(pos) {
    const moves = [];
    for (const move of possibleMoves) {
        const from = move & 0x7F;
        const to = (move >> 7) & 0x7F;
        if (from === pos) {
            moves.push(to);
        }
    }
    return moves;
}

function getPotentialMove(fromPos, toPos) {
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

function posToAlgebraic(pos) {
    const x = pos % 9;
    const y = Math.floor(pos / 9);
    const col = String.fromCharCode('A'.charCodeAt(0) + x);
    const row = 9 - y;
    return col + row;
}

function algebraicToPos(algebraic) {
    if (!algebraic || algebraic.length < 2) return null;
    const col = algebraic[0].toUpperCase();
    const row = parseInt(algebraic.substring(1));
    if (col < 'A' || col > 'I' || row < 1 || row > 9) return null;
    const x = col.charCodeAt(0) - 'A'.charCodeAt(0);
    const y = 9 - row;
    return y * 9 + x;
}

function updateMoveHistoryDisplay() {
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

async function playMove(from, to, unstack = false) {
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
        body: payload,
    });

    const newBoardBuffer = await response.arrayBuffer();
    gameHistory.push(new Uint8Array(boardData));
    boardData = new Uint8Array(newBoardBuffer);
    window.location.hash = btoa(String.fromCharCode.apply(null, boardData));

    const moveNotation = posToAlgebraic(from) + '-' + posToAlgebraic(to);
    moveHistory.push(moveNotation);
    updateMoveHistoryDisplay();

    selectedPiece = null;
    selectedMove = null;
    await getPossibleMoves();
    await renderBoard();
}

// Button event handlers
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

document.querySelector('#unstack-modal .modal-background').addEventListener('click', () => {
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
    try {
        askEngineBtn.disabled = true;
        askEngineBtn.innerText = 'Thinking...';

        const response = await fetch(`${config.backendUrl}/engine-move`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/octet-stream' },
            body: boardData,
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
        statusDiv.innerText = `Error: ${error.message}. Engine may not be available.`;
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

    boardData = gameHistory.pop();
    moveHistory.pop();
    updateMoveHistoryDisplay();
    window.location.hash = btoa(String.fromCharCode.apply(null, boardData));

    selectedPiece = null;
    selectedMove = null;
    await getPossibleMoves();
    await renderBoard();
});

loadGameBtn.addEventListener('click', async () => {
    const text = moveHistoryTextarea.value.trim();
    if (!text) {
        alert('Please enter moves to load');
        return;
    }

    const lines = text.split('\n');
    const moves = [];
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

// Initialize
async function init() {
    statusDiv.innerText = 'Loading...';
    
    initThreeJS();
    
    const response = await fetch(`/config.json`);
    config = await response.json();

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
            const response = await fetch(`${config.backendUrl}/new`);
            const buffer = await response.arrayBuffer();
            boardData = new Uint8Array(buffer);
            moveHistory = [];
            gameHistory = [];
        }
    } else {
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
