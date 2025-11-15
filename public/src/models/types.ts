// Core game types

export interface Config {
  backendUrl: string;
}

export interface Piece {
  color: number;
  bottom: string;
  top: string | null;
}

export interface SelectedPiece {
  from: number;
  to: number[];
}

export interface SelectedMove {
  from: number;
  to: number;
}

export interface PotentialMove {
  from: number;
  to: number;
  unstackable: boolean;
  force_unstack: boolean;
}

export interface TileState {
  position: number;
  highlighted: boolean;
  highlightColor?: 'selected' | 'possible' | 'hovered';
}

// Constants
export const BOARD_SIZE = 9;
export const LAST_BOARD_INDEX = (BOARD_SIZE * BOARD_SIZE) - 1;
export const BOARD_ASPECT_RATIO = 3860 / 3163;
export const PIECE_OFFSET_Y = 0.08;
export const PIECE_TOP_OFFSET_FACTOR = 0.2;

export const PIECE_CODE: Record<number, string> = {
  0b001: 'soldier',
  0b010: 'jester',
  0b011: 'commander',
  0b100: 'paladin',
  0b101: 'guard',
  0b110: 'dragon',
  0b111: 'ballista',
};

export const COLOR_NAME: Record<number, string> = {
  0: 'red',
  1: 'white'
};
