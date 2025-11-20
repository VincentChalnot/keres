use arx_engine::board::{Board, BOARD_SIZE};
use arx_engine::engine::{EngineConfig, MctsEngine, MinimaxConfig, MinimaxEngine, SearchParams};
use arx_engine::game::{Game, Move};
use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

// Shared engine state
struct AppState {
    mcts_engine: Mutex<Option<MctsEngine>>,
    minimax_engine: Mutex<MinimaxEngine>,
}

#[tokio::main]
async fn main() {
    // Initialize the MCTS engine with configuration
    let mcts_config = EngineConfig {
        max_depth: 50,
        simulations_per_move: 1000,
        exploration_constant: 1.414,
        gpu_batch_size: 4096,
        use_gpu_simulation: true,
    };

    let mcts_engine = match MctsEngine::with_config(mcts_config) {
        Ok(e) => {
            println!("✓ MCTS Engine initialized successfully");
            Some(e)
        }
        Err(e) => {
            eprintln!("⚠ Failed to initialize MCTS engine: {}", e);
            eprintln!("  MCTS engine move endpoint will return errors");
            None
        }
    };

    // Initialize the Minimax engine
    let minimax_config = MinimaxConfig {
        max_depth: 6,
        use_quiescence: true,
        use_transposition_table: false,
        time_limit_ms: 4000,
        ..Default::default()
    };
    let minimax_engine = MinimaxEngine::with_config(minimax_config);
    println!("✓ Minimax Engine initialized successfully");

    let state = Arc::new(AppState {
        mcts_engine: Mutex::new(mcts_engine),
        minimax_engine: Mutex::new(minimax_engine),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/new", get(new_game))
        .route("/moves", post(post_moves))
        .route("/play", post(play_move))
        .route("/engine-move", post(engine_move))
        .route("/minimax-move", post(minimax_move))
        .with_state(state)
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
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
    let board = Board::from_binary(board_array).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let game = Game::from_board(board);
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
    let board = Board::from_binary(board_array).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut game = Game::from_board(board);
    let mv = Move::from_u16(u16::from_le_bytes([move_bytes[0], move_bytes[1]]));
    game.apply_move(mv).map_err(|_| StatusCode::BAD_REQUEST)?;
    let new_binary_board = game.to_binary();
    Ok(new_binary_board.to_vec())
}

async fn engine_move(
    State(state): State<Arc<AppState>>,
    payload: Bytes,
) -> Result<Vec<u8>, StatusCode> {
    let board_bytes = payload;
    if board_bytes.len() != BOARD_SIZE + 2 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut board_array = [0u8; BOARD_SIZE + 2];
    board_array.copy_from_slice(&board_bytes);

    // Convert binary board to Board object
    let board = Board::from_binary(board_array).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get the MCTS engine from state
    let mut engine_guard = state
        .mcts_engine
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let engine = engine_guard
        .as_mut()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Search parameters for this call
    let search_params = SearchParams {
        max_depth: 16,
        simulations_per_move: 100000,
        exploration_constant: 1.414,
    };

    // Evaluate all moves
    let scored_moves = engine.evaluate_moves(&board, &search_params).map_err(|e| {
        eprintln!("MCTS Engine error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Select the best move for the current player
    let mv = MctsEngine::select_best_move(&board, &scored_moves).map_err(|e| {
        eprintln!("MCTS Engine error selecting move: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Encode the move for the client
    let move_encoding = mv.to_u16();

    // Return the move as 2-byte little-endian u16
    Ok(move_encoding.to_le_bytes().to_vec())
}

async fn minimax_move(
    State(state): State<Arc<AppState>>,
    payload: Bytes,
) -> Result<Vec<u8>, StatusCode> {
    let board_bytes = payload;
    if board_bytes.len() != BOARD_SIZE + 2 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut board_array = [0u8; BOARD_SIZE + 2];
    board_array.copy_from_slice(&board_bytes);

    // Convert binary board to Board object
    let board = Board::from_binary(board_array).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get the Minimax engine from state
    let mut engine_guard = state
        .minimax_engine
        .lock()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Find best move using the Minimax engine
    let mv = engine_guard.find_best_move(&board).map_err(|e| {
        eprintln!("Minimax Engine error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Encode the move for the client
    let move_encoding = mv.to_u16();

    // Return the move as 2-byte little-endian u16
    Ok(move_encoding.to_le_bytes().to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arx_engine::board::Position;

    #[test]
    fn test_move_encoding_matches_client_expectations() {
        // Create a move
        let mv = Move {
            from: Position::from_u8(15),
            to: Position::from_u8(25),
            unstack: true,
        };

        let encoded = mv.to_u16();

        // Decode as the TypeScript client would
        let from_decoded = (encoded & 0x7F) as u8;
        let to_decoded = ((encoded >> 7) & 0x7F) as u8;
        let unstack_decoded = ((encoded >> 14) & 0x1) != 0;

        assert_eq!(from_decoded, 15);
        assert_eq!(to_decoded, 25);
        assert_eq!(unstack_decoded, true);
    }
}
