//! The `server` binary: binds a listener and serves [`keres_engine::api`].
//!
//! All routing, decoding and hardening lives in the library
//! (`src/api.rs`) so that the integration tests in `tests/api.rs` can drive
//! the exact same `Router` this binary serves, without a socket.

use keres_engine::api::{router, ApiConfig};
use std::env;
use std::net::SocketAddr;

// musl's default allocator has severe lock contention under multi-threading
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(target_env = "musl")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let config = ApiConfig::from_env();
    println!(
        "Config: max_history_moves={} max_concurrent_searches={} search_timeout={:?} cors={:?} auth={}",
        config.max_history_moves,
        config.max_concurrent_searches,
        config.search_timeout,
        config.cors,
        if config.api_token.is_some() {
            "bearer token"
        } else {
            "none"
        },
    );

    let app = router(config);

    // Read PORT from environment variable, fallback to 3000
    let port: u16 = env::var("PORT")
        .ok()
        .and_then(|val| val.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    println!("Listening on {addr}");
    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("Server error: {e}");
        std::process::exit(1);
    }
}

/// Stop accepting new connections on SIGTERM/SIGINT and let in-flight
/// requests finish — container runtimes send SIGTERM and then SIGKILL, so
/// without this an engine search in progress is torn down mid-request.
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    println!("Shutdown signal received, draining in-flight requests");
}
