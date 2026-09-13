# Testing

Four layers, each with a different job. When you add a test, put it in the
layer that owns the behaviour — duplicating a rule check at three levels buys
nothing and makes every future rule change three times as expensive.

| Layer | Location | Runs | What it owns |
|---|---|---|---|
| Unit | `#[cfg(test)]` modules in `src/**` | `cargo test` | Game rules, encoding round-trips, search invariants, GUI state machine |
| Integration | `tests/*.rs` | `cargo test` | The CLI's observable behaviour and the HTTP API's contract, driven from outside the module |
| Smoke | `scripts/smoke_test_server.sh` | `make smoke`, CI's image job | The *shipped artifact*: a real process, a real socket, real HTTP |
| Workflow | `.github/workflows/ci.yaml` | GitHub Actions | Cross-platform builds, dependency audit, container build |

```bash
make test          # everything, incl. the GUI feature's unit tests
make test-core     # skip the minifb feature
make check         # fmt + clippy (-D warnings) + test
make deny          # dependency audit (RUSTSEC, licenses, sources)
make smoke         # build the server, start it, run the wire-protocol script
cargo test --test api                    # one integration binary
cargo test --test api_robustness
cargo test --test protocol_conformance
cargo test --test cli
```

## Unit tests

Rules live with the code that implements them: `src/board.rs`, `src/moves.rs`,
`src/game.rs`, `src/game_over.rs`. These are the authority on what the game
*is*. A search optimisation that breaks one of them is wrong, not
"acceptably approximate" (see `AGENTS.md`).

Two properties are worth protecting deliberately:

- **Decoders are total.** Every `from_*` that a caller-supplied byte can reach
  has a fallible `try_from_*` twin, and the exhaustive sweeps in `src/board.rs`
  and `src/moves.rs` assert that `try_from_u8`/`try_from_u16` accept *exactly*
  the encodable values — not merely that valid input round-trips. That is what
  stops a new bit in the wire format from silently becoming a panic.
- **The generator never produces a move it cannot expand.** `to_moves()`
  panics on a `force_unstack` without `unstackable`; `moves.rs` sweeps a stack
  across the board asserting the generator never emits that combination, and
  that no position generates the same `Move` twice.

## Integration tests

`tests/` drives the binaries and the library from outside.

- `tests/common/mod.rs` — shared helpers. The API tests run the router
  **in-process** through `tower::ServiceExt::oneshot`: no socket is bound, no
  port is chosen, nothing races another test binary. This is why the route
  table lives in `keres_engine::api` and `src/server.rs` is only a `main` that
  binds a listener to it.
- `tests/cli.rs` — the `keres` CLI's exit codes, stdout/stderr split, and
  argument validation.
- `tests/api.rs` — the documented contract of every route: status codes,
  body sizes, CORS, bearer-token auth, body limits, routing.
- `tests/api_robustness.rs` — deterministic fuzzing. Arbitrary bytes into
  every endpoint must produce a 4xx/503, never a `500`, never a panic, never a
  hang. The PRNG is a fixed-seed xorshift written inline, so a failure is
  reproducible from the test name alone.
- `tests/protocol_conformance.rs` — the byte-level contract in
  `docs/PROTOCOL.md`, asserted against hardcoded expectations rather than
  against the encoder. `keres-platform` has independent PHP and TypeScript
  codecs for this format; a change that these tests do not notice is a change
  that silently breaks a consumer this repo cannot see.

### Conventions

- `#[tokio::test]` for anything touching the router.
- Name the test after the behaviour it asserts, not the function it calls:
  `play_rejects_a_move_from_an_empty_square`, not `test_play_2`.
- Assert what a *consumer* observes — status, bytes, headers. Never assert
  internal wiring, field copies, or default values.
- Engine searches cost seconds. Call them a handful of times per binary and
  prefer `/engine-move-game/1` where a search is unavoidable.

## Smoke test

`scripts/smoke_test_server.sh [BASE_URL]` exercises a *running* server over
real HTTP: `/health`, `/new`, `/moves`, `/play`, `/replay-moves`, a real
engine search, and three error cases. Nothing else in the suite covers the
musl static build, the listener, or the health probe an orchestrator depends
on. CI runs it against the built container image; `make smoke` runs it against
a locally built binary on port 3999.

The protocol constants are duplicated in that script on purpose: if the wire
format changes, it has to be updated as deliberately as any other external
consumer would.

## GUI

The GUI renders into a software framebuffer and has no assertions on pixels.
`KERES_SNAPSHOT=<file>.ppm KERES_SCREEN=<screen> ./target/gui/gui` renders one
screen headlessly and exits without opening a window (full screen list: the
doc comment on `run_snapshot` in `src/gui/main.rs`). CI renders five screens on
Linux, macOS and Windows and uploads them as artifacts — enough to catch a
layout routine that panics or a platform-specific build break, not enough to
catch a visual regression.

**Never run `make run-gui` to check a visual change**: it opens a window on the
maintainer's desktop. See `docs/GUI.md` § "Verifying changes visually".

## Continuous integration

`.github/workflows/ci.yaml`:

| Job | Blocking | What it catches |
|---|---|---|
| `Lint workflows` | yes | `actionlint` + `zizmor` over `.github/workflows` (configs in `.github/linters/`) |
| `Test` | yes | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `cargo doc -D warnings` |
| `Dependency audit` | yes | `cargo deny check` — RUSTSEC advisories, yanked crates, licenses, crates.io-only sources |
| `Build GUI` (3 OS) | yes | Cross-platform compile + headless screen snapshots |
| `Build backend image` | yes | Docker build **and** the wire-protocol smoke test against the running container |
| `Push backend image` | main only | Publishes `:latest` and `:<sha>` to GHCR |

Every cargo invocation passes `--locked`: a lockfile change must be a visible
diff in the PR, not something CI resolves differently from the developer's
machine.

### Repository settings this expects

The workflow file cannot enforce these; they live in GitHub's settings:

- **Branch protection on `main`** requiring `Test`, `Lint workflows`,
  `Dependency audit`, `Build GUI (ubuntu-latest | windows-latest |
  macos-latest)` and `Build backend image` to pass, plus a linear history and
  signed commits.
- **Dependabot alerts** and **Dependabot security updates** enabled, so RUSTSEC
  advisories open PRs instead of only failing the audit job.
- **Private vulnerability reporting** enabled (`SECURITY.md` links to the
  advisory form).
