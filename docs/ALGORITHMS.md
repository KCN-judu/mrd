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
3. enumerate aligned reflex-vertex pairs satisfying Definition 7 in this grid
   model: every open unit subinterval has component cells on both sides;
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

Every horizontal--vertical pair is checked against closed geometric
intersection before graph algorithms run.

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

## Paper-to-code traceability

Implementation runtimes below describe the checked practical code, not an
asymptotic claim for the paper as a whole. Let `N` be grid cells, `n` boundary
complexity, `q` effective chords, `E` explicit conflicts, and `sigma` total
biclique vertex occurrences.

| Paper theorem or construction | Source module | Main public function | Runtime used in implementation | Theoretical runtime from paper | Correctness test | Independent oracle | Known limitations |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Colored component extraction | `rect-core::grid` | `ColorGrid::four_connected_components` | `O(N)` flood fill | Input-adapter step, not the paper bottleneck | corner contact splits | Python oracle connectivity | finite grids only |
| Boundary and effective-chord generation, SG Definition 7 | `rect-core::boundary`, `rect-oracle-sg` | `Boundary::from_component`, `enumerate_effective_chords` | exact boundary cancellation plus aligned-reflex pair and span tests | `O(n log n)` enumeration in Soltan--Gorpinevich | rings, topology families, exhaustive grids | exact cover and CP-SAT optima | no ornaments or degenerate formal holes |
| Closed horizontal/vertical chord intersection | `rect-core::geometry` | `closed_chords_intersect` | `O(1)` integer comparisons | `O(1)` predicate | every endpoint case and signed exhaustive segment range | independently coded strict dominance | closed chords; endpoint contact conflicts |
| Endpoint-preserving 4D parity embedding | `rect-dominance::embedding` | `DominanceEmbedding::new`, `assert_pairwise_equivalence` | `O(q log q + h*v)` including pairwise audit | rank embedding plus dominance reporting | endpoint and metamorphic suites | closed geometric predicate | explicit audit is intentionally quadratic |
| Explicit conflict graph | `rect-oracle-sg`, `rect-dominance::embedding` | `build_conflict_graph`, `explicit_graph` | `O(h*v)` | output-sensitive reporting in the paper pipeline | edge equality on every solve | independently built geometric graph | materializes all `E` edges |
| Maximum bipartite matching | `rect-graph::hopcroft_karp` | `hopcroft_karp` | `O(E sqrt(q))` | same classical bound for explicit graph | matching/flow equality | C0 and compressed Dinic | explicit graph required |
| Konig minimum vertex cover | `rect-graph::hopcroft_karp` | `minimum_vertex_cover` | `O(E+q)` after matching | linear alternating reachability | every edge covered and size equals matching | residual-flow cover | bipartite graphs only |
| Cardinal--Yuditsky Theorem 8 partition | `rect-dominance::biclique` | `comparability_theorem_8`, `verify_exact_partition` | recursive sorting plus `O(E+sigma)` certificate audit | general `O(q log^d q)` bound specializes to `sigma = O(q log^4 q)` for `d = 4` | completeness, uniqueness, fabricated-edge, recursion, and coordinate audits | explicit edge set | practical recursive sorting retained |
| C0 flow reduction | `rect-dominance::biclique` | `BicliquePartition::from_explicit_edges` | `O(E)` construction, then Dinic | one biclique per edge baseline | C0 flow equals matching | Hopcroft--Karp | no compression |
| Compressed flow reduction | `rect-dominance::compressed_flow` | `solve_biclique_flow` | `O(q+sigma)` network construction, then Dinic | compact graph plus asymptotically fast exact flow | dense and full differential suites | C0 and Hopcroft--Karp | Dinic backend, not almost-linear flow |
| Integral max flow and residual min cut | `rect-graph::dinic` | `MaxFlowBackend::max_flow_min_cut` via `DinicBackend` | generic Dinic `O(V^2 A)` bound | almost-linear exact backend cited by paper | flow value, cut capacity, no internal large-capacity cut | Hopcroft--Karp cover | integral capacities only |
| Geometric completion, SG Section 10 | `rect-oracle-sg` | `complete_with_selected_chords` | explicit integer cut insertion and cell-region recovery | linear completion after chord choice in the source construction | formula count and metamorphic mapped-back outputs | exact cover and CP-SAT selected rectangles | ordinary grid regions only |
| Final dissection validation | `rect-core::validation` | `validate_dissection` | linear in component cells plus enumerated rectangle area | verification layer, not paper runtime | invoked for every solver output | independently produced outputs use same exact cell contract | integer-grid rectangles only |

The current effective-chord enumerator is exact for the supported grid model,
but it is not an implementation of the `O(n log n)` Soltan--Gorpinevich
enumeration algorithm.

Dinic is the practical implemented flow backend. The almost-linear exact
max-flow result is cited only for the paper's asymptotic theorem and is not
implemented here.

The implementation starts the comparability-bigraph recursion with four
coordinates. Therefore the general Cardinal--Yuditsky bound specializes to
`O(q log^4 q)`, not `O(q log^3 q)`.
