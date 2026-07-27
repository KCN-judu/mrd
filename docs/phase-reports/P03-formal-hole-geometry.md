# P03 Formal-hole SG Sweep and Completion

- Phase: P3
- Branch: `codex/full-implementation`
- Start SHA: `521f82d2a33e31bccfbb1c363fdbf0049751c2d6`
- Implementation commits: `093961f`, `fd1bbc6`, `3d94851`,
  `996ad44`, `659d7fb`, `6cd0845`
- Normative source: Soltan and Gorpinevich, *Minimum Dissection of a
  Rectilinear Polygon with Arbitrary Holes into Rectangles*, Discrete &
  Computational Geometry 9 (1993), Definition 2 (p. 58), Definition 7
  (p. 62), Section 10 Steps 1--4 (pp. 76--78), and Theorem 2,
  DOI `10.1007/BF02189307`
- Started: `2026-07-27T10:39:44Z`
- Audit completed: `2026-07-27T12:41:01Z`
- Correctness disagreements: none

## Scope and acceptance matrix

| Subphase | Source contract | Implemented evidence | Acceptance |
| --- | --- | --- | --- |
| P3.1 | Definition 7 and formal local nonconvexity | exact formal vertex sectors, local measure, pairwise chord Oracle, Fig. 3 fixture | source example and exhaustive isolated-point subsets pass |
| P3.2 | Section 10 Step 1(a)--(d) | axis-generic merge/delete fixed point, provenance, structural counters | source construction equals pairwise Oracle and empty-ornament ordinary sweep |
| P3.3 | Section 10 Step 2 and Theorem 2 | exact symbolic perturbation, explicit matching Oracle, compact biclique flow, `m + c - h - e` | transformed intersections, matching values, deterministic covers, selected family, and optimum count agree |
| P3.4 | Section 10 Steps 3--4 and Definition 2 | ornament barriers, horizontal-then-vertical completion, dense/sparse recovery, formal-boundary coverage | canonical recoveries agree; every formal point/segment is realized; output count is optimal |
| P3.5 | public integration and reproducibility | formal `solve`, five permanent fixtures, machine-readable campaign, theorem/code documentation | production CLI, formal campaign, ordinary differential, and full audit pass |

The minimized mask-9 regression from the 3-by-3 isolated-point lattice proved
that Step 4(a) cannot treat every evolving nonconvex vertex as a horizontal
candidate. The implementation now permits a non-isolated horizontal candidate
only when incident to an east/west barrier. The regression remains covered by
the exhaustive 511-subset formal test and was not hidden by a fallback or a
weakened assertion.

## Phase-specific evidence

| Command | Exit | Duration | Result file or acceptance |
| --- | ---: | ---: | --- |
| `cargo test -p rect-dominance formal -- --nocapture` | 0 | 0.48s | 6 passed; source example, shared endpoint, segment hole, ordinary parity, and exhaustive point lattice |
| `cargo run --quiet -p rect-cli -- benchmark --suite formal-fixtures --output results/p3-formal-fixtures.json` | 0 | 2.14s | 8/8 verified, 0 solver errors, 0 disagreements |
| `target/release/rect-cli benchmark --suite polygon-differential --sizes 3,4 --output results/p3-polygon-differential.json` | 0 | 9.21s repeated timed run | 66,046 inputs, 169,426 components, 167,082 supported and verified, 0 errors/timeouts/disagreements |
| counterexample inspection | 0 | <0.01s | `results/p3-polygon-differential.counterexamples.json` contains 0 entries |

The five formal records are point hole, segment hole, attached hole,
shared-endpoint degeneracy, and the paper's Fig. 3 example. The campaign also
compares empty-ornament formal solving against the fully audited ordinary
solver on a rectangle, an L-shaped polygon, and a polygon with an ordinary
hole. All optimum counts and canonical rectangle lists agree. Fig. 3 produces
exactly `15 + 1 - 5 - 4 = 7` rectangles.

## Mandatory audit

| Command | Exit | Duration | Result |
| --- | ---: | ---: | --- |
| `git status --short` | 0 | <0.01s | only the expected P3 evidence and closeout documentation |
| `git diff --check` | 0 | <0.01s | clean |
| `cargo fmt --all -- --check` | 0 | 0.32s | clean |
| `python3 tools/check_biclique_bound.py` | 0 | 0.10s | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 1.73s | no issues |
| `cargo test --workspace` | 0 | 415.48s | 144 passed, 0 failed, 3 ignored across 13 suites |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 0.88s | warning-free documentation generated |
| `cargo build --workspace --release` | 0 | 21.67s | 43 compilation units completed in optimized profile |
| `python3 tools/check_release_consistency.py` | 0 | 3.61s | v1.3 release and P1 baseline provenance remain consistent |

The three ignored tests are unchanged release-scale campaigns already present
at the phase baseline. The complete implementation and staged evidence diffs
were inspected. Scans found no new ignored-test annotation, fallback,
credential, token, private key, whitespace error, or machine-local absolute
path. A first ordinary-campaign invocation recorded an absolute executable
path; that uncommitted duplicate manifest record was removed, the full
population was rerun through a relative command, JSON structure was checked,
and release consistency was rerun successfully.

## Result inventory and limitations

Committed P3 result files are:

- `results/p3-formal-fixtures.json`;
- `results/p3-polygon-differential.json`;
- `results/p3-polygon-differential.counterexamples.json`; and
- the two corresponding relative-command entries in `results/manifest.json`.

The ordinary polygon model remains a permanent Oracle. Dense recovery,
reference slab validation, explicit conflict matching, pairwise Definition 7,
and ordinary completion remain available and are exercised by audited paths.
The supported formal model still uses one connected ordinary interior
component; disconnected outer components and ordinary contour contacts remain
outside the accepted input model. P3 makes no almost-linear flow claim: the
compact network still uses exact Dinic, which is addressed by later phases.
