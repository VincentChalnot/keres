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
            Err(_) => Box::new(CpuEvaluator { weights: cfg.weights }),
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

        Ok((best, stats))
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
}
