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

The v0.5 path-tree gate additionally covers HLD path, star, balanced, and
irregular trees, clean eligibility/fallback, and endpoint identity round trips.
The current workspace run reports 53 passing tests across 13 suites; the full
command remains the acceptance source of truth below.

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

The effective-chord enumerator is exact for supported grids and defaults to the
prepared grid-interior-run index; the aligned reflex-pair implementation is
retained as the differential reference. Neither path is the classical
`O(n log n)` Soltan--Gorpinevich sweep. The compact flow implementation uses Dinic, not the
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

## v0.5 clean hole-free path-tree evidence

The clean census enumerated every nonempty binary `4x4` grid component. It
covered 168,529 components, of which 167,936 were hole-free and 155,389 passed
the clean certificate. Total effective-chord mass was 55,296 and eligible mass
was 19,908. Rejections were 593 ordinary-hole components and 20,736 shared
boundary-endpoint cases. The eligible q histogram is generated in
`results/v0.5-clean-census.json`, with the same data in CSV and Markdown form.

The path-tree comparison enumerated the binary `3x3` population, retained 871
eligible components, and compared FullyAudited path-tree and four-dimensional
outputs. All 871 had identical optimum counts and canonical rectangles and
passed cell-exact validation. The evidence is in
`results/v0.5-path-tree-comparison.csv`; explicit conflict graphs are used only
inside FullyAudited Oracle audits. CompactOnly path-tree SVG runs preserve the
edge-free execution trace.

The reference dual construction is area-sensitive and, before the final clean
geometry follow-up, used only the vertical-tree/horizontal-path orientation.
The planar-sweep dual construction remains outside the artifact scope.

## v0.5 clean geometry completion evidence

Commit `f2742d9` adds the corrected integer-grid complete-bipartite family and
the symmetric path-tree selector. The clean family campaign covers `t=1,2,3,4`:
all four fixtures are clean, have exactly `|H|=|V|=2t`, and have exactly
`|E|=4t^2`, with zero unsupported cases, solver errors, or counterexamples.
The row-level evidence is `results/v0.5-clean-complete-bipartite.csv` and the
compact summary is `results/v0.5-clean-complete-bipartite-summary.json`.

The complete binary `4x4` path-tree comparison covers all 65,536 masks and
155,389 eligible foreground components. Every component has identical path-tree
and 4D optimum/rectangle output, with 155,389 verified rows and zero
counterexamples or solver errors. The selected orientation was
`vertical-tree-horizontal-paths` for all rows because the deterministic sigma
comparison tied there. The compressed row population is stored as
`results/v0.5-path-tree-comparison-4x4.csv.gz`, with aggregate ranges in
`results/v0.5-path-tree-comparison-4x4-summary.json`.

CompactOnly path-tree runs on the clean family retain
`explicit_conflict_edge_count: null` and all forbidden execution-trace flags
false. The selector now evaluates both orientations; the grid dual remains an
area-sensitive reference construction rather than the paper's planar sweep.

## v0.6 true compact path-tree evidence

The v0.6 implementation and evidence are committed in `b03ae75`. CompactOnly path-tree
records contain endpoint-only `CompactTreePath` values; explicit
`ChordTreePath` edge vectors and per-path BFS are retained only by
FullyAudited. CompactOnly defaults to `BoundaryLaminar`, while
`ReferenceAreaFloodFill` remains available through `--region-dual reference-area`
for differential debugging.

The trace contract records
`full_tree_path_edge_lists_materialized=false`, `per_path_bfs_called=false`,
`area_flood_fill_dual_built=false`, `unit_chord_cuts_materialized=false`, and
`prepared_occupancy_transposed=false` on the CompactOnly boundary path. The
SVG regression uses geometry-only analysis and preserves these flags.

The boundary dual is checked against the area dual by chord-labelled path edge
sets on the small clean population. The compact boundary backend matches the
independent compact 4D output on every non-empty binary 3x3 clean component.
The full 4x4 campaign is an explicit release-mode gate because its 155,389
eligible components are machine-dependent. The scaled clean
complete-bipartite compact benchmark is in
`results/v0.6-clean-complete-bipartite-compact.csv` for `t=1,2,4,8,16,32,64,128`;
all rows are verified and the path-edge record count remains zero while
`q=4t` grows to 512.

The v0.6 evidence has two distinct populations. The historical v0.5 rows above
use the area-dual two-orientation selector. The current v0.6 CompactOnly rows
use `BoundaryLaminar`, endpoint-only HLD, and the finite-grid axis view; the
area dual, physical transpose, explicit path vectors, and per-path BFS remain
reference-only operations. The reproducible full-population command is:

```text
rect-cli benchmark --suite clean-boundary-differential --output results/v0.6-clean-boundary-differential.csv
```

It writes CSV, JSON, and Markdown summaries and compares canonical rectangles,
optimum counts, clean certificates, and forbidden execution-trace flags for
each eligible component rather than merely reporting an aggregate counter.

## v0.7 structural path-tree evidence

The v0.7 branching probe uses only unit-cell geometry and checks both axis
views. It finds a clean `balanced-laminar-9` witness with dual-tree maximum
branching degree 9; the older vertical-only probe was rejected because it
silently skipped horizontal-tree witnesses. No synthetic tree is passed to the
production builder.

The orientation audit covers 160,443 clean components from all nonempty binary
3x3 and 4x4 masks, free polyominoes through ten cells, deterministic clean
structural families, complete-bipartite fixtures, and 256 seeded 8x8 random
candidates. BoundEstimate has 160,443 exact sigma matches, zero positive
regret, and 410 equal-sigma tie-direction differences. Full rows are in
`results/v0.7-path-tree-orientation-audit.csv`; the JSON and Markdown files are
aggregate summaries so the large row population is not duplicated.

The bounded dual differential covers 156,267 clean components from the binary
3x3/4x4 population, structural families, and complete-bipartite sizes 1, 2,
and 4. BoundaryLaminar and ReferenceAreaFloodFill agree on every canonical
rectangle result and sigma, with zero solver errors. Auto fallback covers seven
fixtures: four clean path-tree selections and three non-clean compact 4D
fallbacks. These results are in the corresponding `results/v0.7-*` CSV/JSON
artifacts and are bound to the current release commit in the manifest.

The historical v0.7 path-tree-vs-4D report used q buckets `0-8`, `9-32`,
`33-128`, `129-512`, and `513+`; it records construction, flow, completion,
total time, network, sigma, and owned-allocation fields separately. The v0.7
production default remained `dominance-4d` for CompactOnly and `build-both`
for the path-tree backend.

## v0.8 boundary-indexed adaptive path-tree evidence

The v0.8 implementation adds one reusable `BoundaryIndex` and one shared
effective-chord endpoint table to each prepared solve. CompactOnly diagnostics
therefore report `linear_boundary_vertex_lookup_count=0` and
`clean_endpoint_pair_comparisons=0`; the preserved reference paths retain
linear lookup and pairwise behavior for differential checks.

`EventSweep` and `ReferenceNested` produce identical boundary-gap labels in the
complete committed differential campaign: 950,557 inputs, 1,053,939 components,
and 385,947 clean components. The campaign performs 16,530,980 boundary-index
comparisons, 3,368,464 endpoint-metadata comparisons, 1,053,939 classifier
comparisons, and 771,894 orientation comparisons, with zero mismatches and zero
solver errors. Event rows report zero interval-membership tests and exactly one
push/pop per interval (409,593 each); the nested Oracle performs 52,388,678
membership tests. The endpoint classifier uses a vertex-owner map rather than
an all-pairs endpoint loop, while the reference classifier remains available
under `classify_clean_hole_free_reference`. Full population and failure-handling
details are in `docs/PATH_TREE_GAP_DIFFERENTIAL.md`.

Completion is now an interchangeable backend. `ReferenceRescanCompletion` is
the correctness baseline; `IndexedFrontierCompletion` performs one local
bounding-box scan per axis, refreshes only incident frontier vertices, and
recovers rectangles from dense local cuts. Differential tests compare selected
cuts, added cuts, sorted rectangles, and both cell-exact validators. The
completion contract and complexity scope are fixed in
`docs/GEOMETRIC_COMPLETION.md`.

The deterministic mixed-branching search examined 74,542 production geometry
candidates and retains 16 delta-minimized, translation/dihedral-canonical clean
witnesses. Their cell counts fell from roughly 332--446 before minimization to
47--115 while preserving both chord orientations, degree-3-or-higher branching,
multi-heavy-chain paths, and at least two canonical segment nodes. Complete
JSON/SVG bundles are in `results/path-tree-witnesses/`; the construction and
predicate are documented in `docs/MIXED_BRANCHING_PATH_TREE_FAMILY.md`.

The expanded orientation audit records five positive `BoundEstimate` sigma
regret rows among stored mixed-branching witnesses (maximum absolute regret 2,
ratio 2/4). CompactOnly therefore keeps exact `build-both` as its production
default; `bound-estimate` remains available for explicit benchmark experiments.
FullyAudited also continues to use `build-both`.

The generated v0.8 structural scaling report is
`results/v0.8-path-tree-families.csv`. The geometry-backed
`mixed-branching-connected-sum` family grows from one through eight modules;
for the first four members, q grows `6,14,22,30`, dual regions `4,9,13,17`,
paths `3,6,10,14`, heavy-chain intervals `4,10,15,20`, and canonical nodes
`2,6,8,11`. The eighth member reaches q=61, 33 regions, 29 paths, 39 intervals,
and 24 canonical nodes. The
laminar-chain high-q probe reaches q=512 at scale
128. The representation comparison in `results/v0.8-path-tree-vs-4d.csv`
contains 31 verified geometry-backed rows, includes q=2,052, has zero output
counterexamples, and records nonempty owned-allocation estimates for both sides.
Its summaries use all six requested buckets: `0-8`, `9-32`, `33-128`,
`129-512`, `513-2048`, and `2049+`.

The v0.8 advantage search in `results/v0.8-path-tree-advantage.json` evaluates
27 eligible mixed-orientation rows. It finds 14 strict path-tree sigma
advantages and zero strict 4D advantages in this configured corpus. The best
retained row has sigma 6 versus 8, with 12 versus 14 network arcs and owned
estimates 4,176 versus 7,548 bytes. Every retained comparison has equal optimum
and canonical rectangles; the search objective is the sigma ratio, not the
final optimum count.

## v0.9 boundary-native ordinary polygon evidence

The v0.9 frontend accepts normalized integer-coordinate ordinary polygon loops
without a cell set. Permanent tests cover canonical orientation/start,
redundant collinear vertices, 11 strict negative validator cases, five focused
Definition 7 predicate tests, exact doubled-coordinate predicates, large
coordinate gaps, ordinary holes, comb and spiral-corridor inputs, affine
symmetries, clean `Auto` path-tree dispatch, and 4D fallback for holes.

The grid/polygon end-to-end differential compares complete chord families,
minimum-cover selections, selected and added cut unions, optimum values, and
canonical rectangles. The default all-mask `3x3` gate covers 893 supported
ordinary components. The release-mode all-mask `4x4` gate covers 166,189
supported ordinary components. Both report zero disagreements. Grid components
whose boundaries contain point contact or another rejected formal degeneracy
remain outside the v0.9 ordinary-polygon population.

The extended release-mode population adds every free polyomino through ten
cells, endpoint-contact and topological stress fixtures, the external-Oracle
adversarial corpus, path-tree geometry families through parameter 12, stored
and generated mixed-branching witnesses, dense-conflict grids, complete-
bipartite `t=1..4`, and 1,000 deterministic connected random regions. It covers
7,529 inputs and 7,276 supported polygon components, with 255 explicit ordinary-
model rejections and zero differences. Its `Auto` representation selected the
clean polygon path-tree for 3,153 components and fell back to exact 4D for 4,123
components.

The isolated OR-Tools CP-SAT rerun covers 6,998 bounded grid inputs and 27,228
components across all binary `3x3` grids, free polyominoes through ten cells,
and 13 exported adversarial fixtures. Every CP-SAT, exact-cover, and Rust
comparison agrees; the disagreement count is zero.

The native fixture corpus includes nonuniform coordinate spacing, a
one-billion-unit coordinate gap, two ordinary holes, a comb, a spiral corridor,
a nonuniformly scaled complete-bipartite conflict family, and a stretched
reflex-heavy boundary. The complete-bipartite fixture has four horizontal and
four vertical chords, every cross pair intersects, and `Auto` selects path-tree.
The stretched fixture exercises at least eight reflex vertices and ten compressed
x coordinates. The large-gap rectangle creates exactly two x coordinates, two
y coordinates, and one atomic arrangement cell. The bounded raster Oracle
rejects that fixture at its width limit before allocating cells; production
diagnostics record `raster_oracle_used=false`.

These results do not claim ornaments, point/segment holes, a general
Soltan--Gorpinevich sweep, general `O(n log n)` completion, or an almost-linear
flow backend.

## v1.0 indexed polygon engine evidence

The v1.0 campaigns rerun the complete polygon semantics with the reference and
indexed backends. All binary `3x3` polygons contribute 893 supported components;
all binary `4x4` polygons contribute 166,189. The extended campaign contributes
7,394 supported components from free polyominoes through ten cells, ordinary
holes, endpoint/topology adversaries, path-tree witnesses, complete-bipartite
and dense families, 1,000 deterministic random regions, polygon-native A-H
families, and metamorphic transforms. Every population has zero disagreements.

The 13-case negative campaign combines nine structural polygon categories with
four intentionally invalid rectangle sets. Reference and indexed validators
return the same canonical categories in every case. The A-H scaling campaign
contains 40 verified rows at sizes `1,2,4,8,16`; it records separate preparation,
chord, flow, completion, arrangement, validation, total-time, and owned-byte
diagnostics. Indexed rows report zero Definition 7 full-boundary scans, zero
global completion candidate rebuilds, zero completion full-boundary/full-cut
scans, and zero rectangle-per-cell validator tests.

The committed sources are `results/v1.0-polygon-differential-3x3.json`,
`results/v1.0-polygon-differential-4x4.json`,
`results/v1.0-polygon-backend-differential.json`,
`results/v1.0-polygon-negative.json`,
`results/v1.0-polygon-native-fixtures.json`, and
`results/v1.0-polygon-scaling.csv`. Numerical paper tables are generated from
these files.
