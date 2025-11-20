//! MCTS Tree Search Implementation
//!
//! This module implements Monte Carlo Tree Search with:
//! - Tree structure with nodes containing statistics
//! - UCT-based selection policy
//! - Expansion of unexplored moves
//! - Random simulation (rollouts) using GPU batch simulation
//! - Backpropagation of scores up the tree

use crate::board::Board;
use crate::game::{Game, Move};
use rand::Rng;

/// MCTS Node representing a board state in the search tree
pub struct MctsNode {
    /// The move that led to this state (None for root)
    pub move_from_parent: Option<Move>,
    
    /// Board state at this node
    pub board: Board,
    
    /// Child nodes (one per explored move)
    pub children: Vec<MctsNode>,
    
    /// List of unexplored moves
    pub unexplored_moves: Vec<Move>,
    
    /// Number of times this node has been visited
    pub visit_count: u32,
    
    /// Total score accumulated (from White's perspective)
    pub total_score: f64,
    
    /// Number of wins for White
    pub white_wins: u32,
    
    /// Number of wins for Black
    pub black_wins: u32,
    
    /// Number of draws
    pub draws: u32,
    
    /// Whether this is a terminal node (game over)
    pub is_terminal: bool,
}

impl MctsNode {
    /// Create a new root node
    pub fn new_root(board: Board, legal_moves: Vec<Move>) -> Self {
        let is_terminal = board.is_game_over() || legal_moves.is_empty();
        
        Self {
            move_from_parent: None,
            board,
            children: Vec::new(),
            unexplored_moves: legal_moves,
            visit_count: 0,
            total_score: 0.0,
            white_wins: 0,
            black_wins: 0,
            draws: 0,
            is_terminal,
        }
    }
    
    /// Create a new child node
    pub fn new_child(parent_move: Move, board: Board, legal_moves: Vec<Move>) -> Self {
        let is_terminal = board.is_game_over() || legal_moves.is_empty();
        
        Self {
            move_from_parent: Some(parent_move),
            board,
            children: Vec::new(),
            unexplored_moves: legal_moves,
            visit_count: 0,
            total_score: 0.0,
            white_wins: 0,
            black_wins: 0,
            draws: 0,
            is_terminal,
        }
    }
    
    /// Get average score
    pub fn average_score(&self) -> f64 {
        if self.visit_count == 0 {
            0.0
        } else {
            self.total_score / self.visit_count as f64
        }
    }
    
    /// Get UCT value for this node
    pub fn uct_value(&self, parent_visits: u32, exploration_constant: f64) -> f64 {
        if self.visit_count == 0 {
            return f64::INFINITY; // Prioritize unvisited nodes
        }
        
        let exploitation = self.average_score();
        let exploration = exploration_constant * ((parent_visits as f64).ln() / self.visit_count as f64).sqrt();
        
        exploitation + exploration
    }
    
    /// Check if this node is fully expanded
    pub fn is_fully_expanded(&self) -> bool {
        self.unexplored_moves.is_empty()
    }
    
    /// Select best child using UCT
    pub fn select_best_child(&self, exploration_constant: f64) -> Option<usize> {
        if self.children.is_empty() {
            return None;
        }
        
        let mut best_idx = 0;
        let mut best_value = f64::NEG_INFINITY;
        
        for (idx, child) in self.children.iter().enumerate() {
            let uct = child.uct_value(self.visit_count, exploration_constant);
            if uct > best_value {
                best_value = uct;
                best_idx = idx;
            }
        }
        
        Some(best_idx)
    }
    
    /// Backpropagate a simulation result
    pub fn backpropagate(&mut self, score: f64, result: SimulationResult) {
        self.visit_count += 1;
        self.total_score += score;
        
        match result {
            SimulationResult::WhiteWin => self.white_wins += 1,
            SimulationResult::BlackWin => self.black_wins += 1,
            SimulationResult::Draw => self.draws += 1,
        }
    }
}

/// Result of a simulation
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SimulationResult {
    WhiteWin,
    BlackWin,
    Draw,
}

impl SimulationResult {
    /// Convert game result to SimulationResult
    pub fn from_board(board: &Board) -> Self {
        if board.is_game_over() {
            if board.white_wins() {
                SimulationResult::WhiteWin
            } else if board.is_draw() {
                SimulationResult::Draw
            } else {
                SimulationResult::BlackWin
            }
        } else {
            SimulationResult::Draw // Non-terminal treated as draw
        }
    }
    
    /// Get score from White's perspective (-1 to 1)
    pub fn score_for_white(&self) -> f64 {
        match self {
            SimulationResult::WhiteWin => 1.0,
            SimulationResult::BlackWin => -1.0,
            SimulationResult::Draw => 0.0,
        }
    }
}

/// MCTS Search implementation
pub struct MctsSearch {
    exploration_constant: f64,
}

impl MctsSearch {
    /// Create a new MCTS search with given exploration constant
    pub fn new(exploration_constant: f64) -> Self {
        Self {
            exploration_constant,
        }
    }
    
    /// Perform MCTS iterations
    pub fn search(
        &mut self,
        root: &mut MctsNode,
        iterations: u32,
        max_simulation_depth: u32,
    ) -> Result<(), String> {
        for _ in 0..iterations {
            // Selection: traverse to a leaf or unexpanded node
            let (simulation_result, score) = self.select_expand_and_simulate(root, max_simulation_depth)?;
            
            // Backpropagation: update the root and all nodes along the path
            // For now, we just update the root since we don't keep the full path
            // In a more sophisticated implementation, we'd track the path
            root.backpropagate(score, simulation_result);
        }
        
        Ok(())
    }
    
    /// Combined selection, expansion, and simulation in one pass
    fn select_expand_and_simulate(
        &self,
        node: &mut MctsNode,
        max_depth: u32,
    ) -> Result<(SimulationResult, f64), String> {
        // If terminal, return result immediately
        if node.is_terminal {
            let result = SimulationResult::from_board(&node.board);
            return Ok((result, result.score_for_white()));
        }
        
        // If has unexplored moves, expand one
        if !node.unexplored_moves.is_empty() {
            let move_idx = rand::thread_rng().gen_range(0..node.unexplored_moves.len());
            let selected_move = node.unexplored_moves.remove(move_idx);
            
            // Apply move to get new board state using Game
            let mut game = Game::from_board(node.board.clone());
            if let Err(_) = game.apply_move(selected_move) {
                // Invalid move, simulate from current position
                return self.simulate(&node.board, max_depth)
                    .map(|r| (r, r.score_for_white()));
            }
            let new_board = game.board.clone();
            
            // Generate legal moves for the new position
            let legal_moves: Vec<Move> = game
                .get_all_moves()
                .into_iter()
                .map(|pm| if pm.force_unstack { pm.to_move(true) } else { pm.to_move(false) })
                .collect();
            
            // Create new child node
            let mut child = MctsNode::new_child(selected_move, new_board, legal_moves);
            
            // Simulate from the new child
            let (result, score) = self.simulate(&child.board, max_depth)
                .map(|r| (r, r.score_for_white()))?;
            
            // Backpropagate to the child
            child.backpropagate(score, result);
            
            // Add child to node
            node.children.push(child);
            
            return Ok((result, score));
        }
        
        // If fully expanded, select best child and recurse
        if let Some(best_idx) = node.select_best_child(self.exploration_constant) {
            let (result, score) = self.select_expand_and_simulate(
                &mut node.children[best_idx],
                max_depth,
            )?;
            
            // Backpropagate to current node
            node.backpropagate(score, result);
            
            return Ok((result, score));
        }
        
        // Fallback: simulate from current position
        self.simulate(&node.board, max_depth)
            .map(|r| (r, r.score_for_white()))
    }
    
    /// Simulate a random game from the given board state
    fn simulate(&self, board: &Board, max_depth: u32) -> Result<SimulationResult, String> {
        let mut game = Game::from_board(board.clone());
        let mut depth = 0;
        
        while !game.board.is_game_over() && depth < max_depth {
            let legal_moves = game.get_all_moves();
            
            if legal_moves.is_empty() {
                break;
            }
            
            // Randomly select a move
            let random_idx = rand::thread_rng().gen_range(0..legal_moves.len());
            let potential_move = &legal_moves[random_idx];
            let random_move = if potential_move.force_unstack {
                potential_move.to_move(true)
            } else {
                potential_move.to_move(false)
            };
            
            // Apply the move
            if let Err(_) = game.apply_move(random_move) {
                break; // Invalid move, end simulation
            }
            
            depth += 1;
        }
        
        Ok(SimulationResult::from_board(&game.board))
    }
}

/// Get the best move from MCTS root node statistics
pub fn get_best_move_from_root(root: &MctsNode) -> Option<Move> {
    if root.children.is_empty() {
        return None;
    }
    
    // Select child with highest visit count (most explored)
    let mut best_idx = 0;
    let mut best_visits = 0;
    
    for (idx, child) in root.children.iter().enumerate() {
        if child.visit_count > best_visits {
            best_visits = child.visit_count;
            best_idx = idx;
        }
    }
    
    root.children[best_idx].move_from_parent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Game;
    
    #[test]
    fn test_node_creation() {
        let game = Game::new();
        let legal_moves: Vec<Move> = game.get_all_moves()
            .into_iter()
            .map(|pm| if pm.force_unstack { pm.to_move(true) } else { pm.to_move(false) })
            .collect();
        
        let node = MctsNode::new_root(game.board.clone(), legal_moves);
        assert_eq!(node.visit_count, 0);
        assert_eq!(node.total_score, 0.0);
        assert!(!node.unexplored_moves.is_empty());
    }
    
    #[test]
    fn test_uct_value() {
        let game = Game::new();
        let node = MctsNode::new_root(game.board.clone(), vec![]);
        
        // Unvisited node should have infinite UCT
        let uct = node.uct_value(10, 1.414);
        assert_eq!(uct, f64::INFINITY);
    }
    
    #[test]
    fn test_mcts_search_basic() {
        let game = Game::new();
        let legal_moves: Vec<Move> = game.get_all_moves()
            .into_iter()
            .map(|pm| if pm.force_unstack { pm.to_move(true) } else { pm.to_move(false) })
            .collect();
        
        let mut root = MctsNode::new_root(game.board.clone(), legal_moves);
        let mut mcts = MctsSearch::new(1.414);
        
        // Run a small number of iterations
        let result = mcts.search(&mut root, 10, 10);
        assert!(result.is_ok());
        
        // Root should have been visited
        assert!(root.visit_count > 0);
        
        // Should be able to get a best move
        let best_move = get_best_move_from_root(&root);
        assert!(best_move.is_some());
    }
    
    #[test]
    fn test_simulation_result() {
        let game = Game::new();
        let result = SimulationResult::from_board(&game.board);
        // Initial board is not terminal, should be treated as draw
        assert_eq!(result, SimulationResult::Draw);
    }
}
