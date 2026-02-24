//! Public MCTS engine API for the Keres board game.

use std::sync::Arc;
use parking_lot::RwLock;
use crate::board::Board;
use crate::game::Move;
use super::config::EngineConfig;
use super::evaluator::{CpuEvaluator, Evaluator};
use super::search_tree::KTree;

/// Aggregate statistics returned alongside the chosen move.
pub struct SearchStatistics {
    pub iterations_completed: usize,
    pub nodes_in_tree: usize,
    pub root_visit_count: u32,
}

/// The main engine entry point.
pub struct MctsEngine {
    evaluator: Arc<dyn Evaluator>,
    cfg: EngineConfig,
}

impl MctsEngine {
    /// Build an engine using the CPU evaluator (primary constructor).
    pub fn new(cfg: EngineConfig) -> Self {
        MctsEngine {
            evaluator: Arc::new(CpuEvaluator::new(cfg.weights)),
            cfg,
        }
    }

    /// Build an engine with a caller-supplied evaluator (useful for testing).
    pub fn with_evaluator(cfg: EngineConfig, eval: Arc<dyn Evaluator>) -> Self {
        MctsEngine { evaluator: eval, cfg }
    }

    /// Run MCTS from the given board position and return the best
    /// move together with search statistics.
    pub fn find_move(&self, board: &Board) -> Result<(Move, SearchStatistics), String> {
        let (best, stats, _tree) = self.run_search(board)?;
        Ok((best, stats))
    }

    /// Run MCTS and also return the debug tree snapshot (for the debug-tree CLI).
    pub fn find_move_debug(&self, board: &Board)
        -> Result<(Move, SearchStatistics, super::search_tree::DebugTree), String>
    {
        let (best, stats, tree) = self.run_search(board)?;
        let debug = tree.export_debug();
        Ok((best, stats, debug))
    }

    fn run_search(&self, board: &Board) -> Result<(Move, SearchStatistics, KTree), String> {
        if board.is_game_over() {
            return Err("cannot search from a terminal position".into());
        }

        let tree_params = self.cfg.tree_params_copy();
        let tree = Arc::new(RwLock::new(KTree::with_root(*board, tree_params)));
        let budget = self.cfg.iterations;
        let num_threads = self.cfg.threads;

        std::thread::scope(|s| {
            let per_thread = budget / num_threads;
            let remainder = budget % num_threads;
            let mut handles = Vec::new();

            for thread_idx in 0..num_threads {
                let tree = Arc::clone(&tree);
                let evaluator = Arc::clone(&self.evaluator);
                let iters = per_thread + if thread_idx == 0 { remainder } else { 0 };

                handles.push(s.spawn(move || {
                    for _ in 0..iters {
                        // 1. Selection + virtual loss
                        let (leaf_key, path) = {
                            let mut t = tree.write();
                            let (leaf_key, path) = t.descend_to_leaf();
                            t.inject_penalty(&path);
                            (leaf_key, path)
                        };

                        // 2. Expansion
                        {
                            let mut t = tree.write();
                            if !t.board_of(leaf_key).is_game_over() {
                                t.spawn_children(leaf_key);
                            }
                        }

                        // 3. Evaluation (no lock needed — pure computation)
                        let leaf_score = {
                            let t = tree.read();
                            if let Some(forced) = t.immediate_terminal_score(leaf_key) {
                                forced
                            } else {
                                let leaf_board = *t.board_of(leaf_key);
                                let scores = evaluator.score_positions(&[leaf_board]);
                                scores[0]
                            }
                        };

                        // 4. Back-propagation (retract virtual loss + update)
                        {
                            let mut t = tree.write();
                            t.retract_penalty(&path);
                            t.feed_result(&path, leaf_score);
                        }
                    }
                }));
            }

            for h in handles {
                h.join().unwrap();
            }
        });

        let tree = Arc::try_unwrap(tree).unwrap_or_else(|_| panic!("tree Arc has extra refs")).into_inner();
        let best = tree.pick_best_action()
            .ok_or_else(|| "no legal moves found during search".to_string())?;
        let stats = SearchStatistics {
            iterations_completed: budget,
            nodes_in_tree: tree.pool_len(),
            root_visit_count: tree.root_n(),
        };
        Ok((best, stats, tree))
    }
}

// ══════════  Tests  ══════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    /// A trivial evaluator that always returns a fixed score.
    struct FixedScoreEvaluator { val: f32 }
    impl Evaluator for FixedScoreEvaluator {
        fn score_positions(&self, boards: &[Board]) -> Vec<f32> {
            vec![self.val; boards.len()]
        }
    }

    fn tiny_cfg() -> EngineConfig {
        let mut c = EngineConfig::default();
        c.iterations = 50;
        c.threads = 2;
        c
    }

    #[test]
    fn engine_finds_legal_move_from_opening() {
        let eng = MctsEngine::with_evaluator(
            tiny_cfg(),
            Arc::new(FixedScoreEvaluator { val: 0.5 }),
        );
        let (mv, stats) = eng.find_move(&Board::new()).expect("should find a move");
        assert!(stats.iterations_completed == 50);
        assert!(stats.nodes_in_tree > 1);
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
        let eng = MctsEngine::new(tiny_cfg());
        let result = eng.find_move(&Board::new());
        assert!(result.is_ok());
    }

    #[test]
    fn terminal_board_returns_error() {
        let mut b = Board::new();
        b.set_game_over(true, true, false);
        let eng = MctsEngine::with_evaluator(
            tiny_cfg(),
            Arc::new(FixedScoreEvaluator { val: 0.5 }),
        );
        assert!(eng.find_move(&b).is_err());
    }

    /// Regression test for the reported bug: after the moves
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

        let mut cfg = EngineConfig::default();
        cfg.iterations = 5000;
        let eng = MctsEngine::new(cfg);
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

    /// Debug test to inspect the MCTS tree state for the exposed-king position.
    #[test]
    fn debug_exposed_king_tree_state() {
        let move_bytes: &[u8] = &[0xC5, 0x21, 0x16, 0x0F, 0x3A, 0x19];
        let mut game = crate::game::Game::new();
        for chunk in move_bytes.chunks_exact(2) {
            let mv = Move::from_u16(u16::from_le_bytes([chunk[0], chunk[1]]));
            game.apply_move(mv).expect("replayed move should be valid");
        }

        let bad_move = Move {
            from: crate::Position { x: 0, y: 2 },
            to: crate::Position { x: 1, y: 3 },
            unstack: false,
        };
        let board_after_bad = game.apply_move_copy(bad_move).unwrap();
        let white_game = crate::game::Game::from_board(board_after_bad);
        let white_moves = white_game.get_all_moves();
        let mut found_king_capture = false;
        for pm in &white_moves {
            let m = pm.to_move(false);
            if let Ok(b) = white_game.apply_move_copy(m) {
                if b.is_game_over() && b.white_wins() {
                    found_king_capture = true;
                    eprintln!("Confirmed: white can play {} to capture king", m.to_string());
                }
            }
        }
        assert!(found_king_capture, "Expected white to have a king-capture move");

        let eval = crate::engine::evaluator::CpuEvaluator::new(
            crate::engine::config::ScoringWeights::default(),
        );
        use crate::engine::evaluator::Evaluator;
        let root_score = eval.score_positions(&[game.board])[0];
        let after_bad_score = eval.score_positions(&[board_after_bad])[0];
        eprintln!("Root position (black to move) white-perspective: {:.4}", root_score);
        eprintln!("After bad A7-B6 (white to move) white-perspective: {:.4}", after_bad_score);

        let mut cfg = EngineConfig::default();
        cfg.iterations = 5000;
        let eng = MctsEngine::new(cfg);
        let (best_mv, stats, tree_debug) = eng.find_move_debug(&game.board).unwrap();
        eprintln!("Best move: {} | iterations: {} | nodes: {}",
            best_mv.to_string(), stats.iterations_completed, stats.nodes_in_tree);

        eprintln!("\nRoot children (sorted by mean value, ascending = better for black):");
        let mut children_info: Vec<_> = tree_debug.children.iter().map(|c| {
            (c.action.clone().unwrap_or_default(), c.visits, c.mean_value)
        }).collect();
        children_info.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
        for (action, visits, mean) in &children_info {
            eprintln!("  {} : visits={}, mean={:.4}", action, visits, mean);
        }
    }
}
