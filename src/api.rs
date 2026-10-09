//! The binary HTTP API (see `docs/PROTOCOL.md`).
//!
//! This lives in the library rather than in `src/server.rs` so the whole
//! surface can be exercised in-process by the integration tests in
//! `tests/api.rs` (`Router::oneshot`, no sockets, no ports); `src/server.rs`
//! is only the `main` that binds a listener to [`router`].
//!
//! Everything here treats its input as hostile. The engine ships with no
//! authentication by default and is documented as "sits behind a trusted
//! caller", but that is a deployment assumption, not an enforcement
//! mechanism, so the handlers themselves must hold up on their own:
//!
//! - every payload is length-checked and strictly decoded
//!   ([`crate::board::Board::validate`], [`Move::try_from_u16`],
//!   [`Game::try_make`]) so no byte string can reach a panicking code path;
//! - request bodies are capped per route, well below axum's 2 MB default;
//! - CPU-bound search runs on the blocking pool, under a concurrency limit
//!   and a wall-clock timeout, so one caller cannot monopolise the runtime;
//! - a panic anywhere in a handler becomes a `500` instead of a dropped
//!   connection ([`tower_http::catch_panic`]);
//! - optional bearer-token auth and a CORS allow-list can be switched on
//!   through the environment (see [`ApiConfig::from_env`]).

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::{Any, CorsLayer};

use crate::board::BOARD_SIZE;
use crate::engine::constants::KING_VALUE;
use crate::engine::constants::{MAX_DEPTH, MAX_LEVEL, MIN_LEVEL};
use crate::engine::search::{root_search, RootSearchResult};
use crate::engine::types::SearchConfig;
use crate::game::Game;
use crate::game_over::GameOverReason;
use crate::moves::Move;

/// Wire size of a serialised [`Game`]: 81 board bytes + flags + counter.
pub const GAME_BYTES: usize = BOARD_SIZE + 2;
/// Wire size of a serialised [`Move`].
pub const MOVE_BYTES: usize = 2;
/// Wire size of a `/play` payload: a game followed by the move to apply.
pub const PLAY_BYTES: usize = GAME_BYTES + MOVE_BYTES;

/// Default cap on the length of a replayed move history.
///
/// Generous next to any reachable game (the 40-move no-capture rule bounds a
/// real game at a few thousand plies) but small enough that replaying a
/// maximum-size history stays cheap, which matters because replay happens
/// before any other work and is driven entirely by the request body.
pub const DEFAULT_MAX_HISTORY_MOVES: usize = 4096;

/// Default number of engine searches allowed to run at once.
///
/// A single search already saturates every core through Rayon, so more
/// concurrency buys no throughput — it only multiplies memory (each root
/// move allocates its own transposition-table shard) and latency. Excess
/// callers queue, then get a `503` rather than dragging everyone down.
pub const DEFAULT_MAX_CONCURRENT_SEARCHES: usize = 2;

/// Default wall-clock budget for one engine search.
pub const DEFAULT_SEARCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Default time a request will wait for a search slot before giving up.
pub const DEFAULT_SEARCH_QUEUE_TIMEOUT: Duration = Duration::from_secs(10);

/// Which origins the browser-facing CORS layer permits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CorsPolicy {
    /// `Access-Control-Allow-Origin: *`. The historical behaviour, and
    /// harmless *as long as* the API stays unauthenticated: with no cookies
    /// or tokens in play there is no ambient authority for a hostile page to
    /// borrow. It stops being harmless the moment `api_token` is set, which
    /// is why [`ApiConfig::from_env`] warns about the combination.
    AllowAny,
    /// Only these exact origins.
    AllowList(Vec<String>),
}

/// Runtime configuration for [`router`].
#[derive(Clone, Debug)]
pub struct ApiConfig {
    /// Maximum number of moves accepted in a replayed history.
    pub max_history_moves: usize,
    /// Maximum number of concurrent engine searches.
    pub max_concurrent_searches: usize,
    /// Wall-clock budget for a single engine search.
    pub search_timeout: Duration,
    /// How long a request waits for a free search slot.
    pub search_queue_timeout: Duration,
    /// CORS policy.
    pub cors: CorsPolicy,
    /// When set, every route except `/health` requires
    /// `Authorization: Bearer <token>`.
    pub api_token: Option<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        ApiConfig {
            max_history_moves: DEFAULT_MAX_HISTORY_MOVES,
            max_concurrent_searches: DEFAULT_MAX_CONCURRENT_SEARCHES,
            search_timeout: DEFAULT_SEARCH_TIMEOUT,
            search_queue_timeout: DEFAULT_SEARCH_QUEUE_TIMEOUT,
            cors: CorsPolicy::AllowAny,
            api_token: None,
        }
    }
}

impl ApiConfig {
    /// Build a config from the environment, falling back to [`Default`] for
    /// anything unset or unparseable.
    ///
    /// | Variable                          | Meaning                                        |
    /// |-----------------------------------|------------------------------------------------|
    /// | `KERES_MAX_HISTORY_MOVES`         | move-history cap                                |
    /// | `KERES_MAX_CONCURRENT_SEARCHES`   | concurrent engine searches                      |
    /// | `KERES_SEARCH_TIMEOUT_SECS`       | per-search wall-clock budget                    |
    /// | `KERES_CORS_ALLOWED_ORIGINS`      | comma-separated allow-list (default: `*`)       |
    /// | `KERES_API_TOKEN`                 | require `Authorization: Bearer <token>`         |
    pub fn from_env() -> Self {
        let defaults = ApiConfig::default();
        let usize_var = |name: &str, default: usize| -> usize {
            std::env::var(name)
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(default)
        };

        let cors = match std::env::var("KERES_CORS_ALLOWED_ORIGINS") {
            Ok(v) if !v.trim().is_empty() && v.trim() != "*" => CorsPolicy::AllowList(
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
            ),
            _ => CorsPolicy::AllowAny,
        };

        let api_token = std::env::var("KERES_API_TOKEN")
            .ok()
            .filter(|t| !t.is_empty());

        if api_token.is_some() && cors == CorsPolicy::AllowAny {
            eprintln!(
                "warning: KERES_API_TOKEN is set but CORS still allows any origin; \
                 set KERES_CORS_ALLOWED_ORIGINS to restrict browser callers"
            );
        }

        ApiConfig {
            max_history_moves: usize_var("KERES_MAX_HISTORY_MOVES", defaults.max_history_moves),
            max_concurrent_searches: usize_var(
                "KERES_MAX_CONCURRENT_SEARCHES",
                defaults.max_concurrent_searches,
            ),
            search_timeout: std::env::var("KERES_SEARCH_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .map(Duration::from_secs)
                .unwrap_or(defaults.search_timeout),
            search_queue_timeout: defaults.search_queue_timeout,
            cors,
            api_token,
        }
    }

    /// Largest request body accepted on the history-replaying routes.
    ///
    /// One move of slack past the cap on purpose: sized exactly at the cap,
    /// the body-limit layer would reject every over-long history with a bare
    /// `413` before the handler ran, and the specific "move history of N
    /// exceeds the maximum of M" `400` would be dead code. The slack lets a
    /// caller who is one move over get the diagnosis instead of the blunt
    /// instrument; anything genuinely huge still stops at the layer.
    fn max_history_bytes(&self) -> usize {
        (self.max_history_moves + 1) * MOVE_BYTES
    }
}

/// Shared handler state.
struct AppState {
    config: ApiConfig,
    /// Bounds how many engine searches run at once (see
    /// [`ApiConfig::max_concurrent_searches`]).
    search_slots: Semaphore,
}

/// An error response: a status plus a short, caller-facing explanation.
///
/// The message only ever describes the caller's own payload — it never
/// includes internal state — so it is safe to return verbatim and saves the
/// platform from guessing why a request was rejected.
#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn conflict(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    fn unavailable(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            )],
            self.message,
        )
            .into_response()
    }
}

/// A successful binary response, tagged `application/octet-stream`.
struct Binary(Vec<u8>);

impl IntoResponse for Binary {
    fn into_response(self) -> Response {
        (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/octet-stream"),
            )],
            self.0,
        )
            .into_response()
    }
}

type ApiResult = Result<Binary, ApiError>;

/// Build the API router.
pub fn router(config: ApiConfig) -> Router {
    let cors = match &config.cors {
        CorsPolicy::AllowAny => CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
        CorsPolicy::AllowList(origins) => {
            let parsed: Vec<HeaderValue> = origins
                .iter()
                .filter_map(|o| HeaderValue::from_str(o).ok())
                .collect();
            CorsLayer::new()
                .allow_origin(parsed)
                .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
                .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
        }
    };

    let history_limit = config.max_history_bytes();
    let token = config.api_token.clone();
    let state = Arc::new(AppState {
        search_slots: Semaphore::new(config.max_concurrent_searches),
        config,
    });

    // `/health` is deliberately outside the auth layer: it exists for
    // container orchestration probes, which have no place holding a secret,
    // and it reveals nothing but liveness.
    let public = Router::new().route("/health", get(health));

    let protected = Router::new()
        .route("/new", get(new_game))
        .route(
            "/moves",
            post(post_moves).layer(DefaultBodyLimit::max(GAME_BYTES)),
        )
        .route(
            "/play",
            post(play_move).layer(DefaultBodyLimit::max(PLAY_BYTES)),
        )
        .route(
            "/replay-moves",
            post(replay_moves).layer(DefaultBodyLimit::max(history_limit)),
        )
        .route(
            "/engine-move-board",
            post(engine_move_board).layer(DefaultBodyLimit::max(GAME_BYTES)),
        )
        .route(
            "/engine-move-game",
            post(engine_move_game).layer(DefaultBodyLimit::max(history_limit)),
        )
        .route(
            "/engine-move-game/{level}",
            post(engine_move_game_leveled).layer(DefaultBodyLimit::max(history_limit)),
        )
        .route(
            "/evaluate-game",
            post(evaluate_game).layer(DefaultBodyLimit::max(history_limit)),
        )
        .route(
            "/game-over-reason",
            post(game_over_reason).layer(DefaultBodyLimit::max(history_limit)),
        )
        .route_layer(middleware::from_fn_with_state(token, require_token));

    public
        .merge(protected)
        .layer(cors)
        // Outermost so it also covers the layers above: a panic anywhere
        // becomes a 500 for that one request instead of a killed task and a
        // connection reset with no status at all.
        .layer(CatchPanicLayer::new())
        .with_state(state)
}

/// Bearer-token gate, active only when `KERES_API_TOKEN` is configured.
async fn require_token(
    State(expected): State<Option<String>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let Some(expected) = expected else {
        return next.run(request).await;
    };

    let presented = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match presented {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => {
            next.run(request).await
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"))],
            "missing or invalid bearer token\n",
        )
            .into_response(),
    }
}

/// Length-independent byte comparison, so the response time of a wrong token
/// carries no information about how much of it was right.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() ^ b.len()) as u8;
    // Walk the longer side so the loop count depends only on the (public)
    // configured length, never on where the first mismatch is.
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        diff |= x ^ y;
    }
    diff == 0
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        "ok\n",
    )
}

async fn new_game() -> Binary {
    Binary(Game::new().to_binary().to_vec())
}

async fn post_moves(payload: Bytes) -> ApiResult {
    let game = decode_game(&payload)?;
    let mut response = Vec::with_capacity(game.get_all_moves().len() * MOVE_BYTES);
    for m in game.get_all_moves() {
        response.extend_from_slice(&m.to_u16().to_le_bytes());
    }
    Ok(Binary(response))
}

async fn play_move(payload: Bytes) -> ApiResult {
    if payload.len() != PLAY_BYTES {
        return Err(ApiError::bad_request(format!(
            "expected {PLAY_BYTES} bytes (a {GAME_BYTES}-byte game + a {MOVE_BYTES}-byte move), got {}",
            payload.len()
        )));
    }
    let mut game = decode_game(&payload[..GAME_BYTES])?;
    let raw = u16::from_le_bytes([payload[GAME_BYTES], payload[GAME_BYTES + 1]]);
    let mv = Move::try_from_u16(raw)
        .ok_or_else(|| ApiError::bad_request(format!("0x{raw:04x} is not a valid move")))?;

    // A move the position does not allow is the caller's mistake, not the
    // server's: report it instead of applying it blindly (which used to be
    // able to panic, e.g. moving from an empty square).
    game.try_make(&mv).map_err(ApiError::conflict)?;

    Ok(Binary(game.to_binary().to_vec()))
}

async fn replay_moves(State(state): State<Arc<AppState>>, payload: Bytes) -> ApiResult {
    let game = decode_history(&state.config, &payload)?.0;
    Ok(Binary(game.to_binary().to_vec()))
}

async fn engine_move_board(State(state): State<Arc<AppState>>, payload: Bytes) -> ApiResult {
    let game = decode_game(&payload)?;
    let config = SearchConfig {
        max_depth: MAX_DEPTH,
        ..Default::default()
    };
    run_search(&state, game, config, Vec::new()).await
}

async fn engine_move_game(State(state): State<Arc<AppState>>, payload: Bytes) -> ApiResult {
    let (game, history) = decode_history(&state.config, &payload)?;
    let config = SearchConfig {
        max_depth: MAX_DEPTH,
        ..Default::default()
    };
    run_search(&state, game, config, history).await
}

/// Same as `/engine-move-game`, but plays at a caller-chosen strength level
/// (`SearchConfig::for_level`) instead of always searching at full power —
/// e.g. so a platform client can offer the same difficulty presets as the
/// native GUI. `level` is `MIN_LEVEL..=MAX_LEVEL`; out-of-range values are a
/// client error rather than silently clamped, since a public API contract
/// shouldn't quietly mask a caller bug the way an internal GUI slider can.
async fn engine_move_game_leveled(
    State(state): State<Arc<AppState>>,
    Path(level): Path<u8>,
    payload: Bytes,
) -> ApiResult {
    if !(MIN_LEVEL..=MAX_LEVEL).contains(&level) {
        return Err(ApiError::bad_request(format!(
            "level must be between {MIN_LEVEL} and {MAX_LEVEL}, got {level}"
        )));
    }
    let (game, history) = decode_history(&state.config, &payload)?;
    run_search(&state, game, SearchConfig::for_level(level), history).await
}

/// Wire size of an `/evaluate-game` response: one little-endian `i32`.
pub const EVAL_BYTES: usize = 4;

/// Evaluate the position reached by a move history, at full strength
/// (`MAX_LEVEL`, the same search as `/engine-move-game/10`).
///
/// The history is replayed from the initial position rather than taking a
/// bare board, so the search sees the repetition history and the
/// `moves_without_capture` counter of the actual game. The answer is a
/// little-endian `i32` in engine score units, always from **White's**
/// point of view (positive = White is better). A finished game answers
/// `+KING_VALUE` / `-KING_VALUE` for a win and `0` for a draw without
/// searching.
async fn evaluate_game(State(state): State<Arc<AppState>>, payload: Bytes) -> ApiResult {
    let (game, history) = decode_history(&state.config, &payload)?;
    let white_to_move = game.is_white_to_move();

    let score = if game.is_game_over() {
        if game.is_draw() {
            0
        } else if game.white_wins() {
            KING_VALUE
        } else {
            -KING_VALUE
        }
    } else {
        let config = SearchConfig::for_level(MAX_LEVEL);
        let result = run_blocking_search(&state, game, config, history).await?;
        if result.best_move.is_none() {
            // The side to move has no legal move: it has lost.
            if white_to_move {
                -KING_VALUE
            } else {
                KING_VALUE
            }
        } else if white_to_move {
            result.best_score
        } else {
            -result.best_score
        }
    };

    Ok(Binary(score.to_le_bytes().to_vec()))
}

/// Wire size of a `/game-over-reason` response: one reason code.
pub const REASON_BYTES: usize = 1;

/// Report why the game described by a move history is over.
///
/// The history is replayed from the initial position like `/replay-moves`;
/// the answer is one byte, `0` while the game is still in progress and
/// otherwise a [`GameOverReason`] code (see `docs/PROTOCOL.md`). Replay
/// already refuses a move played after the game ended, so a history that is
/// accepted ends exactly at the position the reason describes. It is a pure
/// rules lookup: no search, so it takes no search slot.
async fn game_over_reason(State(state): State<Arc<AppState>>, payload: Bytes) -> ApiResult {
    let game = decode_history(&state.config, &payload)?.0;
    let code = game.game_over_reason().map_or(0, GameOverReason::code);
    Ok(Binary(vec![code]))
}

/// Strictly decode an 83-byte game payload.
fn decode_game(bytes: &[u8]) -> Result<Game, ApiError> {
    if bytes.len() != GAME_BYTES {
        return Err(ApiError::bad_request(format!(
            "expected {GAME_BYTES} bytes of game state, got {}",
            bytes.len()
        )));
    }
    let mut array = [0u8; GAME_BYTES];
    array.copy_from_slice(bytes);
    // A decode failure is a malformed payload — a client error, not a server
    // one (this used to answer 500).
    let game = Game::from_binary(array).map_err(ApiError::bad_request)?;
    game.board.validate().map_err(ApiError::bad_request)?;
    Ok(game)
}

/// Strictly decode and replay a move history, enforcing the configured cap.
fn decode_history(config: &ApiConfig, bytes: &[u8]) -> Result<(Game, Vec<u64>), ApiError> {
    if !bytes.len().is_multiple_of(MOVE_BYTES) {
        return Err(ApiError::bad_request(format!(
            "move history length must be a multiple of {MOVE_BYTES}, got {}",
            bytes.len()
        )));
    }
    if bytes.len() / MOVE_BYTES > config.max_history_moves {
        return Err(ApiError::bad_request(format!(
            "move history of {} exceeds the maximum of {}",
            bytes.len() / MOVE_BYTES,
            config.max_history_moves
        )));
    }
    Game::replay_moves(bytes).map_err(ApiError::bad_request)
}

/// Run `root_search` off the async runtime, under the concurrency limit and
/// the search timeout, and encode the chosen move.
///
/// The search is CPU-bound and internally parallel (Rayon), so running it
/// directly in a handler would block a Tokio worker for the whole search and
/// starve every other connection on that thread.
async fn run_search(
    state: &Arc<AppState>,
    game: Game,
    config: SearchConfig,
    history: Vec<u64>,
) -> ApiResult {
    let best_move = run_blocking_search(state, game, config, history)
        .await?
        .best_move;

    // No legal move means the game is over — that is a statement about the
    // position the caller sent, so it is a 409, not a 500.
    let best_move =
        best_move.ok_or_else(|| ApiError::conflict("no legal move: the game is over\n"))?;
    Ok(Binary(best_move.to_u16().to_le_bytes().to_vec()))
}

/// The shared part of every search route: wait for a slot, run `root_search`
/// on the blocking pool, enforce the wall-clock budget.
async fn run_blocking_search(
    state: &Arc<AppState>,
    game: Game,
    config: SearchConfig,
    history: Vec<u64>,
) -> Result<RootSearchResult, ApiError> {
    let _permit = tokio::time::timeout(
        state.config.search_queue_timeout,
        state.search_slots.acquire(),
    )
    .await
    .map_err(|_| ApiError::unavailable("engine is busy, retry later\n"))?
    .map_err(|_| ApiError::internal("engine shutting down\n"))?;

    let search = tokio::task::spawn_blocking(move || root_search(&game, &config, &history, None));

    tokio::time::timeout(state.config.search_timeout, search)
        .await
        .map_err(|_| ApiError::unavailable("search exceeded its time budget\n"))?
        .map_err(|_| ApiError::internal("search task failed\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_plain_equality() {
        assert!(constant_time_eq(b"secret", b"secret"));
        assert!(!constant_time_eq(b"secret", b"secreu"));
        assert!(!constant_time_eq(b"secret", b"secre"));
        assert!(!constant_time_eq(b"secret", b"secrets"));
        assert!(!constant_time_eq(b"", b"x"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn default_config_is_permissive_but_bounded() {
        let cfg = ApiConfig::default();
        assert_eq!(cfg.cors, CorsPolicy::AllowAny);
        assert!(cfg.api_token.is_none());
        assert!(cfg.max_history_moves > 0);
        assert!(cfg.max_concurrent_searches > 0);
        // One move of slack, so the handler's specific over-cap 400 is
        // reachable instead of being masked by a bare 413.
        assert_eq!(
            cfg.max_history_bytes(),
            (cfg.max_history_moves + 1) * MOVE_BYTES
        );
    }

    #[test]
    fn decode_game_rejects_wrong_length() {
        assert_eq!(
            decode_game(&[0u8; GAME_BYTES - 1]).unwrap_err().status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            decode_game(&[0u8; GAME_BYTES + 1]).unwrap_err().status,
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn decode_game_accepts_a_fresh_game() {
        let bytes = Game::new().to_binary();
        let game = decode_game(&bytes).expect("a fresh game must decode");
        assert!(game.is_white_to_move());
    }

    #[test]
    fn decode_game_rejects_an_invalid_piece_byte() {
        for bad in [
            0b1000_0000, // reserved bit 7 set
            0b0100_1000, // stack with no bottom piece (LLL == 000)
        ] {
            let mut bytes = Game::new().to_binary();
            bytes[0] = bad;
            let err = decode_game(&bytes).unwrap_err();
            assert_eq!(err.status, StatusCode::BAD_REQUEST, "byte 0b{bad:08b}");
        }
    }

    #[test]
    fn decode_game_rejects_a_second_king() {
        let mut bytes = Game::new().to_binary();
        // Square 1 is a black knight in the initial position; make it a
        // second black king (0b0011_1000).
        bytes[1] = 0b0011_1000;
        let err = decode_game(&bytes).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert!(err.message.contains("king"), "{}", err.message);
    }

    #[test]
    fn decode_history_enforces_the_cap() {
        let config = ApiConfig {
            max_history_moves: 2,
            ..Default::default()
        };
        let err = decode_history(&config, &[0u8; 6]).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
        assert!(err.message.contains("exceeds the maximum"));
    }

    #[test]
    fn decode_history_rejects_odd_lengths() {
        let config = ApiConfig::default();
        let err = decode_history(&config, &[0u8; 3]).unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST);
    }
}
