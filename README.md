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

The supported input model is a finite colored array of unit cells, split by
color and four-connectivity into ordinary nondegenerate grid regions. The
implementation does not accept ornaments, isolated formal-boundary points,
line-segment holes, point holes, arbitrary degenerate formal holes, or general
polygon input. See `docs/KNOWN_LIMITATIONS.md` for the exact scope boundary.

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

cargo run --release -p rect-cli -- polyomino \
  --max-cells 12 --all-solvers \
  --output results/polyomino-summary.json

cargo run --release -p rect-cli -- benchmark \
  --suite adversarial --output results/adversarial.csv

cargo run --release -p rect-cli -- benchmark \
  --suite dense-conflict --sizes 4,8,16,32,64,128 \
  --output results/dense-conflict.csv

cargo run --release -p rect-cli -- generate \
  --family dense-conflict --horizontal 32 --vertical 32 \
  --json /tmp/dense-32.json --svg /tmp/dense-32.svg
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

The optional independent Python/OR-Tools oracle is documented in
`tools/external-oracle/README.md`. Reproducible commands, exact tested
populations, seeds, timeouts, and discrepancy counts are recorded in
`docs/EXPERIMENTS.md` and `results/manifest.json`.

The four-coordinate Cardinal--Yuditsky specialization has representation bound
`O(q log^4 q)`. This repository remains a correctness and experimental
artifact: its exact grid chord enumerator is not the classical `O(n log n)`
sweep, and its practical Dinic backend is not the cited almost-linear
theoretical flow algorithm.
