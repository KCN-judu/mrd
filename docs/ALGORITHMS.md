# Algorithms and acceptance matrix

## Solver A: independent exact cover

`rect-oracle-exact-cover` enumerates every positive-area integer-grid rectangle
whose cells all belong to one component. A dynamic bitset represents each
option. The recursive search chooses an uncovered cell with the fewest currently
disjoint options, rejects overlap, and prunes with
`ceil(uncovered cells / largest available option area)`. The initial incumbent
is the valid all-singleton cover. It returns the selected rectangles, not only a
count.

This solver does not call boundary extraction, effective-chord generation,
matching, dominance, bicliques, or flow.

## Solver B: explicit Soltan--Gorpinevich

For ordinary polygons formed by unit grid cells, `rect-oracle-sg` performs:

1. cancel shared oriented cell edges and trace boundary loops;
2. identify local-nonconvexity vertices by right turns with formal interior on
   the left;
3. enumerate effective chords with the grid interior-run algorithm by default,
   retaining the aligned-reflex pair implementation as a reference Oracle;
4. build every closed horizontal--vertical intersection edge explicitly;
5. run Hopcroft--Karp and alternating-reachability minimum-cover recovery;
6. select the complement independent chord family;
7. add selected cuts, then horizontal and vertical simple chords as in Section
   10, Steps 3--4, and recover rectangular cell regions.

The implementation checks the formula
`r + 1 - holes - effective_chords + matching` and validates the matching,
vertex cover, independent family, completion count, and explicit rectangles.

## Solver C: paper algorithm

`rect-dominance` ranks all relevant coordinates and implements the paper's
parity encoding exactly:

```text
alpha(h) = (2 rank(l), -2 rank(r), 2 rank(y), -2 rank(y))
beta(v)  = (2 rank(x)+1, -2 rank(x)+1,
            2 rank(t)+1, -2 rank(b)+1)
```

`VerificationMode::FullyAudited` checks every horizontal--vertical pair against
closed geometric intersection, materializes the explicit graph, runs
Hopcroft--Karp, and audits the biclique edge multiset. This is the validation
and regression mode used for the v0.2 evidence.

`VerificationMode::CompactOnly` instead follows only:

```text
effective chords -> 4D embedding -> Theorem 8 partition
                 -> compressed flow -> geometric completion
```

It does not call pairwise embedding equivalence, explicit graph construction,
Hopcroft--Karp, C0 construction, or full edge-partition audit. Cross-side
coordinate equality is checked with per-coordinate value indexes, and block
structure is checked in `O(sigma)` by coordinate extrema: for each block and
coordinate, the maximum left embedding value must be strictly below the
minimum right value. This proves every Cartesian-product pair is a valid
dominance edge without expanding the product. Index bounds and duplicate IDs
are checked in the same pass.
Compact-only diagnostics serialize `explicit_conflict_edge_count` as `null`.

On a clean hole-free component, `--representation path-tree` replaces the
four-dimensional edge partition with a geometry-derived region dual and tree
paths. FullyAudited retains the `ReferenceAreaFloodFill` dual, explicit
per-path BFS, physical transpose Oracle, and both orientations as independent
references. CompactOnly uses the `BoundaryLaminar` dual and endpoint-only HLD
records (`CompactTreePath`); its axis view avoids occupancy, boundary, and chord
transposition in the production path for either orientation. Orientation selection is controlled by
`--path-tree-orientation build-both|bound-estimate|vertical-tree|horizontal-tree`;
FullyAudited and CompactOnly default to exact `build-both`; `bound-estimate`
remains an explicit heuristic benchmark policy because the v0.8 witness audit
found positive regret cases.
Heavy-light canonical segment nodes produce the bicliques consumed by the same
compressed flow backend. `--representation auto` selects this representation
only when the clean certificate passes and records a compact 4D fallback
otherwise. The boundary dual is a finite-grid realization rather than the
paper's general planar sweep; see `docs/BOUNDARY_DUAL_CONSTRUCTION.md` and
`docs/PATH_TREE_REPRESENTATION.md`.

Stage C0 creates one biclique per explicit dominance edge. Stage C1 implements
the proof recursion of Cardinal--Yuditsky Theorem 8: split points by the current
coordinate, recurse on low-left/high-right after dropping that coordinate, and
recurse within each half without dropping it. The implementation verifies both
edge-set equality and multiplicity one, so the output is an edge partition.

A fixed-dimensional comparability bigraph has two point sets and an edge from
one side to the other exactly when one point is strictly smaller in every
coordinate. Here the relation is strict coordinatewise dominance in four
coordinates. The parity embedding prevents equal coordinates across the two
sides, as required by the recursive strict-order construction. Theorem 8 gives
the biclique representation and Lemma 12 supplies its constructive generation.

A biclique cover may represent an edge more than once; a biclique partition may
not. The flow reduction only needs a cover, but this implementation claims the
stronger partition property and audits that every explicit edge has
multiplicity exactly one.

Each biclique becomes one internal flow node. Outer arcs have capacity one and
internal arcs use `min(horizontal_count, vertical_count) + 1`. Dinic returns an
exact integral max flow and residual minimum cut. The implementation rejects a
cut that crosses an internal arc, recovers the vertex cover, and compares its
size and flow value to independent Hopcroft--Karp.

Write `U = min(|H|, |V|) + 1`. The unit capacities on `s -> h` and
`v -> t` enforce matching endpoints, and `U - 1` is at least every possible
matching value. A large internal capacity is not needed merely to obtain the
correct maximum-flow value: any particular `h -> z_k` arc is downstream of one
unit `s -> h` arc, and any particular `z_k -> v` arc is upstream of one unit
`v -> t` arc, so each carries at most one unit in an integral matching flow.
The purpose of `U` is certificate recovery: an integral minimum cut of value at
most `U - 1` cannot cut an internal arc, so its outer arcs directly encode a
minimum vertex cover.

## Solver D: boundary-native ordinary polygons

`rect-core::polygon` normalizes one outer orthogonal loop and ordinary hole
loops with exact `i128` signed area, then performs an explicit `O(n^2)` segment
audit. `Boundary::from_polygon` produces the same compact loop/reflex/index
semantics used by the grid pipeline without expanding long edges into units.

`GeneralPolygonPairwiseEnumerator` and `IndexedPolygonPairwiseEnumerator`
remain independent Definition 7 Oracles. Production
`SoltanGorpinevichSweepEnumerator` uses one axis-generic event/status sweep:
for the accepted ordinary-loop model, the unique strict-interior ray of each
reflex vertex emits its nearest reflex boundary hit. The source-mapped
specialization, deterministic endpoint event ordering, bounded certificate, and
excluded formal-boundary cases are recorded in
`docs/SOLTAN_SWEEP_IMPLEMENTATION.md`. FullyAudited compares all three complete
chord families; CompactOnly defaults to `sg-sweep`. The selected family feeds
the unchanged 4D embedding, Theorem 8 partition, compressed flow, and
minimum-cut cover. Clean hole-free polygons may use the boundary-laminar
path-tree partition; `Auto` falls back to 4D otherwise.

`CoordinateCompressedCompletion` inserts selected chords, then horizontal and
vertical simple chords, and flood-fills atomic coordinate rectangles; it remains
the dense reference. CompactOnly uses the same policy with a statically closed
dynamic stabbing index, sparse half-edge face recovery, and an exact vertical
slab validator. The polygon validator independently checks positive area,
containment, holes, interior disjointness, exact union, total area, and the
declared rectangle count. No production polygon stage rasterizes by coordinate
magnitude or materializes the coordinate Cartesian product.

## Solver E: formal-boundary polygons

`rect-core::formal_polygon` implements Soltan--Gorpinevich Definitions 1, 3,
4, and 7 on an ordinary connected region plus a normalized ornament. Section
10 Step 1(a)--(d) is implemented by
`FormalRectilinearPolygon::effective_chords_source`; the pairwise Definition 7
enumerator remains an independent permanent Oracle. Section 10 Step 2 uses an
exact common-denominator symbolic perturbation and proves that every original
orthogonal intersection is preserved.

`rect-dominance::complete_formal_polygon` compares explicit Hopcroft--Karp and
compact biclique-flow minimum covers, selects their maximum independent chord
family, and evaluates Theorem 2 as `m + c - h - e`. Completion inserts the
selected chords, treats ornament elementary segments as barriers, applies the
source horizontal-then-vertical Step 4 policy, and requires dense and sparse
canonical rectangle recovery to agree. Validation covers the ordinary region
exactly and additionally requires every formal vertex and elementary segment
to be realized on rectangle sides.

## Acceptance matrix

| Requirement | Evidence | Acceptance |
| --- | --- | --- |
| Grid components use four-connectivity | `rect-core::grid` unit test | corner-only contact is split |
| Boundary area and holes are exact | boundary invariant and ring test | signed doubled area equals twice the cell count |
| Exact-cover output is optimal and explicit | exhaustive differential tests | matches SG/C0/C1 through all `3x3` grids |
| Effective chord conflicts are explicit | SG certificate checks | every edge is a closed intersection |
| 4D embedding preserves endpoints | independent exhaustive segment test | every pair satisfies intersection iff strict dominance |
| C0 flow reduction is correct | graph unit test and differential suite | Dinic value equals Hopcroft--Karp |
| C1 is a partition, not only a cover | `verify_exact_partition` | every edge has multiplicity exactly one |
| Geometric output is valid | solver-independent validator | positive rectangles cover each component cell exactly once |
| Polygon normalization is canonical | normalization and metamorphic tests | orientation/start/collinear variants normalize identically |
| General polygon chords satisfy Definition 7 | three-backend chord-family differential and source-invariant tests | complete horizontal/vertical families, endpoints, and deterministic IDs match |
| Polygon completion is coordinate native | cut and rectangle differential | selected/added cut unions and rectangles equal the grid Oracle |
| Polygon rectangles form an exact dissection | coordinate-compressed validator | no outside coverage, holes, overlap, or uncovered interior |
| Formal Step 1 matches Definition 7 | source construction versus pairwise Oracle | chord identities and provenance match on every formal fixture and exhaustive point lattice |
| Formal Step 2 preserves conflict semantics | exact transformed-intersection audit | original and transformed closed intersections are identical |
| Formal maximum admissible family is exact | explicit matching versus compact biclique flow | covers, selected chords, and effective number agree |
| Formal completion realizes Definition 2 | dense/sparse recovery and formal-boundary coverage audit | every formal vertex/segment is on rectangle sides and the count equals `m + c - h - e` |
| Empty ornament preserves ordinary semantics | formal fixture differential campaign | optimum counts and canonical rectangles equal the fully audited ordinary solver |

## Paper-to-code traceability

Implementation runtimes below describe the checked practical code, not an
asymptotic claim for the paper as a whole. Let `N` be grid cells, `n` boundary
complexity, `q` effective chords, `E` explicit conflicts, and `sigma` total
biclique vertex occurrences.

| Paper theorem or construction | Source module | Main public function | Runtime used in implementation | Theoretical runtime from paper | Correctness test | Independent oracle | Known limitations |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Colored component extraction | `rect-core::grid` | `ColorGrid::four_connected_components` | `O(N)` flood fill | Input-adapter step, not the paper bottleneck | corner contact splits | Python oracle connectivity | finite grids only |
| Prepared ordinary polygon input | `rect-core::polygon_index` | `PreparedPolygonContext::new_with_validator` | one normalization/validation/boundary/index build with owned metadata | input-model step | exact build-count and backend differential | v0.9 standalone APIs | no ornaments or degenerate holes |
| Formal boundary representation | `rect-core::formal_polygon` | `FormalRectilinearPolygon::new`, `incidence` | deterministic sort plus exact pairwise structural validation and output-sized incidence | Soltan--Gorpinevich Definitions 1, 3, and 4, pp. 58--60 | round-trip, normalization, incidence, formal-domain, negative, and CLI fixture tests | empty-ornament ordinary polygon | one connected ordinary interior component |
| Formal effective chords | `rect-core::formal_polygon` | `effective_chords_source`, `effective_chords_pairwise` | axis-line events and source merge/delete fixed point; pairwise Oracle retained | Definition 7 and Section 10 Step 1(a)--(d), pp. 62, 76--77 | Fig. 3, source invariants, exhaustive isolated-point lattice, ordinary parity | exact pairwise Definition 7 enumerator | no full paper runtime claim beyond recorded counters |
| Formal admissible-family reduction | `rect-dominance::formal` | `analyze_formal_admissible_family` | 4D biclique construction plus exact Dinic; explicit conflict graph and Hopcroft--Karp audited | Section 10 Step 2, pp. 77--78, and Theorem 2 | transformed-intersection, matching, cover, and selected-family equality | explicit closed-intersection graph | exact Dinic backend; almost-linear flow is a later phase |
| Formal completion and validation | `rect-dominance::formal`, `rect-oracle-sg::polygon` | `complete_formal_polygon`, `CoordinateCompressedCompletion::complete_formal` | deterministic candidate passes, indexed ray stops, dense/sparse recovery and dual sparse validation | Section 10 Steps 3--4, pp. 76, 78; Definition 2, p. 58 | five permanent fixtures, 511 point lattices, formal campaign, ordinary parity | ordinary completion, dense recovery, reference slab validator | no claimed classical linear completion bound |
| Exact indexed polygon queries | `rect-core::polygon_index` | `OrthogonalEdgeIndex` | segment-tree stabbing and sorted line groups; output-sensitive reporting | predicate layer | point/segment/ray query differential | linear polygon predicates | integer coordinates only |
| Polygon structural validation | `rect-core::polygon_index` | `OrthogonalSweepValidator` | deterministic orthogonal sweep with exact integer events | input-model step | accepted polygon and broad negative-category differential | `ReferenceQuadraticValidator` | ordinary polygon model only |
| Reference polygon effective chords | `rect-oracle-sg::polygon` | `GeneralPolygonPairwiseEnumerator::enumerate_prepared_with_metrics` | `O(r^2 n)` direct Definition 7 audit | independent Oracle, not production | exact 3x3/4x4 and extended chord-family differential | grid-native enumerator | ordinary polygon model only |
| Indexed polygon pairwise effective chords | `rect-oracle-sg::polygon` | `IndexedPolygonPairwiseEnumerator::enumerate_prepared` | `O(n log n + C polylog n + Z)`-style aligned-pair path | intermediate Oracle path | exact three-backend differential | `GeneralPolygonPairwiseEnumerator` | `C` can be quadratic |
| SG ordinary-polygon sweep effective chords | `rect-oracle-sg::polygon` | `SoltanGorpinevichSweepEnumerator::enumerate_prepared` | `O(n log n + q)` status construction and output writing | Section 10 Step 1, pp. 76--77, specialized to ordinary loops | source-invariant, 3x3/4x4, extended, hole, metamorphic, and candidate-gap campaigns | both pairwise enumerators | no ornaments, isolated points, or degenerate formal holes |
| Indexed polygon completion | `rect-oracle-sg::polygon` | `IndexedPolygonCompletion::complete_prepared` | incremental endpoint/intersection frontier plus indexed ray shooting | general linear/log-linear construction in source model | exact selected/added cuts and rectangles | `CoordinateCompressedCompletion` | no full classical completion bound claim |
| Shared polygon arrangement | `rect-oracle-sg::polygon_arrangement` | `PreparedCoordinateArrangement::new` | scanline span fill, dense barriers/recovery, difference-array validation | verification/output layer | exact rectangle and invalid-output differential | reference recovery/validator | `O(|X||Y|)` storage |
| Dynamic polygon cut index | `rect-oracle-sg::polygon_cut_index` | `DynamicStabbingCutIndex` | static-universe segment-tree stabbing plus canonical line intervals | data-structure implementation of completion ray/query needs | line-map equality and zero scan counters | `DynamicPolygonCutIndex` | ordinary integer-coordinate closure only |
| Sparse polygon recovery | `rect-oracle-sg::polygon_sparse` | `SparseOrthogonalSubdivision::new_with_backend` | closed-endpoint orthogonal sweep plus per-segment splits and half-edge face walk; `O((S+J) log S)` | output layer, not claimed as the source's full general completion bound | exact junction/atomic-edge/face/rectangle equality | v1.2 range scan and dense arrangement | ordinary nondegenerate loops only |
| Sparse polygon validation | `rect-oracle-sg::polygon_sparse` | `SparseSlabValidator::validate_with_backend` | x events plus lazy y segment tree; no successful-path slab rescans | exact dissection validation | exact category differential | v1.2 slab rescan and dense difference-array validator | no degenerate formal holes |
| Boundary and effective-chord generation, SG Definition 7 | `rect-core::boundary`, `rect-oracle-sg` | `PreparedComponentContext`, `GridInteriorRunEnumerator::enumerate_prepared` | one component-local preparation plus output-sensitive prepared-run enumeration; pairwise reference retained | `O(n log n)` enumeration in Soltan--Gorpinevich | exact chord-family equality on exhaustive, polyomino, hole, adversarial, dense, and random populations | `ReferencePairwiseEnumerator` | no ornaments, degenerate formal holes, or general polygon sweep |
| Closed horizontal/vertical chord intersection | `rect-core::geometry` | `closed_chords_intersect` | `O(1)` integer comparisons | `O(1)` predicate | every endpoint case and signed exhaustive segment range | independently coded strict dominance | closed chords; endpoint contact conflicts |
| Endpoint-preserving 4D parity embedding | `rect-dominance::embedding` | `DominanceEmbedding::new`, `assert_pairwise_equivalence` | `O(q log q)` construction; FullyAudited adds `O(h*v)` pairwise audit | rank embedding plus dominance reporting | endpoint and metamorphic suites | closed geometric predicate | pairwise audit is excluded from CompactOnly |
| Explicit conflict graph | `rect-oracle-sg`, `rect-dominance::embedding` | `build_conflict_graph`, `explicit_graph` | `O(h*v)` | output-sensitive reporting in the paper pipeline | edge equality in FullyAudited | independently built geometric graph | excluded from CompactOnly |
| Maximum bipartite matching | `rect-graph::hopcroft_karp` | `hopcroft_karp` | `O(E sqrt(q))` | same classical bound for explicit graph | matching/flow equality in FullyAudited | C0 and compressed Dinic | excluded from CompactOnly |
| Konig minimum vertex cover | `rect-graph::hopcroft_karp` | `minimum_vertex_cover` | `O(E+q)` after matching | linear alternating reachability | every edge covered and size equals matching | residual-flow cover | bipartite graphs only |
| Cardinal--Yuditsky Theorem 8 partition | `rect-dominance::biclique` | `comparability_theorem_8`, `verify_structure`, `verify_exact_partition` | recursive sorting plus `O(sigma)` structure checking; FullyAudited adds `O(E+sigma)` edge audit | general `O(q log^d q)` bound specializes to `sigma = O(q log^4 q)` for `d = 4` | completeness, uniqueness, fabricated-edge, recursion, and coordinate audits | explicit edge set in FullyAudited | practical recursive sorting retained |
| C0 flow reduction | `rect-dominance::biclique` | `BicliquePartition::from_explicit_edges` | `O(E)` construction, then a selected exact backend | one biclique per edge baseline | C0 flow equals matching | Hopcroft--Karp | no compression |
| Compressed flow reduction | `rect-dominance::compressed_flow` | `solve_biclique_flow` | `O(q+sigma)` network construction, then a selected exact backend | compact graph plus asymptotically fast exact flow | dense and full differential suites compare FullyAudited and CompactOnly | C0 and Hopcroft--Karp in FullyAudited | practical backend, not almost-linear flow |
| Integral max flow and residual min cut | `rect-graph::dinic` | `MaxFlowBackend::max_flow_min_cut` via `DinicBackend` or `PushRelabelBackend` | generic Dinic or highest-label push-relabel with global relabel and gap heuristics | almost-linear exact backend cited by paper | flow value, cut capacity, no internal large-capacity cut | mutually differential practical backends and Hopcroft--Karp cover | integral capacities only; no almost-linear claim |
| Geometric completion, SG Section 10 | `rect-oracle-sg` | `complete_with_prepared_backend`, `DenseCutGrid` | indexed component-local frontier with dense cuts by default; ordered reference retained | linear completion after chord choice in the source construction | exact selected/added cut and canonical-rectangle differential populations | `ReferenceRescanCompletion` | ordinary grid regions only |
| Rectangle recovery | `rect-oracle-sg` | `DenseGridRecovery` | dense visited mask, reusable integer queue, prefix-sum rectangularity proof | verification/output layer | exact rectangle equality against `ReferenceHashBfsRecovery` | hash BFS | component-local area storage |
| Final dissection validation | `rect-core::validation` | `validate_dissection_prepared` | reuses prepared occupancy; linear in local area plus rectangle area | verification layer, not paper runtime | ordinary and prepared validators agree | independently produced outputs use same exact cell contract | integer-grid rectangles only |
| Clean eligibility and endpoint alternation | `rect-oracle-sg` | `classify_clean_hole_free`, `endpoints_alternate` | integer loop IDs and modular interval tests | Definition 9.1 / Theorem 9.4 scope for supported grid model | endpoint and hole fixtures | closed chord predicate | ornaments and degenerate formal holes unsupported |
| Region-dual path-tree partition | `rect-dominance::path_tree` | `build_path_tree_partition` | BoundaryLaminar endpoint intervals + endpoint HLD in CompactOnly; area flood-fill Oracle in FullyAudited | `O(q log^2 q)` structural biclique bound; general polygon dual sweep not implemented | tree/path/partition audits and axis-view differential tests | transposed area dual and explicit graph | finite unit-grid clean components only |

The current effective-chord implementation has interchangeable
`ReferencePairwiseEnumerator` and `GridInteriorRunEnumerator` paths. The latter
is a grid-specialized `O(N + r log r + q)` run-index algorithm; it is not an
implementation of the general `O(n log n)` Soltan--Gorpinevich enumeration
algorithm. CompactOnly defaults to grid runs and the CLI override
`--chord-enumerator reference-pairwise|grid-interior-runs` remains available.
See `docs/GRID_CHORD_ENUMERATION.md`.

The production path reuses one `BoundaryIndex` and one effective-chord
endpoint table. CompactOnly therefore performs no linear boundary lookup or
pairwise endpoint comparison. Its O(sigma) biclique check is coordinate
extrema validation for each block, not Cartesian-product expansion.

The exact horizontal-then-vertical completion policy, unit-cut semantics, and
backend differential contract are specified in `docs/GEOMETRIC_COMPLETION.md`.

Dinic and highest-label push-relabel are practical implemented exact flow
backends. The almost-linear exact max-flow result is cited only for the paper's
asymptotic theorem and is not implemented here.

## Exact min-cost circulation baseline

`rect-graph::min_cost::CirculationNetwork` is a separate generic exact
integer circulation Oracle. It first routes signed node demands over residual
capacity, then `refine_feasible` exhaustively enumerates simple signed
residual cycles and augments the lowest exact cost-to-unit-length ratio while
that ratio is negative. Every recorded refinement step exposes the cycle,
augmentation, and objective before/after the update; the recovered result is
checked for exact balances, capacities, objective value, and absence of a
negative residual cycle. Positive residual lengths and cross-multiplied `i128`
comparisons avoid floating-point decisions.

This is deliberately superlinear and is retained as a correctness Oracle. It
does not implement the FOCS 2023 interior-point method, approximate gradients,
hidden-stability witness, fractional rounding, or dynamic min-ratio-cycle data
structure. Those requirements remain gated by
`docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md`.

`rect-graph::min_ratio_cycle::StableMinRatioLedger` is the P8.1 checked
contract layer for Definitions 4.2--4.5. It uses signed incidence vectors,
exact rational accumulated-flow coordinates, positive integer lengths,
auditor-supplied valid-pair witnesses, and replayable `Update`/`Query`/`Detect`
logs. It rejects invalid circulation, witness, approximation, and factor-two
stability claims. It deliberately does not discover a cycle, conceal a witness,
support graph topology changes, or claim dynamic or amortized performance.

`rect-graph::rooted_forest::DynamicRootedForest` is the P8.2 deterministic
baseline for the rooted-forest portion of Definitions 5.2--5.3. It permits
only graph-edge deletion and explicitly listed incident-edge vertex splits;
the selected forest edge set only decreases. Root-path updates/queries use
exact integers, and stretch certificates are checked by static BFS
recomputation of Definition 5.3 from the current snapshot. Its recourse
counters are evidence, not a proof or claim of Lemma 5.4's construction or
runtime.

`rect-graph::decremental_spanner::DecrementalSpanner` is the P8.3 checked
certificate layer for the simple undirected deletion/vertex-split domain of
Theorem 8.2. A certificate supplies its subgraph and explicit simple embedding
path for every active input edge; validation recomputes path endpoints,
connectivity, congestion, path length, and recourse counters. There is no
directed graph, insertion, arbitrary update, expander, or theorem-runtime API.

`rect-graph::lsf_mwu::ForestCollection` is the P8.4 exact small-instance
baseline for Lemma 5.5's forest collection. It deterministically reweights
edges after exact P8.2 stretch measurements, constructs weighted Kruskal trees,
and emits exact per-edge average-stretch certificates with operation counters.
It does not claim Lemma 5.5's `O(log^7 n)` guarantee or its production-scale
construction.

The implementation starts the comparability-bigraph recursion with four
coordinates. Therefore the general Cardinal--Yuditsky bound specializes to
`O(q log^4 q)`, not `O(q log^3 q)`.
