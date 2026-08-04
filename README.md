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
The crate and namespace ownership map is documented in
[`docs/IMPLEMENTATION.md`](docs/IMPLEMENTATION.md).

## Documentation

The consolidated implementation account is
[`docs/IMPLEMENTATION.md`](docs/IMPLEMENTATION.md). It explains each major
subsystem's purpose, implementation evidence, and remaining limitations. The separate
[`docs/BENCHMARK_SAMPLING_REPORT.md`](docs/BENCHMARK_SAMPLING_REPORT.md)
describes the controlled repeated-process benchmark protocol and links its raw
samples. The concise historical record is
[`docs/HISTORY.md`](docs/HISTORY.md); machine-readable campaign artifacts
remain under `results/`.

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
Formal-boundary ornaments, isolated points, and point/segment formal holes use
a source-mapped canonical representation, exact effective-chord construction,
compact matching, formal completion, and Definition 2 validation. The formal
pipeline is available through `solve` with either dominance solver mode.
Disconnected outer components remain unsupported. See
`docs/IMPLEMENTATION.md` and
`docs/KNOWN_LIMITATIONS.md`.

## Quick start

```bash
cargo run --release -p mrd -- solve \
  --solver exact-cover \
  --input test-data/example.json

cargo run --release -p mrd -- solve \
  --solver dominance-compact-only \
  --input test-data/example.json

cargo run --release -p mrd -- solve \
  --solver dominance-compact-only \
  --input-format polygon \
  --input test-data/polygons/nonuniform-l.json \
  --svg /tmp/nonuniform-l.svg

# `sg-sweep` is the ordinary-polygon default; pairwise backends remain selectable.
cargo run --release -p mrd -- solve \
  --solver dominance-compact-only \
  --input-format polygon \
  --polygon-chords sg-sweep \
  --input test-data/polygons/comb.json

# Keep the pairwise reference enumerator available for differential debugging.
cargo run --release -p mrd -- solve \
  --solver dominance-compact-only \
  --chord-enumerator reference-pairwise \
  --input test-data/example.json

cargo run --release -p mrd -- search-path-tree-witness \
  --max-width 12 --max-height 12 --seed 42 --require-clean \
  --output-dir results/path-tree-witnesses

cargo run --release -p mrd -- verify \
  --input test-data/example.json \
  --all-solvers

cargo run --release -p mrd -- verify \
  --input-format formal-polygon \
  --input test-data/polygons/formal-boundary.json \
  --output /tmp/formal-boundary-incidence.json

cargo run --release -p mrd -- solve \
  --solver dominance-compact-only \
  --input-format formal-polygon \
  --input test-data/polygons/formal/source-figure-three.json \
  --output /tmp/formal-source-figure-three.json \
  --svg /tmp/formal-source-figure-three.svg

cargo run --release -p mrd -- benchmark \
  --suite formal-fixtures \
  --output results/formal-fixtures.json

cargo run --release -p mrd -- exhaustive --width 3 --height 3

cargo run --release -p mrd -- random \
  --width 8 --height 8 --cases 10000 --seed 42

cargo run --release -p mrd -- polyomino \
  --max-cells 12 --all-solvers \
  --output results/polyomino-summary.json

cargo run --release -p mrd -- benchmark \
  --suite adversarial --output results/adversarial.csv

cargo run --release -p mrd -- benchmark \
  --suite dense-conflict --sizes 4,8,16,32,64,128 \
  --output results/dense-conflict.csv

cargo run --release -p mrd -- benchmark \
  --suite clean-boundary-differential \
  --output results/v0.6-clean-boundary-differential.csv

cargo run --release -p mrd -- benchmark \
  --suite path-tree-orientation-audit --sizes 1,2,4,8,16,32,64,128 \
  --output results/v0.7-path-tree-orientation-audit.csv

cargo run --release -p mrd -- benchmark \
  --suite path-tree-dual-differential --sizes 1,2,4 \
  --output results/v0.7-path-tree-dual-differential.csv

cargo run --release -p mrd -- benchmark \
  --suite path-tree-vs4d --sizes 1,2,4,8 \
  --output results/v0.7-path-tree-vs-4d.csv

cargo run --release -p mrd -- benchmark \
  --suite path-tree-scaling --sizes 3,8,16,32,64 \
  --output results/v0.8-path-tree-families.csv

cargo run --release -p mrd -- benchmark \
  --suite path-tree-advantage --sizes 1,2,4,8,16,32,64,128 \
  --output results/v0.8-path-tree-advantage.csv

cargo run --release -p mrd -- generate \
  --family dense-conflict --horizontal 32 --vertical 32 \
  --json /tmp/dense-32.json --svg /tmp/dense-32.svg

cargo run --release -p mrd -- generate \
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
is stored in `results/v1.1-polygon-*`. The implementation contracts for the
event sweep and v1.2 sparse completion are consolidated in
`docs/IMPLEMENTATION.md`.

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

### P9 AN19 status

At repository state `8f9ab06ce00c1e80a58e5b6302c14a408fefabd7`,
workspace scan counting, implementation-side invariants, and differential and
regression tests are complete and internally consistent; the full workspace
result is 247 passed with 3 existing ignored tests, and local and remote HEAD
were clean and equal. This is implementation evidence, not an asymptotic
runtime proof.

The formal SIAM version of Abraham--Neiman, DOI `10.1137/17M1115575`, was
obtained and checked. It does not establish the reduced-event
ordering/counting conversion required by the local AN19 proof plan.
Consequently, the reduced-event class bound and AN19 runtime remain unproved,
but P9.3.2d's faithful implementation and exact Oracle differential are
complete. The missing proof is deferred, low-priority proof debt rather than an
implementation blocker: P9.3.3 through P9.5 may proceed to a complete
source-shaped flow backend. After P9.5, low-priority P9.6a must resolve the proof debt before
the backend may be named `AlmostLinear`, report `an19_runtime_verified: true`,
or support an AN19 runtime claim.

The AN19-shaped all-radii event engine is now implemented behind a replaceable
interface with exact rational ordering, a definition-level `source_an19::oracle::event::Engine`,
canonical semantic and queue traces, and six empirical charge maps. A bounded
A--H campaign differentially agrees on all 31 fixed snapshots in
`results/an19-event-adversarial.json`. This establishes source-shaped semantics
on the tested domain. A machine-checked structural certificate additionally
proves at most `3n + 4m + 2` semantic events and `n + 2m + 2` queue items per
fixed snapshot. Reduced runs also carry a stable exact binary-heap certificate
for at most `3 I ceil(log2(max(I,1))) + 2m` counted comparisons. This practical
`O((n+m) log(n+m))` bound does not establish the source-equivalent
`O(m+n log log n)` priority-queue bound, hierarchy-wide amortization, or
claimed AN19 runtime.

CompactOnly uses the verified `indexed-frontier` geometric-completion backend
by default. `reference-rescan` remains available for differential debugging;
see `docs/IMPLEMENTATION.md` for the deterministic policy and exact cut-family
acceptance contract.

The CompactOnly geometry path now shares one component-local prepared context
across grid-run chord enumeration, dense cut completion, dense rectangle
recovery, and final validation. See `docs/IMPLEMENTATION.md`.

## Layered public backend architecture

The repository exposes an explicit three-layer backend model in
`mrd::layered` (`crates/mrd/src/layered.rs`):

1. **Reference-backed exact solver** (`solve_reference`): runs the permanent
   reference backends and returns exact matching, minimum vertex cover,
   selected chords, and rectangle decomposition with `ReferenceExact`
   provenance.
2. **Source backend under a caller-supplied inclusive target**
   (`solve_source_with_target`): runs only the source-shaped execution path.
   A completed run is certified "source-certified under a caller-supplied
   inclusive target" (recovered cost at most the target). Automatic target
   discovery is **not implemented**; failures are reported honestly as
   `UnsupportedOrUndetermined` and never classified as target infeasibility.
3. **Negative certificate verifier** (`verify_source_infeasible_below`,
   `verify_cover_below`, `verify_source_feasible_at_most`): verifies
   `DualLowerBoundCertificate` and compressed cover-below certificates exactly
   and independently.

There is deliberately no `solve_source -> optimum` automatic entry, no
`AutomaticSource` mode, and no binary-search wrapper for `F*`. Every result
records its `SolverProvenance`.

On the supported formal-polygon domain, the reference-backed solver is the
complete production-ready surface. The source-with-target solver is a
research-only, target-bound interface: it can report a certified result for a
caller-supplied target, but it is not an automatic solver and does not carry an
AN19 runtime claim.

CLI selection:

```bash
mrd solve --solver dominance-compact-only --backend reference \
  --input-format formal-polygon --input test-data/polygons/formal/source-figure-three.json

mrd solve --solver dominance-compact-only --backend source-with-target --target -3 \
  --input-format formal-polygon --input test-data/polygons/formal/source-figure-three.json

mrd verify-negative-certificate --network network.json --certificate certificate.json --target -1
```

The source-with-target backend currently supports formal-polygon input only and
requires `--target`. It never silently falls back to a reference backend. See
`docs/HISTORY.md` and `docs/IMPLEMENTATION_MASTER_PLAN.md`.

### Layered benchmark evidence

`mrd benchmark --suite layered --output <path>` writes deterministic,
separated evidence records rather than one opaque hybrid duration. The default
run contains verified polygon-derived rows for complete reference solving,
formal geometry, compact representation, recovery-only completion, and
certificate verification. It also contains a direct-grid `unavailable` row:
direct-grid parity is reserved for P11 and no P10 result is direct-grid
evidence.

Source measurement is opt-in. `--source-target <integer>` records a
caller-supplied inclusive target; `--reference-provided-target` separately
measures a reference solve and labels that target as experimental input. Neither
option performs automatic `F*` discovery. A source result that cannot be
certified is recorded as `source-undetermined`, never as a fallback or target
infeasibility result. Exact source targets are serialized as decimal strings so
every `i128` value is preserved in JSON.

```bash
cargo run --release -p mrd -- benchmark --suite layered \
  --output /tmp/mrd-layered.json

cargo run --release -p mrd -- benchmark --suite layered --source-target -3 \
  --output /tmp/mrd-layered-source.json
```
