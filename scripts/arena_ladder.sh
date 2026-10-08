#!/usr/bin/env bash
# Play every adjacent pair of AI levels against each other with the `arena`
# binary and keep the logs and game records — the measurement behind a
# `SearchConfig::for_level` retune (target: each level scores ~70% against
# the one below).
#
# Usage: scripts/arena_ladder.sh [OUT_DIR] [ARENA_BIN]
#
#   OUT_DIR    where per-pair reports, per-game logs and games.jsonl go
#              (default: arena-runs/<timestamp>)
#   ARENA_BIN  arena binary (default: target/release/arena; `make arena`)
#
# Environment:
#   LEVELS        space-separated ladder to walk (default: "1 2 3 4 5 6 7 8 9 10")
#   GAMES_SHALLOW games per pair when both sides search <= depth 3 (default 200)
#   GAMES_DEEP    games per pair otherwise (default 100)
#   RANDOM_PLIES  random opening plies shared by each colour-swapped pair
#                 (default 4; needed for the deterministic top level)
#
# Pairs run one after another: each search already uses every core.
# Detach it for a long run, e.g.
#   setsid nohup scripts/arena_ladder.sh arena-runs/night >/dev/null 2>&1 &
set -euo pipefail

out=${1:-arena-runs/$(date +%Y%m%d-%H%M%S)}
arena=${2:-target/release/arena}
levels=(${LEVELS:-1 2 3 4 5 6 7 8 9 10})
games_shallow=${GAMES_SHALLOW:-200}
games_deep=${GAMES_DEEP:-100}
random_plies=${RANDOM_PLIES:-4}

mkdir -p "$out"
summary="$out/summary.txt"
: >"$summary"

for ((i = 0; i + 1 < ${#levels[@]}; i++)); do
	lo=${levels[i]}
	hi=${levels[i + 1]}
	# Levels 1-5 search at most 3 plies (see `for_level`); deeper pairs are
	# ~7x slower per move, so they get fewer games.
	if ((hi <= 5)); then games=$games_shallow; else games=$games_deep; fi
	echo "== L$hi vs L$lo ($games games) — started $(date '+%F %T')" | tee -a "$summary"
	"$arena" match "L$hi" "L$lo" --games "$games" --random-plies "$random_plies" \
		--record "$out/games.jsonl" \
		2>"$out/L$hi-vs-L$lo.games.log" | tee "$out/L$hi-vs-L$lo.txt" >>"$summary"
	echo >>"$summary"
done
echo "== done $(date '+%F %T')" >>"$summary"
