# AI strength levels

The engine exposes ten strength levels (`level` 1..=10 on the API, see
[`PROTOCOL.md`](PROTOCOL.md)). Each level is one row of the `LADDER` table in
`SearchConfig::for_level` (`src/engine/types.rs`). This document explains what
the dials mean, how the table was tuned, and the measurements behind the
current values.

## Design goals

- **Even steps.** Each level should score roughly 65–70% against the one
  below (≈ 100–150 Elo), so moving up one level is a noticeable but
  beatable step.
- **Realistic mistakes.** Weak levels lose the way people lose: by
  choosing among plausible moves and by occasionally seeing too little, not
  by playing random moves.
- **Decided positions are respected.** From L4 up, an available win is always
  taken by the fastest route and a lost position is defended for as long as
  possible. Below L4 a small "slip" chance is allowed, so beginners can
  occasionally escape.
- **L10 is the full-strength engine** (`MAX_DEPTH`, no noise), identical to a
  request without a level. Depth 5 was dropped: ~7× the cost per move for a
  strength nobody on the platform needed.

## The dials

| Dial | `SearchConfig` field | Effect |
|---|---|---|
| Depth | `max_depth` | Plies searched. Quiescence and killer moves are enabled from depth 3. |
| Noise | `noise_temperature` | Softmax temperature over root move scores (eval points; a soldier is 10). 0 = always the best move. Scores are clamped to ±`DECIDED_SCORE` first, so noise can never pick a king-losing move while a safe one exists. |
| Blunder | `blunder_chance`, `blunder_depth` | Probability of answering from a shallower search (`blunder_depth` plies) instead of the full one: a plausible-looking oversight, not a random move. |
| Slip | `decided_slip_chance` | In a decided position (`|best| >= DECIDED_SCORE`) the best move is played unless this roll hits, in which case the normal noisy choice is used. 0 from L4 up. |

The search also knows every game-ending rule (`src/engine/search/outcome.rs`):
king capture and annihilation are wins, the 40-move rule and insufficient
material are draws. Wins are scored as `KING_VALUE - ply`, so the fastest win
and the longest defence fall out of a plain argmax.

## Method

All measurements use the `arena` binary (`src/arena.rs`), engine against
engine:

```bash
# One pair; overrides let a candidate be measured before it goes into for_level.
arena match L5:temp=5,blunder=0.07 L4 --games 150 --random-plies 4
# Every adjacent pair of the current table (long: run it detached).
GAMES_SHALLOW=150 GAMES_DEEP=100 scripts/arena_ladder.sh runs/ladder ./arena
```

1. **Random openings.** Deterministic sides would replay the same two games
   forever. `--random-plies 4` starts each colour-swapped pair of games from
   a shared random 4-ply opening, so colour and opening luck cancel out.
2. **Measure adjacent pairs.** Each pair plays 100–150 games. With that
   sample the 95% interval on an Elo difference is about ±50, so steps that
   differ by less than that are indistinguishable; do not chase them.
3. **Check the diagnostics, not only the score.** `match` also reports how
   often each side left its king en prise (split into *avoidable*, *already
   lost* by its own noise-free search, and *unseen*) and how often it missed
   an immediate win. From L4 up both "avoidable" and "missed" must be 0.
4. **Fix the outliers with overrides.** Probe candidate settings for a
   mis-sized step against *both* neighbours with `L<n>:temp=…,blunder=…`
   overrides, then write the winner into `LADDER`. Changing one level moves
   two steps.
5. **Interpolate, then verify.** Near the top, Elo lost is roughly linear in
   noise (≈ 80 Elo per temperature point at depth 4, blunders included), but
   the slope is steep and noisy: bracket the target with two probes, pick the
   midpoint, and measure it directly before committing.
6. **Pick depth by cost.** A depth-4 game costs ~6× a depth-3 one. Levels that
   need lots of noise at depth 4 are better served by depth 3 with less noise
   (cheaper on the server, and the mistakes look more natural).

Hardware note: on a Ryzen 9 5900X a depth-3 pair of 150 games takes ~20 min,
a depth-4 pair of 100 games ~65 min. Two depth-4 matches run side by side
each slow to about one game per minute.

What did *not* work: a uniformly random "L0" floor. Any search beats it
100%, which yields no Elo estimate, so it cannot anchor the scale. L10 is the
arena's reference point; absolute ratings come from the platform's Glicko
ratings against humans.

## Current table and results

Measured 2026-10-08 on evangelion (Ryzen 9 5900X); each step is the level's
Elo over the level directly below it, against the final settings of both.

| Level | Depth | Noise | Blunder (depth) | Slip | Step | Games | Score | 95% interval | ms/move |
|---|---|---|---|---|---|---|---|---|---|
| L1 | 2, no quiescence | 4 | 10% (1) | 20% | — | — | — | — | ~50 |
| L2 | 3 | 10 | 15% (1) | 10% | +458 | 150 | 93.3% | +391 .. +562 | ~85 |
| L3 | 3 | 8 | 12% (1) | 3% | +156 | 150 | 71.0% | +113 .. +204 | ~60 |
| L4 | 3 | 7 | 9% (2) | 0 | +100 | 150 | 64.0% | +59 .. +144 | ~55 |
| L5 | 3 | 5 | 7% (2) | 0 | +92 | 150 | 63.0% | +52 .. +136 | ~55 |
| L6 | 3 | 3 | 5% (2) | 0 | +111 | 120 | 65.4% | +66 .. +159 | ~55 |
| L7 | 3 | 1.5 | 3% (2) | 0 | +139 | 100 | 69.0% | +89 .. +196 | ~55 |
| L8 | 4 | 4 | 4% (3) | 0 | +115 | 100 | 66.0% | +63 .. +174 | ~240 |
| L9 | 4 | 2.5 | 1.5% (3) | 0 | +120 | 120 | 66.7% | +62 .. +186 | ~330 |
| L10 | 4 | 0 | 0 | 0 | +144 | 120 | 69.6% | +87 .. +209 | ~330 |

L2 → L10 spans about 1,100 Elo. Observations:

- **L1 is deliberately far below L2.** It is the only level without
  quiescence, so it cannot see a king capture beyond its horizon and leaves
  its king en prise avoidably on ~1.2% of its moves. It is the beginner
  floor; closing that gap would need a new dial, not more noise.
- **Quiescence is the single biggest strength dial:** L2 with only
  quiescence turned on scored +920 against plain L2 in an earlier probe.
- **From L3 up**, no level left its king avoidably en prise, and from L4 up
  none missed an immediate win, across all runs.
- **The top step is sensitive.** For L9, noise 2 / blunder 1% measured only
  ~70 below L10 (pooled over 180 games), noise 3 / 2% measured 236 below; the
  chosen 2.5 / 1.5% lands at 144.

Ratings on the platform from before this table (when L10 was depth 5 and the
steps were 400–500 Elo) no longer apply.

### History

The previous ladder (depth 2 → 5, uniformly random blunders, noise
temperatures up to 35) had every adjacent step at 90–100% for the stronger
side. Its random choice ignored decided outcomes: in a sample of L7–L8
games, all of L7's king hangs came after its own noise-free search already
saw the position as lost, and L8 declined 11 available king captures.
