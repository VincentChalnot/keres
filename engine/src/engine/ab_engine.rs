//! Public engine API for the Keres board game.
//!
//! Wraps the two-stage alpha-beta search pipeline.

use crate::board::Board;
use crate::game::Move;
use super::config::EngineConfig;

/// Aggregate statistics returned alongside the chosen move.
pub struct SearchStatistics {
    pub nodes_searched: usize,
    pub stage1_moves: usize,
}

/// The main engine entry point.
pub struct Engine {
    cfg: EngineConfig,
}

impl Engine {
    /// Build an engine using the built-in static evaluator.
    pub fn new(cfg: EngineConfig) -> Self {
        Engine { cfg }
    }

    /// Run the two-stage search from the given board position and
    /// return the best move together with search statistics.
    pub fn find_move(&self, board: &Board) -> Result<(Move, SearchStatistics), String> {
        let (mv, stats) = super::search::two_stage_search(board, &self.cfg)?;
        Ok((mv, SearchStatistics {
            nodes_searched: stats.nodes_searched,
            stage1_moves: stats.stage1_moves,
        }))
    }

    /// Run the search and also return a debug tree snapshot (for the
    /// debug-tree CLI command).
    pub fn find_move_debug(&self, board: &Board)
        -> Result<(Move, SearchStatistics, super::search::DebugTree), String>
    {
        let (mv, stats, stage1, stage2) =
            super::search::two_stage_search_debug(board, &self.cfg)?;
        let debug = super::search::build_debug_tree(board, &stage1, &stage2);
        Ok((mv, SearchStatistics {
            nodes_searched: stats.nodes_searched,
            stage1_moves: stats.stage1_moves,
        }, debug))
    }
}

// ══════════  Tests  ══════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    fn default_cfg() -> EngineConfig {
        let mut c = EngineConfig::default();
        c.threads = 1; // deterministic
        c
    }

    #[test]
    fn engine_finds_legal_move_from_opening() {
        let eng = Engine::new(default_cfg());
        let (mv, stats) = eng.find_move(&Board::new()).expect("should find a move");
        assert!(stats.nodes_searched > 1);
        // Verify the move is actually legal
        let game = crate::game::Game::from_board(Board::new());
        let all_moves = game.get_all_moves();
        let legal: Vec<Move> = all_moves.iter().flat_map(|pm| {
            let mut v = vec![pm.to_move(false)];
            if pm.unstackable { v.push(pm.to_move(true)); }
            if pm.force_unstack { v.clear(); v.push(pm.to_move(true)); }
            v
        }).collect();
        assert!(legal.contains(&mv), "engine returned illegal move {mv:?}");
    }

    #[test]
    fn new_constructor_works() {
        let eng = Engine::new(default_cfg());
        let result = eng.find_move(&Board::new());
        assert!(result.is_ok());
    }

    #[test]
    fn terminal_board_returns_error() {
        let mut b = Board::new();
        b.set_game_over(true, true, false);
        let eng = Engine::new(default_cfg());
        assert!(eng.find_move(&b).is_err());
    }

    /// Regression test: after the moves
    ///   1. G2-E2  E7-D6
    ///   2. E3-F4
    /// black's king on E9 is exposed to the white rook on E2.
    /// Black must defend (block or move the king); the engine must NOT
    /// pick a move that leaves the king capturable.
    #[test]
    fn black_must_defend_exposed_king() {
        let move_bytes: &[u8] = &[0xC5, 0x21, 0x16, 0x0F, 0x3A, 0x19];
        let mut game = crate::game::Game::new();
        for chunk in move_bytes.chunks_exact(2) {
            let mv = Move::from_u16(u16::from_le_bytes([chunk[0], chunk[1]]));
            game.apply_move(mv).expect("replayed move should be valid");
        }
        assert!(!game.board.is_white_to_move(), "should be black to move");

        let eng = Engine::new(EngineConfig::default());
        let (mv, _stats) = eng.find_move(&game.board).expect("should find a move");
        let mut game_after = game.clone();
        game_after.apply_move(mv).expect("engine move should be legal");

        let white_game = crate::game::Game::from_board(game_after.board);
        let white_moves = white_game.get_all_moves();
        let king_captured = white_moves.iter().any(|pm| {
            let m = pm.to_move(false);
            if let Ok(b) = white_game.apply_move_copy(m) {
                b.is_game_over() && b.white_wins()
            } else {
                false
            }
        });

        assert!(!king_captured,
            "Engine's move {:?} left the black king capturable!", mv);
    }

    /// Debug test to inspect the search state for the exposed-king position.
    #[test]
    fn debug_exposed_king_tree_state() {
        let move_bytes: &[u8] = &[0xC5, 0x21, 0x16, 0x0F, 0x3A, 0x19];
        let mut game = crate::game::Game::new();
        for chunk in move_bytes.chunks_exact(2) {
            let mv = Move::from_u16(u16::from_le_bytes([chunk[0], chunk[1]]));
            game.apply_move(mv).expect("replayed move should be valid");
        }

        let eng = Engine::new(EngineConfig::default());
        let (best_mv, stats, tree_debug) = eng.find_move_debug(&game.board).unwrap();
        eprintln!("Best move: {} | nodes: {}",
            best_mv.to_string(), stats.nodes_searched);

        eprintln!("\nRoot children (sorted by score):");
        let mut children_info: Vec<_> = tree_debug.children.iter().map(|c| {
            (c.action.clone().unwrap_or_default(), c.score, c.stage1_score)
        }).collect();
        children_info.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        for (action, score, stage1) in &children_info {
            eprintln!("  {} : score={:.4}, stage1={:.4}", action, score, stage1);
        }
    }
}
