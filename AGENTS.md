# AGENTS.md — Keres Engine (Rust)

## Scope

This repository is the whole scope for this agent — the Rust game engine and
AI. It used to be the `engine/` folder of a monorepo shared with the web
platform and marketing site; those now live in separate repositories
([keres-platform](https://github.com/VincentChalnot/keres-platform),
[keres-website](https://github.com/VincentChalnot/keres-website)). Nothing
here should assume those repos are checked out alongside this one — the only
contract with them is the binary HTTP API described in
[`docs/PROTOCOL.md`](docs/PROTOCOL.md) and the published
`ghcr.io/vincentchalnot/keres/backend` image.

## Project overview

Two binaries share one library crate (`keres_engine`, `src/lib.rs`):

| Binary   | Entry point    | Purpose                                                        |
|----------|-----------------|------------------------------------------------------------------|
| `server` | `src/server.rs` | HTTP server exposing the binary wire API (see `docs/PROTOCOL.md`) |
| `keres`  | `src/main.rs`   | CLI: terminal TUI hotseat game, move listing, engine queries, search-tree debugging |

## Layout

| Path                       | Role                                                                 |
|-----------------------------|-----------------------------------------------------------------------|
| `src/board.rs`               | `Board`, `Piece`, `Position`, binary encode/decode (the wire format) |
| `src/game.rs`                | `Game`: turn state, make/unmake moves, game-over tracking, `to_binary`/`from_binary` |
| `src/moves.rs`                | `Move`, `PotentialMove`, `MoveGenerator` — legal move generation per piece type |
| `src/game_over.rs`            | Win/draw condition checks (king capture, 50-move rule, insufficient material, etc.) |
| `src/cli_rendering.rs`        | Terminal board rendering + game-state hashing used by the CLI        |
| `src/tui.rs`                  | Interactive terminal UI ([ratatui](https://ratatui.rs)) for hotseat play |
| `src/engine/`                  | The AI: search, evaluation, types/config                              |
| `src/engine/search/`            | Negamax + alpha-beta (`alpha_beta.rs`, `negamax.rs`), quiescence, killer moves, loop/repetition detection, root entry point (`mod.rs::root_search`) |
| `src/engine/eval/`               | Position evaluation: material, mobility, king safety, pins, promotion, piece-square tables, tempo |
| `src/engine/tt.rs`               | Transposition table                                                    |
| `src/engine/constants.rs`         | Tunables: `MAX_DEPTH` (4), eval weights                                |
| `src/engine/tree_recorder.rs`      | Optional full search-tree recording for `debug-tree`                    |
| `src/server.rs`               | Axum HTTP server — see `docs/PROTOCOL.md` for every route              |
| `src/main.rs`                 | `clap` CLI — subcommands: `play` (default), `show-moves`, `engine-move`, `debug-tree` |
| `docs/PROTOCOL.md`            | Wire protocol reference — regenerate/re-verify against `server.rs`/`board.rs`/`game.rs`/`moves.rs` if any of those change; do not let it drift |

## Conventions

- **No unsafe code, no unnecessary allocation** in hot paths (`engine/search/`,
  `engine/eval/`, `moves.rs`) — this runs per-node in a depth-4+ search tree
  parallelized across threads; allocations there show up directly in AI
  response latency.
- `mimalloc` is the global allocator on the `musl` target only (`#[cfg(target_env
  = "musl")]` in `main.rs`/`server.rs`) — the glibc default allocator is fine
  under normal dev builds; musl's default allocator has severe lock
  contention under Rayon's multi-threading (see the comment above the
  `#[global_allocator]` attribute for the source).
- The wire format (`Board`/`Game`/`Move` binary encode-decode) is
  intentionally compact and bit-packed — see `docs/PROTOCOL.md`. Changing it
  is a breaking change for every consumer (`keres-platform`'s PHP `Model/`
  classes and TypeScript `boardUtils.ts` codecs); coordinate before touching
  `to_binary`/`from_binary`/`to_u16`/`from_u16`, and update `docs/PROTOCOL.md`
  in the same change.
- Search correctness > search speed when the two conflict. Tests under
  `src/game.rs`, `src/moves.rs`, `src/game_over.rs` (`#[cfg(test)]` modules)
  encode the rules; a search optimization that breaks one of those is wrong,
  not "acceptably approximate."
- `cargo fmt` is enforced in CI (`cargo fmt --check`). `cargo clippy` runs in
  CI but is currently informational only (`continue-on-error: true`) — there
  is a pre-existing warning backlog; new code should not add to it even
  though CI won't block on it yet.

## Dev commands

```bash
cargo test --workspace              # unit tests (game rules, encoding round-trips, search)
cargo fmt --check                   # formatting (CI-blocking)
cargo clippy --workspace --all-targets -- -D warnings   # lints (CI-informational; fix what you touch)
cargo run --bin server              # HTTP server on :3000 (PORT env var to override)
cargo run --bin keres                # terminal hotseat game
cargo run --bin keres -- debug-tree --moves <base64> --full-tree   # search-tree debugging
```

No `docker` requirement for engine development — the toolchain runs
natively. `docker compose up --build` (see `compose.yaml`) is only for
running the built server standalone, e.g. to smoke-test `keres-platform`
against a local engine build.

## Testing via the HTTP API

```bash
cargo run --bin server &
curl -s http://localhost:3000/new --output /tmp/board.bin
curl -s -X POST --data-binary @/tmp/board.bin http://localhost:3000/moves --output /tmp/moves.bin
xxd /tmp/moves.bin   # inspect the returned PotentialMove list (see docs/PROTOCOL.md)
```

## Roadmap context

A native GUI binary (no browser, no keres-platform dependency) is planned —
see the README roadmap. When that work starts, it belongs in this repo as a
new binary target (e.g. `src/gui.rs` / a `gui` binary), reusing
`keres_engine`'s existing `Board`/`Game`/`MoveGenerator`/search — do not fork
game logic into a separate crate for it.
