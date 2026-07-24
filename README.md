# Keres — Game Engine

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![License: GPLv3](https://img.shields.io/badge/License-GPLv3-blue.svg)
![Live](https://img.shields.io/badge/status-live-brightgreen)

> A Rust engine for **Keres**, an original abstract strategy game: rules
> simpler than chess, tactical depth from a stacking mechanic that has no
> chess equivalent. This engine is the single source of truth for the
> rules — the [web platform](https://github.com/VincentChalnot/keres-platform)
> never interprets game state, it only stores and forwards what this engine
> produces.

🎮 **[Play online at playkeres.com](https://playkeres.com)** — powered by
this engine plus [keres-platform](https://github.com/VincentChalnot/keres-platform)
(Symfony/TypeScript) and [keres-website](https://github.com/VincentChalnot/keres-website)
(Hugo marketing site). See [`playkeres.com/rules`](https://playkeres.com/rules)
for the full illustrated rules.

---

## What's here

- **Game logic** (`src/board.rs`, `src/game.rs`, `src/moves.rs`,
  `src/game_over.rs`): board representation, legal move generation, stacking
  rules, promotion, draw/win detection.
- **AI engine** (`src/engine/`): Negamax with alpha-beta pruning, quiescence
  search, a transposition table, killer-move ordering, and
  loop/repetition-aware search, parallelized with
  [Rayon](https://github.com/rayon-rs/rayon).
- **HTTP server** (`src/server.rs`, binary target `server`): the binary wire
  API consumed by keres-platform. See [`docs/PROTOCOL.md`](docs/PROTOCOL.md)
  for the exact byte layout.
- **CLI / TUI** (`src/main.rs`, `src/tui.rs`, binary target `keres`): play a
  hotseat game in the terminal, inspect legal moves, ask the engine for a
  move, or dump a full search tree for debugging.

## The AI

```
Search:          Negamax + alpha-beta pruning
Depth:           4 ply (MAX_DEPTH, src/engine/constants.rs)
Quiescence:      enabled past the horizon
Move ordering:   MVV-LVA + killer moves
Transposition:   hash table (src/engine/tt.rs)
Parallelism:     Rayon work-stealing thread pool
Response time:   ~200ms on a modern CPU, ~2-3s on a 2 vCPU VPS
```

At depth 4 the engine makes zero tactical errors and independently converges
on opening lines that experienced human players discover. MCTS (plain or
AlphaZero-style) was evaluated and rejected — see the root project README's
history for why: Keres's stacking mechanic doesn't guarantee game
termination under random play, which breaks vanilla MCTS rollouts, and a
trained policy/value network is a separate project the current deterministic
Negamax search already outperforms for this game size.

## Build & run

Requires a stable Rust toolchain (see `Cargo.toml` for the edition).

```bash
# HTTP server (the binary wire API, see docs/PROTOCOL.md)
cargo run --bin server
# PORT env var selects the listen port (default 3000)

# Terminal hotseat game (TUI)
cargo run --bin keres
# equivalent: cargo run --bin keres -- play
# resume a saved game: cargo run --bin keres -- play --board <base64 Game>

# List legal moves for a board (all squares, or one position)
cargo run --bin keres -- show-moves [--board <base64>] [coordinates]

# Ask the engine for its move on a board
cargo run --bin keres -- engine-move [--board <base64>]

# Dump the search tree (JSONL) for a move sequence — tuning/debugging
cargo run --bin keres -- debug-tree [--moves <base64>] [--full-tree] \
  [--max-depth N] [--no-tt] [--no-ab] [--no-quiescence] [--no-killers]
```

```bash
# Test / lint (also run in CI, see .github/workflows/ci.yaml)
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets
```

### Docker

```bash
docker compose up --build
# server listening on http://localhost:3000 (BACKEND_PORT to override the host port)
```

The Dockerfile builds a fully static `x86_64-unknown-linux-musl` binary into
a `scratch` image — no libc, no shell, nothing but the `server` binary. CI
publishes it to `ghcr.io/vincentchalnot/keres/backend` on every push to
`main` (see `.github/workflows/ci.yaml`).

## Roadmap

- **Native GUI** — a standalone binary (no browser, no platform/server
  dependency) for people who want to play Keres against the AI without going
  through playkeres.com. Planned; not started.
- Adjustable AI difficulty (currently fixed at depth 4).

## License

GPLv3 — see [`LICENSE`](LICENSE). This engine is, and will remain, open
source: it's the entry point for anyone who wants to inspect the rules
implementation, embed the engine elsewhere, or build their own client
against the wire protocol. `keres-platform` (the web app) and
`keres-website` (the marketing site) are proprietary.

*Solo project by [Vincent Chalnot](https://github.com/VincentChalnot).*
