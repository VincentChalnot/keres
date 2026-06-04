# AGENTS.md — Keres Platform (Symfony + TypeScript)

## Scope

This agent operates exclusively inside the `platform/` directory, which is the
root of this workspace. Do **not** modify anything outside this directory.

| Path                      | Access         | Notes                                                                 |
|---------------------------|----------------|-----------------------------------------------------------------------|
| `platform/`               | Read + Write   | This workspace                                                        |
| `../docs/`                | Read-only      | General game rules and architecture — useful context for the renderer |
| `../engine/src/server.rs` | Read-only      | API endpoint signatures and binary payload shapes only                |
| `../engine/` (rest)       | **Off-limits** | Rust game engine, game rules, AI logic — never touch                  |
| `../` (rest)              | **Off-limits** | Monorepo root — irrelevant to this workspace                          |

## Project Overview

`platform/` is a **gameplay-agnostic** web platform for abstract board games.
It does **not** implement game rules. All game logic lives in the Rust engine
(`../engine/`), communicated via a binary HTTP API.

Platform responsibilities:

- User authentication and game session management (Symfony/PHP)
- Relaying moves to/from the Rust engine and persisting the resulting board tree
- Rendering the board state as SVG (TypeScript)
- Building analytics data structures (`BoardPosition` tree) for future ML use

When in doubt about game rules or piece behaviour, consult `../docs/` — but
**never let that knowledge leak into PHP business logic**. The platform must
remain playable with any abstract game whose engine respects the binary API
contract. Only the TypeScript renderer is allowed to hold game-specific
knowledge (piece names, SVG representations, movement descriptions).

## Stack

- **PHP 8.3+** / **Symfony 7.3** — FrankenPHP as application server
- **Doctrine ORM 3** — PostgreSQL, migrations in `migrations/`
- **Symfony Messenger** — async jobs via `async` transport
- **Symfony Mercure** — server-sent events pushed to the frontend
- **Vite + TypeScript** — vanilla TS, no framework, no Stimulus/Symfony UX
- **Docker Compose** — the sole dev environment, nothing runs on the host

## Architecture

### PHP / Symfony (`src/`)

| Directory                              | Role                                                    |
|----------------------------------------|---------------------------------------------------------|
| `src/Action/`                          | Symfony controllers (HTTP actions)                      |
| `src/Entity/`                          | Doctrine ORM entities                                   |
| `src/Model/`                           | Value objects and binary DTOs (no Doctrine)             |
| `src/Engine/`                          | Rust API bridge — stable, low-churn code                |
| `src/Service/`                         | Business logic services                                 |
| `src/Message/` + `src/MessageHandler/` | Symfony Messenger async jobs                            |
| `src/Event/`                           | Domain events (autoconfigured via `#[AsEventListener]`) |
| `src/Form/`                            | Symfony forms                                           |
| `src/Security/`                        | Authentication / authorization                          |
| `src/Repository/`                      | Doctrine repositories                                   |
| `src/Command/`                         | Symfony console commands                                |

### TypeScript / Vite (`assets/`)

| File / Directory                    | Role                                                                            |
|-------------------------------------|---------------------------------------------------------------------------------|
| `assets/typescript/src/app.ts`      | Main entry point — wires all components                                         |
| `src/models/types.ts`               | Core domain types: `Board`, `Piece`, `Move`, `PotentialMove`, `TileState`       |
| `src/utils/boardUtils.ts`           | **All** binary encode/decode functions — single source of truth for wire format |
| `src/controllers/GameController.ts` | Central mediator: clicks, drag, API calls, Mercure events                       |
| `src/models/GameState.ts`           | In-memory reactive state                                                        |
| `src/network/GameAPI.ts`            | HTTP client (`/api`, `application/octet-stream`)                                |
| `src/network/MercureClient.ts`      | Mercure SSE subscription                                                        |
| `src/views/IBoardView.ts`           | Renderer interface                                                              |
| `src/views/SVGBoardView.ts`         | **Active renderer** — SVG, inline sprite sheet                                  |
| `src/views/ThreeJSBoardView.ts`     | Inactive renderer — do not modify unless explicitly asked                       |

## Domain Model

### Key Entities

```
BoardPosition   — unique board state (81-byte binary key, globally deduped across all games)
    ↑ fromBoardPosition / toBoardPosition
Move            — directed edge in the board tree (2-byte moveData)
    ↑ move
GameMove        — one move within a specific Game, pointing into the shared Move tree
    ↑ gameMoves
Game            — a game session (owner, opponent type, game-over state, optimistic lock)
User            — authenticated player
UserAuth        — OAuth provider binding (provider + providerId, unique pair)
```

`BoardPosition` + `Move` form a **shared, append-only tree across all games**
used for analytics and future neural network training. `Game` + `GameMove`
reference into that tree — never duplicate board or move data.

### Binary Wire Format (PHP ↔ Rust engine)

All engine communication uses raw binary over HTTP (`Content-Type: application/octet-stream`).
Do **not** introduce JSON serialization on this path.

**Board state — 83 bytes** (`BoardData` PHP / `Board` TS):

| Bytes | Content                                                                                |
|-------|----------------------------------------------------------------------------------------|
| 0–80  | 81 squares of the 9×9 board (one byte per cell, piece encoding owned by the engine)    |
| 81    | Flags (big-endian): `0x80` whiteToMove, `0x40` gameOver, `0x20` whiteWins, `0x10` draw |
| 82    | `movesWithoutCapture` counter (uint8, 50-move rule)                                    |

**Move — 2 bytes** (`MoveData`): opaque blob, no client-side parsing beyond storage.

**PHP serialization** lives in `src/Model/` — `BoardData`, `MoveData`, `MovesData`, `BoardMovesData`.  
**TypeScript codecs** live exclusively in `assets/typescript/src/utils/boardUtils.ts` — all
`encode*` / `decode*` functions must stay there and nowhere else.

> ⚠️ **Known technical debt**: `src/Model/` mixes DTO concerns with
> serialization/deserialization logic. Do not worsen it. If you touch this area,
> prefer moving serialization into dedicated classes.

## Engine API Bridge (`src/Engine/`)

Two endpoints, both `POST`, binary in/out, base URL injected via `$backendApiUrl`
(env var `BACKEND_API_URL`):

| Endpoint            | Request                       | Response               |
|---------------------|-------------------------------|------------------------|
| `/replay-moves`     | `MovesData` binary (2N bytes) | 83 bytes → `BoardData` |
| `/engine-move-game` | `MovesData` binary (2N bytes) | 2 bytes → `MoveData`   |

- `EngineApi` — makes raw HTTP calls
- `GameEngine` — consumes results, updates `Game` entity, handles game-over detection
- `BoardTreeManager` — deduplicates `BoardPosition` rows using the 81-byte position key

`src/Engine/` is in scope but is stable and low-churn. Be conservative here.

## Dev Environment

**Everything runs in Docker. Never run PHP or npm commands on the host.**

### Start the environment

```bash
docker compose -f compose.yaml -f compose.override.yaml up --build -d --remove-orphans --force-recreate
```

### PHP commands (inside the PHP container)

```bash
docker compose exec php bin/console <command>

# Common examples:
docker compose exec php bin/console doctrine:migrations:migrate
docker compose exec php bin/console doctrine:migrations:diff
docker compose exec php bin/console debug:router
docker compose exec php bin/console messenger:consume async
docker compose exec php bin/console cache:clear
```

### TypeScript / Node commands (inside the Node container)

```bash
docker compose exec node npm run dev         # Vite HMR dev server (local.playkeres.com:5173)
docker compose exec node npm run build       # Production build → public/build/
docker compose exec node npm run type-check  # TypeScript strict check (must pass, no emit)
```

### Code style — PHP (run locally, no container needed)

```bash
composer cs:check   # Dry-run — shows violations without modifying files
composer cs:fix     # Applies PHP CS Fixer fixes in place
```

Run `composer cs:check` before considering any PHP task complete.
Run `composer cs:fix` to auto-correct style issues.

## Conventions

### PHP

- **PSR-12** coding standard enforced by PHP CS Fixer
- **PHP 8 attributes** for all Doctrine mappings, Symfony routing, security,
  and Messenger configuration — no annotations, no YAML mappings for these
- Autowiring everywhere — no explicit service declarations in `services.yaml`
- Constructor argument named `$backendApiUrl` receives `BACKEND_API_URL`
  automatically via the global `bind` in `services.yaml`
- `Game` uses Doctrine optimistic locking (`@Version`) — be aware when updating
  `Game` outside of `GameEngine` (which handles the manual version increment)
- Messenger: `ProcessAiMoveMessage` → `async` transport; everything else → `sync`

### TypeScript

- **Strict mode** — `tsc --noEmit` must return zero errors before any task is done
- Vanilla TypeScript with Vite — no Stimulus, no Symfony UX, no frontend framework
- `Board` (in `src/models/types.ts`) is the canonical board state type
- All binary codec lives exclusively in `src/utils/boardUtils.ts` — do not add
  `encode*` / `decode*` logic anywhere else

## Async & Real-time Flow

```
User action → GameController → GameAPI (HTTP /api, binary)
                          ↓
    Symfony Action → GameEngine → EngineApi → Rust
                          ↓
                  Mercure hub (SSE)
                          ↓
MercureClient → GameController → GameState → SVGBoardView
```

AI moves are dispatched as `ProcessAiMoveMessage` to the `async` transport and
processed by a Messenger consumer, then pushed to the frontend via Mercure.
