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

Experimental results, test populations, raw benchmark samples, and references
are described in the evidence sections of this document; the machine-readable
artifacts remain under `results/`.  Only the forward plan, historical record,
and JSON manifest schema remain separate operational documents:
[`IMPLEMENTATION_MASTER_PLAN.md`](IMPLEMENTATION_MASTER_PLAN.md),
[`HISTORY.md`](HISTORY.md), and `final-manifest.schema.json`.  The plan and
history are process records; they do not redefine the implementation contract
stated here.

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

## Evidence and Reproducibility

### Evidence taxonomy

The repository distinguishes implementation facts, finite correctness evidence,
local measurements, and source-level proof obligations.  An implementation
fact is established by code paths, counters, and invariants.  A finite
campaign supports only the recorded population, generator, seed, and filters.
A timing sample describes the recorded binary and host under its protocol.  A
source theorem is claimed only after the source assumptions, construction, and
matching counters have all been checked.  No campaign below is used to infer a
universal correctness theorem or an asymptotic runtime.

The machine-readable result files are the numerical source of truth.  Markdown
descriptions explain population design and interpretation, while manifests bind
each report to its producing commit, command, environment, and result files.
Counterexamples are minimized and retained as regressions; a zero-disagreement
report is not evidence that an excluded input class was solved.

### Core correctness populations

The permanent grid adapter enumerates all 512 binary `3 x 3` masks, separates
both colors by four-connectivity, and compares exact cover, explicit SG, C0,
FullyAudited compressed flow, and CompactOnly.  The nonempty-mask population
contains 511 masks and 897 foreground components in the direct-parity census;
all recorded comparisons have zero disagreement.  The explicit release-mode
`4 x 4` campaign covers all 65,536 masks and 337,058 monochromatic components,
with 337,058 comparisons per exact solver and no skipped, timed-out, or
disagreeing component.

The seeded `8 x 8` campaign contains 10,000 inputs, 162,162 components, and
zero counterexamples.  Exact cover compares 160,900 components and skips 1,262
above its configured 40-cell limit; SG and both dominance paths compare all
162,162.  Complete canonical free-polyomino enumeration through 10 cells
contains 6,473 inputs, while the larger through-12-cell validation contains
87,146 canonical polyominoes plus two ordinary-hole fixtures.  The bounded
external CP-SAT population independently parses input and enumerates valid
rectangles without Rust geometry: 6,998 inputs and 27,228 components, including
all `3 x 3` grids, free polyominoes through ten cells, and 13 adversarial grids.
Every selected component agrees across CP-SAT, exact cover, SG, C0, and
compressed flow; 11 larger adversarial inputs are explicit input-filter skips.

The deterministic adversarial population contains endpoint contacts, rings and
multiple holes, narrow corridors, combs, staircases, spirals, dense conflicts,
reflex-heavy shapes, long runs, disconnected same-color regions, and diagonal
contact.  Its 17 inputs and 19 components have zero unsupported components,
timeouts, solver errors, and disagreements.  Metamorphic tests apply
translation, reflections, rotations, diagonal reflection, and positive scaling;
32 mapped-back solver results preserve exact validation and optimum counts.

### Grid and polygon differential contract

The grid-to-polygon adapter converts only accepted normalized boundary loops.
For every accepted component, the differential compares the canonical boundary
and reflex semantics, both complete chord families, minimum-cover selection,
selected and added cut unions, optimum count, coordinate rectangles, and both
native validators.  Equal optimum values alone are insufficient.  The default
`3 x 3` gate contains 893 accepted ordinary polygon components; the release
`4 x 4` gate contains 166,189.  Both populations have zero chord, cut, and
rectangle disagreements.

The bounded raster adapter in `verification::polygon` is an independent
small-coordinate Oracle.  It has mandatory width, height, and cell limits, and
production polygon diagnostics require `raster_oracle_used=false`.  Large
coordinate fixtures therefore test the native boundary path rather than being
silently rasterized.

The extended ordinary-polygon corpus contains free polyominoes, ordinary-hole
fixtures, endpoint and topology stress, path-tree families, dense and
complete-bipartite families, 1,000 deterministic random regions, and affine
metamorphic variants.  The v0.9 report records 7,529 inputs, 7,276 supported
components, 255 explicit ordinary-model rejections, and zero differences.  The
isolated CP-SAT rerun agrees on all 27,228 selected components.  Unsupported
formal ornaments, isolated points, point/segment holes, contour contacts, and
disconnected outer components remain declared scope boundaries.

### Biclique, flow, and metamorphic evidence

Every feasible FullyAudited compressed invocation checks nonempty biclique
blocks, unique IDs, actual Cartesian-product edges, no missing or fabricated
edge, multiplicity exactly one, recursive decrease, and termination.  The
recorded correctness populations and six dense instances exercise 532,947
compact solver/audit invocations with zero missing, fabricated, or duplicate
edges.

The deterministic dense-conflict family has six geometry-backed instances.  At
sizes 4 through 128, `q` grows from 16 to 512, explicit conflict edges from 32
to 16,896, and `sigma` from 40 to 1,619.  C0 arcs grow from 80 to 34,304,
whereas compact arcs grow from 56 to 2,131; the observed arc reduction rises
from 30.00% to 93.79%.  These are measurements of one family and do not imply
an asymptotic law.  Owned allocation estimates are not process peak RSS.

The prepared-grid and indexed-completion campaigns compare selected cuts,
added cuts, sorted rectangles, and both cell-exact validators.  On the dense
family with `q=512,1024,2048,4096`, indexed-frontier completion measured
4.183x, 1.800x, 1.821x, and 1.541x relative to reference rescan in the
recorded single-run campaign.  The comparison is local diagnostic evidence,
not a portable performance guarantee.

### Path-tree evidence

Path-tree evidence is generated from unit-cell polygons, never from synthetic
trees passed into production.  The geometry families include laminar chains,
laminar stars, balanced laminar branching, and asymmetric orientations.  Each
row records dual vertices and edges, depth, branching, heavy-chain count,
intervals, canonical segment nodes, bicliques, `sigma`, path length, and
tree-edge occurrence.  The regression guards

```text
sum_heavy_intervals <= C1 * path_count * ceil_log2(q + 1)
sum_canonical_nodes <= C2 * path_count * ceil_log2(q + 1)^2
tree_edge_occurrences <= C3 * tree_edge_count * ceil_log2(q + 1)
```

are theorem-shaped engineering guards, not formal proofs.  The corrected clean
complete-bipartite family satisfies `|H|=|V|=2t` and `|E|=4t^2` for `t=1..4`;
its paired endpoint construction is a permanent regression.  The
mixed-branching search examines geometry-backed candidates, delta-minimizes
them, and retains 16 canonical witnesses with 47--115 cells.  Each has both
chord orientations, degree at least three, a multi-heavy-chain path, and at
least two canonical nodes.  The connected-sum family grows through eight
modules; its first four rows have `(q, regions, paths, intervals, nodes)` equal
to `(6,4,3,4,2)`, `(14,9,6,10,6)`, `(22,13,10,15,8)`, and
`(30,17,14,20,11)`.

The boundary-gap differential compares the indexed `EventSweep` with the
linear `ReferenceNested` backend over 950,557 inputs, 1,053,939 components,
and 385,947 clean components.  It records 16,530,980 boundary-index
comparisons, 3,368,464 endpoint-metadata comparisons, 1,053,939 classifier
comparisons, and 771,894 orientation comparisons, with zero mismatches and
zero solver errors.  The event backend performs 409,593 pushes and pops and
zero interval-membership tests; the reference performs 52,388,678 membership
tests.  Both orientations, dual edges, boundary gaps, endpoint regions,
compact paths, HLD arrays, biclique partitions, rectangles, optimum counts,
and validators are compared.

The orientation audit covers 160,443 clean components and historically found
no positive `BoundEstimate` regret.  The later stored mixed-branching witness
audit intentionally adds five positive-regret rows, with maximum absolute
regret 2 and ratio `2/4`.  `BuildBothExact` therefore remains the production
default; `BoundEstimate` remains a named benchmark heuristic.

### Ordinary-polygon backend evidence

The v1.0 backend differential compares `chord::oracle::Pairwise`,
`chord::oracle::Indexed`, and `chord::experiment::Sweep` on normalized
boundaries, reflex vertices, full Definition 7 chord families, endpoint
metadata, clean certificates, representation and flow values, cuts, rectangles,
and validators.  The 3 x 3 and 4 x 4 supported populations contain 893 and
166,189 components respectively; the extended corpus contains 7,394 and the
native fixture corpus 40.  Every supported row agrees.  The 13-case negative
campaign preserves validator categories exactly.

The v1.1 sweep campaign retains the same structural equality and requires zero
aligned-pair iterations, all-pair iterations, Definition 7 fallback checks,
full-boundary scans, and duplicate outputs.  Candidate-gap family B at size 16
has `n=260`, `r=128`, `C=2,048`, `q=124`, with 8,128 reference pair
iterations, 2,048 indexed pair iterations, and 776 sweep event/status
operations.  Ordinary-hole family C has `n=68`, 16 holes, `r=64`, `C=1,024`,
`q=30`, with 2,016 reference iterations, 1,024 indexed iterations, and 264
sweep operations.  These counters test the implementation specialization; they
do not replace the source proof.

The v1.2 four-path completion differential compares coordinate-compressed
reference, indexed line-map completion, indexed dynamic stabbing with dense
recovery, and dynamic stabbing with sparse recovery and slab validation.  All
paths must agree on selected cuts, added-cut order, canonical unions,
rectangles, area, and validator category.  CompactOnly traces require false
flags for atomic cells, occupied arrays, barrier arrays, and coverage
difference arrays.

### Polygon-native scaling and crossover

The polygon-native scaling families construct integer-coordinate boundaries
directly.  They include staircase-sparse, many-coordinates/few-faces,
staggered-hole coordinate products, completion-heavy notches, sparse path-tree,
ordinary-hole 4D fallback, aligned-reflex-heavy, and huge-coordinate (`10^12`)
families.  Each row records boundary complexity, holes, reflex count, aligned
candidate count `C`, chord and cut counts, phase times, indexed counters, owned
estimates, and exact equality flags.  The v1.0 campaign contains 40 verified
rows at sizes `1,2,4,8,16`; the v1.3 output-sensitive report contains 56
verified rows at sizes 16, 32, 64, and 128.

The v1.3 comparison runs dense coordinate arrangement, reference range-scan
subdivision, output-sensitive orthogonal-sweep subdivision, event-tree
validation, and opt-in `auto` recovery on identical final cuts.  Sparse memory
first beats dense estimates between 32 and 128 for ten families; four do not
cross by 128.  Sparse recovery first wins at 128 for six families; eight do
not cross.  `auto` selected the measured faster backend on all 56 rows, with
maximum observed phase regret 88 microseconds and retained-memory regret 1,696
bytes.  No universal crossover is claimed.  The combined size-256 reference
run exceeded nine minutes while CPU-bound and memory-stable; it is recorded as
a resource limit, not as a zero or extrapolated result.

### Direct-grid parity sampling

The direct-parity benchmark is a timing sample, not a correctness population
and not a causal performance experiment.  One measured process runs:

```bash
target/release/mrd benchmark --suite direct-grid-parity --output <temporary-relative-path>
```

It visits 511 nonzero `3 x 3` masks, 897 foreground components, and 1,794
paired ranked/direct comparisons in both FullyAudited and CompactOnly modes.
Ranked coordinates execute before direct parity in the fixed CLI order.  The
standard protocol builds one release binary, runs three unrecorded warmups,
then 31 fresh measured processes.  A process is rejected if it returns nonzero,
reports a mismatch or solver error, changes finite counts, changes direct zero
counters, changes ranked counters, or omits a verification mode.  No measured
sample is dropped for convenience.

For the recorded campaign, the binary was built with `rustc 1.89.0` on macOS
26.5 arm64, Apple M4, 10 logical CPUs.  The process-wall-time median was
66,311 microseconds with Q1 65,546.5 and Q3 67,264.  Direct embedding time
had median 1 microsecond versus 10 for ranked; direct all-phase time had median
6,033 versus 6,917 for ranked.  The direct-to-ranked all-phase ratio median was
0.8713 with IQR 0.8678--0.8816.  Seven direct embedding observations were
quantized to zero microseconds, so the stable conclusion is structural: all 31
runs reported zero direct rank sorts, rank-map entries, and rank-map owned
bytes.  These numbers describe one binary, host, fixed order, and workload.

The raw protocol and observations are retained in
`results/benchmark-sampling.json` and
`results/benchmark-sampling-runs.csv`.  Their report includes executable hash,
source revision, public environment metadata, full per-process reports, and
quartiles using inclusive linear interpolation (R type 7).  No confidence
interval, p-value, normality assumption, cross-machine speedup, peak RSS,
energy, or asymptotic inference is made.

### Paper-scaling empirical protocol

The schema-v2 in-process geometry diagnostic and its exact reproduction contract
are specified in [`PAPER_KERNEL_SCALING_SCHEMA.md`](PAPER_KERNEL_SCALING_SCHEMA.md).
Its completed phase audit is recorded in
[`phase-reports/P17-geometry-phase-diagnostic.md`](phase-reports/P17-geometry-phase-diagnostic.md).

**Role.** The paper-scaling campaign measures end-to-end behavior of retained
exact MRD paths on paired finite-grid families. Its purpose is to distinguish
observed representation effects from an asymptotic theorem. It does not replace
the exhaustive, differential, or external-oracle populations retained above.

**Design.** `verification::paper_scaling` receives a versioned request with a
family, target size, seed, algorithm, and exact-cover cell limit. It generates
one connected foreground component and records generation attempts, grid
dimensions, foreground cells `N`, component count, boundary size `B`, reflex
count, `|H|`, `|V|`, `q`, `K` when an explicit graph is materialized, biclique
count, total biclique incidence `sigma`, compressed network nodes/arcs, and
the exact rectangle optimum. The four noninterchangeable labels are:

| Label | Exact path | Intended comparison role |
| --- | --- | --- |
| `compact-mrd` | direct-grid parity embedding, compact Theorem 8 bicliques, compressed flow, indexed completion, exact validation | Production compact path; it never materializes `K` while timed. |
| `explicit-hopcroft-karp` | full H/V conflict graph, Hopcroft--Karp, Konig cover, identical indexed completion, exact validation | Conventional explicit-graph baseline, including graph construction. |
| `explicit-c0-flow` | materialized dominance graph, one C0 biclique per edge, explicit flow network, Dinic, cover recovery, indexed completion | Explicit-versus-compressed flow decomposition. |
| `exact-cover-oracle` | independent bitset branch-and-bound rectangle cover | Small-instance correctness Oracle only; never folded into timing baselines. |

The compact sample deliberately leaves `K` null in its timed child process.
The paired runner fills a provenance-labelled structural sidecar from the
paired explicit Hopcroft--Karp row after timing. This prevents a hidden
explicit-graph construction from contaminating compact wall time while still
permitting `K` versus `M` analysis.

The predeclared families are random connected growth with a fixed SplitMix64
seed and one recorded generation attempt; dense conflict; alternating-notch
sparse conflict; alternating comb/staircase; a multi-hole grid family;
orthogonal-spiral polyominoes; and the clean complete-bipartite representation
crossover family. The public configurations are
`results/paper-scaling-smoke-config.json` and
`results/paper-scaling-config.json`. The latter fixes seven families, eight
sizes, three warm-ups, 31 repetitions through target 27, 15 thereafter, and a
60-second timeout before any full results are inspected.

**Measurement and validity.** The Rust sample records native integer
nanoseconds for instance generation, preprocessing, chord generation,
embedding, explicit graph construction, biclique construction, network
construction, flow execution, cover recovery, selection, completion, recovery,
and verification when applicable. The runner records process wall time and its
derived non-solver remainder separately. It uses a fresh release process for
every row and records its actual counterbalanced execution order. Per-child RSS
is null on this host because no portable per-child probe is used; null is never
read as zero. Every successful row independently validates coverage. The
runner rejects a campaign after preserving raw data if successful paired
solvers disagree on their optimum or an algorithm gives nondeterministic
canonical rectangles. Timed exact-cover rows above the predeclared cell limit
remain `unsupported`; timeouts are censored rather than treated as exact times.

**Analysis and current evidence.** `tools/analyze_paper_scaling.py` consumes
only raw rows and emits quartiles, MAD, paired compact/explicit ratios with a
fixed-seed 10,000-resample bootstrap interval, OLS log-median fits, Theil--Sen
sensitivity slopes, residuals, booktabs tables, and SVG figures. A fit names
its independent variable (`N`, `B`, `q`, `K`, or `M`) and is emitted only after
six valid size levels satisfying the predeclared minimum target size. The
complete full campaign is archived in the separately named
`results/paper-scaling-full.*` artifacts. It contains 5,824 planned and
observed identities across seven families, eight target sizes, four algorithms,
three warm-ups, and the predeclared 31/15 repetition schedule. The terminal
population is 4,522 successful rows and 1,302 retained `unsupported` rows from
the bounded exact-cover Oracle, with zero timeout, error, invalid row, or paired
correctness mismatch. All 1,288 measured instance groups have successful
production-path comparisons.

The pilot's 1,008 child-process walls sum to 45.967 seconds, giving a linear
full-plan child-wall projection of 265.587 seconds. The completed full run
records 312.813 seconds of child-process wall time and 1,255.110 seconds of
runner wall time. The residual includes launch, validation, and persistence
work; in particular, resumability deliberately atomically serializes the whole
checkpoint after every terminal record. The protocol does not allocate that
residual among these activities, and it is not attributed to a solver. The
statistical timing population continues to use each child's recorded
`process_wall_time_ns`, not runner elapsed time.

The full analysis emits six-level empirical fits over target size for every
production path in every family. Fits against `N`, `B`, `q`, `K`, and `M` are
reported only where the corresponding representation is defined; in particular,
`K` is unavailable for sparse-conflict and polyomino, and several polyomino
structural variables are not defined. The exact-cover Oracle has no
valid six-level timing fit because its unsupported boundary is part of the
protocol. The paired compact/Hopcroft--Karp ratio is close to one on every
family; the fixed crossover rule reports target 60 only for the
`representation-crossover` family and does not establish a universal ranking.

The measured process-wall quantity includes fresh-process startup and is
therefore a local observation on the recorded Apple M4 host. The generated
slopes, bootstrap intervals, `K`--`M` comparison, and phase decomposition are
descriptive evidence over the declared interval. They do not prove an
asymptotic bound, an AN19 runtime, or the source-flow backend's unresolved
target-decision contract.

**Boundary.** Fresh-process startup remains part of the measured full
process-wall quantity, so the ratios are not a hardware-independent speed
claim. A fitted exponent from the full protocol is an empirical descriptor over
its stated interval, not the algorithm's complexity and not proof of an
`O(n^1.5)` or AN19 claim.
CP-SAT stays outside the performance comparison unless a separate fair protocol
is committed. The detailed supported/unsupported wording is in
[`PAPER_BENCHMARK_CLAIMS.md`](PAPER_BENCHMARK_CLAIMS.md); the legacy
`EXPERIMENTS.md`, `BENCHMARK_SAMPLING_REPORT.md`, and `KNOWN_LIMITATIONS.md`
material is consolidated in this document rather than restored as compatibility
files.

### Test and release protocol

The repository quality gate is:

```bash
cargo fmt --all -- --check
python3 tools/check_biclique_bound.py
python3 tools/check_source_flow_audit.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo build --workspace --release
python3 tools/check_release_consistency.py
```

The tests cover boundary area and loop invariants, exact-cover outputs,
Hopcroft--Karp and flow cuts, signed closed-segment versus strict-dominance
pairs, exact biclique partitions, endpoint and topological adversaries,
metamorphic transformations, free-polyomino enumeration, stored regression
bundles, all binary `3 x 3` grids, and seeded CLI families.  CI runs these
gates plus bounded adversarial smoke tests; the full verification workflow
adds exhaustive, external-oracle, dense, sparse, path-tree, orientation,
source-flow, and regression campaigns and archives their machine-readable
outputs.

The P9 QA boundary is explicit.  At the recorded audit SHA the workspace had
247 tests passed and 3 existing ignored tests, with clean local and remote
branches.  This validates implementation counters, invariants, exact
certificates, and mutation rejection only.  It does not validate AN19's
asymptotic runtime.  The fixed-snapshot event command differentially checks
the exact Oracle/reduced traces, six charge maps, A--H families, highway
halving, trace mutations, and practical heap certificate; it leaves global
amortization, source-equivalent queue bounds, and AN19 runtime false.

The layered backend tests verify reference provenance, honest source failures,
caller-target success provenance, exact negative certificates, target parsing,
missing-target rejection, and explicit unsupported grid input.  The slow
Appendix B.1 source test remains ignored by design; its honest-failure contract
is tested.  `tools/check_source_flow_audit.py` rejects an automatic source mode,
an automatic `solve_source` entry, and binary-search wrappers.

### Reproduction outputs

The following result families are immutable evidence inputs to the release
manifest rather than duplicate prose tables:

| Evidence | Result source |
| --- | --- |
| Exhaustive and random grid correctness | `results/final-campaigns/`, `results/random-8x8-seed42.json` |
| External exact comparison | `results/v1.3-external-oracle.json` and earlier versioned reports |
| Polygon backend differential | `results/v1.0-*`, `results/v1.1-*`, `results/v1.2-*`, `results/v1.3-*` |
| Path-tree families and witnesses | `results/v0.7-*`, `results/v0.8-*`, `results/path-tree-witnesses/` |
| AN19 event traces | `results/an19-event-adversarial.json` and Markdown summary |
| Direct parity sample | `results/benchmark-sampling.json`, `results/benchmark-sampling-runs.csv` |
| Release provenance | `results/release-index.json`, `results/manifest.json`, `results/paper-tables.md` |

When regenerating evidence, change the result filename if the workload,
repetitions, seed, machine, compiler, backend order, timeout, or statistic
changes. Never overwrite a historical artifact with a different protocol.

## References and Source Mapping

1. Soltan and Gorpinevich, *Minimum Dissection of a Rectilinear Polygon with
   Arbitrary Holes into Rectangles*, Discrete & Computational Geometry 9,
   57--79, 1993. DOI `10.1007/BF02189307`.  Definitions 1, 3, 4, and 7 and
   Section 10 supply the formal boundary, effective chord, and completion
   contracts implemented in this document.
2. Eppstein, *Graph-Theoretic Solutions to Computational Geometry Problems*,
   arXiv:0908.3916, 2009.  This is background for the graph reduction, not a
   substitute for Definition 7.
3. Hopcroft and Karp, *An n^(5/2) Algorithm for Maximum Matchings in Bipartite
   Graphs*, SIAM Journal on Computing 2(4), 1973. DOI `10.1137/0202019`.
   `graph::hopcroft_karp` is the explicit matching and Konig-cover Oracle.
4. Knuth, *Dancing Links*, arXiv:cs/0011047, 2000.  The exact-cover Oracle
   uses the constrained branching idea with dynamic bitsets, not pointer-based
   dancing links.
5. Cardinal and Yuditsky, *Compact Representation of Semilinear and
   Terrain-Like Graphs*, ESA 2025, LIPIcs 351, Article 67. DOI
   `10.4230/LIPIcs.ESA.2025.67`.  Theorem 8 and Lemma 12 supply the constructive
   four-dimensional biclique partition and its `O(q log^4 q)` specialization.
6. Dinitz, *An Algorithm for the Solution of the Problem of Maximal Flow in a
   Network with Power Estimation*, Doklady Akademii Nauk SSSR 194, 1970.
   `graph::dinic` provides a practical integral exact max-flow backend.
7. Goldberg and Tarjan, *A New Approach to the Maximum-Flow Problem*, Journal
   of the ACM 35(4), 1988.  The highest-label push-relabel backend is another
   practical exact backend with global relabel and gap counters.
8. van den Brand et al., *A Deterministic Almost-Linear Time Algorithm for
   Minimum-Cost Flow*, arXiv:2309.16629 / FOCS 2023.  The repository retains
   exact superlinear circulation and min-ratio Oracles; it does not claim the
   cited IPM, hidden-stability, dynamic-cycle, or almost-linear machinery.
9. Abraham and Neiman, *Using Petal-Decompositions to Build a Low Stretch
   Spanning Tree*, SIAM Journal on Computing 48(2), 2019, 227--248. DOI
   `10.1137/17M1115575`.  The source was checked, but it does not establish
   the reduced-event ordering/counting conversion required by P9.3.2d.
10. Sleator and Tarjan, *A Data Structure for Dynamic Trees*, STOC 1981. DOI
    `10.1145/800076.802464`.  This is a pinned predecessor reference for the
    source-shaped dynamic tree contracts; it does not establish the repository
    implementation's missing runtime claims.

## Consolidated Limitations

The following limitations are normative and should be read with every result:

- The accepted geometry consists of finite unit-cell grids and one ordinary
  nondegenerate integer-coordinate outer loop with ordinary two-dimensional
  holes.  Ornaments, isolated formal-boundary points, point/segment holes,
  arbitrary contour contacts, and multiple disconnected outer components are
  represented or rejected explicitly but are not all accepted by the solver.
- The ordinary polygon sweep is source-mapped only for the ordinary-loop
  specialization.  It does not implement formal merge/delete degeneracies or
  claim the source's general sweep bound.  Pairwise and indexed chord Oracles
  remain permanent.
- Grid interior runs are exact for unit-cell components but are not the general
  polygon `O(n log n)` enumerator.  Biclique recursion retains straightforward
  recursive sorting; four coordinates imply `O(q log^4 q)` rather than a
  three-coordinate bound.
- CompactOnly avoids explicit conflicts and full partition expansion, while
  FullyAudited retains them as correctness checks.  The compact coordinate-
  extrema check is not an explicit edge-multiset proof by itself.
- Prepared occupancy, cuts, recovery, and validation are dense in a component's
  local bounding box.  Polygon sparse recovery avoids the coordinate product,
  but dense arrangements remain reference Oracles and sparse structures support
  only ordinary nondegenerate loops.
- The optional polygon `auto` recovery policy is evidence-backed only for the
  recorded scaling corpus and remains opt-in.  There is no universal dense/
  sparse crossover, and the complete size-256 reference run exceeded its
  practical time budget.
- Dinic and highest-label push-relabel are practical integral exact backends.
  The cited deterministic almost-linear exact-flow algorithm is not
  implemented.  Experimental agreement is finite evidence, not an
  `n^(1+o(1))` claim.
- Process peak RSS, allocator overhead, energy, cache counters, and cross-host
  timing are not measured.  Null peak-memory fields mean unmeasured, not zero;
  owned allocation estimates are diagnostic payload estimates only.
- Automatic source target discovery is not implemented.  The layered source
  API requires a caller-supplied target, labels failures as
  `UnsupportedOrUndetermined`, and never falls back to a reference solver or
  infers target infeasibility.
- The AN19 source-shaped event engine is exact on its supported finite
  snapshots, but the reduced-event ordering/counting conversion, source-
  equivalent priority-queue bound, hierarchy-wide amortization, and production
  AN19 runtime remain unproved.  P9.3.2d implementation is complete and proof
  work is low-priority P9.6a; empirical counts do not close the proof.
- The exact-cover Oracle is exponential and intentionally limited to small
  components.  Exhaustive `4 x 4` and larger campaigns are explicit release
  commands because their duration is machine-dependent.
- JSON colors are compared as exact `serde_json::Value` values.  SVG is a
  diagnostic view only.  Unsupported classes remain visible in manifests and
  are not silently treated as solved inputs.
