import { Config } from '../models/types';

/**
 * API client for game backend
 */
export class GameAPI {
  constructor(private config: Config) {}

  /**
   * Get a new game board
   */
  async getNewGame(): Promise<Uint8Array> {
    const response = await fetch(`${this.config.backendUrl}/new`);
    const buffer = await response.arrayBuffer();
    return new Uint8Array(buffer);
  }

  /**
   * Get possible moves for current board state
   */
  async getPossibleMoves(boardData: Uint8Array): Promise<number[]> {
    const response = await fetch(`${this.config.backendUrl}/moves`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body: boardData as BodyInit,
    });
    const buffer = await response.arrayBuffer();
    const moves = new Uint16Array(buffer);
    return Array.from(moves);
  }

  /**
   * Play a move and get the new board state
   */
  async playMove(boardData: Uint8Array, from: number, to: number, unstack = false): Promise<Uint8Array> {
    let moveBits = (from & 0x7F) | ((to & 0x7F) << 7);
    if (unstack) {
      moveBits |= (1 << 14);
    }
    const moveBuffer = new Uint16Array([moveBits]).buffer;
    const payload = new Uint8Array(boardData.length + 2);
    payload.set(new Uint8Array(boardData), 0);
    payload.set(new Uint8Array(moveBuffer), boardData.length);

    const response = await fetch(`${this.config.backendUrl}/play`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/octet-stream' },
      body: payload as BodyInit,
    });

    const newBoardBuffer = await response.arrayBuffer();
    return new Uint8Array(newBoardBuffer);
  }

  /**
   * Get engine move for current board state
   */
  async getEngineMove(boardData: Uint8Array): Promise<{ from: number; to: number; unstack: boolean }> {
    const response = await fetch(`${this.config.backendUrl}/engine-move`, {
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

    return { from, to, unstack: unstack === 1 };
  }

  /**
   * Load configuration
   */
  static async loadConfig(): Promise<Config> {
    const response = await fetch('/config.json');
    return await response.json();
  }
}
