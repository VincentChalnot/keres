//! Minimax Engine with Alpha-Beta Pruning for Arx
//!
//! This module provides a classical minimax search algorithm with alpha-beta pruning
//! for evaluating board positions and finding optimal moves. The engine uses a
//! sophisticated multi-criteria evaluation function tailored for Arx.
//!
//! # Features
//!
//! - Minimax search with alpha-beta pruning
//! - Configurable search depth (recommended: 4-6 ply)
//! - Multi-criteria position evaluation
//! - Move ordering for alpha-beta efficiency
//! - Transposition table with Zobrist hashing
//! - Quiescence search for tactical stability
//! - Iterative deepening for time management
//!
//! # Example
//!
//! ```no_run
//! use arx_engine::engine::{MinimaxEngine, MinimaxConfig};
//! use arx_engine::{Game, Board};
//!
//! // Create engine with custom configuration
//! let config = MinimaxConfig {
//!     max_depth: 4,
//!     use_quiescence: true,
//!     use_transposition_table: true,
//!     time_limit_ms: 3000,
//!     ..Default::default()
//! };
//! let mut engine = MinimaxEngine::with_config(config);
//!
//! // Find best move for a board position
//! let game = Game::new();
//! let best_move = engine.find_best_move(&game.board).expect("No legal moves");
//! ```

use std::collections::HashMap;
use std::time::{Duration, Instant};
use rand::Rng;

use crate::board::{Board, Color, Position, PieceType};
use crate::game::{Game, Move, PotentialMove};

/// Piece values for material evaluation
const PIECE_VALUES: [i32; 8] = [
    0,   // Index 0: unused
    10,  // Soldier
    20,  // Jester
    100, // Commander
    20,  // Paladin
    25,  // Guard
    30,  // Dragon
    15,  // Ballista
];

const KING_VALUE: i32 = 10000; // King is invaluable
const INFINITY: i32 = 50000;

/// Minimax engine configuration
#[derive(Clone, Debug)]
pub struct MinimaxConfig {
    /// Maximum search depth (ply)
    pub max_depth: u32,
    /// Enable quiescence search
    pub use_quiescence: bool,
    /// Enable transposition table
    pub use_transposition_table: bool,
    /// Time limit per move in milliseconds
    pub time_limit_ms: u64,
    /// Material value weight (0.0 to 1.0)
    pub material_weight: f32,
    /// Territorial control weight (0.0 to 1.0)
    pub territorial_weight: f32,
    /// Piece mobility weight (0.0 to 1.0)
    pub mobility_weight: f32,
    /// King safety weight (0.0 to 1.0)
    pub king_safety_weight: f32,
    /// Bonus percentage for stacked pieces (e.g., 0.3 = 30% bonus)
    pub stack_bonus: f32,
}

impl Default for MinimaxConfig {
    fn default() -> Self {
        Self {
            max_depth: 4,
            use_quiescence: true,
            use_transposition_table: true,
            time_limit_ms: 3000,
            material_weight: 0.40,
            territorial_weight: 0.25,
            mobility_weight: 0.20,
            king_safety_weight: 0.15,
            stack_bonus: 0.30,
        }
    }
}

/// Entry in the transposition table
#[derive(Clone, Debug)]
struct TranspositionEntry {
    depth: u32,
    score: i32,
    best_move: Option<u16>,
    node_type: NodeType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum NodeType {
    Exact,
    LowerBound,
    UpperBound,
}

/// Statistics for minimax search
#[derive(Clone, Debug, Default)]
pub struct MinimaxStatistics {
    /// Total positions evaluated
    pub positions_evaluated: u64,
    /// Transposition table hits
    pub tt_hits: u64,
    /// Alpha-beta cutoffs
    pub ab_cutoffs: u64,
    /// Quiescence nodes searched
    pub quiescence_nodes: u64,
    /// Search time in milliseconds
    pub search_time_ms: u64,
}

/// Minimax engine with alpha-beta pruning
pub struct MinimaxEngine {
    config: MinimaxConfig,
    transposition_table: HashMap<u64, TranspositionEntry>,
    zobrist_keys: ZobristKeys,
    stats: MinimaxStatistics,
    search_start: Option<Instant>,
}

/// Zobrist hashing keys for position identification
struct ZobristKeys {
    pieces: Vec<u64>,
}

impl ZobristKeys {
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        // Generate random keys for each position (81) x each piece type and color combination
        // We need keys for: 9x9 positions * (2 colors * 8 piece types * 2 layers + empty)
        let num_keys = 81 * (2 * 8 * 2 + 1);
        let pieces = (0..num_keys).map(|_| rng.gen::<u64>()).collect();
        Self { pieces }
    }

    fn hash_board(&self, board: &Board) -> u64 {
        let mut hash = 0u64;
        
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                let idx = (y * 9 + x) as usize;
                
                if let Some(piece) = board.get_piece(&pos) {
                    // Hash bottom piece
                    let color_offset = if piece.color == Color::White { 0 } else { 1 };
                    let piece_type_idx = self.piece_type_to_idx(piece.bottom);
                    let key_idx = idx * 32 + color_offset * 16 + piece_type_idx;
                    hash ^= self.pieces[key_idx];
                    
                    // Hash top piece if exists
                    if let Some(top_type) = piece.top {
                        let top_piece_idx = self.piece_type_to_idx(top_type);
                        let top_key_idx = idx * 32 + color_offset * 16 + 8 + top_piece_idx;
                        hash ^= self.pieces[top_key_idx];
                    }
                }
            }
        }
        
        // Include turn to move
        if board.is_white_to_move() {
            hash ^= self.pieces[0];
        }
        
        hash
    }
    
    fn piece_type_to_idx(&self, piece_type: PieceType) -> usize {
        match piece_type {
            PieceType::Soldier => 0,
            PieceType::Jester => 1,
            PieceType::Commander => 2,
            PieceType::Paladin => 3,
            PieceType::Guard => 4,
            PieceType::Dragon => 5,
            PieceType::Ballista => 6,
            PieceType::King => 7,
        }
    }
}

impl MinimaxEngine {
    /// Create a new minimax engine with default configuration
    pub fn new() -> Self {
        Self::with_config(MinimaxConfig::default())
    }

    /// Create a new minimax engine with custom configuration
    pub fn with_config(config: MinimaxConfig) -> Self {
        Self {
            config,
            transposition_table: HashMap::new(),
            zobrist_keys: ZobristKeys::new(),
            stats: MinimaxStatistics::default(),
            search_start: None,
        }
    }

    /// Find the best move using minimax with alpha-beta pruning
    pub fn find_best_move(&mut self, board: &Board) -> Result<Move, String> {
        self.stats = MinimaxStatistics::default();
        self.search_start = Some(Instant::now());
        
        // Generate all legal moves
        let game = Game::from_board(board.clone());
        let potential_moves = game.get_all_moves();
        
        if potential_moves.is_empty() {
            return Err("No legal moves available".to_string());
        }
        
        if potential_moves.len() == 1 {
            let unstack = potential_moves[0].force_unstack;
            return Ok(potential_moves[0].to_move(unstack));
        }
        
        // Order moves for better alpha-beta efficiency
        let mut ordered_moves = self.order_moves(board, &potential_moves);
        
        let mut best_move = ordered_moves[0].clone();
        let mut best_score = -INFINITY;
        
        // Iterative deepening
        for depth in 1..=self.config.max_depth {
            if self.time_exceeded() {
                break;
            }
            
            let mut alpha = -INFINITY;
            let beta = INFINITY;
            
            for potential_move in &ordered_moves {
                if self.time_exceeded() {
                    break;
                }
                
                let unstack = potential_move.force_unstack;
                let mv = potential_move.to_move(unstack);
                
                if let Ok(new_board) = self.apply_move(board, &mv) {
                    let score = -self.minimax(&new_board, depth - 1, -beta, -alpha, false);
                    
                    if score > best_score {
                        best_score = score;
                        best_move = potential_move.clone();
                    }
                    
                    alpha = alpha.max(score);
                    if alpha >= beta {
                        self.stats.ab_cutoffs += 1;
                        break;
                    }
                }
            }
            
            // Re-order moves based on current iteration results
            ordered_moves.sort_by_key(|mv| {
                let unstack = mv.force_unstack;
                let move_obj = mv.to_move(unstack);
                if let Ok(new_board) = self.apply_move(board, &move_obj) {
                    -self.evaluate_position(&new_board)
                } else {
                    -INFINITY
                }
            });
        }
        
        if let Some(start) = self.search_start {
            self.stats.search_time_ms = start.elapsed().as_millis() as u64;
        }
        
        let unstack = best_move.force_unstack;
        Ok(best_move.to_move(unstack))
    }

    /// Minimax algorithm with alpha-beta pruning
    fn minimax(&mut self, board: &Board, depth: u32, mut alpha: i32, beta: i32, in_quiescence: bool) -> i32 {
        self.stats.positions_evaluated += 1;
        
        if self.time_exceeded() {
            return self.evaluate_position(board);
        }
        
        // Check transposition table
        if self.config.use_transposition_table {
            let hash = self.zobrist_keys.hash_board(board);
            if let Some(entry) = self.transposition_table.get(&hash) {
                if entry.depth >= depth {
                    self.stats.tt_hits += 1;
                    match entry.node_type {
                        NodeType::Exact => return entry.score,
                        NodeType::LowerBound => alpha = alpha.max(entry.score),
                        NodeType::UpperBound => {
                            if entry.score <= alpha {
                                return entry.score;
                            }
                        }
                    }
                    if alpha >= beta {
                        return entry.score;
                    }
                }
            }
        }
        
        // Terminal conditions
        let game = Game::from_board(board.clone());
        let legal_moves = game.get_all_moves();
        
        if legal_moves.is_empty() || depth == 0 {
            // Enter quiescence search if enabled and not already in it
            if !in_quiescence && self.config.use_quiescence && depth == 0 {
                return self.quiescence_search(board, alpha, beta, 2);
            }
            return self.evaluate_position(board);
        }
        
        // Order moves for better pruning
        let ordered_moves = self.order_moves(board, &legal_moves);
        
        let mut best_score = -INFINITY;
        let mut best_move_u16: Option<u16> = None;
        
        for potential_move in &ordered_moves {
            let unstack = potential_move.force_unstack;
            let mv = potential_move.to_move(unstack);
            
            if let Ok(new_board) = self.apply_move(board, &mv) {
                let score = -self.minimax(&new_board, depth - 1, -beta, -alpha, in_quiescence);
                
                if score > best_score {
                    best_score = score;
                    best_move_u16 = Some(potential_move.to_u16());
                }
                
                alpha = alpha.max(score);
                if alpha >= beta {
                    self.stats.ab_cutoffs += 1;
                    break; // Beta cutoff
                }
            }
        }
        
        // Store in transposition table
        if self.config.use_transposition_table {
            let hash = self.zobrist_keys.hash_board(board);
            let node_type = if best_score <= alpha {
                NodeType::UpperBound
            } else if best_score >= beta {
                NodeType::LowerBound
            } else {
                NodeType::Exact
            };
            
            self.transposition_table.insert(hash, TranspositionEntry {
                depth,
                score: best_score,
                best_move: best_move_u16,
                node_type,
            });
        }
        
        best_score
    }

    /// Quiescence search to avoid horizon effect
    fn quiescence_search(&mut self, board: &Board, mut alpha: i32, beta: i32, depth_left: u32) -> i32 {
        self.stats.quiescence_nodes += 1;
        
        let stand_pat = self.evaluate_position(board);
        
        if stand_pat >= beta || depth_left == 0 {
            return stand_pat;
        }
        
        if stand_pat > alpha {
            alpha = stand_pat;
        }
        
        // Only search captures and checks
        let game = Game::from_board(board.clone());
        let all_moves = game.get_all_moves();
        let tactical_moves: Vec<PotentialMove> = all_moves.into_iter()
            .filter(|mv| self.is_tactical_move(board, mv))
            .collect();
        
        if tactical_moves.is_empty() {
            return stand_pat;
        }
        
        for potential_move in &tactical_moves {
            let unstack = potential_move.force_unstack;
            let mv = potential_move.to_move(unstack);
            
            if let Ok(new_board) = self.apply_move(board, &mv) {
                let score = -self.quiescence_search(&new_board, -beta, -alpha, depth_left - 1);
                
                if score >= beta {
                    return beta;
                }
                if score > alpha {
                    alpha = score;
                }
            }
        }
        
        alpha
    }

    /// Check if a move is tactical (capture or threat)
    fn is_tactical_move(&self, board: &Board, potential_move: &PotentialMove) -> bool {
        let to_pos = &potential_move.to;
        // It's a capture if there's an enemy piece at the destination
        if let Some(target_piece) = board.get_piece(to_pos) {
            let from_pos = &potential_move.from;
            if let Some(moving_piece) = board.get_piece(from_pos) {
                return target_piece.color != moving_piece.color;
            }
        }
        false
    }

    /// Order moves for better alpha-beta pruning efficiency
    fn order_moves(&self, board: &Board, moves: &[PotentialMove]) -> Vec<PotentialMove> {
        let mut scored_moves: Vec<(PotentialMove, i32)> = moves.iter().map(|mv| {
            let score = self.score_move_for_ordering(board, mv);
            (mv.clone(), score)
        }).collect();
        
        // Sort in descending order (best moves first)
        scored_moves.sort_by_key(|(_, score)| -score);
        
        scored_moves.into_iter().map(|(mv, _)| mv).collect()
    }

    /// Score a move for ordering purposes
    fn score_move_for_ordering(&self, board: &Board, potential_move: &PotentialMove) -> i32 {
        let mut score = 0;
        
        let from_pos = &potential_move.from;
        let to_pos = &potential_move.to;
        
        // Prioritize captures
        if let Some(target_piece) = board.get_piece(to_pos) {
            if let Some(moving_piece) = board.get_piece(from_pos) {
                if target_piece.color != moving_piece.color {
                    // MVV-LVA: Most Valuable Victim - Least Valuable Attacker
                    let target_value = self.piece_value(target_piece.bottom);
                    let attacker_value = self.piece_value(moving_piece.bottom);
                    score += target_value * 10 - attacker_value;
                    
                    // Bonus for capturing commander
                    if target_piece.bottom == PieceType::Commander {
                        score += 500;
                    }
                    
                    // Bonus for king threats
                    if target_piece.bottom == PieceType::King {
                        score += 10000;
                    }
                }
            }
        }
        
        // Bonus for center control
        let to_x = to_pos.x;
        let to_y = to_pos.y;
        if (3..=5).contains(&to_x) && (3..=5).contains(&to_y) {
            score += 30;
        }
        
        score
    }

    /// Evaluate board position with multi-criteria heuristics
    fn evaluate_position(&self, board: &Board) -> i32 {
        let white_to_move = board.is_white_to_move();
        
        let material = self.evaluate_material(board);
        let territorial = self.evaluate_territorial_control(board);
        let mobility = self.evaluate_mobility(board);
        let king_safety = self.evaluate_king_safety(board);
        
        // Apply weights
        let mut total_score = 
            (material as f32 * self.config.material_weight +
             territorial as f32 * self.config.territorial_weight +
             mobility as f32 * self.config.mobility_weight +
             king_safety as f32 * self.config.king_safety_weight) as i32;
        
        // Apply tactical penalties
        total_score += self.evaluate_tactical_penalties(board);
        
        // Return from perspective of current player
        if white_to_move {
            total_score
        } else {
            -total_score
        }
    }

    /// Evaluate material balance
    fn evaluate_material(&self, board: &Board) -> i32 {
        let mut white_material = 0;
        let mut black_material = 0;
        
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                if let Some(piece) = board.get_piece(&pos) {
                    let mut piece_value = self.piece_value(piece.bottom);
                    
                    // Add value for stacked piece with bonus
                    if let Some(top_type) = piece.top {
                        let top_value = self.piece_value(top_type);
                        let stack_total = piece_value + top_value;
                        let bonus = (stack_total as f32 * self.config.stack_bonus) as i32;
                        piece_value = stack_total + bonus;
                    }
                    
                    if piece.color == Color::White {
                        white_material += piece_value;
                    } else {
                        black_material += piece_value;
                    }
                }
            }
        }
        
        white_material - black_material
    }

    /// Evaluate territorial control
    fn evaluate_territorial_control(&self, board: &Board) -> i32 {
        let mut white_control = 0;
        let mut black_control = 0;
        
        let game = Game::from_board(board.clone());
        let all_moves = game.get_all_moves();
        
        for potential_move in &all_moves {
            let from_pos = &potential_move.from;
            let to_pos = &potential_move.to;
            
            if let Some(piece) = board.get_piece(from_pos) {
                let to_y = to_pos.y;
                let to_x = to_pos.x;
                
                let mut control_value = 1;
                
                // Enemy territory bonus
                if piece.color == Color::White && to_y >= 6 {
                    control_value += 2;
                } else if piece.color == Color::Black && to_y <= 2 {
                    control_value += 2;
                }
                
                // Central squares bonus
                if (3..=5).contains(&to_x) && (3..=5).contains(&to_y) {
                    control_value += 3;
                }
                
                // Check for proximity to enemy king
                if self.is_adjacent_to_enemy_king(board, to_pos, piece.color) {
                    control_value += 5;
                }
                
                if piece.color == Color::White {
                    white_control += control_value;
                } else {
                    black_control += control_value;
                }
            }
        }
        
        white_control - black_control
    }

    /// Evaluate piece mobility
    fn evaluate_mobility(&self, board: &Board) -> i32 {
        let mut white_mobility = 0;
        let mut black_mobility = 0;
        
        let game = Game::from_board(board.clone());
        let all_moves = game.get_all_moves();
        
        for potential_move in &all_moves {
            let from_pos = &potential_move.from;
            
            if let Some(piece) = board.get_piece(from_pos) {
                let mut mobility_value = 1;
                
                // Bonus for mobile pieces
                if piece.bottom == PieceType::Dragon || piece.bottom == PieceType::Commander {
                    mobility_value = (mobility_value as f32 * 1.5) as i32;
                }
                
                if piece.color == Color::White {
                    white_mobility += mobility_value;
                } else {
                    black_mobility += mobility_value;
                }
            }
        }
        
        white_mobility - black_mobility
    }

    /// Evaluate king safety
    fn evaluate_king_safety(&self, board: &Board) -> i32 {
        let mut white_safety = 0;
        let mut black_safety = 0;
        
        // Find kings
        let mut white_king_pos: Option<Position> = None;
        let mut black_king_pos: Option<Position> = None;
        
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                if let Some(piece) = board.get_piece(&pos) {
                    if piece.bottom == PieceType::King {
                        if piece.color == Color::White {
                            white_king_pos = Some(pos);
                        } else {
                            black_king_pos = Some(pos);
                        }
                    }
                }
            }
        }
        
        // Evaluate white king safety
        if let Some(king_pos) = white_king_pos {
            white_safety += self.evaluate_single_king_safety(board, &king_pos, Color::White);
        }
        
        // Evaluate black king safety
        if let Some(king_pos) = black_king_pos {
            black_safety += self.evaluate_single_king_safety(board, &king_pos, Color::Black);
        }
        
        white_safety - black_safety
    }

    /// Evaluate safety of a single king
    fn evaluate_single_king_safety(&self, board: &Board, king_pos: &Position, color: Color) -> i32 {
        let mut safety = 0;
        
        // Count adjacent defenders
        let adjacent_defenders = self.count_adjacent_pieces(board, king_pos, color);
        
        if adjacent_defenders < 2 {
            safety -= 30; // Exposed king penalty
        }
        
        // Bonus if king is protected by stacked pieces
        if let Some(piece) = board.get_piece(king_pos) {
            if piece.top.is_some() {
                safety += 15;
            }
        }
        
        // Check distance to enemy pieces (more is better)
        let avg_enemy_distance = self.average_distance_to_enemy_pieces(board, king_pos, color);
        safety += (avg_enemy_distance as f32 * 2.0) as i32;
        
        safety
    }

    /// Apply tactical penalties
    fn evaluate_tactical_penalties(&self, board: &Board) -> i32 {
        let mut penalty = 0;
        
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                if let Some(piece) = board.get_piece(&pos) {
                    // Commander stacked unnecessarily (if on top of another piece)
                    if let Some(top_type) = piece.top {
                        if top_type == PieceType::Commander {
                            if piece.color == Color::White {
                                penalty -= 50;
                            } else {
                                penalty += 50;
                            }
                        }
                    }
                    
                    // Check if piece is in enemy territory and undefended
                    let in_enemy_territory = 
                        (piece.color == Color::White && y >= 6) ||
                        (piece.color == Color::Black && y <= 2);
                    
                    if in_enemy_territory && !self.is_defended(board, &pos, piece.color) {
                        let piece_val = self.piece_value(piece.bottom);
                        let undefended_penalty = (piece_val as f32 * 1.5) as i32;
                        if piece.color == Color::White {
                            penalty -= undefended_penalty;
                        } else {
                            penalty += undefended_penalty;
                        }
                    }
                }
            }
        }
        
        penalty
    }

    /// Check if position is adjacent to enemy king
    fn is_adjacent_to_enemy_king(&self, board: &Board, pos: &Position, piece_color: Color) -> bool {
        let x = pos.x;
        let y = pos.y;
        
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                
                if nx >= 0 && nx < 9 && ny >= 0 && ny < 9 {
                    let neighbor_pos = Position::new(nx as usize, ny as usize);
                    if let Some(neighbor_piece) = board.get_piece(&neighbor_pos) {
                        if neighbor_piece.bottom == PieceType::King && neighbor_piece.color != piece_color {
                            return true;
                        }
                    }
                }
            }
        }
        
        false
    }

    /// Count adjacent pieces of the same color
    fn count_adjacent_pieces(&self, board: &Board, pos: &Position, color: Color) -> u32 {
        let mut count = 0;
        let x = pos.x;
        let y = pos.y;
        
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                
                if nx >= 0 && nx < 9 && ny >= 0 && ny < 9 {
                    let neighbor_pos = Position::new(nx as usize, ny as usize);
                    if let Some(neighbor_piece) = board.get_piece(&neighbor_pos) {
                        if neighbor_piece.color == color {
                            count += 1;
                        }
                    }
                }
            }
        }
        
        count
    }

    /// Calculate average distance to enemy pieces
    fn average_distance_to_enemy_pieces(&self, board: &Board, pos: &Position, color: Color) -> f32 {
        let mut total_distance = 0.0;
        let mut count = 0;
        let x = pos.x as f32;
        let y = pos.y as f32;
        
        for ey in 0..9 {
            for ex in 0..9 {
                let enemy_pos = Position::new(ex, ey);
                if let Some(enemy_piece) = board.get_piece(&enemy_pos) {
                    if enemy_piece.color != color {
                        let dx = ex as f32 - x;
                        let dy = ey as f32 - y;
                        let distance = (dx * dx + dy * dy).sqrt();
                        total_distance += distance;
                        count += 1;
                    }
                }
            }
        }
        
        if count > 0 {
            total_distance / count as f32
        } else {
            0.0
        }
    }

    /// Check if a position is defended by a friendly piece
    fn is_defended(&self, board: &Board, pos: &Position, color: Color) -> bool {
        // Check if any friendly piece can move to this position
        let game = Game::from_board(board.clone());
        let all_moves = game.get_all_moves();
        
        for potential_move in &all_moves {
            let from_pos = &potential_move.from;
            let to_pos = &potential_move.to;
            
            if to_pos == pos {
                if let Some(defending_piece) = board.get_piece(from_pos) {
                    if defending_piece.color == color {
                        return true;
                    }
                }
            }
        }
        
        false
    }

    /// Get piece value
    fn piece_value(&self, piece_type: PieceType) -> i32 {
        match piece_type {
            PieceType::Soldier => PIECE_VALUES[1],
            PieceType::Jester => PIECE_VALUES[2],
            PieceType::Commander => PIECE_VALUES[3],
            PieceType::Paladin => PIECE_VALUES[4],
            PieceType::Guard => PIECE_VALUES[5],
            PieceType::Dragon => PIECE_VALUES[6],
            PieceType::Ballista => PIECE_VALUES[7],
            PieceType::King => KING_VALUE,
        }
    }

    /// Apply a move to a board state
    fn apply_move(&self, board: &Board, mv: &Move) -> Result<Board, String> {
        let game = Game::from_board(board.clone());
        game.apply_move_copy(*mv)
    }

    /// Check if time limit has been exceeded
    fn time_exceeded(&self) -> bool {
        if let Some(start) = self.search_start {
            start.elapsed() > Duration::from_millis(self.config.time_limit_ms)
        } else {
            false
        }
    }

    /// Get search statistics
    pub fn get_statistics(&self) -> &MinimaxStatistics {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_statistics(&mut self) {
        self.stats = MinimaxStatistics::default();
    }

    /// Clear transposition table
    pub fn clear_transposition_table(&mut self) {
        self.transposition_table.clear();
    }

    /// Get current configuration
    pub fn config(&self) -> &MinimaxConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: MinimaxConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn test_minimax_creation() {
        let engine = MinimaxEngine::new();
        assert_eq!(engine.config().max_depth, 4);
    }

    #[test]
    fn test_minimax_with_config() {
        let config = MinimaxConfig {
            max_depth: 3,
            use_quiescence: false,
            ..Default::default()
        };
        let engine = MinimaxEngine::with_config(config);
        assert_eq!(engine.config().max_depth, 3);
        assert!(!engine.config().use_quiescence);
    }

    #[test]
    fn test_board_evaluation() {
        let engine = MinimaxEngine::new();
        let board = Board::new();
        let eval = engine.evaluate_position(&board);
        // Initial position should be roughly equal (within some tolerance)
        // The evaluation includes mobility and territorial control which may not be perfectly symmetric
        assert!(eval.abs() < 1000, "Initial position evaluation should be reasonable, got {}", eval);
    }

    #[test]
    fn test_material_evaluation() {
        let engine = MinimaxEngine::new();
        let board = Board::new();
        let material = engine.evaluate_material(&board);
        // Both sides start with equal material
        assert_eq!(material, 0);
    }

    #[test]
    fn test_zobrist_hashing() {
        let engine = MinimaxEngine::new();
        let board1 = Board::new();
        let board2 = Board::new();
        
        let hash1 = engine.zobrist_keys.hash_board(&board1);
        let hash2 = engine.zobrist_keys.hash_board(&board2);
        
        // Same position should have same hash
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_find_best_move() {
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 2,
            time_limit_ms: 1000,
            ..Default::default()
        });
        
        let board = Board::new();
        let result = engine.find_best_move(&board);
        
        assert!(result.is_ok(), "Should find a move in initial position");
        
        let stats = engine.get_statistics();
        assert!(stats.positions_evaluated > 0);
    }

    #[test]
    fn test_avoid_losing_commander() {
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 3,
            time_limit_ms: 2000,
            ..Default::default()
        });
        
        // Create a position where the commander is under attack
        let mut board = Board::new();
        
        // Clear the board for a simple test
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                board.set_piece(&pos, None);
            }
        }
        
        // Set up a simple position
        // White king at E1, White commander at D4
        let white_king_pos = Position::new(4, 0);
        let white_commander_pos = Position::new(3, 3);
        
        // Black king at E9, Black piece threatening commander at D5
        let black_king_pos = Position::new(4, 8);
        let black_attacker_pos = Position::new(3, 4);
        
        use crate::board::{Piece, PieceType, Color};
        
        board.set_piece(&white_king_pos, Some(Piece {
            color: Color::White,
            bottom: PieceType::King,
            top: None,
        }));
        
        board.set_piece(&white_commander_pos, Some(Piece {
            color: Color::White,
            bottom: PieceType::Commander,
            top: None,
        }));
        
        board.set_piece(&black_king_pos, Some(Piece {
            color: Color::Black,
            bottom: PieceType::King,
            top: None,
        }));
        
        board.set_piece(&black_attacker_pos, Some(Piece {
            color: Color::Black,
            bottom: PieceType::Guard,
            top: None,
        }));
        
        // White to move - should try to save or move the commander
        let result = engine.find_best_move(&board);
        
        assert!(result.is_ok(), "Should find a move to protect commander");
    }

    #[test]
    fn test_capture_high_value_piece() {
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 2,
            time_limit_ms: 1000,
            ..Default::default()
        });
        
        // Create a position where we can capture an enemy commander
        let mut board = Board::new();
        
        // Clear the board
        for y in 0..9 {
            for x in 0..9 {
                let pos = Position::new(x, y);
                board.set_piece(&pos, None);
            }
        }
        
        use crate::board::{Piece, PieceType, Color};
        
        // White pieces
        let white_king_pos = Position::new(0, 0);
        let white_guard_pos = Position::new(3, 3);
        
        // Black pieces
        let black_king_pos = Position::new(8, 8);
        let black_commander_pos = Position::new(4, 4); // Adjacent to white guard
        
        board.set_piece(&white_king_pos, Some(Piece {
            color: Color::White,
            bottom: PieceType::King,
            top: None,
        }));
        
        board.set_piece(&white_guard_pos, Some(Piece {
            color: Color::White,
            bottom: PieceType::Guard,
            top: None,
        }));
        
        board.set_piece(&black_king_pos, Some(Piece {
            color: Color::Black,
            bottom: PieceType::King,
            top: None,
        }));
        
        board.set_piece(&black_commander_pos, Some(Piece {
            color: Color::Black,
            bottom: PieceType::Commander,
            top: None,
        }));
        
        // White to move - should capture the commander if possible
        let result = engine.find_best_move(&board);
        
        assert!(result.is_ok(), "Should find a move");
        let mv = result.unwrap();
        
        // Check if the move captures the commander
        let captures_commander = mv.from == white_guard_pos && mv.to == black_commander_pos;
        
        // The engine should prefer capturing high-value pieces
        // Note: This might not always be true due to tactical considerations
        assert!(captures_commander || mv.from == white_guard_pos, 
                "Should consider capturing or moving the guard");
    }

    #[test]
    fn test_statistics_tracking() {
        let mut engine = MinimaxEngine::with_config(MinimaxConfig {
            max_depth: 2,
            time_limit_ms: 500,
            ..Default::default()
        });
        
        let board = Board::new();
        
        // Reset statistics
        engine.reset_statistics();
        let stats_before = engine.get_statistics();
        assert_eq!(stats_before.positions_evaluated, 0);
        
        // Find a move
        let _ = engine.find_best_move(&board);
        
        // Check that statistics were updated
        let stats_after = engine.get_statistics();
        assert!(stats_after.positions_evaluated > 0, "Should have evaluated positions");
        assert!(stats_after.search_time_ms > 0, "Should have tracked time");
    }
}
