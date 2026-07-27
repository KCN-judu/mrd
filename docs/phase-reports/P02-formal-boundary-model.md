# P02 Formal Boundary Model

- Phase: P2
- Branch: `codex/full-implementation`
- Start SHA: `fe1be927a99d1734afb72bd9cc46ee394112da48`
- Implementation commits: `95abbcfae36190eaee8264db73236d5bd7d57ba3`,
  `b8c2d15996400c3bce0ac364adb536982d894532`
- Normative source: Soltan and Gorpinevich, *Minimum Dissection of a
  Rectilinear Polygon with Arbitrary Holes into Rectangles*, Discrete &
  Computational Geometry 9 (1993), Definitions 1, 3, and 4, pp. 58--60,
  DOI `10.1007/BF02189307`
- Started: `2026-07-27T10:07:31Z`
- Audit completed: `2026-07-27T10:37:14Z`
- Correctness disagreements: none

## Scope and acceptance matrix

| Class | Issue | Source or Oracle | Implemented change | Acceptance evidence |
| --- | --- | --- | --- | --- |
| implementation gap | The ordinary polygon type could not represent `Or P`, isolated points, or point/segment formal holes | source conditions (1)--(3), p. 58, and Definition 1, p. 59 | validated `FormalRectilinearPolygon`, `Ornament`, and canonical segments over the unchanged ordinary-region Oracle | focused construction, round-trip, and fixture tests pass |
| semantic gap | Formal interior predicates did not exclude ornament contacts | `Int P = P without Bd P`, Definition 1 | `RectilinearDomain` implementation removes every point or open-segment contact with the ornament | formal-domain tests pass; empty ornament matches the ordinary Oracle on 2,025 point and 24,840 segment queries |
| representation gap | Formal vertices, elementary segments, provenance, incidence, and formal holes were unavailable | Definitions 3 and 4, pp. 59--60 | deterministic derived incidence with stable IDs and connected formal-boundary components | fixture yields 18 vertices, 16 elementary segments, one exterior, and four formal holes |
| validation gap | Invalid ornament geometry had no structured rejection | source conditions (1)--(3), p. 58 | structured errors for containment, alignment, duplicates, overlap, and non-vertex intersections | negative-category tests pass without weakened assertions |
| integration gap | CLI input could silently be mistaken for an ordinary polygon | P2 contract and P3 phase boundary | tagged formal JSON detection, canonical `verify` output, and explicit solver rejection | end-to-end verify succeeds; solve exits with the P3-unavailable error |
| documentation gap | Public scope still described all formal-boundary data as unsupported | source mapping and implemented API | formal model, algorithms, input, limitations, references, and README documentation updated | staged documentation and local-path inspection pass |

The source permits topological contours to share vertices and multiple outer
components. P2 deliberately preserves the stricter existing
`RectilinearPolygon` topological-region Oracle, so those ordinary contour cases
remain unsupported. This restriction is documented and does not weaken any
accepted ornament condition.

## Phase-specific evidence

| Command | Exit | Duration | Result |
| --- | ---: | ---: | --- |
| `cargo test -p rect-core formal_polygon` | 0 | 0.03s command wall time; 0.07s reported test time | 6 passed; canonicalization, incidence, formal predicates, structured negatives, and round-trip covered |
| `cargo test -p rect-cli formal_polygon` | 0 | 4.08s | tagged fixture auto-detection and canonical round-trip passed |
| `cargo test -p rect-cli native_polygon_fixture_corpus_validates_and_solves` | 0 | 0.01s reported test time | all seven permanent ordinary fixtures retained empty-ornament incidence and ordinary solving behavior |
| `cargo run --quiet -p rect-cli -- verify --input-format formal-polygon --input test-data/polygons/formal-boundary.json --output tmp/p2-formal-verify.json` | 0 | 1.06s | canonical validation output generated |
| `jq` incidence summary over the generated output | 0 | <0.01s | 18 vertices, 16 elementary segments, 1 exterior, 4 formal holes |
| `cargo run --quiet -p rect-cli -- solve --solver dominance-compressed --input-format formal-polygon --input test-data/polygons/formal-boundary.json` | 1 expected | 0.29s | explicit P3-unavailable error; no ornament-dropping fallback |

The empty-ornament differential exhaustively compares every doubled-coordinate
point in `[-2, 42]^2` and every horizontal and vertical open segment with
integer endpoints in `[-1, 21]` at doubled coordinates in `[-2, 42]` on a
two-hole region. All 26,865 predicate comparisons agree exactly with the
ordinary polygon Oracle.

## Mandatory audit

| Command | Exit | Duration | Result |
| --- | ---: | ---: | --- |
| `git status --short` | 0 | <0.01s | only untracked source-review material under `tmp/` |
| `git diff --check` | 0 | <0.01s | clean |
| `cargo fmt --all -- --check` | 0 | 0.25s | clean |
| `python3 tools/check_biclique_bound.py` | 0 | 0.03s | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 4.42s | no issues |
| `cargo test --workspace` | 0 | 398.79s reported suite time | 129 passed, 0 failed, 3 ignored |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 4.61s | warning-free documentation generated |
| `cargo build --workspace --release` | 0 | 13.69s | all six crates built in optimized mode |
| `python3 tools/check_release_consistency.py` | 0 | 1.99s | v1.3 release and P1 baseline provenance remain consistent |

The three ignored tests are unchanged, explicitly named release-scale 4x4 and
extended polygon differential campaigns. P1 executed and froze them
independently; P2 neither altered their annotations nor their covered modules.
The complete staged diff was inspected. Secret, credential, private-key,
machine-local absolute-path, stale generated-evidence, fallback, and accidental
ignore scans found no issue. The generated `tmp/` source PDF, page renders, and
CLI audit output are intentionally untracked and excluded from commits.

## Result inventory and limitations

Committed P2 evidence consists of the canonical formal-boundary fixture, its
focused and Oracle-differential tests, the source-mapped model documentation,
and this report. No generated release artifact was changed, so the permanent
release-consistency checker remains anchored to the immutable v1.3 and P1
baselines.

P2 does not enumerate effective chords, execute the formal-hole SG event model,
complete subdivisions, validate formal-hole dissections, or solve formal input.
Those are explicit P3 requirements. Ordinary `RectilinearPolygon` and all
ordinary solver backends remain available as permanent Oracles.
