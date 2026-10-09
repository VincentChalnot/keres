//! End-to-end coverage of the binary HTTP API (`keres_engine::api`).
//!
//! Every test drives a real `axum::Router` in-process (see `tests/common`)
//! and asserts only what a client can observe: the status code, the response
//! bytes and the response headers. The wire format itself is
//! `docs/PROTOCOL.md`; where a payload has to be decoded to be checked, it is
//! decoded with the same library types a client would use.

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use common::*;
use keres_engine::api::{self, ApiConfig, CorsPolicy, GAME_BYTES, MOVE_BYTES, PLAY_BYTES};
use keres_engine::board::{Board, Color, Piece, PieceType, Position};
use keres_engine::game::Game;
use keres_engine::moves::{Move, PotentialMove};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// A router that requires `Authorization: Bearer <token>` everywhere but
/// `/health`.
fn token_router(token: &str) -> Router {
    api::router(ApiConfig {
        api_token: Some(token.to_string()),
        ..Default::default()
    })
}

/// A router restricted to a single browser origin.
fn allow_list_router(origin: &str) -> Router {
    api::router(ApiConfig {
        cors: CorsPolicy::AllowList(vec![origin.to_string()]),
        ..Default::default()
    })
}

fn get_with_header(path: &str, name: header::HeaderName, value: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(name, value)
        .body(Body::empty())
        .expect("valid request")
}

/// The `/play` payload: an 83-byte game followed by the 2-byte move.
fn play_body(game: &[u8], raw_move: u16) -> Vec<u8> {
    let mut body = Vec::with_capacity(PLAY_BYTES);
    body.extend_from_slice(game);
    body.extend_from_slice(&move_bytes(raw_move));
    body
}

/// Pick a move out of a `/moves` response that can be played without
/// unstacking, and encode it as the `Move` `/play` expects.
fn first_plain_move(potentials: &[u16]) -> u16 {
    let raw = *potentials
        .iter()
        .find(|p| **p & 0x8000 == 0)
        .expect("the position offers a move that does not force an unstack");
    potential_to_move(raw)
}

/// Every legal `Move` encoding for a position, sorted.
fn legal_move_encodings(game: &Game) -> Vec<u16> {
    let mut encodings: Vec<u16> = game
        .get_all_moves()
        .iter()
        .flat_map(|potential| potential.to_moves())
        .map(|mv| mv.to_u16())
        .collect();
    encodings.sort_unstable();
    encodings
}

/// `POST /moves` for a game payload, returning the raw `PotentialMove` list.
async fn moves_for(game: &[u8]) -> Vec<u16> {
    let response = post("/moves", game.to_vec()).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    response.u16s()
}

/// A move from the empty centre of the board (E5→F5), syntactically valid but
/// never legal in the opening position.
const MOVE_FROM_EMPTY_SQUARE: u16 = (41 << 7) | 40;

// ── /health ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_reports_ok_as_plain_text() {
    let response = get("/health").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body, b"ok\n");
    assert_eq!(
        response.content_type.as_deref(),
        Some("text/plain; charset=utf-8")
    );
}

#[tokio::test]
async fn health_stays_reachable_when_a_bearer_token_is_configured() {
    let response = send(token_router("secret"), get_request("/health")).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body, b"ok\n");
}

// ── /new ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn new_returns_a_fresh_white_to_move_game() {
    let response = get("/new").await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body.len(), GAME_BYTES);
    assert_eq!(
        response.content_type.as_deref(),
        Some("application/octet-stream")
    );

    let flags = response.body[81];
    assert_eq!(flags & 0x80, 0x80, "white must be to move in a fresh game");
    assert_eq!(flags & 0x40, 0, "a fresh game is not over");
    assert_eq!(flags & 0x10, 0, "a fresh game is not a draw");
    assert_eq!(response.body[82], 0, "the no-capture counter starts at 0");

    let mut bytes = [0u8; GAME_BYTES];
    bytes.copy_from_slice(&response.body);
    let decoded = Game::from_binary(bytes).expect("/new must emit a decodable game");
    assert_eq!(decoded.to_binary(), Game::new().to_binary());
}

// ── /moves ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn moves_lists_exactly_the_engines_legal_moves_for_the_opening() {
    let game = new_game_bytes().await;
    let response = post("/moves", game).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.content_type.as_deref(),
        Some("application/octet-stream")
    );
    assert_eq!(
        response.body.len() % MOVE_BYTES,
        0,
        "a move list is a whole number of u16s"
    );

    let mut returned = response.u16s();
    assert!(!returned.is_empty(), "the opening position has legal moves");
    for raw in &returned {
        assert!(
            PotentialMove::try_from_u16(*raw).is_some(),
            "0x{raw:04x} is not a decodable PotentialMove"
        );
    }

    let mut expected: Vec<u16> = Game::new()
        .get_all_moves()
        .iter()
        .map(|potential| potential.to_u16())
        .collect();
    returned.sort_unstable();
    expected.sort_unstable();
    assert_eq!(returned, expected);
}

// ── /play ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn play_applies_a_legal_move_and_passes_the_turn() {
    let game = new_game_bytes().await;
    let chosen = first_plain_move(&moves_for(&game).await);

    let response = post("/play", play_body(&game, chosen)).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), GAME_BYTES);
    assert_eq!(
        response.content_type.as_deref(),
        Some("application/octet-stream")
    );
    assert_ne!(response.body, game, "the move must change the position");
    assert_eq!(
        response.body[81] & 0x80,
        0,
        "the turn must pass to black after white moves"
    );
}

#[tokio::test]
async fn play_rejects_replaying_a_move_whose_source_square_is_now_empty() {
    let game = new_game_bytes().await;
    let chosen = first_plain_move(&moves_for(&game).await);
    let after = post("/play", play_body(&game, chosen)).await;
    assert_eq!(after.status, StatusCode::OK);

    let response = post("/play", play_body(&after.body, chosen)).await;

    assert_eq!(response.status, StatusCode::CONFLICT, "{}", response.text());
    assert_eq!(
        response.content_type.as_deref(),
        Some("text/plain; charset=utf-8")
    );
    assert!(
        response.text().contains("no piece at"),
        "unexpected message: {}",
        response.text()
    );
}

#[tokio::test]
async fn play_rejects_a_body_that_is_not_exactly_85_bytes() {
    let mut too_long = new_game_bytes().await;
    too_long.push(0); // 84 bytes: a game plus one stray byte.

    let long = post("/play", too_long).await;
    assert_eq!(long.status, StatusCode::BAD_REQUEST, "{}", long.text());

    let empty = post("/play", Vec::new()).await;
    assert_eq!(empty.status, StatusCode::BAD_REQUEST, "{}", empty.text());
}

#[tokio::test]
async fn play_rejects_a_move_with_the_reserved_high_bit_set() {
    let game = new_game_bytes().await;
    let chosen = first_plain_move(&moves_for(&game).await);

    // Bit 15 is `PotentialMove::force_unstack`; it is not part of the `Move`
    // layout, so a client sending one where the other is expected is caught.
    let response = post("/play", play_body(&game, chosen | 0x8000)).await;

    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
    assert!(
        response.text().contains("not a valid move"),
        "unexpected message: {}",
        response.text()
    );
}

#[tokio::test]
async fn play_rejects_a_decodable_move_that_is_illegal_in_the_position() {
    let game = new_game_bytes().await;

    let response = post("/play", play_body(&game, MOVE_FROM_EMPTY_SQUARE)).await;

    assert_eq!(response.status, StatusCode::CONFLICT, "{}", response.text());
}

#[tokio::test]
async fn play_rejects_a_game_payload_with_an_invalid_square_byte() {
    let mut game = new_game_bytes().await;
    game[0] = 0x80; // bit 7 is reserved and must be 0.
    let chosen = MOVE_FROM_EMPTY_SQUARE;

    let response = post("/play", play_body(&game, chosen)).await;

    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
    assert!(
        response.text().contains("square 0"),
        "unexpected message: {}",
        response.text()
    );
}

// ── /replay-moves ────────────────────────────────────────────────────────────

#[tokio::test]
async fn replaying_an_empty_history_returns_the_initial_game() {
    let response = post("/replay-moves", Vec::new()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body, new_game_bytes().await);
}

#[tokio::test]
async fn replaying_a_history_matches_playing_the_moves_one_at_a_time() {
    let mut game = new_game_bytes().await;
    let mut history = Vec::new();

    for _ in 0..3 {
        let chosen = first_plain_move(&moves_for(&game).await);
        history.extend_from_slice(&move_bytes(chosen));
        let played = post("/play", play_body(&game, chosen)).await;
        assert_eq!(played.status, StatusCode::OK, "{}", played.text());
        game = played.body;
    }

    let response = post("/replay-moves", history).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body, game);
}

#[tokio::test]
async fn replaying_an_odd_length_history_is_rejected() {
    let response = post("/replay-moves", vec![0u8; 3]).await;

    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
    assert!(
        response.text().contains("multiple of 2"),
        "unexpected message: {}",
        response.text()
    );
}

#[tokio::test]
async fn replaying_a_history_names_the_index_of_the_first_illegal_move() {
    let game = new_game_bytes().await;
    let legal = first_plain_move(&moves_for(&game).await);

    let mut history = Vec::new();
    history.extend_from_slice(&move_bytes(legal));
    history.extend_from_slice(&move_bytes(MOVE_FROM_EMPTY_SQUARE));

    let response = post("/replay-moves", history).await;

    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
    assert!(
        response.text().contains("move 1"),
        "the error must name the failing index, got: {}",
        response.text()
    );
}

/// One move over the cap gets the handler's specific diagnosis; a grossly
/// oversized body still stops at the body-limit layer.
///
/// `DefaultBodyLimit` is deliberately sized one move past
/// `max_history_moves` so both answers are reachable — see
/// `ApiConfig::max_history_bytes`.
#[tokio::test]
async fn a_history_one_move_over_the_cap_is_a_four_hundred_naming_the_cap() {
    let router = api::router(ApiConfig {
        max_history_moves: 4,
        ..Default::default()
    });

    let response = send(
        router,
        post_request("/replay-moves", vec![0u8; 5 * MOVE_BYTES]),
    )
    .await;

    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
    assert!(
        response.text().contains("exceeds the maximum of 4"),
        "the error must name the cap, got: {}",
        response.text()
    );
}

#[tokio::test]
async fn a_grossly_oversized_history_is_refused_by_the_body_limit() {
    let router = api::router(ApiConfig {
        max_history_moves: 4,
        ..Default::default()
    });

    let response = send(
        router,
        post_request("/replay-moves", vec![0u8; 64 * MOVE_BYTES]),
    )
    .await;

    assert_eq!(
        response.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{}",
        response.text()
    );
}

// ── Engine routes ────────────────────────────────────────────────────────────

#[tokio::test]
async fn engine_move_board_returns_a_legal_move_for_the_position() {
    let game = new_game_bytes().await;

    let response = post("/engine-move-board", game).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), MOVE_BYTES);
    assert_eq!(
        response.content_type.as_deref(),
        Some("application/octet-stream")
    );

    let raw = response.u16s()[0];
    let mv = Move::try_from_u16(raw).expect("the engine must answer with a decodable move");
    assert!(
        legal_move_encodings(&Game::new()).contains(&mv.to_u16()),
        "{mv} is not legal in the opening position"
    );
}

#[tokio::test]
async fn engine_move_game_returns_a_legal_move_for_the_replayed_history() {
    let game = new_game_bytes().await;
    let opening = first_plain_move(&moves_for(&game).await);
    let history = move_bytes(opening).to_vec();

    let response = post("/engine-move-game", history.clone()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), MOVE_BYTES);

    let raw = response.u16s()[0];
    let mv = Move::try_from_u16(raw).expect("the engine must answer with a decodable move");
    let (position, _) = Game::replay_moves(&history).expect("the history is legal");
    assert!(
        legal_move_encodings(&position).contains(&mv.to_u16()),
        "{mv} is not legal after the opening move"
    );
}

#[tokio::test]
async fn engine_move_game_at_level_one_returns_a_legal_move() {
    let response = post("/engine-move-game/1", Vec::new()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), MOVE_BYTES);

    let raw = response.u16s()[0];
    let mv = Move::try_from_u16(raw).expect("the engine must answer with a decodable move");
    assert!(
        legal_move_encodings(&Game::new()).contains(&mv.to_u16()),
        "{mv} is not legal in the opening position"
    );
}

#[tokio::test]
async fn engine_move_game_rejects_a_level_outside_one_to_ten() {
    for path in ["/engine-move-game/0", "/engine-move-game/11"] {
        let response = post(path, Vec::new()).await;
        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "{path}: {}",
            response.text()
        );
        assert!(
            response.text().contains("level must be between 1 and 10"),
            "{path}: unexpected message: {}",
            response.text()
        );
    }
}

#[tokio::test]
async fn engine_move_game_rejects_a_non_numeric_level() {
    let response = post("/engine-move-game/abc", Vec::new()).await;

    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
}

#[tokio::test]
async fn evaluate_game_returns_a_white_relative_score_for_a_history() {
    let response = post("/evaluate-game", Vec::new()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), api::EVAL_BYTES);
    let score = i32::from_le_bytes(response.body.clone().try_into().unwrap());
    assert!(
        score.abs() < 500,
        "the opening should not be decided: {score}"
    );
}

#[tokio::test]
async fn evaluate_game_rejects_an_illegal_history() {
    // 0xFFFF is not a decodable move.
    let response = post("/evaluate-game", vec![0xFF, 0xFF]).await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );

    let response = post("/evaluate-game", vec![0x00]).await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
}

// ── /game-over-reason ────────────────────────────────────────────────────────

/// White wins in five plies by capturing the black king:
/// B3-C4 E9-E8 C2-A4 D7-E6 A4-E8.
const KING_CAPTURE_HISTORY: [u8; 10] = [0xb7, 0x17, 0x84, 0x06, 0xc1, 0x16, 0x95, 0x0f, 0xad, 0x06];

async fn reason_of(history: &[u8]) -> u8 {
    let response = post("/game-over-reason", history.to_vec()).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), api::REASON_BYTES);
    assert_eq!(
        response.content_type.as_deref(),
        Some("application/octet-stream")
    );
    response.body[0]
}

#[tokio::test]
async fn game_over_reason_is_zero_for_a_game_in_progress() {
    assert_eq!(reason_of(&[]).await, 0, "the initial position");

    let first = first_plain_move(&moves_for(&new_game_bytes().await).await);
    assert_eq!(reason_of(&move_bytes(first)).await, 0, "after one move");

    // One ply short of the king capture.
    assert_eq!(
        reason_of(&KING_CAPTURE_HISTORY[..KING_CAPTURE_HISTORY.len() - 2]).await,
        0
    );
}

#[tokio::test]
async fn game_over_reason_reports_a_king_capture() {
    assert_eq!(reason_of(&KING_CAPTURE_HISTORY).await, 1);
    let (game, _) = Game::replay_moves(&KING_CAPTURE_HISTORY).expect("the history is legal");
    assert!(game.is_game_over() && game.white_wins());
}

#[tokio::test]
async fn game_over_reason_reports_the_forty_move_rule() {
    let (history, game) = scripted_game(0);
    assert!(game.is_draw() && game.moves_without_capture() >= 40);
    assert_eq!(reason_of(&history).await, 2);
}

#[tokio::test]
async fn game_over_reason_reports_insufficient_material() {
    let (history, game) = scripted_game(4);
    assert!(game.is_draw() && game.moves_without_capture() < 40);
    assert_eq!(reason_of(&history).await, 3);
}

#[tokio::test]
async fn game_over_reason_rejects_a_move_played_after_the_game_ended() {
    let mut history = KING_CAPTURE_HISTORY.to_vec();
    // Any decodable move: the replay must refuse it because the game is over.
    history.extend_from_slice(&move_bytes(MOVE_FROM_EMPTY_SQUARE));
    let response = post("/game-over-reason", history).await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
}

#[tokio::test]
async fn game_over_reason_rejects_a_malformed_history() {
    for body in [vec![0x00], vec![0xFF, 0xFF], vec![0u8; 6]] {
        let response = post("/game-over-reason", body.clone()).await;
        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "{body:?}: {}",
            response.text()
        );
    }
}

#[tokio::test]
async fn engine_move_board_reports_a_position_with_no_legal_move_as_a_conflict() {
    // Only the black king is left, and it is white's turn: white has nothing
    // to move. That is a statement about the caller's position, not a server
    // failure, so it must be a 409 rather than a 500.
    let mut board = Board::empty();
    board.set_piece(
        &Position::new(4, 0),
        Some(Piece::new(Color::Black, PieceType::King, None)),
    );
    let game = Game::from_board(board);
    assert!(game.is_white_to_move());

    let response = post("/engine-move-board", game.to_binary().to_vec()).await;

    assert_eq!(response.status, StatusCode::CONFLICT, "{}", response.text());
    assert!(
        response.text().contains("no legal move"),
        "unexpected message: {}",
        response.text()
    );
}

// ── Routing ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn an_unknown_path_is_not_found() {
    let response = get("/nope").await;

    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn a_get_on_a_post_only_route_is_method_not_allowed() {
    let response = get("/moves").await;

    assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn a_post_on_a_get_only_route_is_method_not_allowed() {
    let response = post("/new", Vec::new()).await;

    assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
}

// ── Body limits ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn moves_refuses_a_body_larger_than_a_game() {
    let response = post("/moves", vec![0u8; 200]).await;

    assert_eq!(
        response.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{}",
        response.text()
    );
}

// ── CORS ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn the_default_policy_allows_any_origin() {
    let request = get_with_header("/health", header::ORIGIN, "https://anywhere.test");
    let response = raw(default_router(), request).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("*")
    );
}

#[tokio::test]
async fn an_allow_list_echoes_a_permitted_origin() {
    let request = get_with_header("/health", header::ORIGIN, "https://example.test");
    let response = raw(allow_list_router("https://example.test"), request).await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("https://example.test")
    );
}

#[tokio::test]
async fn an_allow_list_sends_no_allow_origin_header_to_a_foreign_origin() {
    let request = get_with_header("/health", header::ORIGIN, "https://evil.test");
    let response = raw(allow_list_router("https://example.test"), request).await;

    assert!(
        response
            .headers()
            .get("access-control-allow-origin")
            .is_none(),
        "a disallowed origin must not be granted access"
    );
}

#[tokio::test]
async fn a_preflight_is_answered_before_the_token_gate() {
    // Browsers never attach `Authorization` to a preflight, so the CORS layer
    // has to answer it outside the auth layer or no browser client could ever
    // reach a protected route.
    let router = api::router(ApiConfig {
        cors: CorsPolicy::AllowList(vec!["https://example.test".to_string()]),
        api_token: Some("secret".to_string()),
        ..Default::default()
    });

    let response = raw(router, preflight("/play", "https://example.test")).await;

    assert!(
        response.status().is_success(),
        "preflight answered with {}",
        response.status()
    );
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .and_then(|v| v.to_str().ok()),
        Some("https://example.test")
    );
}

// ── Bearer-token auth ────────────────────────────────────────────────────────

#[tokio::test]
async fn a_protected_route_without_a_token_is_unauthorized() {
    let response = raw(token_router("secret"), get_request("/new")).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .and_then(|v| v.to_str().ok()),
        Some("Bearer")
    );
}

#[tokio::test]
async fn a_protected_route_with_the_wrong_token_is_unauthorized() {
    let request = get_with_header("/new", header::AUTHORIZATION, "Bearer wrong");
    let response = send(token_router("secret"), request).await;

    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.text(), "missing or invalid bearer token\n");
}

#[tokio::test]
async fn a_protected_route_with_the_configured_token_succeeds() {
    let request = get_with_header("/new", header::AUTHORIZATION, "Bearer secret");
    let response = send(token_router("secret"), request).await;

    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body.len(), GAME_BYTES);
}
