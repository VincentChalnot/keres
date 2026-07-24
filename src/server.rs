use axum::{
    body::Bytes,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use keres_engine::board::BOARD_SIZE;
use keres_engine::game::Game;
use keres_engine::moves::Move;
use std::env;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};

// musl's default allocator has severe lock contention under multi-threading
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/new", get(new_game))
        .route("/moves", post(post_moves))
        .route("/play", post(play_move))
        .route("/replay-moves", post(replay_moves))
        .route("/engine-move-board", post(engine_move_board))
        .route("/engine-move-game", post(engine_move_game))
        .layer(cors);

    // Read PORT from environment variable, fallback to 3000
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|val| val.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    println!("Listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}

async fn new_game() -> impl IntoResponse {
    let game = Game::new();
    let binary_board = game.to_binary();
    (StatusCode::OK, binary_board)
}

async fn post_moves(payload: Bytes) -> Result<Vec<u8>, StatusCode> {
    let board_bytes = payload;
    if board_bytes.len() != BOARD_SIZE + 2 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut board_array = [0u8; BOARD_SIZE + 2];
    board_array.copy_from_slice(&board_bytes);
    let game = Game::from_binary(board_array).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let moves = game.get_all_moves();
    let mut response = Vec::new();
    for m in moves {
        response.extend_from_slice(&m.to_u16().to_le_bytes());
    }
    Ok(response)
}

async fn play_move(payload: Bytes) -> Result<Vec<u8>, StatusCode> {
    let payload = payload;
    if payload.len() < BOARD_SIZE + 4 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let board_bytes = &payload[..BOARD_SIZE + 2];
    let move_bytes = &payload[BOARD_SIZE + 2..BOARD_SIZE + 4];
    let mut board_array = [0u8; BOARD_SIZE + 2];
    board_array.copy_from_slice(board_bytes);
    let mut game = Game::from_binary(board_array).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mv = Move::from_u16(u16::from_le_bytes([move_bytes[0], move_bytes[1]]));
    let _undo = game.make(&mv);
    let new_binary_board = game.to_binary();
    Ok(new_binary_board.to_vec())
}

async fn replay_moves(payload: Bytes) -> Result<Vec<u8>, StatusCode> {
    // Payload is a binary list of moves, each move is 2 bytes (u16 little-endian)
    let move_bytes = payload;
    let game = Game::from_moves(&move_bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    let final_board = game.to_binary();
    Ok(final_board.to_vec())
}

async fn engine_move_board(payload: Bytes) -> Result<Vec<u8>, StatusCode> {
    let board_bytes = payload;
    if board_bytes.len() != BOARD_SIZE + 2 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut board_array = [0u8; BOARD_SIZE + 2];
    board_array.copy_from_slice(&board_bytes);

    let game = Game::from_binary(board_array).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    use keres_engine::engine::constants::MAX_DEPTH;
    use keres_engine::engine::search::root_search;
    use keres_engine::engine::types::SearchConfig;

    let config = SearchConfig {
        max_depth: MAX_DEPTH,
        ..Default::default()
    };

    let result = root_search(&game, &config, &[], None);
    let best_move = result.best_move.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(best_move.to_u16().to_le_bytes().to_vec())
}

async fn engine_move_game(payload: Bytes) -> Result<Vec<u8>, StatusCode> {
    let move_bytes = payload;

    use keres_engine::engine::constants::MAX_DEPTH;
    use keres_engine::engine::search::root_search;
    use keres_engine::engine::types::SearchConfig;

    if move_bytes.len() % 2 != 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Reconstruct the game history: collect the hash of every position that
    // has been played so far (including the initial position), excluding the
    // current (final) position.  root_search always pushes the root hash into
    // the LoopDetector itself, so adding it here too would be redundant.
    let mut game = Game::new();
    let mut game_history: Vec<u64> = Vec::new();

    for i in (0..move_bytes.len()).step_by(2) {
        // Capture the hash BEFORE applying the move (i.e., each position from
        // which a move was made in the real game).
        game_history.push(game.board_hash());
        let mv = Move::from_u16(u16::from_le_bytes([move_bytes[i], move_bytes[i + 1]]));
        game.make(&mv);
    }

    let config = SearchConfig {
        max_depth: MAX_DEPTH,
        ..Default::default()
    };

    let result = root_search(&game, &config, &game_history, None);
    let best_move = result.best_move.ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(best_move.to_u16().to_le_bytes().to_vec())
}
