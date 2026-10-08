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
- **HTTP server** (`src/api.rs` + `src/server.rs`, binary target `server`):
  the binary wire API consumed by keres-platform. See
  [`docs/PROTOCOL.md`](docs/PROTOCOL.md) for the exact byte layout and
  [`SECURITY.md`](SECURITY.md) for what it does and does not defend against.
- **CLI** (`src/main.rs`, binary target `keres`): inspect legal moves, ask the
  engine for a move, or dump a full search tree for debugging — plain text,
  no UI.
- **Native GUI** (`src/gui/`, Cargo bin target `gui`, behind the `gui`
  feature): a self-contained [minifb](https://github.com/emoon/minifb)
  desktop app for hotseat or vs-AI play — no browser, no server. Build with
  `cargo build --bin gui --features gui`; the file it ships as (a GitHub
  Release download, or `make gui`'s output) is named `keres` /
  `keres.exe`, or `Keres.app` on macOS — see `make macos-app` and
  `scripts/package_macos_app.sh`.
- **Tests** (`src/**` unit modules, `tests/`, `scripts/smoke_test_server.sh`):
  the game rules, the wire format's byte layout, the CLI's behaviour, and a
  deterministic fuzz suite that asserts no byte string can make the API panic.
  `make check` runs the lot; see [`docs/TESTING.md`](docs/TESTING.md).

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

The ten strength levels (`level` on the API) and how they were tuned with the
`arena` binary are documented in [`docs/LEVELS.md`](docs/LEVELS.md).

## Build & run

Requires a stable Rust toolchain (see `Cargo.toml` for the edition).

A `Makefile` builds each binary with the profile and features that suit it:

- `make server` / `make cli` / `make arena` — `--release` (speed; the AI search is latency-critical)
- `make gui` — the size-optimized `gui` profile (`opt-level="z"`, fat LTO,
  `panic="abort"`, strip) → ~577 KB (the micro-keres <1.44 MB lineage)
- `make all` / `make test` / `make check` / `make sizes` — run `make help` for all

Or the equivalent cargo commands directly:

```bash
# HTTP server (the binary wire API, see docs/PROTOCOL.md)
cargo run --bin server
# PORT env var selects the listen port (default 3000)

# List legal moves for a board (all squares, or one position)
cargo run --bin keres -- show-moves [--board <base64>] [coordinates]

# Ask the engine for its move on a board
cargo run --bin keres -- engine-move [--board <base64>]

# Dump the search tree (JSONL) for a move sequence — tuning/debugging
cargo run --bin keres -- debug-tree [--moves <base64>] [--full-tree] \
  [--max-depth N] [--no-tt] [--no-ab] [--no-quiescence] [--no-killers]

# Engine-vs-engine arena for tuning the strength levels (SearchConfig::for_level).
# A side is a level preset plus optional overrides (depth, temp, blunder, bdepth, slip, qs, killers).
cargo run --release --bin arena -- match L7 L8:temp=3,blunder=0.01 --games 20 [--random-plies 4] [--record games.jsonl]
# Every adjacent level pair, logs + JSONL game records under arena-runs/ (long: run it detached)
scripts/arena_ladder.sh
# Per-move score loss of one configuration vs. its own noise-free search
cargo run --release --bin arena -- quality L5 --games 4

# Native desktop GUI (hotseat or vs-AI), size-optimized via the `gui` profile.
# The `gui` Cargo feature pulls in minifb; it's optional so the server/CLI stay
# free of the X11 dependency. (`make gui` / `make run-gui` do the same.)
cargo run --profile gui --bin gui --features gui
```

```bash
# Test / lint (also run in CI, see .github/workflows/ci.yaml)
cargo test --workspace --features gui
cargo fmt --check
cargo clippy --workspace --all-targets --features gui
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

- **Native GUI** — shipped as the `gui` binary (`src/gui/`, minifb): play
  hotseat or against the AI in a self-contained desktop app, no browser or
  server required.
- Adjustable AI difficulty (currently fixed at depth 4).

## License

GPLv3 — see [`LICENSE`](LICENSE). This engine is, and will remain, open
source: it's the entry point for anyone who wants to inspect the rules
implementation, embed the engine elsewhere, or build their own client
against the wire protocol. `keres-platform` (the web app) and
`keres-website` (the marketing site) are proprietary.

*Solo project by [Vincent Chalnot](https://github.com/VincentChalnot).*
