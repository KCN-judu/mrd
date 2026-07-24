# Exact minimum rectangular dissection verifier

This Rust 2024 workspace implements independent exact solvers for minimum
monochromatic rectangular partitions of finite colored grids:

- `exact-cover`: a bitset branch-and-bound Algorithm X oracle;
- `sg-explicit`: explicit effective chords, Hopcroft--Karp, Konig cover recovery,
  and constructive geometric completion;
- `dominance-c0`: the paper's 4D embedding with one biclique per edge and Dinic;
- `dominance-compressed`: the constructive Cardinal--Yuditsky Theorem 8
  biclique partition and the compressed Dinic network.

All correctness-critical geometry uses integers. Grid rectangles are half-open;
geometric chords are closed. Every solver returns explicit rectangles and runs
the same cell-exact output validator.

## Quick start

```bash
cargo run --release -p rect-cli -- solve \
  --solver exact-cover \
  --input test-data/example.json

cargo run --release -p rect-cli -- verify \
  --input test-data/example.json \
  --all-solvers

cargo run --release -p rect-cli -- exhaustive --width 3 --height 3

cargo run --release -p rect-cli -- random \
  --width 8 --height 8 --cases 10000 --seed 42
```

Add `--svg dissection.svg` to `solve` to render the source cells, boundary,
reflex vertices, effective chords, selected chords, and output rectangles. If a
grid has multiple monochromatic components, one file per component is written.

The input format is:

```json
{
  "width": 3,
  "height": 3,
  "cells": ["a", "a", "a", "a", "b", "a", "a", "a", "a"]
}
```

Colors are arbitrary JSON scalar or structured values compared by exact JSON
equality. See `docs/KNOWN_LIMITATIONS.md` before using non-grid polygon inputs.

