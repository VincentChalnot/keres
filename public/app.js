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

let config = null;
let boardData = null;
let possibleMoves = [];
let selectedPiece = null; // { from: int, to: int[] }
let selectedMove = null; // { from: int, to: int }
let boardFlipped = false; // Track if the board is flipped
let hoveredPiece = null; // Track currently hovered piece position
let moveHistory = []; // Array of moves in format "A1-B2"
let gameHistory = []; // Array of board states (Uint8Array)

const BOARD_SIZE = 9;
const LAST_BOARD_INDEX = (BOARD_SIZE * BOARD_SIZE) - 1;

// Board rendering constants
const BOARD_WIDTH = 800;
const BOARD_HEIGHT = 655; // 800 * (3163/3860) to maintain aspect ratio
const PIECE_OFFSET_Y = -20; // Isometric offset: pieces positioned slightly above tiles
const TILE_SIZE = 88; // Size of each tile for positioning (adjusted for board image)
const PIECE_SIZE = 80; // Display size for piece sprites
const OVERLAY_SIZE = 70; // Size of overlay indicators

// Board margins to align with the actual board image tiles
const BOARD_START_X = 8; // Left margin
const BOARD_START_Y = 80; // Top margin (adjusted to show bottom pieces)

let boardWrapper = null;
let boardOverlay = null;

const PIECE_CODE_TO_NAME = {
    0b001: 'soldier',
    0b010: 'jester',
    0b011: 'commander',
    0b100: 'paladin',
    0b101: 'guard',
    0b110: 'dragon',
    0b111: 'ballista',
};

const PIECE_CODE = {
    0b001: 'S',
    0b010: 'J',
    0b011: 'C',
    0b100: 'P',
    0b101: 'G',
    0b110: 'D',
    0b111: 'B',
};

/**
 * Get board position coordinates for a given tile index
 * @param {number} pos - Position index (0-80)
 * @returns {{x: number, y: number}} - Pixel coordinates on the board
 */
function getTilePosition(pos) {
    const col = pos % 9;
    const row = Math.floor(pos / 9);
    
    // Calculate position based on board dimensions
    const x = BOARD_START_X + col * TILE_SIZE;
    const y = BOARD_START_Y + row * TILE_SIZE;
    
    return { x, y };
}

/**
 * Get the tile index from mouse coordinates
 * @param {number} x - Mouse X coordinate relative to board
 * @param {number} y - Mouse Y coordinate relative to board
 * @returns {number|null} - Tile index (0-80) or null if outside board
 */
function getTileFromPosition(x, y) {
    const col = Math.floor((x - BOARD_START_X) / TILE_SIZE);
    const row = Math.floor((y - BOARD_START_Y) / TILE_SIZE);
    
    if (col < 0 || col >= 9 || row < 0 || row >= 9) {
        return null;
    }
    
    return row * 9 + col;
}

/**
 * Creates the board HTML structure dynamically
 */
function createBoard() {
    // Clear container
    boardContainer.innerHTML = '';
    
    // Create board wrapper
    boardWrapper = document.createElement('div');
    boardWrapper.className = 'board-wrapper';
    boardWrapper.style.width = BOARD_WIDTH + 'px';
    boardWrapper.style.height = BOARD_HEIGHT + 'px';
    
    // Create overlay layer for tile states
    boardOverlay = document.createElement('div');
    boardOverlay.className = 'board-overlay';
    boardWrapper.appendChild(boardOverlay);
    
    boardContainer.appendChild(boardWrapper);
}

function decodePiece(piece) {
    if (piece === 0) return '';
    const color = (piece >> 6) & 0b1;
    const payload = piece & 0b00111111;

    if (payload === 0b111000) {
        return { top: 'K', bottom: null, color: color, topName: 'king', bottomName: null };
    }

    const topCode = (payload >> 3) & 0b111;
    const bottomCode = payload & 0b111;

    if (topCode === 0) { // Single piece
        if (PIECE_CODE[bottomCode]) {
            return { 
                top: PIECE_CODE[bottomCode], 
                bottom: null, 
                color: color,
                topName: PIECE_CODE_TO_NAME[bottomCode],
                bottomName: null
            };
        }
    } else { // Stacked piece
        if (PIECE_CODE[topCode] && PIECE_CODE[bottomCode]) {
            return { 
                top: PIECE_CODE[topCode], 
                bottom: PIECE_CODE[bottomCode], 
                color: color,
                topName: PIECE_CODE_TO_NAME[topCode],
                bottomName: PIECE_CODE_TO_NAME[bottomCode]
            };
        }
    }
    return ''; // Invalid code
}

/**
 * Get the sprite filename for a piece
 * @param {string} pieceName - Name of the piece (e.g., 'soldier', 'king')
 * @param {number} color - Color (0 for black/red, 1 for white)
 * @param {boolean} reversed - Whether to use reversed sprite (for opponent view)
 * @returns {string} - Filename of the sprite
 */
function getSpriteFilename(pieceName, color, reversed) {
    // Color mapping: 0 (black) -> red, 1 (white) -> white
    const colorName = color === 1 ? 'white' : 'red';
    const suffix = reversed ? '-reversed' : '';
    return `images/${pieceName}-${colorName}${suffix}.png`;
}

function renderBoard() {
    const turn = boardData[81] === 1 ? "White" : "Black";
    statusDiv.innerText = `${turn}'s turn to play.`;
    
    // Clear current pieces and overlays
    const oldPieces = boardWrapper.querySelectorAll('.piece');
    oldPieces.forEach(p => p.remove());
    
    const oldOverlays = boardOverlay.querySelectorAll('.tile-overlay');
    oldOverlays.forEach(o => o.remove());
    
    // Render overlays first (under pieces)
    for (let pos = 0; pos < 81; pos++) {
        const visualPos = boardFlipped ? (LAST_BOARD_INDEX - pos) : pos;
        const tilePos = getTilePosition(visualPos);
        
        // Check if this tile needs an overlay
        let overlayClass = null;
        if (selectedPiece && selectedPiece.from === pos) {
            overlayClass = 'selected';
        } else if (selectedPiece && selectedPiece.to.includes(pos)) {
            overlayClass = 'possible-move';
        } else if (hoveredPiece !== null) {
            const hoveredMoves = getMovesForPiece(hoveredPiece);
            if (hoveredMoves.includes(pos) && (!selectedPiece || selectedPiece.from !== hoveredPiece)) {
                overlayClass = 'hovered-move';
            }
        }
        
        if (overlayClass) {
            const overlay = document.createElement('div');
            overlay.className = `tile-overlay ${overlayClass}`;
            overlay.style.left = (tilePos.x + (TILE_SIZE - OVERLAY_SIZE) / 2) + 'px';
            overlay.style.top = (tilePos.y + (TILE_SIZE - OVERLAY_SIZE) / 2) + 'px';
            overlay.style.width = OVERLAY_SIZE + 'px';
            overlay.style.height = OVERLAY_SIZE + 'px';
            boardOverlay.appendChild(overlay);
        }
    }
    
    // Render pieces
    for (let pos = 0; pos < 81; pos++) {
        const pieceVal = boardData[pos];
        const piece = decodePiece(pieceVal);
        
        if (piece && piece.topName) {
            const visualPos = boardFlipped ? (LAST_BOARD_INDEX - pos) : pos;
            const tilePos = getTilePosition(visualPos);
            
            // Determine if we need reversed sprite
            // From current player's perspective: opponent pieces are seen from behind
            const currentPlayerColor = boardData[81]; // 0 = black, 1 = white
            const isOpponentPiece = piece.color !== currentPlayerColor;
            const useReversed = isOpponentPiece !== boardFlipped; // Flip logic when board is flipped
            
            // Create piece image
            const pieceImg = document.createElement('img');
            pieceImg.className = 'piece';
            pieceImg.src = getSpriteFilename(piece.topName, piece.color, useReversed);
            pieceImg.style.left = (tilePos.x + (TILE_SIZE - PIECE_SIZE) / 2) + 'px';
            pieceImg.style.top = (tilePos.y + (TILE_SIZE - PIECE_SIZE) / 2 + PIECE_OFFSET_Y) + 'px';
            pieceImg.style.width = PIECE_SIZE + 'px';
            pieceImg.style.height = PIECE_SIZE + 'px';
            
            // Add title for debugging/accessibility
            let title = piece.top;
            if (piece.bottom) {
                title += `+${piece.bottom}`;
            }
            title += ` (${piece.color === 1 ? 'White' : 'Red'})`;
            pieceImg.title = title;
            
            boardWrapper.appendChild(pieceImg);
        }
    }
}

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

function isStacked(pos) {
    const pieceVal = boardData[pos];
    const payload = pieceVal & 0b0111111;
    const topCode = (payload >> 3) & 0b111;
    return topCode !== 0;
}

/**
 * Convert position index (0-80) to algebraic notation (A1-I9)
 */
function posToAlgebraic(pos) {
    const x = pos % 9;
    const y = Math.floor(pos / 9);
    const col = String.fromCharCode('A'.charCodeAt(0) + x);
    const row = 9 - y;
    return col + row;
}

/**
 * Convert algebraic notation (A1-I9) to position index (0-80)
 */
function algebraicToPos(algebraic) {
    if (!algebraic || algebraic.length < 2) return null;
    const col = algebraic[0].toUpperCase();
    const row = parseInt(algebraic.substring(1));
    if (col < 'A' || col > 'I' || row < 1 || row > 9) return null;
    const x = col.charCodeAt(0) - 'A'.charCodeAt(0);
    const y = 9 - row;
    return y * 9 + x;
}

/**
 * Update the move history textarea
 */
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

    // Save current board state to history before updating
    gameHistory.push(new Uint8Array(boardData));

    boardData = new Uint8Array(newBoardBuffer);

    // Update URL
    window.location.hash = btoa(String.fromCharCode.apply(null, boardData));

    // Record move in algebraic notation
    const moveNotation = posToAlgebraic(from) + '-' + posToAlgebraic(to);
    moveHistory.push(moveNotation);
    updateMoveHistoryDisplay();

    selectedPiece = null;
    selectedMove = null;
    await getPossibleMoves();
    renderBoard();
}

// Board click handler
function handleBoardClick(e) {
    const rect = boardWrapper.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    const visualPos = getTileFromPosition(x, y);
    if (visualPos === null) return;
    
    // Map visual position back to actual position based on orientation
    const pos = boardFlipped ? (LAST_BOARD_INDEX - visualPos) : visualPos;

    if (selectedPiece) {
        if (selectedPiece.to.includes(pos)) {
            // This is a move
            selectedMove = { from: selectedPiece.from, to: pos };
            const potentialMove = getPotentialMove(selectedPiece.from, pos);
            if (potentialMove && potentialMove.unstackable) {
                // Show modal only if the move is unstackable
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

// Board hover handler
function handleBoardHover(e) {
    const rect = boardWrapper.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    
    const visualPos = getTileFromPosition(x, y);
    if (visualPos === null) {
        if (hoveredPiece !== null) {
            hoveredPiece = null;
            renderBoard();
        }
        return;
    }
    
    const pos = boardFlipped ? (LAST_BOARD_INDEX - visualPos) : visualPos;
    const pieceVal = boardData[pos];
    const piece = decodePiece(pieceVal);
    
    // Only highlight if friendly piece and not currently selected
    if (piece && piece.color === boardData[81] && (!selectedPiece || selectedPiece.from !== pos)) {
        if (hoveredPiece !== pos) {
            hoveredPiece = pos;
            renderBoard();
        }
    } else {
        if (hoveredPiece !== null) {
            hoveredPiece = null;
            renderBoard();
        }
    }
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

// Close modal
document.querySelector('#unstack-modal .modal-background').addEventListener('click', () => {
    unstackModal.classList.remove('is-active');
    selectedPiece = null;
    selectedMove = null;
    renderBoard();
});

// Switch sides button handler
switchSidesBtn.addEventListener('click', () => {
    boardFlipped = !boardFlipped;
    selectedPiece = null;
    selectedMove = null;
    createBoard();
    renderBoard();
});

// Ask Engine button handler
askEngineBtn.addEventListener('click', async () => {
    try {
        // Disable button while processing
        askEngineBtn.disabled = true;
        askEngineBtn.innerText = 'Thinking...';

        // Request engine move
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

        // Decode the move
        const from = engineMove & 0x7F;
        const to = (engineMove >> 7) & 0x7F;
        const unstack = (engineMove >> 14) & 0x1;

        // Apply the move
        await playMove(from, to, unstack === 1);

    } catch (error) {
        console.error('Error getting engine move:', error);
        statusDiv.innerText = `Error: ${error.message}. Engine may not be available.`;
    } finally {
        // Re-enable button
        askEngineBtn.disabled = false;
        askEngineBtn.innerText = 'Ask Engine';
    }
});

// Undo button handler
undoBtn.addEventListener('click', async () => {
    if (gameHistory.length === 0) {
        alert('No moves to undo');
        return;
    }

    // Restore previous board state
    boardData = gameHistory.pop();

    // Remove last move from history
    moveHistory.pop();
    updateMoveHistoryDisplay();

    // Update URL
    window.location.hash = btoa(String.fromCharCode.apply(null, boardData));

    selectedPiece = null;
    selectedMove = null;
    await getPossibleMoves();
    renderBoard();
});

// Load game button handler
loadGameBtn.addEventListener('click', async () => {
    const text = moveHistoryTextarea.value.trim();
    if (!text) {
        alert('Please enter moves to load');
        return;
    }

    // Parse moves from textarea
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

    // Start a new game
    const response = await fetch(`${config.backendUrl}/new`);
    const buffer = await response.arrayBuffer();
    boardData = new Uint8Array(buffer);
    moveHistory = [];
    gameHistory = [];

    // Apply each move
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

        // Get possible moves for current board state
        await getPossibleMoves();

        // Check if this move is legal
        const moves = getMovesForPiece(fromPos);
        if (!moves.includes(toPos)) {
            alert(`Illegal move: ${moveNotation}`);
            return;
        }

        // Always move full stack when loading from history
        await playMove(fromPos, toPos, false);
    }

    renderBoard();
});

async function init() {
    // Show loading message
    statusDiv.innerText = 'Loading...';
    
    // Create the empty board structure first
    createBoard();
    
    // Add event listeners to board
    boardWrapper.addEventListener('click', handleBoardClick);
    boardWrapper.addEventListener('mousemove', handleBoardHover);

    // Then fetch config and initialize game
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
            // When loading from URL, clear history since we don't know the moves
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

    await getPossibleMoves(config);
    renderBoard();
    updateMoveHistoryDisplay();
}

init();
