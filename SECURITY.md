# Security Policy

## Supported versions

Only the tip of `main` and the most recent tagged release are supported.
There are no long-lived maintenance branches; fixes land on `main` and are
shipped in the next tag and the next
`ghcr.io/vincentchalnot/keres/backend:latest` push.

## Reporting a vulnerability

Report privately through GitHub's **[Report a
vulnerability](https://github.com/vincentchalnot/keres/security/advisories/new)**
form (Security → Advisories). Do not open a public issue for anything that
lets a caller crash, hang, or read memory it should not.

Please include: the affected binary (`server`, `keres`, or `gui`), the commit
or image digest, and the smallest payload or command that reproduces the
problem. A reproducer as a byte string or a `curl --data-binary` invocation is
worth more than a description.

Expect an acknowledgement within a week. There is no bounty programme.

## Threat model

This engine is designed to sit **behind a trusted caller** — in practice
[`keres-platform`](https://github.com/VincentChalnot/keres-platform) — not to
be exposed directly to end users. That is a deployment assumption, so the
server does not rely on it:

- Every payload is length-checked and strictly decoded before it reaches any
  game logic. Off-board square indices, reserved bit patterns, boards with two
  kings, and illegal moves are all client errors (`400`/`409`), never panics.
- Request bodies are capped per route, far below axum's 2 MB default.
- The CPU-bound engine search runs on the blocking pool under a concurrency
  limit and a wall-clock timeout, so one caller cannot monopolise the runtime
  (`503` when the engine is saturated).
- A panic in a handler is caught and answered as `500` for that one request
  instead of dropping the connection.
- The crate contains no `unsafe` code.

What is **not** in the threat model:

- **Authentication is optional and off by default.** Set `KERES_API_TOKEN` to
  require `Authorization: Bearer <token>` on every route except `/health`.
- **CORS is wide open by default** (`Access-Control-Allow-Origin: *`). Set
  `KERES_CORS_ALLOWED_ORIGINS` to a comma-separated allow-list for any
  deployment reachable from a browser. Setting a token while leaving CORS open
  logs a warning at startup.
- **There is no transport security.** Terminate TLS in front of the server.
- **There is no rate limiting per client.** The concurrency limit bounds total
  engine work, not requests from a single origin; put a reverse proxy in front
  if that matters.
- **Engine strength and move choice are not a security boundary.** The
  `/engine-move-game/:level` endpoint trusts its level argument to the extent
  of validating the range only.

## Dependency hygiene

`cargo deny check` runs on every push and pull request (see `deny.toml` and
the `Dependency audit` job in `.github/workflows/ci.yaml`): RUSTSEC
advisories, yanked crates, the license allow-list, and a crates.io-only source
policy are all hard failures. Dependabot opens weekly update PRs for Cargo,
GitHub Actions, and the Docker base image.
