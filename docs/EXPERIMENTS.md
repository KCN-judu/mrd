# Reproducible release experiments

Evidence date: 2026-07-25 (Asia/Tokyo).

## v0.3 compact execution evidence

The compact-pipeline hardening and grid-run differential implementation is
committed in `20de1aa` and the follow-up evidence/documentation commit. The
reference implementation remains available; CompactOnly uses
`GridInteriorRunEnumerator` after exact chord-set equality on every binary
`3x3` and `4x4` component (512 and 65,535 non-empty masks respectively).
The differential population produced no missing or fabricated chords.
An additional 100,000 connected random-walk regions on grids from `5x5`
through `16x16`, using seed `0x6d72642d76303300`, also produced exact
horizontal and vertical chord-family equality. The machine-readable summary is
`results/v0.3-chord-differential.json`.

Exact family equality was additionally checked for all 87,146 free
polyominoes through 12 cells, two explicit ordinary-hole fixtures, and 25
endpoint, topological, external-oracle, and dense adversarial fixtures. The
FullyAudited grid-run mode performs this reference-family comparison before
building its explicit graph and running matching.

The CompactOnly contract tests assert that pairwise embedding audit, explicit
conflict graph construction, Hopcroft--Karp, C0 construction, and full edge
partition expansion are all false. FullyAudited tests assert the corresponding
operations are true. CompactOnly SVG rendering uses `analyze_geometry_with`
and the same grid-run families; it does not invoke `rect_oracle_sg::analyze`.
Every serialized CompactOnly component reports
`explicit_conflict_edge_count: null` and an execution trace with all forbidden
flags false.

Geometry-backed dense runs (the generator's horizontal and vertical targets
are equal, so total `q` is twice the target) recorded the following compact
path sizes: `q=512` -> `sigma=1,619`, `q=1,024` -> `sigma=3,280`,
`q=2,048` -> `sigma=6,647`, and `q=4,096` -> `sigma=13,400`. The largest
completed run had 6,285 compressed vertices and 17,496 arcs. Process peak RSS
is intentionally not claimed; `peak_memory_bytes` remains null. Result
diagnostics include labeled owned-allocation estimates for chord vectors,
embedding points, biclique vectors, and flow storage. These are component-owned
estimates, not process peak memory.

The dedicated `dense-compact-only` benchmark ran targets `128,256,512,1024`,
corresponding to total chord counts `q=512,1024,2048,4096`. Chord enumeration
took 3,958, 13,691, 56,780, and 285,717 microseconds respectively. At `q=4096`
the owned estimates were 131,072 bytes for chord vectors, 262,144 for embedding
points, 229,672 for biclique vectors, 190,248 for flow storage, and 591,248 for
the certificate payload. The complete per-phase CSV is
`results/v0.3-compact-dense.csv`.

The bounded external OR-Tools 9.15 CP-SAT population was rerun for v0.3:
6,998 inputs and 27,228 components, including all `3x3` binary grids, free
polyominoes through 10 cells, and 13 selected adversarial grids. All 27,228
components were solved and compared, with 0 timeouts, unsupported components,
or disagreements. The separate evidence file is
`results/v0.3-external-oracle.json`; it does not replace the v0.2 population.

The final tagged lineage also reran the seed-42 random suite: 10,000 `8x8`
inputs, 162,162 components, and 0 counterexamples. The adversarial benchmark
verified all 19 components from 17 inputs. Their separate evidence files are
`results/v0.3-random-8x8-seed42.json` and `results/v0.3-adversarial.csv`.

## Historical v0.2 evidence

The original v0.2 paper-table population and the commands in Sections 1--8
below were produced from Git commit
`32faff61bc4577ab50010e5d253afe83f7655d83` with
`rustc 1.89.0 (29483883e 2025-08-04)`, the Cargo release profile, macOS
26.5 on an Apple M4, and integer-only correctness paths. The random seed was
42. The external OR-Tools 9.15 CP-SAT oracle used seed 0 and a 30-second
timeout per component. Rust solvers had no wall-clock timeout. Unless stated
otherwise, skipped means a configured size filter prevented a particular
oracle comparison; it does not mean that a supported Rust input failed.

The machine-readable environment, commands, seeds, timeout, populations, and
generated table paths are in `results/manifest.json`. Paper-ready correctness,
compression, and scope tables are generated in `results/paper-tables.md` and
the adjacent CSV files. Those files are the source of truth for tabular
numbers; this report explains their populations and limits rather than
maintaining duplicate tables. The later v0.3 files named above retain their
own producing commits; `results/release-index.json` separates those release
populations explicitly.

## Quality gates

```bash
cargo fmt --all -- --check
python3 tools/check_biclique_bound.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

The release gate run passed. The workspace test command reported 30 passing
tests across 13 test binaries and doc-test groups. These tests include
endpoint contacts, flow-capacity certificates, dense and topological
adversarial families, mapped-back metamorphic validation, biclique
edge-multiset auditing, and stored-regression replay.

## 1. Exhaustive binary-grid verification

```bash
target/release/rect-cli exhaustive \
  --width 4 --height 4 \
  --output results/exhaustive-4x4.json
```

The complete population is all 65,536 binary `4x4` grids and their 337,058
four-connected monochromatic components. Exact cover, SG, C0, and compressed
flow each made 337,058 comparisons. All 337,058 components were solved and
validated, with 0 skipped, 0 timed out, and 0 disagreements. The measured
wall time was 6.60 seconds.

## 2. Random-grid verification

```bash
target/release/rect-cli random \
  --width 8 --height 8 --cases 10000 --seed 42 \
  --output results/random-8x8-seed42.json
```

The seeded population contains 10,000 binary `8x8` grids and 162,162
components. SG, C0, and compressed flow solved and validated all 162,162
components. Exact cover compared 160,900 components and skipped 1,262 above
its 40-cell limit. There were 0 timeouts and 0 disagreements. The measured
wall time was 9.42 seconds.

## 3. Free-polyomino verification

```bash
target/release/rect-cli benchmark \
  --suite polyomino --max-cells 10 --oracle-cell-limit 40 \
  --output results/polyomino.csv

target/release/rect-cli polyomino \
  --max-cells 12 --all-solvers --oracle-cell-limit 40 \
  --output /tmp/mrd-polyomino-max12-32faff6.json
```

The committed structural CSV contains 6,473 canonical free polyominoes
through 10 cells plus one ordinary-hole fixture: 6,474 inputs and components,
all solved, with 0 skipped, 0 timed out, and 0 disagreements. The larger
validation contains the known 87,146 canonical free polyominoes through 12
cells plus two separately generated ordinary-hole fixtures. All 87,148 inputs
and components were verified, with 0 in every other status, in 6.40 seconds.
The record-level max-12 JSON is not committed because it is about 35 MB; its
command and aggregate are retained here and in the release notes.

## 4. Adversarial verification

```bash
target/release/rect-cli benchmark \
  --suite adversarial \
  --output results/adversarial.csv
```

This deterministic population contains 17 grids and 19 foreground
components spanning endpoint contacts, rings and multiple holes, narrow
corridors, combs and double combs, staircases, spirals, dense conflicts,
reflex-heavy shapes, long runs, disconnected same-color regions, and
diagonal-only contact. SG, C0, and compressed flow solved all 19 components;
exact cover compared 9 and skipped 10 above its configured cell limit. There
were 0 unsupported components, 0 timeouts, 0 solver errors, and 0
disagreements. No minimized regression fixture was created because no
counterexample occurred.

## 5. External CP-SAT comparisons

```bash
tools/external-oracle/verify_suite.py \
  --rect-cli target/release/rect-cli \
  --exhaustive-width 3 --exhaustive-height 3 \
  --polyomino-max-cells 10 \
  --adversarial-dir /tmp/mrd-adversarial-final-32faff6 \
  --max-adversarial-grid-cells 20000 \
  --max-component-cells 40 --exact-cover-cell-limit 40 \
  --max-time-seconds 30 \
  --work-dir /tmp/mrd-external-final-32faff6 \
  --output results/external-oracle.json
```

The Python oracle independently parses grid JSON and enumerates valid
monochromatic rectangles without calling Rust geometry. It selected 6,998
inputs and 27,228 components: all 512 binary `3x3` grids (1,794 components),
all 6,473 free polyominoes through 10 cells (25,390 components), and 13
adversarial grids (44 components). CP-SAT, exact cover, SG, C0, and compressed
flow all compared all 27,228 selected components. All 6,998 inputs were
verified; 11 larger adversarial grids were explicitly skipped by the input
filter, while selected components had 0 unsupported, 0 solver errors, 0
timeouts, and 0 disagreements. Wall time was 41.21 seconds.

## 6. Biclique-partition audits

Every compressed-solver invocation for a feasible explicit graph audits the
block multiset against independently generated geometric conflict edges. The
audit checks nonempty blocks, unique vertex IDs, actual Cartesian-product
edges, no missing or fabricated edge, multiplicity exactly one, strict
recursive decrease, and termination. The recorded correctness populations
plus the six dense instances exercised 532,947 compact solver/audit
invocations. Every audit passed; 0 edges were missing, fabricated, or
duplicated, so 0 inputs were skipped, timed out, or disagreed at this layer.
Exact discrepancy counts are retained even when stored offending-edge samples
are bounded.

## 7. Dense-conflict compression benchmarks

```bash
target/release/rect-cli benchmark \
  --suite dense-conflict --sizes 4,8,16,32,64,128 \
  --output results/dense-conflict.csv
```

All six geometry-backed grids and their six components were solved, with 0
skipped, 0 timed out, and 0 disagreements. From size 4 to 128, `q` grew from
16 to 512, explicit conflict edges from 32 to 16,896, and biclique incidence
size `sigma` from 40 to 1,619. C0 arcs grew from 80 to 34,304 while compact
arcs grew from 56 to 2,131, an observed arc reduction from 30.00% to 93.79%.
These measurements demonstrate compression on this deterministic family; they
do not establish a new asymptotic law. Phase timings and all intermediate
sizes are in the generated compression table.

## 8. Metamorphic tests

```bash
cargo test -p rect-verify transforms
```

The fixture is transformed by translation, horizontal and vertical
reflection, rotations by 90, 180, and 270 degrees, main-diagonal reflection,
and uniform scaling by two. Exact cover, SG, C0, and compressed flow produce
32 mapped-back dissections, all validated against the original cells and all
with invariant optimum counts. This deterministic test has one source grid,
eight transformed cases, 32 solver results, and 0 skipped, timed out, or
disagreed results. A separate count-invariance regression covers the same
geometric laws at the public verification layer.

## 9. Known unsupported input classes

The input model is finite colored unit-cell grids with ordinary nondegenerate
holes. Ornaments, isolated formal-boundary points, line-segment holes, point
holes, arbitrary degenerate formal holes, and general polygon input are
outside the accepted model. They are listed in every benchmark's metadata and
in the generated scope table. Consequently the experiment population for
these classes is 0 grids and 0 components: 0 solved, 0 configured skips, 0
timeouts, and 0 disagreements. This is a declared scope boundary, not evidence
that those theoretical cases are implemented.

## 10. Gap to the theoretical asymptotic algorithm

The effective-chord enumerator is exact for supported grids but uses aligned
reflex-pair tests rather than the classical `O(n log n)`
Soltan--Gorpinevich sweep. The compact flow implementation uses Dinic, not the
cited deterministic almost-linear exact-flow backend. The constructive
four-coordinate dominance recursion is implemented and audited with the
correct `O(q log^4 q)` Cardinal--Yuditsky upper bound, but the artifact as a
whole is not an end-to-end `n^(1+o(1))` implementation. This scope statement
does not add a separate experimental population: 0 grids and components were
claimed for the unimplemented backends, hence 0 solved, skipped, timed out, or
disagreed results for them.

## v0.4 indexed completion evidence

The v0.4 campaign keeps the v0.3 populations unchanged and compares the two
completion backends on the geometry-backed dense family. Targets 128, 256, 512,
and 1024 correspond to total effective-chord counts 512, 1024, 2048, and 4096.
Both backends use `GridInteriorRunEnumerator` and CompactOnly flow; only the
completion backend changes. Generated files are
`results/v0.4-dense-completion.csv`, `results/v0.4-completion-table.csv`, and
`results/v0.4-completion-table.md`.

Measured geometric-completion speedups for indexed frontier over reference
rescan were 4.183x, 1.800x, 1.821x, and 1.541x at q=512, 1024, 2048, and 4096.
The indexed backend performed two initial local-bbox scans per component and
all exact differential suites compared selected cuts, added cuts, and sorted
rectangles. Owned-allocation estimates are recorded; process peak RSS remains
unmeasured.

## v0.5 prepared grid pipeline evidence

The v0.5 campaign separates conflict-heavy, completion-heavy, and area-heavy
workloads. Every recorded CompactOnly row reports one prepared-component
construction. The staircase family adds 28, 120, 496, and 2,016 horizontal
unit cuts for requested sizes 8, 16, 32, and 64; the orthogonal spiral adds 8,
16, 32, and 64. Families whose simple-chord count stayed constant are retained
as regression fixtures but are not claimed as primary completion-heavy data.

At q=4096 on the dense-conflict geometry, reference hash recovery measured
470,725 microseconds and dense recovery measured 8,229 microseconds. Prepared
grid-run enumeration measured 229 and 224 microseconds respectively. On the
256 by 256 solid area workload, recovery measured 15,942 microseconds for the
reference and 425 microseconds for the dense backend. These are single-run
measurements, not portable performance guarantees. Source rows are
`results/v0.5-completion-heavy.csv`, `results/v0.5-area-heavy.csv`, and
`results/v0.5-dense-completion.csv`.
