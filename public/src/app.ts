import {GameState} from './models/GameState';
import {GameAPI} from './network/GameAPI';
import ThreeJSBoardView from './views/ThreeJSBoardView';
import {GameController} from './controllers/GameController';

/**
 * Main application entry point
 */
class ArxGame {
    private gameState: GameState;
    private api!: GameAPI;
    private view!: ThreeJSBoardView;
    private controller!: GameController;

    // DOM elements
    private boardContainer: HTMLDivElement;
    private statusDiv: HTMLDivElement;
    private unstackModal: HTMLDivElement;
    private moveStackBtn: HTMLButtonElement;
    private moveUnstackBtn: HTMLButtonElement;
    private switchSidesBtn: HTMLButtonElement;
    private moveHistoryTextarea: HTMLTextAreaElement;
    private loadGameBtn: HTMLButtonElement;
    private undoBtn: HTMLButtonElement;
    private askEngineBtn: HTMLButtonElement;

    constructor() {
        this.gameState = new GameState();

        // Get DOM elements
        this.boardContainer = document.getElementById('board-container') as HTMLDivElement;
        this.statusDiv = document.getElementById('status') as HTMLDivElement;
        this.unstackModal = document.getElementById('unstack-modal') as HTMLDivElement;
        this.moveStackBtn = document.getElementById('move-stack') as HTMLButtonElement;
        this.moveUnstackBtn = document.getElementById('move-unstack') as HTMLButtonElement;
        this.switchSidesBtn = document.getElementById('switch-sides-btn') as HTMLButtonElement;
        this.moveHistoryTextarea = document.getElementById('move-history') as HTMLTextAreaElement;
        this.loadGameBtn = document.getElementById('load-game-btn') as HTMLButtonElement;
        this.undoBtn = document.getElementById('undo-btn') as HTMLButtonElement;
        this.askEngineBtn = document.getElementById('ask-engine-btn') as HTMLButtonElement;
    }

    async initialize(): Promise<void> {
        this.statusDiv.innerText = 'Loading...';

        // Load configuration
        const config = await GameAPI.loadConfig();
        this.api = new GameAPI(config);

        // Initialize view
        this.view = new ThreeJSBoardView(this.gameState);
        this.view.initialize(this.boardContainer);

        // Initialize controller
        this.controller = new GameController(this.gameState, this.api, this.view);
        await this.controller.initialize();

        // Setup UI event listeners
        this.setupEventListeners();

        // Update UI
        this.updateStatus();
        this.updateMoveHistoryDisplay();
    }

    private setupEventListeners(): void {
        // Unstack modal buttons
        this.moveStackBtn.addEventListener('click', () => this.handleMoveStack());
        this.moveUnstackBtn.addEventListener('click', () => this.handleMoveUnstack());

        // Modal background close
        const modalBackground = this.unstackModal.querySelector('.modal-background');
        modalBackground?.addEventListener('click', () => this.handleModalClose());

        // Game controls
        this.switchSidesBtn.addEventListener('click', () => this.handleSwitchSides());
        this.askEngineBtn.addEventListener('click', () => this.handleAskEngine());
        this.undoBtn.addEventListener('click', () => this.handleUndo());
        this.loadGameBtn.addEventListener('click', () => this.handleLoadGame());

        // Custom event for unstack modal
        window.addEventListener('showUnstackModal', () => {
            this.unstackModal.classList.add('is-active');
        });
    }

    private async handleMoveStack(): Promise<void> {
        this.unstackModal.classList.remove('is-active');
        const selectedMove = this.controller.getSelectedMove();
        if (selectedMove) {
            await this.controller.playMove(selectedMove.from, selectedMove.to, false);
            this.updateStatus();
            this.updateMoveHistoryDisplay();
        }
    }

    private async handleMoveUnstack(): Promise<void> {
        this.unstackModal.classList.remove('is-active');
        const selectedMove = this.controller.getSelectedMove();
        if (selectedMove) {
            await this.controller.playMove(selectedMove.from, selectedMove.to, true);
            this.updateStatus();
            this.updateMoveHistoryDisplay();
        }
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
            this.askEngineBtn.disabled = true;
            this.askEngineBtn.innerText = 'Thinking...';
            await this.controller.requestEngineMove();
            this.updateStatus();
            this.updateMoveHistoryDisplay();
        } catch (error) {
            console.error('Error getting engine move:', error);
            this.statusDiv.innerText = `Error: ${(error as Error).message}. Engine may not be available.`;
        } finally {
            this.askEngineBtn.disabled = false;
            this.askEngineBtn.innerText = 'Ask Engine';
        }
    }

    private async handleUndo(): Promise<void> {
        await this.controller.undoMove();
        this.updateStatus();
        this.updateMoveHistoryDisplay();
    }

    private async handleLoadGame(): Promise<void> {
        const text = this.moveHistoryTextarea.value.trim();
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

        try {
            await this.controller.loadGameFromMoves(moves);
            this.updateStatus();
            this.updateMoveHistoryDisplay();
        } catch (error) {
            alert((error as Error).message);
        }
    }

    private updateStatus(): void {
        const turn = this.controller.getCurrentTurn();
        this.statusDiv.innerText = `${turn}'s turn to play.`;
    }

    private updateMoveHistoryDisplay(): void {
        const history = this.controller.getMoveHistory();
        let text = '';
        for (let i = 0; i < history.length; i += 2) {
            text += history[i];
            if (i + 1 < history.length) {
                text += ' ' + history[i + 1];
            }
            text += '\n';
        }
        this.moveHistoryTextarea.value = text;
    }
}

// Initialize the game when DOM is ready
const game = new ArxGame();
game.initialize();
