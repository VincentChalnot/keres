//! Public engine API for the Keres board game.
//!
//! Wraps the two-stage search pipeline:
//! - Stage 1: exhaustive alpha-beta MultiPV search
//! - Stage 2: refined search on Stage 1 candidates (null move, LMR, selective extensions)

use crate::board::Board;
use crate::game::Move;
use super::config::EngineConfig;
use super::search_config::SearchConfig;
use super::stage_config::StageConfig;
use super::stage1;

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
        if board.is_game_over() {
            return Err("cannot search from a terminal position".into());
        }

        let s1_config = self.to_stage1_config();
        let s2_config = StageConfig::stage2();
        let threads = self.cfg.threads;

        let (s1_result, _s1_stats, tt) = stage1::stage1_search_with_config(board, &s1_config, threads);

        if s1_result.top_moves.is_empty() {
            return Err("no legal moves found".into());
        }

        // Decide whether to run Stage 2
        let final_result = if stage1::all_same_root_move(&s1_result.top_moves) {
            s1_result
        } else {
            let s2_engine = stage1::Stage2Engine::new(s2_config, tt);
            s2_engine.search(board, &s1_result.top_moves)
        };

        Ok((final_result.best_move, SearchStatistics {
            nodes_searched: final_result.nodes_visited as usize,
            stage1_moves: final_result.top_moves.len(),
        }))
    }

    /// Run the search and also return a debug tree snapshot (for the
    /// debug-tree CLI command).
    pub fn find_move_debug(&self, board: &Board)
        -> Result<(Move, SearchStatistics, super::search::DebugTree), String>
    {
        if board.is_game_over() {
            return Err("cannot search from a terminal position".into());
        }

        let config = self.to_search_config();
        let (result, _stats) = stage1::stage1_search(board, &config);

        if result.top_moves.is_empty() {
            return Err("no legal moves found".into());
        }

        // Build debug tree from Stage 1 results
        let debug = super::search::build_debug_tree(board, &result.top_moves);

        Ok((result.best_move, SearchStatistics {
            nodes_searched: result.nodes_visited as usize,
            stage1_moves: result.top_moves.len(),
        }, debug))
    }

    fn to_search_config(&self) -> SearchConfig {
        SearchConfig {
            depth: self.cfg.stage1_depth,
            top_moves: 3,
            threads: self.cfg.threads,
            ..SearchConfig::default()
        }
    }

    fn to_stage1_config(&self) -> StageConfig {
        let mut cfg = StageConfig::stage1();
        cfg.depth = self.cfg.stage1_depth as u8;
        cfg
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
