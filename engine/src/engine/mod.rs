//! Public API of the Keres AI engine.

pub mod constants;
pub mod eval;
pub mod search;
pub mod tree_recorder;
pub mod tt;
pub mod types;

pub use eval::evaluate_absolute;
pub use search::{root_search, RootSearchResult, SearchStats};
pub use types::SearchConfig;

/// Run an engine search on `game` and return the best move.
///
/// Uses `MAX_DEPTH` from constants and all optimizations enabled.
pub fn find_best_move(
    game: &crate::game::Game,
    config: Option<SearchConfig>,
    recorder: Option<&tree_recorder::TreeRecorder>,
) -> Option<crate::moves::Move> {
    let cfg = config.unwrap_or_default();
    let result = root_search(game, &cfg, &[], recorder);
    result.best_move
}
