# Search tree export

`keres debug-tree --full-tree` streams every node the search visits to
stdout, for tuning, debugging, or bulk-scoring positions.

```bash
keres debug-tree [--moves <base64>] --full-tree [--tree-format jsonl|binary] [--max-depth N]
```

Human-readable stats (time, best move, score, PV) go to stderr, so stdout
contains nothing but the tree. The recorder lives in
`src/engine/tree_recorder.rs`. It writes each node as soon as that node's
score is known, so memory use doesn't grow with the size of the tree.

## Tree structure

- The searched position itself is the implicit node `0`. It is never written.
- Root moves have `parent_id = 0` and `depth = 0`. Every other node is
  exactly one ply below its parent: `depth = parent.depth + 1`.
- `depth` is the ply the move is played from. Nodes with
  `depth < max_depth` come from the main search, and nodes with
  `depth >= max_depth` from quiescence (captures only).
- `id`s are unique and start at 1. They are **not sorted** in the
  output: a node is written after all its children (post-order), and the
  parallel root searches interleave. Rebuild the tree from `parent_id`.
- `score` is in engine units, from the point of view of **the side that
  played the move**: positive is good for that side. A forced win is
  `1000 - ply`, a forced loss `-(1000 - ply)`.

## What the scores mean

The export dumps the search as it ran. It is not an exhaustive table of
exact values, so read scores with these caveats:

- **Only root moves (`depth = 0`) have exact scores.** Each root move is
  searched with a full window. Deeper nodes are searched with the
  alpha-beta window inherited from their parent, so their score can be a
  bound (the true value is at least, or at most, the score) rather than
  the true value.
- **Not every move is present.** Moves cut off by alpha-beta are never
  searched, and a transposition-table hit returns without searching (so
  without recording) its subtree.
- **Game-ending moves are never recorded**, at any depth. They are scored
  without a search below them.
- **Search depth shrinks as you go down.** A node at `depth = d` was
  searched `max_depth - d` plies deeper (plus quiescence). For example, to
  keep only scores backed by at least 4 plies of search from a depth-8
  dump, keep `depth <= 4`.
- Node counts can differ by a few hundredths of a percent between identical
  runs. The best move is the same.

## JSONL format (`--tree-format jsonl`, default)

One object per line:

```json
{"id":181985,"parent_id":0,"depth":0,"move":"H1-G3","unstack":false,"score":15}
```

`move` uses `A1-B2` notation. A trailing `-` (`A1-B2-`) means only the
top piece of a stack moves, and the `unstack` field states this explicitly.
A record is about 87 bytes.

## Binary format (`--tree-format binary`)

Fixed 13-byte records, back to back, with no header or separator. All
integers are little-endian, as in the [wire protocol](PROTOCOL.md):

| offset | size | type  | field                                                      |
|--------|------|-------|------------------------------------------------------------|
| 0      | 4    | `u32` | `id`                                                       |
| 4      | 4    | `u32` | `parent_id` (`0` = searched position)                      |
| 8      | 1    | `u8`  | `depth`                                                    |
| 9      | 2    | `u16` | move, [`Move` layout](PROTOCOL.md#move--2-bytes-u16-little-endian) (bit 14 = unstack) |
| 11     | 2    | `i16` | `score`                                                    |

- File size is always a multiple of 13. Node count = size / 13.
- `score` is capped at the `i16` range (±32767). Real scores stay far
  inside it.
- If a search would produce more than `u32::MAX` nodes, the run stops
  with an error rather than writing ids that wrap around.

Decoding example (Python):

```python
import struct
with open("tree.bin", "rb") as f:
    for id_, parent, depth, move, score in struct.iter_unpack("<IIBHh", f.read()):
        unstack = bool(move & 0x4000)
```

## Sizes

From the opening position (53 root moves), with all search features on:

| `--max-depth` | nodes   | JSONL    | binary  | search time        |
|---------------|---------|----------|---------|--------------------|
| 4             | 414 k   | 35 MB    | 5.4 MB  | < 1 s              |
| 5             | 3.0 M   | 260 MB   | 39 MB   | ~5 s               |
| 8             | ~1.75 G | ~150 GB* | 22.8 GB | ~18 min (desktop)  |

\* Estimated from the binary size.

Each extra ply multiplies the node count by roughly 7. For large dumps,
use the binary format, or pipe either format through `zstd -T0`.
