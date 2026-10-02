//! Shared helpers for the HTTP API integration tests.
//!
//! The API is driven in-process through `tower::ServiceExt::oneshot`: no
//! socket is bound, no port is picked, nothing races another test binary.
//! That is the whole reason the route table lives in `keres_engine::api`
//! rather than in `src/server.rs`.

#![allow(dead_code)] // Each test binary uses a different subset.

use axum::body::Body;
use axum::http::{HeaderValue, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use keres_engine::api::{self, ApiConfig};
use tower::ServiceExt;

/// A router with the default configuration.
pub fn default_router() -> Router {
    api::router(ApiConfig::default())
}

/// What a test needs from a response: the status, the content type and the
/// raw body.
pub struct Captured {
    pub status: StatusCode,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl Captured {
    /// Body as text — error responses are `text/plain`.
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Body decoded as a little-endian `u16` list (a move list).
    pub fn u16s(&self) -> Vec<u16> {
        assert_eq!(
            self.body.len() % 2,
            0,
            "a move list must be a whole number of u16s, got {} bytes",
            self.body.len()
        );
        self.body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect()
    }
}

/// Send one request through a router and capture the whole response.
pub async fn send(router: Router, request: Request<Body>) -> Captured {
    let response = router
        .oneshot(request)
        .await
        .expect("the router is infallible");
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("the body is fully buffered")
        .to_bytes()
        .to_vec();
    Captured {
        status,
        content_type,
        body,
    }
}

/// Send a response-header-preserving request and return the raw response.
pub async fn raw(router: Router, request: Request<Body>) -> axum::response::Response {
    router
        .oneshot(request)
        .await
        .expect("the router is infallible")
}

pub fn get_request(path: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri(path)
        .body(Body::empty())
        .expect("valid request")
}

pub fn post_request(path: &str, body: impl Into<Vec<u8>>) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(path)
        .header("content-type", "application/octet-stream")
        .body(Body::from(body.into()))
        .expect("valid request")
}

/// `GET /path` against a default router.
pub async fn get(path: &str) -> Captured {
    send(default_router(), get_request(path)).await
}

/// `POST /path` against a default router.
pub async fn post(path: &str, body: impl Into<Vec<u8>>) -> Captured {
    send(default_router(), post_request(path, body)).await
}

/// The 83 bytes of a fresh game, fetched through `/new` (so the tests use the
/// same bytes a real client would).
pub async fn new_game_bytes() -> Vec<u8> {
    let response = get("/new").await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.body.len(), api::GAME_BYTES);
    response.body
}

/// Encode a `Move` as the 2 little-endian bytes `/play` expects.
pub fn move_bytes(raw: u16) -> [u8; 2] {
    raw.to_le_bytes()
}

/// `PotentialMove` → the `Move` payload for the "do not unstack" variant.
pub fn potential_to_move(potential: u16) -> u16 {
    potential & 0x3FFF
}

/// Build an `Origin`-carrying preflight request.
pub fn preflight(path: &str, origin: &str) -> Request<Body> {
    Request::builder()
        .method(Method::OPTIONS)
        .uri(path)
        .header("origin", HeaderValue::from_str(origin).expect("ascii"))
        .header("access-control-request-method", "POST")
        .body(Body::empty())
        .expect("valid request")
}
