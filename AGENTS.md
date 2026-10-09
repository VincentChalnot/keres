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

Four binaries share one library crate (`keres_engine`, `src/lib.rs`):

| Binary   | Entry point    | Purpose                                                        |
|----------|-----------------|------------------------------------------------------------------|
| `server` | `src/server.rs` | Thin `main`: binds a listener to `keres_engine::api::router` (routes live in `src/api.rs`; see `docs/PROTOCOL.md`) |
| `keres` | `src/main.rs`   | CLI: move listing, engine queries, search-tree debugging (plain text, no UI) |
| `arena` | `src/arena.rs`  | Engine-vs-engine harness for tuning the strength levels (`SearchConfig::for_level`): `match` (score, Elo estimate, game lengths/endings, king hangs) and `quality` (per-move score loss vs. the noise-free search) |
| `gui`   | `src/gui/main.rs` | Native minifb desktop GUI for hotseat or vs-AI play (behind the `gui` Cargo feature) |

## Layout

| Path                       | Role                                                                 |
|-----------------------------|-----------------------------------------------------------------------|
| `src/board.rs`               | `Board`, `Piece`, `Position`, binary encode/decode (the wire format) |
| `src/game.rs`                | `Game`: turn state, make/unmake moves, game-over tracking, `to_binary`/`from_binary` |
| `src/moves.rs`                | `Move`, `PotentialMove`, `MoveGenerator` — legal move generation per piece type |
| `src/game_over.rs`            | Win/draw condition checks (king capture, 50-move rule, insufficient material, etc.) |
| `src/cli_rendering.rs`        | Terminal board rendering + game-state hashing used by the CLI        |
| `src/engine/`                  | The AI: search, evaluation, types/config                              |
| `src/engine/search/`            | Negamax + alpha-beta (`alpha_beta.rs`, `negamax.rs`), quiescence, killer moves, loop/repetition detection, root entry point (`mod.rs::root_search`) |
| `src/engine/eval/`               | Position evaluation: material, mobility, king safety, pins, promotion, piece-square tables, tempo |
| `src/engine/tt.rs`               | Transposition table                                                    |
| `src/engine/constants.rs`         | Tunables: `MAX_DEPTH` (4), eval weights                                |
| `src/engine/tree_recorder.rs`      | Optional full search-tree recording for `debug-tree`: streamed JSONL or packed binary (format in `docs/SEARCH_TREE.md`) |
| `src/api.rs`                  | The HTTP API: route table, `ApiConfig`, strict payload decoding, body limits, search concurrency/timeouts, CORS + optional bearer auth. Lives in the library so `tests/api.rs` can drive it in-process |
| `src/server.rs`               | `main` only: binds a listener to `api::router`, graceful shutdown on SIGTERM/SIGINT |
| `src/main.rs`                 | `clap` CLI — subcommands: `show-moves`, `engine-move`, `debug-tree`, `openings` |
| `src/arena.rs`                | `clap` level-tuning harness — subcommands: `match`, `quality`; sides are `L<level>[:depth=,temp=,blunder=,bdepth=,slip=,qs=,killers=]` so a retune can be measured before it goes into `for_level`; `--record` writes JSONL game records. `scripts/arena_ladder.sh` runs every adjacent level pair |
| `src/gui/`                    | Native minifb GUI binary (`gui` target, `gui` Cargo feature): app state machine, software rasterizer, autosave — ported from micro-keres |
| `tests/`                      | Integration tests: `cli.rs`, `api.rs`, `api_robustness.rs` (deterministic fuzzing), `protocol_conformance.rs` (byte-level wire contract) |
| `scripts/smoke_test_server.sh`| End-to-end check against a *running* server; the only thing covering the shipped artifact's listener and `/health` |
| `docs/PROTOCOL.md`            | Wire protocol reference — regenerate/re-verify against `api.rs`/`board.rs`/`game.rs`/`moves.rs` if any of those change; do not let it drift |
| `docs/TESTING.md`             | Which test layer owns what, how to run each, and the CI job matrix (read before adding a test) |
| `docs/GUI.md`                 | GUI reference: canvas/layout model, pixel-art asset pipeline, and the headless snapshot workflow for checking a visual change (read it before touching `src/gui/`) |
| `docs/LEVELS.md`              | AI strength levels: what each `for_level` dial does, the arena tuning method, and the measured results behind the current table (update it whenever `LADDER` changes) |
| `docs/SEARCH_TREE.md`         | `debug-tree --full-tree` export: tree structure, score semantics, JSONL and binary record layouts (update it whenever `tree_recorder.rs` output changes) |

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
- **Anything a caller-supplied byte can reach must be fallible.** Every
  `from_*` decoder that the HTTP API or a save file can feed has a
  `try_from_*` twin, and `Game::try_make` validates a move before applying
  it. A malformed payload is a `400`/`409`, never a panic; `tests/
  api_robustness.rs` fuzzes every route asserting exactly that. Adding a
  panicking path on that surface is a security bug, not a style preference —
  see `SECURITY.md`.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`,
  `cargo doc -D warnings` and `cargo deny check` all block CI. The clippy
  backlog that used to force `continue-on-error` is cleared; do not
  reintroduce it. Every CI cargo invocation passes `--locked`, so a lockfile
  change has to be a visible diff in the PR.
- Read `docs/TESTING.md` before adding a test: four layers (unit, integration,
  smoke, workflow) each own different behaviour, and duplicating a rule check
  across them makes every future rule change cost three edits.

## Commit messages

- **Subject**: Conventional Commits, `type(scope): summary` — types `feat`,
  `fix`, `chore`, `docs`, `test`, `refactor`; scope is the area touched
  (`engine`, `api`, `gui`, `ci`, `cli`, `deps`, …), comma-separated when
  there are several. Lowercase, imperative, no trailing period, aim for
  ≤ 72 characters.
- **Body** (blank line after the subject, wrapped at ~76 columns): explain
  *why* — the symptom, the cause, the measurement — not a restatement of the
  diff. Use `- ` bullets for discrete changes, and short labelled sections
  (`Engine:`, `Tooling:`, `Docs:`) when a commit spans areas. Trivial
  changes may be subject-only.
- **One logical change per commit.** Mention behaviour changes visible to
  consumers (wire format, API levels, CLI flags) explicitly.
- Commits are GPG-signed. Keep any `Co-Authored-By:` trailers at the end.

## Dev commands

A `Makefile` encodes the correct profile + Cargo feature per binary — prefer it
(`make cli`, `make server`, `make arena`, `make gui`, `make all`, `make test`, `make check`,
`make deny`, `make smoke`, `make sizes`; run `make help` for the full list).
The GUI builds with the size-optimized `gui` profile (`opt-level="z"`, LTO,
`panic="abort"`, strip); the server/CLI build with `--release` (speed; AI
latency-critical). Raw cargo equivalents:

```bash
cargo test --workspace --features gui   # unit + integration tests (rules, encoding, search, API, CLI, GUI logic)
cargo fmt --check                   # formatting (CI-blocking)
cargo clippy --workspace --all-targets --features gui -- -D warnings   # lints (CI-blocking)
cargo deny check                    # RUSTSEC advisories, licenses, sources (CI-blocking; see deny.toml)
cargo run --bin server              # HTTP server on :3000 (PORT env var to override)
cargo run --bin keres -- engine-move  # ask the engine for its best move (plain-text CLI)
cargo run --bin keres -- debug-tree --moves <base64> --full-tree   # search-tree debugging
cargo run --release --bin arena -- match L7 L8:temp=3 --games 20 --random-plies 4   # level tuning
cargo run --profile gui --bin gui --features gui   # native GUI, size-optimized (or: make run-gui)
make smoke                          # build the server, start it, run the wire-protocol script against it
```

**Never launch the GUI to verify a visual change.** `make run-gui` opens a
real window on the maintainer's desktop, and a screenshot of that desktop
captures whatever else they have open. The binary has a headless
render-to-file path built for exactly this check — see
[`docs/GUI.md`](docs/GUI.md#verifying-changes-visually) § "Verifying changes
visually":

```bash
KERES_SNAPSHOT=/tmp/out.ppm KERES_SCREEN=menu ./target/gui/gui
magick /tmp/out.ppm -filter point -resize 300% /tmp/out.png   # nearest-neighbor, as the real window scales
```

It never opens a window or touches the display (so it works over SSH/CI),
and redirects save/settings I/O to throwaway temp dirs. `KERES_SCREEN`
selects the scenario (`splash`, `menu`, `playing`, `rules`, `load_game`,
`hover_*`, …); the authoritative list is `run_snapshot`'s doc comment in
`src/gui/main.rs`. Add `KERES_MOUSE=x,y` to check a hover state.

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

The native GUI binary (`gui`, `src/gui/main.rs`, behind the `gui` Cargo
feature) is a self-contained minifb desktop app ported from the micro-keres
contest build. It reuses `keres_engine`'s `Board`/`Game`/`MoveGenerator`/search
— game logic is never forked into a separate crate. The `gui` feature is
optional so the server/CLI (and the Docker server build) stay free of the
minifb/X11 dependency; build it with `cargo run --bin gui --features gui`.
