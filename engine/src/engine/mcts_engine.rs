//! Public MCTS engine API for the Keres board game.

use crate::board::Board;
use crate::game::Move;
use super::config::EngineConfig;
use super::gpu_batch_processor::{CpuEvaluator, Evaluator, GpuEvaluator};
use super::gpu_context::get_shared_context;
use super::search_tree::KTree;

/// Aggregate statistics returned alongside the chosen move.
pub struct SearchStatistics {
    pub iterations_completed: usize,
    pub nodes_in_tree: usize,
    pub root_visit_count: u32,
}

/// The main engine entry point.
pub struct MctsEngine {
    evaluator: Box<dyn Evaluator>,
    cfg: EngineConfig,
}

impl MctsEngine {
    /// Build an engine that tries GPU evaluation first and falls
    /// back to CPU if no adapter is available.
    pub fn with_config(cfg: EngineConfig) -> Result<Self, String> {
        // Attempt GPU path
        let gpu_result = get_shared_context().and_then(|ctx| {
            GpuEvaluator::try_build(ctx, cfg.weights, cfg.dispatch.clone())
        });

        let eval_box: Box<dyn Evaluator> = match gpu_result {
            Ok(gpu_ev) => Box::new(gpu_ev),
            Err(_) => {
                // Only allow CPU fallback if MCTS_ALLOW_CPU=1
                match std::env::var("MCTS_ALLOW_CPU") {
                    Ok(val) if val == "1" => Box::new(CpuEvaluator { weights: cfg.weights }),
                    _ => {
                        eprintln!("Error: GPU evaluation unavailable and MCTS_ALLOW_CPU is not set to 1. Aborting.");
                        std::process::exit(1);
                    }
                }
            }
        };

        Ok(MctsEngine { evaluator: eval_box, cfg })
    }

    /// Build an engine that always uses the CPU evaluator.
    pub fn cpu_only(cfg: EngineConfig) -> Self {
        MctsEngine {
            evaluator: Box::new(CpuEvaluator { weights: cfg.weights }),
            cfg,
        }
    }

    /// Build an engine with a caller-supplied evaluator (useful for testing).
    pub fn with_evaluator(cfg: EngineConfig, eval: Box<dyn Evaluator>) -> Self {
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
        let mut tree = KTree::with_root(*board, tree_params);

        let budget = self.cfg.iterations;

        for _iter in 0..budget {
            // 1. Selection
            let (leaf_key, path) = tree.descend_to_leaf();

            // 2. Expansion (skip for terminal positions)
            if !tree.board_of(leaf_key).is_game_over() {
                tree.spawn_children(leaf_key);
            }

            // 3. Evaluation
            let leaf_board = *tree.board_of(leaf_key);
            let scores = self.evaluator.score_positions(&[leaf_board]);
            let leaf_score = scores[0];

            // 4. Back-propagation
            tree.feed_result(&path, leaf_score);
        }

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
        c
    }

    #[test]
    fn engine_finds_legal_move_from_opening() {
        let eng = MctsEngine::with_evaluator(
            tiny_cfg(),
            Box::new(FixedScoreEvaluator { val: 0.5 }),
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
    fn cpu_only_constructor_works() {
        let eng = MctsEngine::cpu_only(tiny_cfg());
        let result = eng.find_move(&Board::new());
        assert!(result.is_ok());
    }

    #[test]
    fn terminal_board_returns_error() {
        let mut b = Board::new();
        b.set_game_over(true, true, false);
        let eng = MctsEngine::with_evaluator(
            tiny_cfg(),
            Box::new(FixedScoreEvaluator { val: 0.5 }),
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
        // Replay moves: base64 "xSEWDzoZ" = 3 moves
        let move_bytes: &[u8] = &[0xC5, 0x21, 0x16, 0x0F, 0x3A, 0x19];
        let mut game = crate::game::Game::new();
        for chunk in move_bytes.chunks_exact(2) {
            let mv = Move::from_u16(u16::from_le_bytes([chunk[0], chunk[1]]));
            game.apply_move(mv).expect("replayed move should be valid");
        }
        // Now it's black's turn. The white rook on E2 has a clear line to E9.
        assert!(!game.board.is_white_to_move(), "should be black to move");

        let mut cfg = EngineConfig::default();
        cfg.iterations = 5000;
        let eng = MctsEngine::cpu_only(cfg);
        let (mv, _stats) = eng.find_move(&game.board).expect("should find a move");

        // After black plays, white should NOT be able to immediately capture the king
        let mut game_after = game.clone();
        game_after.apply_move(mv).expect("engine move should be legal");

        // Check: it should NOT be game over after white's next best reply
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

        // Check: can white capture the king after a non-defensive black move?
        // A7-B6: non-defensive black move (soldier from A7=(0,2) to B6=(1,3))
        let bad_move = Move {
            from: crate::Position { x: 0, y: 2 },  // A7
            to: crate::Position { x: 1, y: 3 },    // B6
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

        // Evaluate positions with the CPU evaluator
        let eval = crate::engine::gpu_batch_processor::CpuEvaluator {
            weights: crate::engine::config::ScoringWeights::default(),
        };
        use crate::engine::gpu_batch_processor::Evaluator;
        let root_score = eval.score_positions(&[game.board])[0];
        let after_bad_score = eval.score_positions(&[board_after_bad])[0];
        eprintln!("Root position (black to move) white-perspective: {:.4}", root_score);
        eprintln!("After bad A7-B6 (white to move) white-perspective: {:.4}", after_bad_score);

        // Run MCTS and inspect root children
        let mut cfg = EngineConfig::default();
        cfg.iterations = 5000;
        let eng = MctsEngine::cpu_only(cfg);
        let (best_mv, stats, tree_debug) = eng.find_move_debug(&game.board).unwrap();
        eprintln!("Best move: {} | iterations: {} | nodes: {}",
            best_mv.to_string(), stats.iterations_completed, stats.nodes_in_tree);

        // Print top-level children and their scores
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
