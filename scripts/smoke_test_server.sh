#!/usr/bin/env bash
# End-to-end smoke test for a *running* `server` binary (native or the
# container image). Unit and integration tests drive `api::router` in-process;
# this is the only check that the shipped artifact actually binds a port,
# speaks the binary protocol over real HTTP, and answers the health probe an
# orchestrator will use.
#
#   scripts/smoke_test_server.sh [BASE_URL]
#
# BASE_URL defaults to http://127.0.0.1:3000. Exits non-zero on the first
# failed expectation.
#
# The protocol constants below are duplicated from docs/PROTOCOL.md on
# purpose: if the wire format changes, this script must be updated as
# deliberately as any other external consumer would have to be.

set -euo pipefail

BASE_URL="${1:-http://127.0.0.1:3000}"

GAME_BYTES=83
MOVE_BYTES=2

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

fail() {
    echo "SMOKE FAIL: $*" >&2
    exit 1
}

ok() {
    echo "  ok: $*"
}

# Byte size of a file, portable across GNU/BSD stat.
size_of() {
    wc -c <"$1" | tr -d ' '
}

# GET/POST returning the HTTP status on stdout and the body into $2.
http_get() {
    curl -sS -o "$2" -w '%{http_code}' "$BASE_URL$1"
}

http_post() {
    curl -sS -o "$3" -w '%{http_code}' -X POST \
        -H 'Content-Type: application/octet-stream' \
        --data-binary "@$2" "$BASE_URL$1"
}

echo "Smoke-testing $BASE_URL"

# --- /health: the orchestrator's liveness probe ---------------------------
status="$(http_get /health "$tmp/health")"
[ "$status" = "200" ] || fail "/health returned $status"
[ "$(cat "$tmp/health")" = "ok" ] || fail "/health body was '$(cat "$tmp/health")'"
ok "/health"

# --- /new: a fresh 83-byte game -------------------------------------------
status="$(http_get /new "$tmp/game.bin")"
[ "$status" = "200" ] || fail "/new returned $status"
[ "$(size_of "$tmp/game.bin")" = "$GAME_BYTES" ] ||
    fail "/new returned $(size_of "$tmp/game.bin") bytes, expected $GAME_BYTES"
ok "/new -> $GAME_BYTES bytes"

# --- /moves: a non-empty, well-sized PotentialMove list --------------------
status="$(http_post /moves "$tmp/game.bin" "$tmp/moves.bin")"
[ "$status" = "200" ] || fail "/moves returned $status"
moves_bytes="$(size_of "$tmp/moves.bin")"
[ "$moves_bytes" -gt 0 ] || fail "/moves returned no moves for the start position"
[ $((moves_bytes % MOVE_BYTES)) -eq 0 ] ||
    fail "/moves returned $moves_bytes bytes, not a multiple of $MOVE_BYTES"
ok "/moves -> $((moves_bytes / MOVE_BYTES)) moves"

# --- /play: first PotentialMove, played as a whole-stack move -------------
# Clear bits 14-15 (unstackable/force_unstack) to turn the PotentialMove into
# the plain Move the server expects; the first move of the game is never a
# forced unstack, since the start position has no stacks.
head -c "$MOVE_BYTES" "$tmp/moves.bin" >"$tmp/move.bin"
low="$(od -An -tu1 -N1 -j0 "$tmp/move.bin" | tr -d ' ')"
high="$(od -An -tu1 -N1 -j1 "$tmp/move.bin" | tr -d ' ')"
printf "$(printf '\\%03o\\%03o' "$low" "$((high & 0x3F))")" >"$tmp/move.bin"
cat "$tmp/game.bin" "$tmp/move.bin" >"$tmp/play.bin"

status="$(http_post /play "$tmp/play.bin" "$tmp/after.bin")"
[ "$status" = "200" ] || fail "/play returned $status"
[ "$(size_of "$tmp/after.bin")" = "$GAME_BYTES" ] ||
    fail "/play returned $(size_of "$tmp/after.bin") bytes, expected $GAME_BYTES"
cmp -s "$tmp/game.bin" "$tmp/after.bin" && fail "/play did not change the game state"
ok "/play -> $GAME_BYTES bytes, state advanced"

# --- /replay-moves: the same move, replayed from the initial position -----
status="$(http_post /replay-moves "$tmp/move.bin" "$tmp/replayed.bin")"
[ "$status" = "200" ] || fail "/replay-moves returned $status"
cmp -s "$tmp/after.bin" "$tmp/replayed.bin" ||
    fail "/replay-moves disagrees with /play for the same move"
ok "/replay-moves matches /play"

# --- engine: a real search must return one legal move ---------------------
status="$(http_post /engine-move-game/1 "$tmp/move.bin" "$tmp/engine.bin")"
[ "$status" = "200" ] || fail "/engine-move-game/1 returned $status"
[ "$(size_of "$tmp/engine.bin")" = "$MOVE_BYTES" ] ||
    fail "/engine-move-game/1 returned $(size_of "$tmp/engine.bin") bytes"
ok "/engine-move-game/1 -> $MOVE_BYTES bytes"

# --- error handling: hostile input must not be a 5xx ----------------------
printf 'not a game' >"$tmp/garbage.bin"
status="$(http_post /moves "$tmp/garbage.bin" "$tmp/ignored.bin")"
[ "$status" = "400" ] || fail "/moves on garbage returned $status, expected 400"
ok "/moves rejects garbage with 400"

status="$(http_post /engine-move-game/99 "$tmp/move.bin" "$tmp/ignored.bin")"
[ "$status" = "400" ] || fail "/engine-move-game/99 returned $status, expected 400"
ok "/engine-move-game rejects an out-of-range level with 400"

status="$(http_get /does-not-exist "$tmp/ignored.bin")"
[ "$status" = "404" ] || fail "unknown route returned $status, expected 404"
ok "unknown route -> 404"

echo "Smoke test passed."
