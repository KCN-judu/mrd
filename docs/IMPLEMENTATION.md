# Minimum Rectangular Dissection Implementation

## Scope and Reading Contract

This document is the authoritative technical account of the implemented
Minimum Rectangular Dissection (MRD) system.  It describes the representation,
algorithmic pipeline, invariants, independent reference implementations, and
claim boundaries of the current workspace.  It intentionally distinguishes
four kinds of statement throughout:

- **Role** states the problem a subsystem is responsible for solving.
- **Design** states the data representation and deterministic procedure used by
  the implementation.
- **Evidence** names the independent comparison, certificate, or campaign that
  supports the stated behavior.
- **Boundary** records unsupported inputs, unimplemented source machinery, or
  a complexity claim that the repository does not make.

Experimental results, test populations, raw benchmark samples, release
provenance, the forward plan, and the historical record remain separate:
[`EXPERIMENTS.md`](EXPERIMENTS.md),
[`BENCHMARK_SAMPLING_REPORT.md`](BENCHMARK_SAMPLING_REPORT.md),
[`TESTING.md`](TESTING.md), [`KNOWN_LIMITATIONS.md`](KNOWN_LIMITATIONS.md),
[`IMPLEMENTATION_MASTER_PLAN.md`](IMPLEMENTATION_MASTER_PLAN.md), and
[`HISTORY.md`](HISTORY.md).  Those documents are evidence and process records;
they do not redefine the implementation contract stated here.

The implementation handles finite colored unit-cell grids, ordinary
integer-coordinate rectilinear polygons, and the supported formal-boundary
extensions described below.  All correctness-critical predicates use exact
integer or rational arithmetic.  SVG output is diagnostic only and never
participates in a decision.

## Architecture and Ownership

### Role

The workspace separates pure domain transformations, deliberately slow
references, optimized experimental paths, verification, and process effects so
that a result's ownership and provenance are visible at the API boundary.

### Design

The packages are organized by responsibility rather than a repeated ambient
prefix:

| Package | Responsibility |
| --- | --- |
| `mrd-domain` | Immutable geometry, normalized inputs, certificates, and validation contracts. |
| `graph` | Reusable exact graph primitives and source-shaped graph algorithms. |
| `exact-cover-oracle` | Deliberately slow definition-level rectangle-cover Oracle. |
| `sg-oracle` | Soltan--Gorpinevich grid and polygon Oracles, plus named experimental backends. |
| `dominance` | Experimental MRD representations, biclique construction, and flow reductions. |
| `verification` | Differential campaigns, adversarial generators, benchmark assembly, and reports. |
| `mrd` | CLI parsing, filesystem IO, timing, process exit, and backend dispatch. |

`oracle` and `experiment` namespaces distinguish an independent reference from
an optimized or source-shaped implementation.  Their shared parent contains
only stable domain types, traits, certificates, and pure orchestration.  A name
therefore omits context that its namespace already supplies: for example,
`dominance::experiment::Mode`, `sg_oracle::grid::oracle::Pairwise`, and
`sg_oracle::grid::experiment::InteriorRuns` name the operation rather than
repeating the domain.

The refactor is intentionally breaking.  Crate roots do not re-export removed
paths and there is no compatibility layer.  Backend selection uses enums,
generics, and monomorphized traits; it introduces neither a trait object nor a
runtime registry.  Domain operations accept explicit inputs and return values
or structured errors.  Local mutation is confined to graph, sweep, queue, and
arena implementations, while IO, clocks, command dispatch, and process exit
remain in `mrd::application`.

### Evidence and boundary

Verification depends on production and Oracle APIs, but algorithm crates do
not depend on verification campaigns.  This directional dependency makes a
differential test able to expose a production bug without allowing a production
path to invoke the verifier as a fallback.  Namespaces improve auditability;
they do not by themselves establish correctness or complexity.

## Exact Geometric Models

### Shared conventions

**Role.** All solver paths must agree on contact, orientation, and coverage so
that a compact representation cannot silently change the dissection problem.

**Design.** Coordinates are signed `i64`, graph indices and grid ranks are
`usize`, and dominance coordinates are checked `i128` expressions of ranks.
A grid cell `(x, y)` is geometrically closed, while coverage and output
rectangles use `[x0, x1) x [y0, y1)`.  Output rectangles have positive integer
width and height.  Chords are nonzero closed segments, so endpoint contact is a
conflict.  Boundary edges have the interior on their left; cancelling shared
opposite cell edges makes outer loops positive in signed area and hole loops
negative.  Exact signed doubled areas use `i128`.

Collinear unit edges are simplified to elementary contour segments before
reflex classification.  A right turn is a local-nonconvexity vertex on either
an outer or hole loop because formal interior remains on the left.  For a
unit-cell component, a horizontal chord has strict interior exactly when every
crossed unit interval has component cells immediately above and below; the
vertical condition exchanges the axes.

**Evidence and boundary.** The geometry module has independent closed-chord
and strict-dominance predicates, including endpoint cases and signed-range
tests.  These conventions cover the supported input models only; the ordinary
grid specialization has no ornament or degenerate-boundary semantics.

### Unit-cell components

**Role.** The grid adapter converts a finite colored array into independent
four-connected components suitable for exact dissection.

**Design.** `ColorGrid::four_connected_components` flood-fills colors through
edge adjacency only.  Boundary extraction cancels shared oriented cell edges,
traces normalized loops, and records reflex vertices, maximal interior runs,
and component-local occupancy in `PreparedComponentContext`.  The prepared
context owns the mask, prefix sums, horizontal and vertical runs, boundary, and
row/column reflex groups; production stages borrow it rather than rebuild
equivalent indexes.

**Evidence and boundary.** Corner-only contacts are tested to split.  Dense
preparation, recovery, and validation use `O(A)` local-bounding-box storage
for area `A`; this is a finite-grid engineering choice rather than a general
polygon representation.

### Ordinary boundary-native polygons

**Role.** `RectilinearPolygon` represents an ordinary rectilinear polygon
without expanding coordinate magnitude into a grid.

**Design.** The accepted model has one simple nondegenerate outer loop, zero or
more pairwise boundary-disjoint ordinary two-dimensional hole loops, signed
`i64` coordinates, and connected interior.  Normalization removes repeated
closing vertices, merges consecutive collinear edges, orients the outer loop
counter-clockwise and holes clockwise, rotates each loop to its
lexicographically smallest vertex, and sorts holes by their normalized vertex
sequences.

`PreparedPolygonContext` constructs normalization, structural validation,
`Boundary`, `BoundaryIndex`, `OrthogonalEdgeIndex`, reflex groups, and base
coordinate vectors once per production solve.  Its edge index keeps
deterministic horizontal and vertical line groups plus segment-tree stabbing
indexes.  Strict point location uses a doubled-coordinate half-open vertical
crossing rule; boundary contact and ray shooting use closed intervals.

**Evidence and boundary.** `polygon::oracle::Validator` retains a readable
`O(n^2)` segment-pair audit.  `polygon::experiment::Validator` is the exact
event/index production validator and differentially agrees on accepted inputs,
negative categories, and deterministic first failures.  The ordinary model
rejects self-contact, multiple outer components, nested holes, zero-length or
nonorthogonal edges, overlapping boundaries, and every formal degeneracy.

### Formal-boundary polygons

**Role.** `FormalRectilinearPolygon` extends an ordinary connected region with
the ornaments and degenerate holes required by Soltan--Gorpinevich Definitions
1, 3, and 4, while retaining a canonical exact model for the dissection path.

**Design.** The formal model stores normalized ornaments, isolated points,
point holes, segment holes, elementary segments, and explicit incidence.  It
uses tagged JSON and exact structural validation.  The ordinary region remains
the positive-area carrier; zero-area formal features are represented as
barriers and coverage obligations rather than approximated by area.

Formal effective chords implement Section 10 Step 1(a)--(d) with axis-line
events and a merge/delete fixed point.  The independent pairwise Definition 7
enumerator remains available.  Step 2 applies common-denominator symbolic
perturbation and audits preservation of every original closed orthogonal
intersection.  The selected maximum admissible family is completed by adding
selected chords first, then the source-ordered horizontal and vertical simple
chords.  Ornament elementary segments are barriers.  The result must satisfy
the independently computed Theorem 2 count `m + c - h - e`, where `m` is the
formal local-nonconvexity count, `c` the component count, `h` the formal-hole
count, and `e` the selected effective-chord count recorded by the pipeline.

**Evidence and boundary.** Formal fixtures include isolated point holes,
interior and boundary-attached segment holes, shared-endpoint regressions, and
the source Figure 3 example.  Exhaustive nonempty subsets of a `3 x 3`
isolated-point lattice, ordinary/formal parity, source invariants, and
dense/sparse recovery agreement are retained as separate evidence.  Validation
requires exact ordinary-region coverage *and* that each formal vertex and
elementary segment lies on rectangle sides, realizing Definition 2 where area
validation alone is insufficient.  Support remains limited to one connected
ordinary interior component.

## Independent Exact Solvers

### Rectangle-cover Oracle

**Role.** `exact-cover-oracle` supplies a definition-level optimum for small
grid components without sharing geometry, matching, biclique, or flow code
with the main pipeline.

**Design.** It enumerates every positive-area integer-grid rectangle contained
in a component, represents each candidate with a dynamic bitset, and runs
branch-and-bound search.  Search selects an uncovered cell with the fewest
currently disjoint options, rejects overlap, starts from the singleton cover,
and prunes with `ceil(uncovered cells / largest available option area)`.

**Evidence and boundary.** The Oracle returns rectangles rather than only a
count and is compared with the SG and dominance solvers on exhaustive small
populations.  It is exponential and deliberately restricted to small
components.

### Explicit Soltan--Gorpinevich Oracle

**Role.** `sg-oracle` independently realizes the constructive chord, matching,
and completion route for ordinary unit-cell components.

**Design.** It extracts boundary loops, identifies local-nonconvexity vertices,
enumerates effective chords, constructs each horizontal--vertical closed
intersection explicitly, runs Hopcroft--Karp and alternating-reachability
minimum-cover recovery, selects the independent complement, completes the
geometry, and recovers cell regions.  It checks
`r + 1 - holes - effective_chords + matching`, matching validity, cover
validity, independent-family validity, completion counts, and rectangles.

**Evidence and boundary.** This path is intentionally separate from the
dominance representation and remains a permanent reference.  Its grid
specialization is not a general formal-boundary sweep.

## Effective Chords

### Definition 7 contract

**Role.** Effective chords are the common geometric object consumed by all
matching representations.  A candidate filter must not be mistaken for the
complete Definition 7 predicate.

**Design.** A valid chord is a nonzero horizontal or vertical segment whose
open interval is in the polygon interior except for permitted finite boundary
points, whose endpoints have the required local-nonconvexity and elementary
segment provenance, and whose interior boundary contacts have the required
unique orthogonal elementary-segment form.  The pairwise polygon Oracle splits
at boundary intersections and checks every open subinterval midpoint by exact
strict-interior predicates; collinear overlaps and edge-interior crossings are
rejected.

**Evidence and boundary.** `chord::oracle::Pairwise` is the permanent
definition-level ordinary-polygon Oracle, with `O(r^2 n)` direct auditing.
`chord::oracle::Indexed` uses aligned reflex groups and the orthogonal edge
index, but is an intermediate Oracle with candidate-dependent cost, not the
source sweep.  Complete chord-set disagreement between any retained backend is
a correctness failure.

### Grid interior-run enumeration

**Role.** `grid::experiment::InteriorRuns` avoids pairwise reflex enumeration
for ordinary finite unit-cell components.

**Design.** On a fixed horizontal line, it scans maximal column runs for which
the two cells adjacent to every unit interval belong to the component.  Aligned
reflex vertices in one run form exactly the valid chord pairs.  The vertical
procedure exchanges axes.  It obtains each prepared run slice with
`partition_point`, without rebuilding a cell hash set, rescanning line data, or
allocating a filtered vector per run.

**Evidence and boundary.** Maximal runs partition the exact unit intervals
checked by the pairwise predicate, yielding a direct equivalence proof for the
supported model.  Differential tests compare canonical chord sets with
`grid::oracle::Pairwise`.  The cost is `O(N + r log r + q)` up to constant-time
mask lookups, for scanned local area `N`, `r` reflex vertices, and `q` emitted
chords.  It is not the paper's general `O(n log n)` polygon sweep and excludes
formal ornaments and degenerate holes.

### Ordinary-polygon event sweep

**Role.** `chord::experiment::Sweep` is the production effective-chord path
for the accepted ordinary-loop polygon model.

**Design.** The source procedure constructs open-interior aligned chords,
filters endpoint configurations, merges eligible adjacent chords, and applies
the endpoint test.  In an ordinary non-contact loop every vertex has one
horizontal and one vertical elementary segment, so the merge/delete cases have
no supported configuration.  The specialization lemma is therefore: the first
boundary hit of the unique strict-interior axis ray from a reflex vertex forms
an effective chord precisely when that hit is reflex.

One axis-generic sweep runs twice.  At each scan coordinate it inserts
orthogonal segments, queries reflex vertices in transverse coordinate and
stable-ID order, then removes segments.  Closed-at-event status ensures
incident segments are visible; the source coordinate is excluded from the ray
query.  A `BTreeSet` predecessor/successor query identifies the nearest
blocker, and a canonical owner rule prevents duplicate output.  Construction
uses native integer coordinates and stable IDs; doubled coordinates occur only
in audits.

**Evidence and boundary.** Fully audited paths recheck every result with both
pairwise Definition 7 backends, validate first-hit provenance and event order,
and retain bounded traces plus aggregate metrics.  For ordinary loops the
status work is `O(n log n + q)`.  The original source does not prescribe this
Rust event order or status layout, and the claimed bound does not cover its
ornament array, isolated points, segment/point holes, or merge/delete cases.

## Conflict Representation and Exact Matching

### Parity embedding

**Role.** The paper representation reduces closed horizontal--vertical chord
intersection to strict dominance while preserving endpoint contact.

**Design.** `dominance::embedding` ranks relevant coordinates and maps
horizontal and vertical chords respectively to

```text
alpha(h) = (2 rank(l), -2 rank(r), 2 rank(y), -2 rank(y))
beta(v)  = (2 rank(x)+1, -2 rank(x)+1,
            2 rank(t)+1, -2 rank(b)+1)
```

The even/odd offset excludes cross-side coordinate equality, so a closed
intersection is equivalent to strict four-coordinate dominance.  FullyAudited
checks every pair against an independently implemented geometric predicate and
materializes the explicit graph.  CompactOnly does not do so; it checks block
validity by coordinate extrema and records no explicit conflict-edge count.

**Evidence and boundary.** Endpoint, metamorphic, exhaustive segment, and
explicit-graph tests establish the stated equivalence on supported inputs.
Pairwise expansion is deliberately excluded from CompactOnly, so its evidence
is the separately audited constructive recursion and output certificates.

### Biclique partition

**Role.** Cardinal--Yuditsky Theorem 8 replaces an explicit dominance graph by
a compressed edge partition suitable for the same matching reduction.

**Design.** Stage C0 creates one biclique per explicit conflict edge.  Stage
C1 recursively splits points by a current coordinate, recurses on
low-left/high-right after dropping that coordinate, and recurses within each
half without dropping it.  The implementation checks structure in `O(sigma)`
by coordinate extrema and, in FullyAudited mode, checks edge-set equality and
multiplicity one against the explicit graph.  Here `sigma` is the total
biclique vertex occurrence count.

The implementation starts the comparability-bigraph recursion with four
coordinates. Therefore the general Cardinal--Yuditsky bound specializes to
`O(q log^4 q)`, not `O(q log^3 q)`.

**Evidence and boundary.** Fabricated-edge, recursion, completeness,
uniqueness, and coordinate audits test the stronger partition invariant; the
flow reduction itself would require only a cover.  Recursive re-sorting remains
a practical implementation choice, so this document does not claim optimized
constants beyond the checked structure.

### Practical exact flow

**Role.** The compressed biclique network computes the maximum bipartite
matching and a recoverable minimum vertex cover without materializing every
conflict edge in CompactOnly mode.

**Design.** Every biclique becomes an internal flow node.  Outer arcs have unit
capacity; internal arcs have
`U = min(horizontal_count, vertical_count) + 1`.  `U - 1` bounds every possible
matching value, so a minimum cut of value at most `U - 1` cannot cross an
internal arc and its outer arcs encode a vertex cover.  The solver rejects an
internal-arc cut before recovering the cover.

`graph::dinic` and a highest-label push-relabel backend with global relabel and
gap heuristics are practical exact alternatives.  FullyAudited compares flow
value and certificates with explicit Hopcroft--Karp and Konig cover recovery;
the two practical flow backends are also differentially compared.

**Evidence and boundary.** All capacities are integral, and output cover,
selected chord family, and rectangle recovery are checked.  Neither practical
backend implements the cited deterministic almost-linear flow algorithm.

## Deterministic Completion and Grid Recovery

### Grid completion

**Role.** Completion turns the independent effective chord family into an
explicit minimum rectangular dissection under one observable policy.

**Design.** Selected effective chords are materialized as unit cuts first.
Completion then runs horizontal before vertical.  Candidate vertices use
increasing `(y, x)` order, with `East, North, West, South` direction priority
filtered by phase.  A direction is eligible only when both adjacent quadrants
are interior, the ray is unblocked, the quadrants share a local angle component,
and that component has at least three interior quadrants.  Extension adds unit
segments while cells remain on both sides and stops at the first perpendicular
boundary or cut, existing same-axis cut, or boundary.

`ReferenceRescanCompletion` scans the entire vertex domain to select each next
chord and again to prove phase completion.  `IndexedFrontierCompletion` scans
once per axis, retains eligible rays in reference order, revalidates stale heap
items using generations, and refreshes only affected vertices.  `DenseCutGrid`
is its only mutable cut authority.  `DenseGridRecovery` uses a local visited
mask, reusable queue, and occupancy prefix sums to prove every recovered region
is its integer bounding rectangle.

**Evidence and boundary.** Acceptance requires equality of selected cuts,
added horizontal cuts, added vertical cuts, and sorted rectangles, not merely
the optimum count.  The reference hash-BFS recovery and rescan completion stay
available as independent Oracles.  The indexed target
`O(P log P + L log P + R)` is a practical unit-grid specialization for `P`
vertices, inserted length `L`, and recovered cells `R`; it is not a claim about
general polygon completion.

## Boundary-Native Polygon Completion

### Coordinate closure and reference arrangement

**Role.** Polygon completion must preserve the grid policy while avoiding a
raster proportional to coordinate magnitude.

**Design.** `CoordinateCompressedCompletion` is the exact reference.  It
inserts selected chords first; runs horizontal then vertical with the same
candidate and direction order; uses exact half-integer probes for local
quadrants; and stops rays at the first boundary or existing cut.  Its coordinate
arrangement floods atomic open rectangles separated by cuts and requires each
region to be a coordinate rectangle.

The finite-universe lemma states that, for a supported ordinary polygon, every
candidate, simple-cut endpoint, and ray blocker has both coordinates in the
union of boundary-vertex and selected-chord endpoint coordinates.  The proof
is inductive: initially all candidates are boundary vertices; inserted-cut
endpoints retain the property; cut intersections combine an existing vertical
and horizontal coordinate; and every ray blocker shares one source coordinate
and obtains the other from an existing boundary or cut.  Debug and audited
paths reject a value leaving this universe as a semantic error.

**Evidence and boundary.** The arrangement uses `O(|X||Y|)` time and storage
and is retained as a dense reference, not as the CompactOnly production path.
Reference comparisons include selected-cut order, canonical cut unions, and
rectangles.

### Dynamic orthogonal cut index and frontier

**Role.** The production completion index answers membership, ray-stopping, and
orthogonal-intersection queries without rescanning all cuts or boundary lines.

**Design.** `IndexedPolygonCompletion` owns the deterministic production
policy and calls `polygon_cut_index::experiment::Index` for its dynamic-cut
queries.  The index stores canonical collinear interval unions in
per-coordinate `BTreeSet`s.  Perpendicular segments are inserted into canonical
nodes of an insert-only logical segment tree over the finite coordinate
universe; only populated nodes allocate ordered sets.  Point stabbing visits
one root-to-leaf path, while nearest predecessor/successor and range-reporting
queries aggregate results across that path.  The incremental frontier adds only
cut endpoints and reported opposite-orientation intersections and lazily
revalidates stale candidates.

**Evidence and boundary.** Differential tests require exact equality with the
line-map Oracle for cuts, cut order, unions, and rectangles.  Diagnostics
require zero coordinate-line and interval scans in production.  The documented
targets are `O(log^2 M)` insertion and nearest queries and
`O(log^2 M + k)` reporting for universe size `M` and `k` reported
intersections.  The structure is insert-only because completion never removes a
cut and applies only to the ordinary coordinate-closure model.

### Sparse subdivision and validation

**Role.** CompactOnly recovery and validation avoid materializing the Cartesian
coordinate product after completion.

**Design.** `polygon_sparse::subdivision::experiment` normalizes boundary and
cut provenance, finds closed-endpoint orthogonal intersections, splits at
crossings and T-junctions, creates paired directed half-edges, orders the four
directions exactly, follows the left-face successor, and classifies each cycle
with an exact half-unit left probe.  Bridges may have twin half-edges in one
face and are not assumed to split it.  A cycle is a rectangle only when its
reduced boundary has exactly four bounding-box corners and its exact perimeter
and area equal that box.

Its sweep orders same-`x` events as
`HorizontalStart < VerticalQuery < HorizontalEnd`, so both endpoints are
active for a vertical query.  Every crossing, T-junction, or shared endpoint
is then reported once and added to both source split lists.  The expected work
is `O((S + J) log S)` for `S` input segments and `J` reported intersections;
the diagnostic contract requires zero candidate pair tests.

`polygon_sparse::validator::experiment` first checks rectangle areas and total
area, then processes x events in `PolygonToggle < RectangleEnd <
RectangleStart` order.  A lazy y segment tree retains parity-zero and
parity-one coverage minima/maxima.  One root check per open slab detects
overlap, uncovered interior, and outside coverage in that priority; it descends
only to recover an error witness.

**Evidence and boundary.** The range-scan subdivision, slab-rescan validator,
and dense arrangement remain selectable differential Oracles.  FullyAudited
requires equality of junctions, atomic segments, faces, rectangles, and
validator results.  Sparse completion is limited to ordinary nondegenerate
rectilinear loops and makes no claim about the source's general completion
bound.

## Clean Hole-Free Path-Tree Representation

### Eligibility and dual construction

**Role.** A clean hole-free component admits a geometry-derived path-tree
partition that can replace the general four-dimensional representation.

**Design.** Eligibility checks one outer loop, no ordinary holes, proper chord
interiors, and distinct endpoint identities.  Endpoint alternation on the
cyclic boundary uses modular interval membership and is compared in
FullyAudited mode with closed chord intersection.

`BoundaryLaminar` selects the lowest boundary gap not incident to a fixed-tree
chord endpoint, rotates the cyclic order around that root gap, and represents
each tree chord by the endpoint interval that excludes the gap.  Proper
noncrossing arcs are laminar.  Sorting by `(start, -end, chord_id)` and checking
the containment stack rejects crossings.  The stack creates one outer region
and one inner region per chord, hence a connected acyclic dual with `|T| + 1`
regions and `|T|` edges.  Boundary gaps are labelled by their deepest active
interval; endpoint sectors determine the incident regions without occupancy
lookup.

**Evidence and boundary.** `ReferenceAreaFloodFill` inserts tree chords as unit
cuts and computes the same regions by flood fill.  FullyAudited compares duals,
endpoint regions, and paths.  The event-driven gap labeller records zero
membership scans and one push/pop per interval; the reference records its
`n * |T|` checks.  This finite-grid construction has cost
`O(n + |T| log |T|)` including sorting and does not claim the paper's general
planar sweep.

### Path partition and orientation

**Role.** The dual turns crossing chords into tree paths and then into the
same biclique/flow interface used by the dominance representation.

**Design.** Each opposite-orientation chord becomes a `CompactTreePath` with
two endpoint regions.  Heavy-light decomposition stores parent, parent edge,
depth, subtree size, heavy child, chain identity, and edge position, breaking
equal subtree sizes by region ID.  Endpoint-only paths emit disjoint canonical
segment-tree intervals without enumerating tree edges.  Each canonical node
defines a biclique from selecting paths and its chain-edge range.

There are symmetric vertical-tree/horizontal-path and horizontal-tree/
vertical-path views.  `BuildBothExact` builds both and selects the smaller
actual `sigma`, breaking ties toward the vertical tree; this is the production
default.  `BoundEstimate` uses

```text
L = ceil(log2(q + 1))
vertical = |H| L^2 + |V| L
horizontal = |V| L^2 + |H| L
```

before construction, while `VerticalTree` and `HorizontalTree` are diagnostic
controls.  The horizontal CompactOnly view swaps axes logically and does not
clone a transposed boundary or occupancy object.

**Evidence and boundary.** FullyAudited retains explicit per-path BFS,
physical transpose, both orientations, and explicit partition comparison.  The
mixed-branching witness campaign contains positive regret for `BoundEstimate`,
so it remains a benchmark heuristic rather than the default.  All selector
evidence is finite and is not a general sigma theorem.

## Exact Circulation and Source-Shaped Flow Research

### Superlinear exact baselines

**Role.** The research flow path requires exact semantic references before a
dynamic or amortized implementation can be trusted.

**Design.** `graph::min_cost::CirculationNetwork` is a generic integer
circulation Oracle.  It routes signed demands through residual capacity, then
enumerates simple signed residual cycles and augments the lowest exact
cost-to-unit-length ratio while negative.  It records each augmentation and
checks capacities, balances, objective value, and absence of a negative
residual cycle with cross-multiplied `i128` comparisons.

`graph::min_ratio_cycle::StableMinRatioLedger` is the checked Definitions
4.2--4.5 contract layer: signed incidence, exact rational accumulated flow,
positive lengths, auditor-provided valid-pair witnesses, factor-two stability,
and replayable `Update`/`Query`/`Detect` logs.  `DynamicRootedForest` provides
the P8.2 decremental forest baseline and recomputes stretch by static BFS.
`DecrementalSpanner` verifies explicit embeddings, congestion, deletion/split
validity, and recourse for the simple undirected P8.3 domain.

**Evidence and boundary.** These are exact semantic and certificate baselines.
They do not discover hidden witnesses, supply a dynamic min-ratio-cycle data
structure, implement an interior-point method, or claim an amortized bound.

### Source map and finite source-shaped components

**Role.** The near-linear flow work maps published prerequisites to isolated
Rust contracts so that a missing theorem is visible rather than replaced by an
unlabelled heuristic.

**Design.** The primary specification is Jan van den Brand et al.,
*A Deterministic Almost-Linear Time Algorithm for Minimum-Cost Flow*,
arXiv:2309.16629v1.  Its implementation map includes deterministic rounding
(KP15), exact min-ratio-cycle representations and IPM accounting, dynamic
low-stretch forests, multiplicative-weights forest collections, static
embedding composition, finite certified expander witnesses, one-level
decomposition, decremental-path semantics, Algorithm 4 replay, tree chains,
and source-min-ratio session recovery.

`source_lsf::experiment::mwu::Collection` constructs exactly `k`
source-shaped low-stretch forests through weighted-copy expansion and records a
finite rational certificate.  `source_spanner` contains bounded exact witness
and replay implementations with explicit unsupported-domain rejection.
`source_min_ratio` records immutable snapshots, stable branch IDs, compact
cycle decoding, source-declared candidate maintenance, terminal/core recourse,
one explicit session update, and recovery into compressed matching/cover data.

**Evidence and boundary.** These components have exact finite certificates and
separately retained enumerating/replay Oracles.  They are not constructions of
the general source theorems, and they make no `O(log^7 n)`, expander,
decremental, or almost-linear runtime claim until their stated source
assumptions are represented and audited.

### AN19 all-radii event engine

**Role.** The AN19-shaped layer provides faithful exact event semantics and
trace material for a future proof or counterexample without claiming the
missing global runtime argument.

**Design.** `source_an19::oracle::event::Engine` explicitly generates and
orders definition-level fixed-snapshot events.  The independent
`source_an19::experiment::event::Engine` processes exact reduced costs

```text
c_x(u, v) = ell(u, v) + d(x, u) - d(x, v)
```

using a stable exact binary heap.  It preserves symbolic source-edge labels,
unsplit rounded lengths, highway-halving state, stable source-edge and
segment-lineage IDs, partition depth, and projection identity.  It neither
expands a length into unit edges nor merges events merely because original
length labels agree.  The backend interface reserves an explicitly unsupported
replacement for a future proof-backed engine.

Every processed or discarded event records its snapshot, depth, source and
lineage provenance, orientation, exact length and reduced cost, exact radius,
queue insertion/pop order, stale state and reason, endpoints, affected
incidence, generations, tie-break fields, and charge candidates.  Aggregate
counters group event work by depth, source edge, lineage, label, projection,
portal split, contraction, and charge map.  Six empirical charge maps report
target counts, fiber histograms, worst witnesses, and finite-population growth;
they are explicitly analysis data, not a proof.

For one fixed snapshot with `n` remaining vertices and `m` supplied graph
edges, the local structural certificate proves at most `3n + 4m + 2` semantic
event records and `n + 2m + 2` queue insertions/pops.  The heap certificate
proves at most `3 I ceil(log2(max(I, 1))) + 2m` counted comparisons for `I`
insertions, hence a practical `O((n + m) log(n + m))` comparison bound per
fixed snapshot.

**Evidence and boundary.** Oracle/reduced-engine differential testing on the
bounded A--H campaign records 31 fixed snapshots, full traces, and local
certificates.  The formal SIAM version of Abraham--Neiman, *Using
Petal-Decompositions to Build a Low Stretch Spanning Tree*, SIAM J. Comput.
48(2), 2019, DOI [10.1137/17M1115575](https://doi.org/10.1137/17M1115575),
was obtained and checked.  It does **not** establish the conversion from the
exact reduced costs above to a bounded ordered set of reduced-event equivalence
classes.

The unresolved proof obligation is: given the event objects generated by the
cited AN19 construction, prove an explicit upper bound on the reduced-event
equivalence classes and justify the ordering transformation used by this
implementation with sufficient detail to derive the claimed runtime.  The
fixed-snapshot cardinality and binary-heap bounds do not establish the
source-equivalent `O(m + n log log n)` ordering bound or hierarchy-wide
amortization.  Therefore `global_amortization_proved`,
`priority_queue_bound_proved`, and `an19_runtime_verified` remain false.

P9.3.2d's faithful implementation and exact Oracle differential are complete,
but its runtime proof is deferred low-priority P9.6a work.  This deferral does
not block P9.3.3 through P9.5 source-shaped backend work.  It does block the
`AlmostLinear` name, any `an19_runtime_verified: true` report, and every AN19
asymptotic runtime claim.

## Layered Public Solver Boundary

### Role

The public solver surface must preserve the distinction between a complete
reference result, a target-bound source experiment, and an independently
verified negative certificate.

### Design

`mrd::layered` exposes three deliberately separate operations:

| Surface | Semantics | Provenance |
| --- | --- | --- |
| `solve_reference` | Complete reference solve on its supported domain. | `ReferenceExact` |
| `solve_source_with_target` | Source-shaped execution under a caller-supplied inclusive target. | `SourceCertifiedAtMost { target }` on success |
| `verify_source_infeasible_below`, `verify_cover_below`, `verify_source_feasible_at_most` | Exact independent certificate checks. | Certificate-specific |

There is no `AutomaticSource` mode, no `solve_source -> optimum` operation,
and no binary-search wrapper for `F*`.  The source layer never calls a
reference solver, never infers `F*`, and never turns an execution failure into
target infeasibility.  `Backend::require_complete()` returns
`Error::Incomplete` for the research layer.  `mrd::layered::experiment`
serializes category, input representation, target provider, outcome, and stage
timings; reference-provided targets are labelled experiment inputs.

### Evidence and boundary

Layered benchmark records keep reference solve, source execution, recovery,
and verification timings separate.  The source-with-target path is currently
formal-polygon-only and can be slow on the full Figure 3 fixture.  P9.5e.3g.3
automatic target discovery remains the active hard blocker.  The reference
surface is production-ready only for its supported formal-polygon domain; the
source surface is research-only and carries no AN19 runtime claim.

## End-to-End Acceptance Contract

The system accepts a result only when each applicable layer has its required
artifact, rather than relying on equality of final rectangle counts alone.

| Layer | Required acceptance artifact | Independent reference or audit |
| --- | --- | --- |
| Grid extraction | Four-connectivity, loop area, hole, and reflex invariants. | Grid and boundary tests. |
| Effective chords | Canonical full chord set and closed-intersection semantics. | Pairwise Definition 7 or grid Oracle. |
| Embedding | Intersection iff strict dominance. | Geometric predicate and explicit graph in FullyAudited mode. |
| Bicliques | Valid blocks and multiplicity-one partition. | Explicit-edge audit in FullyAudited mode. |
| Flow and cover | Integral flow, no internal cut, valid cover. | Hopcroft--Karp and practical-backend differential. |
| Completion | Exact selected/added cut families and canonical rectangles. | Reference completion and recovery. |
| Ordinary polygons | Canonical normalization and structural validity. | `polygon::oracle::Validator`. |
| Formal polygons | Definition 7/2 provenance and formal-boundary coverage. | Pairwise Oracle, transformation audit, dense/sparse differential. |
| Sparse polygon path | Junctions, half-edges, faces, rectangles, and validator category. | Range-scan, slab-rescan, and dense Oracles. |
| Path-tree path | Eligibility, dual, endpoint regions, paths, and partition. | Flood-fill/BFS/transpose references. |
| Source-shaped flow | Exact trace semantics and local certificates. | Definition-level event Oracle. |

A disagreement is minimized into a permanent regression rather than hidden by
a fallback.  Finite tests, workspace scans, local timing, and bounded
source-flow traces provide the evidence stated in their reports; none is
promoted here into a universal correctness or asymptotic theorem.

## Claim Boundaries

1. Grid interior runs are exact for finite unit-cell components but are not the
   general Soltan--Gorpinevich polygon sweep.
2. The ordinary-polygon event sweep is source-mapped only for its ordinary-loop
   specialization; formal merge/delete cases use the separate formal path.
3. Dynamic cut indexing, sparse subdivision, and event-driven validation avoid
   the coordinate Cartesian product on the CompactOnly path, but retain dense
   implementations as differential Oracles and do not claim the classical
   general completion bound.
4. The clean path-tree construction is a finite-grid interval realization, not
   the paper's general planar dual sweep; `BoundEstimate` is a heuristic, not a
   proven selector.
5. Dinic and highest-label push-relabel are implemented exact flow backends.
   The deterministic almost-linear flow algorithm is not implemented.
6. The AN19 event engine is exact and source-shaped on its supported fixed
   snapshots.  Its global amortization and source-equivalent reduced-event
   ordering bound remain unproved, so it is not an automatic or almost-linear
   solver.
7. The ordinary polygon model rejects boundary contacts and disconnected outer
   components.  Formal support is explicit and is not inferred by accepting an
   ordinary input with invalid topology.

Any change to this document must preserve these distinctions and update the
corresponding evidence, limitation, plan, or history record when it changes a
claim boundary.
