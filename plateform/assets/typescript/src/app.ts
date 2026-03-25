import {GameState} from './models/GameState';
import {GameAPI} from './network/GameAPI';
import SVGBoardView from './views/SVGBoardView';
import {GameController} from './controllers/GameController';
import {IBoardView} from './views/IBoardView';
import {decodeMoveListFromBase64} from './utils/boardUtils';
import {PIECE_RULES} from './models/pieceRules';
import {Piece} from './models/types';

const OPPONENT_TYPE_AI = 0;
const OPPONENT_TYPE_HOTSEAT = 1;

/**
 * Main application entry point
 */
class KeresGame {
    private gameState: GameState;
    private api!: GameAPI;
    private view!: IBoardView;
    private controller!: GameController;

    // DOM elements
    private boardContainer: HTMLElement;
    private statusDiv: HTMLDivElement;
    private unstackModal: HTMLDivElement;
    private moveStackBtn: HTMLButtonElement;
    private moveUnstackBtn: HTMLButtonElement;
    private switchSidesBtn: HTMLButtonElement | null;
    private moveHistoryBody: HTMLTableSectionElement;
    private prevMoveBtn: HTMLButtonElement;
    private nextMoveBtn: HTMLButtonElement;
    private undoBtn: HTMLButtonElement;
    private askEngineBtn: HTMLButtonElement | null;
    private toggleThreatsBtn: HTMLButtonElement;
    private pieceDetailModal: HTMLDivElement;
    private pieceDetailName: HTMLElement;
    private pieceDetailDescription: HTMLElement;
    private pieceDetailMovement: HTMLElement;
    private rulesPanel: HTMLElement | null;
    private rulesPanelToggle: HTMLElement | null;
    private rulesPanelHovered: HTMLElement | null;
    private rulesPanelList: HTMLElement | null;
    private gameMode: number = 0; // opponent type as int
    private playerWhite: boolean = true; // true if player is white

    constructor() {
        this.gameState = new GameState();

        // Get DOM elements
        this.boardContainer = document.getElementById('board-container') as HTMLElement;
        this.statusDiv = document.getElementById('status') as HTMLDivElement;
        this.unstackModal = document.getElementById('unstack-modal') as HTMLDivElement;
        this.moveStackBtn = document.getElementById('move-stack') as HTMLButtonElement;
        this.moveUnstackBtn = document.getElementById('move-unstack') as HTMLButtonElement;
        this.switchSidesBtn = document.getElementById('switch-sides-btn') as HTMLButtonElement | null;
        this.moveHistoryBody = document.getElementById('move-history-body') as HTMLTableSectionElement;
        this.prevMoveBtn = document.getElementById('prev-move-btn') as HTMLButtonElement;
        this.nextMoveBtn = document.getElementById('next-move-btn') as HTMLButtonElement;
        this.undoBtn = document.getElementById('undo-btn') as HTMLButtonElement;
        this.askEngineBtn = document.getElementById('ask-engine-btn') as HTMLButtonElement | null;
        this.toggleThreatsBtn = document.getElementById('toggle-threats-btn') as HTMLButtonElement;
        this.pieceDetailModal = document.getElementById('piece-detail-modal') as HTMLDivElement;
        this.pieceDetailName = document.getElementById('piece-detail-name') as HTMLElement;
        this.pieceDetailDescription = document.getElementById('piece-detail-description') as HTMLElement;
        this.pieceDetailMovement = document.getElementById('piece-detail-movement') as HTMLElement;
        this.rulesPanel = document.getElementById('rules-panel');
        this.rulesPanelToggle = document.getElementById('rules-panel-toggle');
        this.rulesPanelHovered = document.getElementById('rules-panel-hovered');
        this.rulesPanelList = document.getElementById('rules-panel-list');
        
        // Read game mode and player color from data attributes
        this.gameMode = parseInt(this.boardContainer.getAttribute('data-opponent-type') || '0', 10);
        this.playerWhite = (this.boardContainer.getAttribute('data-player-white') === 'true');
    }

    async initialize(): Promise<void> {
        this.statusDiv.innerText = 'Loading...';

        // Load configuration
        this.api = new GameAPI();

        // Initialize view
        this.view = new SVGBoardView(this.gameState) as IBoardView;
        await this.view.initialize(this.boardContainer as any);

        // Initialize controller
        this.controller = new GameController(this.gameState, this.api, this.view);

        // Initialize Mercure for AI mode
        if (this.gameMode === OPPONENT_TYPE_AI) {
            const gameUuid = this.boardContainer.getAttribute('data-game-uuid');
            if (gameUuid) {
                this.controller.initializeMercure(gameUuid);
            }
        }

        // Read moves from data-moves attribute
        const movesBase64 = this.boardContainer.getAttribute('data-moves') || '';
        const moves = decodeMoveListFromBase64(movesBase64);
        await this.controller.setMoves(moves);

        // In AI mode, set board orientation based on player color
        if (this.gameMode === OPPONENT_TYPE_AI && !this.playerWhite) {
            // If player is black, flip the board so blacks are at the bottom
            await this.controller.flipBoard();
        }
        // In hotseat mode, determine orientation based on last move
        else if (this.gameMode === OPPONENT_TYPE_HOTSEAT && moves.length % 2 === 1) {
            // Odd number of moves means black just played, so show white's perspective
            await this.controller.flipBoard();
        }

        // Setup UI event listeners
        this.setupEventListeners();
        this.initializeRulesPanel();

        // Update UI
        this.updateStatus();
        this.updateMoveHistoryDisplay();
        this.updateNavigationButtons();
        this.updateToggleThreatsButton();
    }

    private setupEventListeners(): void {
        // Unstack modal buttons
        this.moveStackBtn.addEventListener('click', () => this.handleMoveStack());
        this.moveUnstackBtn.addEventListener('click', () => this.handleMoveUnstack());

        // Modal background close
        const modalBackground = this.unstackModal.querySelector('.modal-background');
        if (modalBackground) {
            modalBackground.addEventListener('click', () => this.handleModalClose());
        }

        // Game controls
        if (this.switchSidesBtn) {
            this.switchSidesBtn.addEventListener('click', () => this.handleSwitchSides());
        }
        if (this.askEngineBtn) {
            this.askEngineBtn.addEventListener('click', () => this.handleAskEngine());
        }
        this.undoBtn.addEventListener('click', () => this.handleUndo());
        this.prevMoveBtn.addEventListener('click', () => this.handlePrevMove());
        this.nextMoveBtn.addEventListener('click', () => this.handleNextMove());
        this.toggleThreatsBtn.addEventListener('click', () => this.handleToggleThreats());

        // Custom event for unstack modal
        window.addEventListener('showUnstackModal', () => {
            this.unstackModal.classList.add('is-active');
        });

        // Custom event for piece detail modal
        window.addEventListener('showPieceDetail', ((e: CustomEvent) => {
            this.showPieceDetailModal(e.detail.piece, e.detail.clientX, e.detail.clientY);
        }) as EventListener);

        // Piece detail modal dismiss
        const pieceDetailBg = this.pieceDetailModal.querySelector('.modal-background');
        if (pieceDetailBg) {
            pieceDetailBg.addEventListener('click', () => this.pieceDetailModal.classList.remove('is-active'));
        }
        const pieceDetailClose = document.getElementById('piece-detail-close');
        if (pieceDetailClose) {
            pieceDetailClose.addEventListener('click', () => this.pieceDetailModal.classList.remove('is-active'));
        }

        // Custom event for board state changes (e.g., from browser history navigation)
        window.addEventListener('boardStateChanged', () => {
            this.updateStatus();
            this.updateMoveHistoryDisplay();
            this.updateNavigationButtons();
        });

        // Rules panel toggle
        if (this.rulesPanelToggle && this.rulesPanel) {
            this.rulesPanelToggle.addEventListener('click', () => {
                this.rulesPanel!.classList.toggle('collapsed');
                this.rulesPanelToggle!.textContent = this.rulesPanel!.classList.contains('collapsed') ? '▶' : '◀';
            });
        }

        // Update rules panel on hover
        window.addEventListener('boardHoverChanged', ((e: CustomEvent) => {
            this.updateRulesPanelHovered(e.detail.pos);
        }) as EventListener);
    }

    private async handleMoveStack(fullStack: boolean = false): Promise<void> {
        this.unstackModal.classList.remove('is-active');
        const selectedPosition = this.gameState.getSelectedPosition();
        const clickedDestination = this.gameState.getClickedDestination();
        if (selectedPosition !== null && clickedDestination !== null) {
            await this.controller.playMove(selectedPosition, clickedDestination, fullStack);
            
            // Auto-rotate board in hotseat mode after each move
            if (this.gameMode === OPPONENT_TYPE_HOTSEAT) {
                await this.controller.flipBoard();
            }
            
            this.updateStatus();
            this.updateMoveHistoryDisplay();
            this.updateNavigationButtons();
        }
    }

    private async handleMoveUnstack(): Promise<void> {
        await this.handleMoveStack(true);
    }

    private handleModalClose(): void {
        this.unstackModal.classList.remove('is-active');
        this.controller.clearSelectedMove();
    }

    private async handleSwitchSides(): Promise<void> {
        await this.controller.flipBoard();
    }

    private async handleAskEngine(): Promise<void> {
        try {
            if (this.askEngineBtn) {
                this.askEngineBtn.disabled = true;
                this.askEngineBtn.innerText = 'Thinking...';
            }
            await this.controller.requestEngineMove();
            this.updateStatus();
            this.updateMoveHistoryDisplay();
        } catch (error) {
            console.error('Error getting engine move:', error);
            this.statusDiv.innerText = `Error: ${(error as Error).message}. engine may not be available.`;
        } finally {
            if (this.askEngineBtn) {
                this.askEngineBtn.disabled = false;
                this.askEngineBtn.innerText = 'Ask Engine';
            }
        }
    }

    private async handleUndo(): Promise<void> {
        await this.controller.undoMove();
        this.updateStatus();
        this.updateMoveHistoryDisplay();
        this.updateNavigationButtons();
    }

    private async handlePrevMove(): Promise<void> {
        await this.controller.previousMove();
    }

    private async handleNextMove(): Promise<void> {
        await this.controller.nextMove();
    }

    private handleToggleThreats(): void {
        this.controller.toggleShowThreats();
        this.updateToggleThreatsButton();
    }

    private updateToggleThreatsButton(): void {
        if (this.controller.isShowThreats()) {
            this.toggleThreatsBtn.innerText = 'Hide Threats';
        } else {
            this.toggleThreatsBtn.innerText = 'Show Threats';
        }
    }

    private updateStatus(): void {
        const board = this.gameState.getBoard();
        if (!board) {
            this.statusDiv.innerText = 'Loading...';
            return;
        }

        // Check if game is over
        if (board.isGameOver()) {
            this.statusDiv.innerText = board.getGameResult();
            // Disable engine buttons when game is over
            if (this.askEngineBtn) this.askEngineBtn.disabled = true;
            return;
        }

        // Check if board is locked
        if (this.controller.isBoardLocked()) {
            // In AI mode, show "Waiting for AI..." message
            if (this.gameMode === OPPONENT_TYPE_AI) {
                this.statusDiv.innerText = 'Waiting for AI...';
            } else {
                this.statusDiv.innerText = `Viewing history - Navigate to latest move to continue playing`;
            }
            if (this.askEngineBtn) this.askEngineBtn.disabled = true;
            return;
        }

        // Normal turn display
        const turn = this.controller.getCurrentTurn();
        this.statusDiv.innerText = `${turn}'s turn to play.`;

        // Re-enable engine buttons if they were disabled
        if (this.askEngineBtn) this.askEngineBtn.disabled = false;
    }

    private updateMoveHistoryDisplay(): void {
        const history = this.controller.getMoveHistory();
        
        // Clear the table body
        this.moveHistoryBody.innerHTML = '';
        
        // Build rows with white and black moves
        for (let i = 0; i < history.length; i += 2) {
            const row = document.createElement('tr');
            
            // Move number
            const numCell = document.createElement('td');
            numCell.textContent = `${Math.floor(i / 2) + 1}.`;
            row.appendChild(numCell);
            
            // White move
            const whiteCell = document.createElement('td');
            whiteCell.textContent = history[i] || '';
            row.appendChild(whiteCell);
            
            // Black move
            const blackCell = document.createElement('td');
            blackCell.textContent = history[i + 1] || '';
            row.appendChild(blackCell);
            
            this.moveHistoryBody.appendChild(row);
        }
    }

    private updateNavigationButtons(): void {
        this.prevMoveBtn.disabled = !this.controller.canNavigateToPrevious();
        this.nextMoveBtn.disabled = !this.controller.canNavigateToNext();
    }

    private showPieceDetailModal(piece: Piece, _clientX: number, _clientY: number): void {
        // Show info for the top piece (or the only piece)
        const pieceType = piece.top || piece.bottom;
        const rule = PIECE_RULES[pieceType];
        if (!rule) return;

        this.pieceDetailName.textContent = rule.name;
        this.pieceDetailDescription.textContent = rule.description;
        this.pieceDetailMovement.textContent = rule.movement;

        // If stacked, show info about both pieces
        if (piece.top) {
            const bottomRule = PIECE_RULES[piece.bottom];
            if (bottomRule) {
                this.pieceDetailDescription.textContent =
                    `${rule.name} (top): ${rule.description}\n\n${bottomRule.name} (bottom): ${bottomRule.description}`;
                this.pieceDetailMovement.textContent =
                    `${rule.name}: ${rule.movement} · ${bottomRule.name}: ${bottomRule.movement}`;
                this.pieceDetailName.textContent = `${rule.name} + ${bottomRule.name}`;
            }
        }

        this.pieceDetailModal.classList.add('is-active');
    }

    private initializeRulesPanel(): void {
        if (!this.rulesPanelList) return;
        // Populate the panel with all piece rules
        for (const [, rule] of Object.entries(PIECE_RULES)) {
            const div = document.createElement('div');
            div.className = 'rules-panel-piece';
            div.innerHTML = `<div class="rules-piece-name">${rule.name}</div><div class="rules-piece-movement">${rule.movement}</div>`;
            this.rulesPanelList.appendChild(div);
        }
    }

    private updateRulesPanelHovered(pos: number | null): void {
        if (!this.rulesPanelHovered) return;
        if (pos === null) {
            this.rulesPanelHovered.innerHTML = '<p class="rules-panel-hint">Hover a piece to see its rules</p>';
            return;
        }
        const piece = this.controller.getPieceAt(pos);
        if (!piece) {
            this.rulesPanelHovered.innerHTML = '<p class="rules-panel-hint">Hover a piece to see its rules</p>';
            return;
        }
        const pieceType = piece.top || piece.bottom;
        const rule = PIECE_RULES[pieceType];
        if (!rule) return;

        let html = `<div class="rules-panel-piece"><div class="rules-piece-name">${rule.name}</div><div class="rules-piece-movement">${rule.movement}</div><div style="font-size:0.8rem;color:#999;margin-top:0.25rem">${rule.description}</div></div>`;
        if (piece.top) {
            const bottomRule = PIECE_RULES[piece.bottom];
            if (bottomRule) {
                html += `<div class="rules-panel-piece"><div class="rules-piece-name">${bottomRule.name} (bottom)</div><div class="rules-piece-movement">${bottomRule.movement}</div></div>`;
            }
        }
        this.rulesPanelHovered.innerHTML = html;
    }
}

// Initialize the game when DOM is ready
const game = new KeresGame();
game.initialize();
