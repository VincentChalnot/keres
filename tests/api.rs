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

    let response = post_slow("/engine-move-board", game).await;

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

    let response = post_slow("/engine-move-game", history.clone()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), MOVE_BYTES);

    let raw = response.u16s()[0];
    let mv = Move::try_from_u16(raw).expect("the engine must answer with a decodable move");
    let position = Game::from_moves(&history).expect("the history is legal");
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
    let response = post_slow("/evaluate-game", Vec::new()).await;

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

// ── /evaluate-moves ──────────────────────────────────────────────────────────

/// `POST` through a router whose search budget is generous: these tests run a
/// full level-10 search in a debug build, in parallel with the rest of the
/// suite, which can exceed the default 60 s on a small CI runner and turn a
/// correct answer into a `503`.
async fn post_slow(path: &str, body: Vec<u8>) -> Captured {
    let router = api::router(ApiConfig {
        search_timeout: std::time::Duration::from_secs(900),
        ..Default::default()
    });
    send(router, post_request(path, body)).await
}

/// Decode `/evaluate-moves` records into `(move, score)` pairs.
fn move_evals(body: &[u8]) -> Vec<(u16, i32)> {
    assert_eq!(body.len() % api::MOVE_EVAL_BYTES, 0);
    body.chunks(api::MOVE_EVAL_BYTES)
        .map(|r| {
            (
                u16::from_le_bytes([r[0], r[1]]),
                i32::from_le_bytes([r[2], r[3], r[4], r[5]]),
            )
        })
        .collect()
}

#[tokio::test]
async fn evaluate_moves_scores_every_legal_move_best_first() {
    let response = post_slow("/evaluate-moves", Vec::new()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    let records = move_evals(&response.body);

    // One record per legal move (stacked/unstacked variants count apart).
    let expected: usize = Game::new()
        .get_all_moves()
        .iter()
        .map(|m| m.to_moves().len())
        .sum();
    assert_eq!(records.len(), expected);
    // White to move: best (highest White-relative) first, so non-increasing.
    assert!(records.windows(2).all(|w| w[0].1 >= w[1].1));
}

#[tokio::test]
async fn evaluate_moves_is_white_relative_and_matches_evaluate_game() {
    // One white move played: Black is to move, so its best move (the first
    // record) carries the *lowest* White-relative score.
    let first = Game::new().get_all_moves()[0].to_moves()[0];
    let history = first.to_u16().to_le_bytes().to_vec();
    let response = post_slow("/evaluate-moves", history.clone()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    let records = move_evals(&response.body);
    assert!(!records.is_empty());
    assert!(records.windows(2).all(|w| w[0].1 <= w[1].1));

    // The best record is the position's score, as /evaluate-game reports it.
    let eval = post_slow("/evaluate-game", history).await;
    let best = i32::from_le_bytes(eval.body.clone().try_into().unwrap());
    assert_eq!(records[0].1, best);
}

#[tokio::test]
async fn evaluate_moves_rejects_an_illegal_history() {
    let response = post("/evaluate-moves", vec![0xFF, 0xFF]).await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
}

#[tokio::test]
async fn evaluate_moves_answers_nothing_for_a_finished_game() {
    let response = post("/evaluate-moves", KING_CAPTURE_HISTORY.to_vec()).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert!(response.body.is_empty());
}

// ── /analyze-puzzle ──────────────────────────────────────────────────────────

/// A3-B4 A9-A8 B4-C5 B9-A7: Black's last move leaves the stack on A7 to a
/// White ballista that the A8 ballista guards insufficiently.
fn blunder_history() -> Vec<u8> {
    let sq = |x: usize, y: usize| Position::new(x, y);
    [
        (sq(0, 6), sq(1, 5)),
        (sq(0, 0), sq(0, 1)),
        (sq(1, 5), sq(2, 4)),
        (sq(1, 0), sq(0, 2)),
    ]
    .into_iter()
    .flat_map(|(from, to)| {
        Move {
            from,
            to,
            unstack: false,
        }
        .to_u16()
        .to_le_bytes()
    })
    .collect()
}

/// The first White move of the game, as a one-move history.
fn one_move_history() -> Vec<u8> {
    let first = Game::new().get_all_moves()[0].to_moves()[0];
    first.to_u16().to_le_bytes().to_vec()
}

/// `(flags, steps)` of an `/analyze-puzzle` body, each step as its 19 bytes;
/// asserts the header and the total length agree.
fn analysis_steps(body: &[u8]) -> (u8, Vec<&[u8]>) {
    assert!(body.len() >= 5, "header is 5 bytes, got {}", body.len());
    assert_eq!(body[0], 1, "format version");
    let steps = body[4] as usize;
    assert_eq!(body.len(), 5 + 19 * steps, "5 header bytes + 19 per step");
    (body[1], body[5..].chunks(19).collect())
}

fn step_reply(step: &[u8]) -> u16 {
    u16::from_le_bytes([step[10], step[11]])
}

#[tokio::test]
async fn analyze_puzzle_answers_a_header_and_fixed_width_steps() {
    let history = one_move_history();
    let response = post_slow("/analyze-puzzle?max_plies=3", history.clone()).await;

    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(
        response.content_type.as_deref(),
        Some("application/octet-stream")
    );
    let (flags, steps) = analysis_steps(&response.body);
    assert_eq!(flags & !1, 0, "only bit 0 is defined");
    assert!(
        (1..=2).contains(&steps.len()),
        "three plies are at most two solver steps"
    );
    // Black is to move after the history: its best move must be playable.
    let mut game = Game::from_moves(&history).expect("legal history");
    let best = u16::from_le_bytes([steps[0][0], steps[0][1]]);
    let mv = Move::try_from_u16(best).expect("a decodable move");
    game.try_make(&mv).expect("the solver's move is legal");
    // Scores are White-relative: Black's best is the lowest.
    let best_score = i32::from_le_bytes(steps[0][2..6].try_into().unwrap());
    let second_score = i32::from_le_bytes(steps[0][6..10].try_into().unwrap());
    assert!(best_score <= second_score);
    assert!((1..=10).contains(&steps[0][12]), "found_at_level");
    assert_eq!(steps[0][18] & 0xF0, 0, "theme bits 12-15 are always 0");
}

#[tokio::test]
async fn analyze_puzzle_honours_the_ply_budget() {
    let one = post_slow("/analyze-puzzle?max_plies=1", Vec::new()).await;
    assert_eq!(one.status, StatusCode::OK, "{}", one.text());
    let (flags, steps) = analysis_steps(&one.body);
    assert_eq!(
        flags, 0,
        "an empty move list has no last move to trigger on"
    );
    assert_eq!(steps.len(), 1);
    assert_eq!(step_reply(steps[0]), 0xFFFF, "budget spent: no reply");

    // Anything below 1 is raised to 1: the same deterministic answer.
    let zero = post_slow("/analyze-puzzle?max_plies=0", Vec::new()).await;
    assert_eq!(zero.body, one.body);
    let negative = post_slow("/analyze-puzzle?max_plies=-7", Vec::new()).await;
    assert_eq!(negative.body, one.body);

    let two = post_slow("/analyze-puzzle?max_plies=2", Vec::new()).await;
    let (_, steps) = analysis_steps(&two.body);
    assert_eq!(steps.len(), 1);
    assert_ne!(step_reply(steps[0]), 0xFFFF, "two plies: move and reply");
}

#[tokio::test]
async fn analyze_puzzle_flags_a_piece_left_hanging_by_the_last_move() {
    let response = post_slow("/analyze-puzzle?max_plies=1", blunder_history()).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    let (flags, _) = analysis_steps(&response.body);
    assert_eq!(flags, 1, "HANGING_AFTER_LAST_MOVE");
}

#[tokio::test]
async fn analyze_puzzle_rejects_a_bad_move_list_or_budget() {
    for body in [
        vec![0xFF, 0xFF],
        vec![0x00],
        one_move_history()[..1].to_vec(),
    ] {
        let response = post("/analyze-puzzle", body).await;
        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "{}",
            response.text()
        );
    }
    let response = post("/analyze-puzzle?max_plies=many", Vec::new()).await;
    assert_eq!(
        response.status,
        StatusCode::BAD_REQUEST,
        "{}",
        response.text()
    );
}

#[tokio::test]
async fn analyze_puzzle_answers_409_for_a_finished_game() {
    let response = post("/analyze-puzzle", KING_CAPTURE_HISTORY.to_vec()).await;
    assert_eq!(response.status, StatusCode::CONFLICT, "{}", response.text());
}

#[tokio::test]
async fn analyze_puzzle_sits_behind_the_bearer_token() {
    let response = send(
        token_router("secret"),
        post_request("/analyze-puzzle", Vec::new()),
    )
    .await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
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
    let game = Game::from_moves(&KING_CAPTURE_HISTORY).expect("the history is legal");
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

// ── Threefold repetition ─────────────────────────────────────────────────────

/// A 4-ply shuffle from the initial position that brings it back exactly:
/// a White piece steps sideways onto an empty square (`0x2345`, square 69 to
/// 70) and back (`0x22c6`), and so does a Black one (`0x0601`, 1 to 12, and
/// `0x008c`). The initial position is the 1st occurrence, it is back at ply 4
/// (2nd) and at ply 8 (3rd).
const SHUFFLE_CYCLE: [u16; 4] = [0x2345, 0x0601, 0x22c6, 0x008c];

/// The first `plies` moves of the endlessly repeated shuffle, as a history.
fn shuffle_history(plies: usize) -> Vec<u8> {
    SHUFFLE_CYCLE
        .iter()
        .cycle()
        .take(plies)
        .flat_map(|m| m.to_le_bytes())
        .collect()
}

#[tokio::test]
async fn game_over_reason_reports_threefold_repetition() {
    assert_eq!(reason_of(&shuffle_history(8)).await, 4);
    let game = Game::from_moves(&shuffle_history(8)).expect("the history is legal");
    assert!(game.is_game_over() && game.is_draw() && !game.white_wins());
    // The counter differs on every occurrence of the position; it plays no part.
    assert_eq!(game.moves_without_capture(), 8);
}

#[tokio::test]
async fn the_second_occurrence_of_a_position_does_not_end_the_game() {
    for plies in [4, 5, 6, 7] {
        assert_eq!(reason_of(&shuffle_history(plies)).await, 0, "{plies} plies");
    }
}

#[tokio::test]
async fn replay_moves_flags_a_threefold_repetition_as_a_draw() {
    let response = post("/replay-moves", shuffle_history(8)).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body.len(), GAME_BYTES);
    // White to move (0x80), game over (0x40), draw (0x10), nobody wins (0x20).
    assert_eq!(response.body[81], 0x80 | 0x40 | 0x10);
    assert_eq!(response.body[82], 8);

    let response = post("/replay-moves", shuffle_history(7)).await;
    assert_eq!(response.body[81], 0, "black to move, game in progress");
}

#[tokio::test]
async fn a_history_continuing_past_a_threefold_repetition_is_refused_everywhere() {
    let mut history = shuffle_history(8);
    history.extend_from_slice(&SHUFFLE_CYCLE[0].to_le_bytes());
    for path in [
        "/replay-moves",
        "/game-over-reason",
        "/engine-move-game",
        "/engine-move-game/5",
        "/evaluate-game",
        "/evaluate-moves",
    ] {
        let response = post(path, history.clone()).await;
        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "{path}: {}",
            response.text()
        );
        assert!(response.text().contains("already over"), "{path}");
    }
}

#[tokio::test]
async fn a_game_finished_by_repetition_answers_like_any_finished_game() {
    let history = shuffle_history(8);

    let response = post("/evaluate-moves", history.clone()).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert!(response.body.is_empty());

    let response = post("/evaluate-game", history.clone()).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text());
    assert_eq!(response.body, 0i32.to_le_bytes(), "a draw scores 0");

    for path in ["/engine-move-game", "/engine-move-game/5"] {
        let response = post(path, history.clone()).await;
        assert_eq!(
            response.status,
            StatusCode::CONFLICT,
            "{path}: {}",
            response.text()
        );
    }

    let replayed = post("/replay-moves", history).await;
    let response = post("/engine-move-board", replayed.body).await;
    assert_eq!(response.status, StatusCode::CONFLICT, "{}", response.text());
}

#[tokio::test]
async fn the_engine_does_not_play_in_a_game_finished_by_a_king_capture() {
    for path in ["/engine-move-game", "/engine-move-game/5"] {
        let response = post(path, KING_CAPTURE_HISTORY.to_vec()).await;
        assert_eq!(
            response.status,
            StatusCode::CONFLICT,
            "{path}: {}",
            response.text()
        );
    }
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
