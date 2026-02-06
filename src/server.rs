use axum::{
    body::Bytes,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use keres_engine::board::{Board, BOARD_SIZE};
use keres_engine::engine::{EngineConfig};
use keres_engine::game::{Game, Move};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

// Shared engine state
struct AppState {
    mcts_engine: Mutex<Option<MctsEngine>>,
}

#[tokio::main]
async fn main() {
    // @todo Initialize the MCTS engine with configuration
    let mcts_config = EngineConfig {
        // This is just an example.
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

    let state = Arc::new(AppState {
        mcts_engine: Mutex::new(mcts_engine),
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/new", get(new_game))
        .route("/moves", post(post_moves))
        .route("/play", post(play_move))
        .route("/replay-moves", post(replay_moves))
        .route("/engine-move", post(engine_move))
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

async fn replay_moves(payload: Bytes) -> Result<Vec<u8>, StatusCode> {
    // Payload is a binary list of moves, each move is 2 bytes (u16 little-endian)
    let move_bytes = payload;
    
    // Validate that the payload length is a multiple of 2
    if move_bytes.len() % 2 != 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    // Start with a new game
    let mut game = Game::new();
    
    // Replay each move
    for i in (0..move_bytes.len()).step_by(2) {
        let move_u16 = u16::from_le_bytes([move_bytes[i], move_bytes[i + 1]]);
        let mv = Move::from_u16(move_u16);
        
        // Apply the move and return error if it's invalid
        game.apply_move(mv).map_err(|_| StatusCode::BAD_REQUEST)?;
    }
    
    // Return the final board state
    let final_board = game.to_binary();
    Ok(final_board.to_vec())
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

    // @todo implement MCTS engine move selection

    // Encode the move for the client
    let move_encoding = mv.to_u16();

    // Return the move as 2-byte little-endian u16
    Ok(move_encoding.to_le_bytes().to_vec())
}
