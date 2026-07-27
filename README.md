# Exact minimum rectangular dissection verifier

This Rust 2024 workspace implements independent exact solvers for minimum
rectangular partitions of finite colored grids and ordinary integer-coordinate
rectilinear polygons:

- `exact-cover`: a bitset branch-and-bound Algorithm X oracle;
- `sg-explicit`: explicit effective chords, Hopcroft--Karp, Konig cover recovery,
  and constructive geometric completion;
- `dominance-c0`: the paper's 4D embedding with one biclique per edge and Dinic;
- `dominance-compressed`: the constructive Cardinal--Yuditsky Theorem 8
  biclique partition and the compressed Dinic network, available as both a
  fully audited path and a compact-only path that does not materialize conflict
  edges.
- `--representation path-tree|auto`: the clean hole-free region-dual tree
  representation, with an explicit eligibility certificate and compact 4D
  fallback.
- `--path-tree-orientation build-both|bound-estimate|vertical-tree|horizontal-tree`:
  choose or audit the path-tree orientation without changing the correctness
  Oracles. FullyAudited and CompactOnly default to exact `build-both`; the
  `bound-estimate` selector remains an explicit heuristic benchmark policy.

All correctness-critical geometry uses integers. Grid rectangles are half-open;
polygon rectangles use native `i64` coordinates; geometric chords are closed.
Every solver returns explicit rectangles and runs its exact native validator.

The v1.3 polygon sparse path uses an output-sensitive closed-endpoint
orthogonal intersection sweep, an event-driven exact slab validator, and a
physically sparse dynamic stabbing tree. The v1.2 range-scan subdivision and
slab-rescan validator remain explicit differential Oracles. CLI overrides are
`--subdivision-builder reference-range-scan|orthogonal-sweep` and
`--sparse-validator reference-slab-rescan|event-segment-tree`; polygon recovery
also accepts the opt-in `--polygon-recovery auto` crossover policy.

The grid model is a finite colored array of unit cells split by color and
four-connectivity. The polygon model is one ordinary nondegenerate outer loop
with zero or more ordinary two-dimensional holes and no boundary contact.
Formal-boundary ornaments, isolated points, and point/segment formal holes now
have a source-mapped canonical representation and incidence validator. Their
effective-chord, completion, and solving pipeline remains unavailable until
the formal-hole geometry phase. Disconnected outer components remain
unsupported. See `docs/FORMAL_BOUNDARY_MODEL.md` and
`docs/KNOWN_LIMITATIONS.md`.

## Quick start

```bash
cargo run --release -p rect-cli -- solve \
  --solver exact-cover \
  --input test-data/example.json

cargo run --release -p rect-cli -- solve \
  --solver dominance-compact-only \
  --input test-data/example.json

cargo run --release -p rect-cli -- solve \
  --solver dominance-compact-only \
  --input-format polygon \
  --input test-data/polygons/nonuniform-l.json \
  --svg /tmp/nonuniform-l.svg

# `sg-sweep` is the ordinary-polygon default; pairwise backends remain selectable.
cargo run --release -p rect-cli -- solve \
  --solver dominance-compact-only \
  --input-format polygon \
  --polygon-chords sg-sweep \
  --input test-data/polygons/comb.json

# Keep the pairwise reference enumerator available for differential debugging.
cargo run --release -p rect-cli -- solve \
  --solver dominance-compact-only \
  --chord-enumerator reference-pairwise \
  --input test-data/example.json

cargo run --release -p rect-cli -- search-path-tree-witness \
  --max-width 12 --max-height 12 --seed 42 --require-clean \
  --output-dir results/path-tree-witnesses

cargo run --release -p rect-cli -- verify \
  --input test-data/example.json \
  --all-solvers

cargo run --release -p rect-cli -- verify \
  --input-format formal-polygon \
  --input test-data/polygons/formal-boundary.json \
  --output /tmp/formal-boundary-incidence.json

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

cargo run --release -p rect-cli -- benchmark \
  --suite clean-boundary-differential \
  --output results/v0.6-clean-boundary-differential.csv

cargo run --release -p rect-cli -- benchmark \
  --suite path-tree-orientation-audit --sizes 1,2,4,8,16,32,64,128 \
  --output results/v0.7-path-tree-orientation-audit.csv

cargo run --release -p rect-cli -- benchmark \
  --suite path-tree-dual-differential --sizes 1,2,4 \
  --output results/v0.7-path-tree-dual-differential.csv

cargo run --release -p rect-cli -- benchmark \
  --suite path-tree-vs4d --sizes 1,2,4,8 \
  --output results/v0.7-path-tree-vs-4d.csv

cargo run --release -p rect-cli -- benchmark \
  --suite path-tree-scaling --sizes 3,8,16,32,64 \
  --output results/v0.8-path-tree-families.csv

cargo run --release -p rect-cli -- benchmark \
  --suite path-tree-advantage --sizes 1,2,4,8,16,32,64,128 \
  --output results/v0.8-path-tree-advantage.csv

cargo run --release -p rect-cli -- generate \
  --family dense-conflict --horizontal 32 --vertical 32 \
  --json /tmp/dense-32.json --svg /tmp/dense-32.svg

cargo run --release -p rect-cli -- generate \
  --family clean-complete-bipartite --t 1 \
  --json /tmp/clean-k2-2.json --svg /tmp/clean-k2-2.svg
```

Add `--svg dissection.svg` to `solve` to render the source cells, boundary,
reflex vertices, effective chords, selected chords, and output rectangles. If a
grid has multiple monochromatic components, one file per component is written.

Grid input is:

```json
{
  "width": 3,
  "height": 3,
  "cells": ["a", "a", "a", "a", "b", "a", "a", "a", "a"]
}
```

Polygon input is a tagged JSON object:

```json
{
  "type": "rectilinear-polygon",
  "outer": [[0, 0], [10, 0], [10, 10], [0, 10]],
  "holes": [[[2, 2], [2, 4], [4, 4], [4, 2]]]
}
```

`--input-format auto|grid|polygon` defaults to `auto`. Polygon production uses
one `PreparedPolygonContext`, the orthogonal sweep validator, the axis-generic
`sg-sweep` Definition 7 construction, the existing 4D compact matching
backend, incremental indexed completion, dynamic orthogonal cut stabbing,
sparse face-walk recovery, and sparse slab validation. It never rasterizes by
coordinate magnitude or materializes the coordinate Cartesian product on the
CompactOnly path. `reference-pairwise` and
`indexed-pairwise` remain selectable with `--polygon-chords` as independent
correctness Oracles.

The v0.9 boundary-native evidence is recorded in
`results/v0.9-polygon-differential.json` and generated into
`results/paper-tables.md`; historical v0.2--v0.8.1 result populations remain
separate and immutable. v1.0 indexed-backend evidence is stored in the
`results/v1.0-polygon-*` reports and documented in
`docs/POLYGON_BACKEND_DIFFERENTIAL.md`. The v1.1 three-backend sweep evidence
is stored in `results/v1.1-polygon-*` and source-mapped in
`docs/SOLTAN_SWEEP_IMPLEMENTATION.md`. v1.2 adds the sparse completion
contracts in `docs/DYNAMIC_ORTHOGONAL_CUT_INDEX.md`,
`docs/SPARSE_POLYGON_SUBDIVISION.md`, and `docs/SPARSE_POLYGON_VALIDATION.md`.

Colors are arbitrary JSON scalar or structured values compared by exact JSON
equality. See `docs/KNOWN_LIMITATIONS.md` before using non-grid polygon inputs.

CompactOnly production geometry uses the indexed boundary metadata, grid-run
chord enumeration, laminar event sweep, and indexed frontier completion. The
pairwise endpoint, nested-gap, and rescan paths remain independent references;
their differential campaigns are recorded separately in `docs/EXPERIMENTS.md`.

The optional independent Python/OR-Tools oracle is documented in
`tools/external-oracle/README.md`. Reproducible commands, exact tested
populations, seeds, timeouts, and discrepancy counts are recorded in
`docs/EXPERIMENTS.md` and `results/manifest.json`.

The four-coordinate Cardinal--Yuditsky specialization has representation bound
`O(q log^4 q)`. For the accepted ordinary-loop polygon model, `sg-sweep` uses
`O(n log n + q)` event/status construction without aligned-pair enumeration.
This repository remains a correctness and experimental artifact: the sweep does
not implement the source's formal-boundary features, indexed completion does
not claim the full classical completion bound, and the practical Dinic backend
is not the cited almost-linear theoretical flow algorithm.

CompactOnly uses the verified `indexed-frontier` geometric-completion backend
by default. `reference-rescan` remains available for differential debugging;
see `docs/GEOMETRIC_COMPLETION.md` for the deterministic policy and exact
cut-family acceptance contract.

The CompactOnly geometry path now shares one component-local prepared context
across grid-run chord enumeration, dense cut completion, dense rectangle
recovery, and final validation. See `docs/PREPARED_GRID_PIPELINE.md`.
