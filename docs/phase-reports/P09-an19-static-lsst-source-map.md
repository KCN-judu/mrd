# P09.3.2 AN19 Static LSST Source Map

## Status

Source recovery, exact single-petal and symbolic weighted-portal gates, compact
weighted hierarchy, recursive contraction, and fast membership-event processing
are implemented. This report maps the complete 2012 AN19 manuscript and the
complete 22-page 2019 SIAM journal text (`10.1137/17M1115575`) to
implementation gates. It does not claim that the manuscript Section 7 / final
Section 6 runtime bound is implemented.
The single-petal implementation commit is `a57e48c`; the symbolic weighted
portal/contraction commit is `6769ec1`; and the exact weighted Figure 6
selection commit is `3bb0400`. The stable augmented hierarchy workspace commit
is `839cb5c`; the certified unit-length Figures 4--5 composition commit is
`20b0421`; compact weighted hierarchy, recursive contraction, and fast events
are `cdf732d`, `d6b8e6b`, and `3d3afe2`; and the work-accounting and rounded
length prerequisites are `720f0cb` and `27d5773`. Dense cluster-local node
projection and allocation counters are `6901703`.
Top-level source-edge projection attribution and aggregate segment/class audits
are `e4f54af`.
Fixed first-target projection and shortest-path reuse is `bc61592`.
Mutation-aware unchanged-cluster projection caching is `14e9abb`.
Incremental cached projection updates across portal splits are `8d68d59`.
Symbolic source-length labels and their projection audit are `5cc49f0`.
Recursive source-scale and radius-shrink certificates are `b050625`.
Projection materialization and portal-fragment source charges are `60fdfe4`.

## Resolved source questions

- The available manuscript is 20 PDF pages and includes the full algorithms in
  Figures 4--6, Claims 1--15, the weighted extension, and the fast petal
  construction. It is not merely the six-page introductory overview suggested
  by the initially extracted front matter.
- The final SIAM text is 22 pages (SIAM J. Comput. 48(2), pp. 227--248). Its
  Section 6 moves arbitrary-weight handling and the runtime argument together,
  but otherwise repeats the manuscript's interface: round original weights to
  powers of two, form reduced arcs
  `w(u,v)-(d(v,x)-d(u,x))`, and invoke improved Dijkstra on the reduced graph.
  It supplies no bound on the number of distinct reduced arc costs.
- The deterministic flow paper defines `\O` as `\widetilde O`. Its Static LSST
  theorem's `\O(m)` runtime is therefore consistent with AN19's
  `O(m log n log log n)` bound.
- AN19 uses ordinary stretch `d_T(u,v)/ell_e`. The deterministic paper adds
  one. The implementation must add `||v||_1` when translating AN19's weighted
  total-stretch certificate.
- AN19 starts with unweighted multiplicities. Lemma 5.4 supplies exactly that
  reduction by replacing edge `e` with
  `ceil(m v_e / ||v||_1)` copies, with at most `2m` copies total. The existing
  weighted-copy certificate implements and checks this boundary.

## Figure-to-code contract

### Figure 4: hierarchical petal decomposition

For a connected induced cluster `X`, center `x0`, and target `t`:

1. Return a deterministic shortest-path tree when the current radius is at
   most `10 log n log log n`.
2. Run Figure 5 and recurse on every stigma/petal cluster.
3. Halve each original highway edge at most once for recursive distance
   calculations. Preserve its original length for the returned tree and
   stretch audit.
4. Join the recursive trees with exactly the Figure 5 portal edges. The result
   must contain `|X|-1` original edges and pass the exact spanning-tree audit.

Shortest paths use a lexicographic edge-ID tie break. This deterministically
implements the manuscript's unique-shortest-path perturbation without using
floating point or changing certified distances.

### Figure 5: one petal decomposition

Let `Delta = rad_x0(X)`, `r0 = Delta/2`, and `Y0 = X`. Construct the first
petal with budget `Delta/4`, then construct remaining petals with budget
`Delta/8` while a vertex outside `B_X(x0,r0)` remains. The final remainder is
the stigma. Every emitted cluster must be connected; the portal edge for petal
`j` joins its center `x_j` to the predecessor `y_j` on `P_x0,t_j`; and `y_j`
must remain in a later petal or the stigma.

The first target may require an imaginary path, and weighted targets/centers
may lie inside an edge. The implementation must represent these portal points
symbolically and prove that suppressing them maps the augmented tree to a tree
of original edges. Rounding a portal to a vertex is not source-equivalent.

### Figure 6: `create-petal`

For budget `R`, let

`W_r = union_{p in P_x0,t, d_Y(p,t)<=r}
B_(Y,rho(Y,x0,p))(p, (r-d_Y(p,t))/2)`.

Let `L = ceil(log log n)`. Choose the minimum `p in [1,L]` satisfying

`|E(W_((1+p/L)R/2))| <=
2|E(X)| / 2^((log m)^(1-p/L))`,

then set `a = (1+(p-1)/L)R/2`,
`chi = |E(X)|/|E(W_a)|`, and increase `r` from `a` until

`|boundary(W_r)| < |E(W_r)| 8 L ln(chi) / R`.

All comparisons must be certified. Algebraic threshold powers and logarithms
may use the existing outward-rounded fixed-point interval machinery, but an
ambiguous interval must increase precision rather than choose a side.

## Weighted and fast construction gates

- Contract edges shorter than `rad(X)/n^2` only with an explicit expansion map
  that recovers a tree of the uncontracted graph.
- Use imaginary portal points for weighted paths; do not subdivide every unit
  of length or charge runtime to scaled numeric magnitude.
- Claim 15 permits a petal to be generated by directed region growing. For
  `(u,v)`, use reduced length
  `ell(u,v) - (d(v,x0)-d(u,x0))`, and halve the directed highway portions.
- Process only radii at which the petal changes. Record directed relaxations,
  radius events, contractions, portal splits, and original-edge touches.
- The source runtime gate requires total
  `O(m log n log log n)` work, equivalently `\widetilde O(m)` in the
  deterministic paper's notation. Expanded-copy work is charged against the
  already checked `2m` bound.

## Acceptance order

1. Exact single-petal membership and Figure 6 stopping certificates on
   unweighted fixtures.
2. Symbolic interior portals, contractions, and exact recovery to original
   edges on rational weighted fixtures.
3. Full hierarchical construction with radius, tree, and weighted-stretch
   certificates, differentially checked against `ExactStaticLsstOracle` on its
   bounded domain.
4. Region-growing event implementation and source-shaped counters establishing
   the near-linear bound without graph expansion or an Oracle fallback.

Until all four gates pass, P9.3.2 remains in progress and no AN19, Lemma 5.4,
or production runtime claim is made.

## Implemented single-petal gate

`An19UnweightedPetal` implements Figure 6 on the original unit-length vertex
domain:

- exact cone-union membership thresholds for every vertex of `Y`;
- deterministic shortest paths with lexicographic edge-ID tie breaking;
- the minimum `p in [1,L]` window, with the irrational threshold comparison
  certified by outward-rounded logarithm/exponential intervals;
- exact radius-event advancement until the strict boundary inequality holds;
- internal/boundary edge counts and explicit shortest-path, relaxation,
  comparison, and radius-event counters;
- an explicit `None` center for a radius lying inside an edge, preventing this
  gate from silently rounding a portal that P9.3.2c must represent.

This implementation deliberately recomputes exact shortest paths and all
membership thresholds. It is a source-semantics baseline for one petal, not the
Claim 15 region-growing runtime implementation.

## Implemented weighted-portal gate

- `An19WeightedPetalAtRadius` evaluates Claim 15 with exact rational reduced
  directed lengths. It represents a radius inside an edge as the original edge
  ID, orientation, and exact offset, without unit subdivision.
- Each highway records the exact original-edge portion whose forward directed
  cost is halved. The directed region-growing result is differentially equal
  to the cone-union baseline on their shared unit-length domain.
- `An19HighwayLedger` stores canonical exact intervals on original edges,
  atomically rejects positive overlap, merges touching intervals, and computes
  the effective endpoint-to-endpoint length after every portion is halved once.
- `An19ShortEdgeContraction` computes the cluster radius, contracts exactly the
  edges shorter than `rad(X)/n^2`, retains original IDs on quotient edges, and
  expands a certified quotient tree with deterministic internal forests into
  an original-edge spanning tree.

These pieces establish P9.3.2c's symbolic representation and recovery
contracts. They do not yet compose Figure 5 recursively; that hierarchy and
its radius/stretch proof remain P9.3.2d.

P9.3.2d now also has an exact weighted Figure 6 baseline.
`An19WeightedPetal` treats each successive highway edge as one parametric
interval: with the current forward edge removed, every directed shortest-path
distance has the form `min(A, B - 3r/2)`. It derives exact vertex-entry radii,
then reuses the certified window and strict stopping comparisons. On the shared
unit domain it exactly matches the cone-union window index, selected radius,
and vertex set. This implementation deliberately reruns exact shortest paths
per highway interval and therefore makes no fast region-growing runtime claim.

The hierarchy workspace now retains stable augmented edge IDs while dense
active projections feed Figure 6. It supports exact rational interior splits
in either orientation, provenance-free virtual leaves, and oriented rational
interval provenance for every original edge segment. Original-tree recovery
requires either every active segment of an original edge or none, rejects
inactive and partial selections, and independently proves acyclicity and
connectivity on the original endpoints. This is the representation substrate
for Figures 4--5, not their completed recursive composition.

`An19HierarchicalLsst` now completes Figures 4--5 on the exact unit-length
source domain. It constructs the source's imaginary first path with at most
`O(n)` virtual unit segments, creates later `Delta/2` targets and petal portals
at exact rational points, recursively halves each highway edge once, joins
subtrees with the recorded portal edges, and suppresses the augmented tree to
original edge IDs. Recovery first proves the full augmented tree, including
virtual and split vertices, is connected and acyclic; it then independently
proves the suppressed original tree is connected and acyclic.

Every recursive call emits an exact radius witness containing its effective
cluster edges and center distances. The verifier checks edge triangle
inequalities, tight predecessors, the maximum radius, and the Figure 4 base
threshold. A separate original-graph audit recomputes the tree, deterministic
paper `1 + stretch` convention, weighted total stretch, and total weight.
Tests cover a 500-vertex nonvirtual recursion, the top-level imaginary path and
suppression on a 500-vertex unit path, certificate mutation rejection, and all
38 connected labeled simple graphs on four vertices against
`ExactStaticLsstOracle`.

The production hierarchy now accepts arbitrary positive rational lengths,
uses one compact exact imaginary edge instead of numeric subdivision, performs
short-edge contraction recursively, and expands quotient trees back to
original edge IDs. The fast Figure 6 path derives all membership events in one
multi-source run and incrementally maintains incident volume, boundary size,
and reciprocal-length boundary cost. Differential fixtures compare the fast
path with the parametric and repeated-shortest-path Oracles.

Every augmented segment now also carries a symbolic label consisting of its
top-level source edge, unsplit rounded source length, and highway-halving state.
The label is deliberately separate from the segment's exact materialized
length. Portal splits copy it to both children; an in-place cached split retains
the existing dense label and appends the same label; and recursive quotient
construction validates the inherited root source before preserving the label
and halving state. Provenance-free virtual edges use the same representation
with no root source and are counted separately.

Projection materialization recomputes the exact sets of effective symbolic
source and virtual lengths, where a halved label contributes half its unsplit
length. Incremental shape observations use the cached sets, and
`An19ProjectionAudit::verify` cross-checks their maxima against hierarchy
metrics. On the deterministic 500-node unit path, the maximum remains 16
materialized projection length classes, but only 2 symbolic source-label
classes and 3 symbolic virtual-label classes. This isolates portal splitting
from the source-length-class count. It does not establish that distinct
candidate distances in the all-radii Figure 6 event stream can be ordered by
those labels.

Logical recursion now distinguishes a child partition from a same-scale
quotient contraction. Each radius certificate stores its preorder parent,
partition depth, same-scale flag, and the top-level source of every radius
edge. The final verifier independently rebuilds the full per-source
scale-occurrence vector from those radius edges. It rejects a missing or
forward parent, a contraction child at a new depth, a contraction without a
certified parent contraction, or a partition child whose exact radius exceeds
three quarters of its parent's radius. It also cross-checks logical calls,
contractions, maximum depth, source-attribution scans, total and maximum source
participation, and every per-source count against the hierarchy metrics.

AN19 final Section 6 observes that active source radii span at most `2 n^2`
and Claims 5--6 shrink each partition radius by at least `3/4`. Since
`(3/4)^3 < 1/2`, the checked ceiling is `6 ceil(log2 n) + 4` calls. Every
successful result additionally requires a source to occur at most once per
logical depth. On the deterministic 500-node nonvirtual unit path, 46 logical
calls reach depth 8; 2,256 source-scale participations have per-source maximum
9 and require 2,919 attribution scans. On the 500-node alternating rational
path, 1,983 participations have per-source maximum 7 and require 2,016 scans;
uniform scaling preserves all three values. Same-scale quotient contraction,
parent/depth, source-root, aggregate, and ceiling mutations are rejected.

This closes the logical recursive-scale certificate. It does not yet charge
all implementation work to those scales: the unit-path source with 9 scale
participations can still have 33 materialized projection occurrences, and
portal fragments remain in descendant projections. Consequently
`AggregateRegressionOnly` remains the amortization mode.

Projection charging now separates those repeated segment occurrences into a
single top-level source materialization per full projection and the additional
portal fragments of that source. Every interior split is attributed before the
workspace mutation is forgotten: either to its top-level source edge or to the
provenance-free virtual class. The verifier cross-checks the per-source vectors,
totals, maxima, and hierarchy metrics, then enforces the implementation's four
possible full-projection entries per certified scale plus one initialization
entry. Extra source fragments are bounded by the source's split count times
that scale charge. Provenance-free segment occurrences are independently
bounded by `(virtual leaves + virtual splits)` times the active scale charge.

On the 500-node nonvirtual unit path, the unchanged 5,974 projected edge
occurrences now decompose into 4,533 source projection materializations, 61
extra source-fragment materializations, and 1,380 provenance-free segment
occurrences. There are 27 source-attributed and 22 virtual splits; maximum
per-source materializations, extra fragments, and splits are 17, 16, and 2.
The alternating rational path has 4,181 source materializations, 45 extra
source fragments, 22 source splits, 10 virtual splits, and respective
per-source maxima 16, 12, and 1. Uniform scaling preserves all counters, while
materialization, fragment, split, maximum, and aggregate mutations are
rejected.

This closes the active projection-materialization and descendant-fragment
interfaces. It does not charge inactive entries retained in workspace incident
indexes, projected node slots, or the unresolved all-radii candidate-event
work, so `StructuralSourceBound` is still not selected.

## Runtime acceptance audit

| Issue | Observed | Source requirement | Current acceptance |
| --- | --- | --- | --- |
| global workspace scans | Recursive projections formerly scanned every augmented edge, including unrelated sibling clusters. | Section 7 charges work to the current cluster and then uses logarithmic edge participation. | `720f0cb` maintains stable incident-edge indexes and projects only the current cluster. Verified by hierarchy differentials and mutation tests. |
| repeated Figure 5 distances | The implementation formerly recomputed `d_X(x0, .)` for every later petal and located a target using a new path in `Y`. | Figure 5 fixes `d_X` and `P_x0,t(X)` for the entire decomposition; Claim 1 proves the fixed path remains in `Y`. | `720f0cb` rebuilds once after the imaginary first path and reuses fixed `X` distances for later target selection. |
| omitted heap work | Push/pop counts treated a binary-heap operation as one unit and did not count its comparisons. | A runtime certificate must charge the actual priority-queue implementation. | `720f0cb` counts every heap comparison. The certificate reports `BinaryHeap`, so it cannot claim the Section 7 runtime. |
| weighted length classes | Exact rational input could contain `m` distinct lengths. | Section 7 rounds down to powers of two so only `O(log n)` active length classes remain after scale restriction. | `27d5773` rounds production workspace lengths to `base * 2^j`, preserves original lengths for provenance/stretch, proves the factor-two interval, and is invariant under uniform scaling. |
| source priority queue | Production shortest paths and fixed-radius Claim 15 runs now use original edge-length classes. Potential reweighting changes every ordinary reduced arc `l+d(x,u)-d(x,v)` back to `l`; ordered highway source labels represent the half-length path and an interior portal exactly. The all-radii Figure 6 event stream still groups by reduced directed cost, and a 128-node power-of-two chord fixture produces 162 classes. | OMSW10 Sections 5--6 bounds its queue by the original edge-length set `L`; KMPb Corollary 5.5 states its fast `ConeCut` bound for `k` distinct cone distances. EEST05 Definition 4.4 charges an original edge length only when a path leaves the forward-edge ideal, but AN19 explicitly uses the different excess metric. Final AN19 Section 6, p. 245, repeats the jump from original power-of-two weights to improved Dijkstra on the reduced graph without bounding the reduced cost classes. | **Blocked.** `ece2722` closes the fixed-radius subproblem with 456 directed-distance differentials and a source-class counterexample audit. Neither the public manuscript nor the final journal text proves that AN19's exact event order has `O(log n)` classes; KMPb Lemma 5.6 also moves from distinct graph lengths to its `ConeCut` call without giving the missing conversion. Keep `ReducedLengthMonotone` until an authoritative correction or an independently proved exact rational event-order structure is available. |
| recursive amortization | The aggregate certificate still uses a fixed `1024` factor. Recursive projections remap every noncontiguous augmented cluster to exactly `0..|X|`. `e4f54af` also keeps top-level source IDs independent of quotient-local recovery provenance through portal splits and nested contractions, and stores only per-source and aggregate projection counters. On the deterministic 500-node unit path, one original length class becomes as many as 16 active projection classes. Before fixed-path reuse, 16,948 projected edge occurrences included 4,332 provenance-free segments and one original edge reached 111 occurrences. `bc61592` and `14e9abb` reduce materialized edge occurrences to 12,274 and workspace scans to 31,498. `8d68d59` then applies 39 exact portal splits in place across 83 cache hits, reducing those figures to 5,974 and 18,290. Its length-class multiset keeps the active maximum at 16 rather than hiding cached segment classes; one source edge still has 33 materialized occurrences. `5cc49f0` independently propagates unsplit source labels through portal splits and quotient recursion: the 16 active classes reduce to 2 symbolic source classes and 3 symbolic virtual classes. `b050625` independently reconstructs 2,256 logical source-scale participations from radius certificates; their per-source maximum is 9 across maximum partition depth 8, and same-scale contractions do not create a false level. `60fdfe4` decomposes the 5,974 occurrences into 4,533 source materializations, 61 extra source fragments, and 1,380 provenance-free occurrences, and structurally charges each class to certified scales and 49 attributed splits. | The proof charges every edge to `O(log n)` recursive scales and obtains `O((m+n log log n) log n)`. | **Open.** Logical source scales, active projection materializations, and descendant source/virtual fragments are now structurally certified. Inactive incident-index scans, projected node slots, and the exact all-radii candidate-event order remain outside a complete structural work bound. Keep `AggregateRegressionOnly` and `ReducedLengthMonotone` until those interfaces are proved. |

The fixed `1024 * m * ceil(log n) * ceil(log log n)` ceiling is therefore only
a regression guard for observed counters. The source-edge audit exposes where
work accumulates but does not convert the observed totals into an asymptotic
bound. It is not accepted as a proof. P9.3.2d is `blocked`,
`An19AmortizationMode` remains `AggregateRegressionOnly`, and no AN19
production runtime or full Lemma 5.4 completion is claimed.

## Persisted source blocker

AN19 defines the cone ball around `p` by the excess
`d(p,v)+d(p,x)-d(v,x)`. Claim 15 realizes this as directed arc costs
`l(u,v)+d(x,u)-d(x,v)`. EEST05 Definition 4.4 instead gives zero cost to a
forward arc and charges the full original length to every other traversed
edge. The latter is a concentric system but is not the former exact metric, so
substituting EEST cones would change Figure 6 membership and is not accepted.

The complete final SIAM text for DOI `10.1137/17M1115575` has now been
inspected, including rendered pp. 245--246 and the full extracted Section 6.
It confirms the final `O(m log n log log n)` theorem but does not supply the
missing exact event-order reduction. The public manuscript, final journal
text, KMPb, EEST05, OMSW10, and ABN08 therefore leave the same source gap. The
next source action is an authoritative erratum or author clarification that
resolves this interface. The independent implementation alternative is a
proved exact rational event-order data structure with work counters matching
the theorem; comparison sorting or an unstated bounded-integer assumption is
not sufficient.

## Focused evidence

| Command | Exit | Duration | Result |
| --- | ---: | ---: | --- |
| `git status --short` | 0 | <0.01 s | clean at the audited implementation/documentation HEAD |
| `git diff --check` | 0 | <0.01 s | clean |
| `cargo test -p rect-graph source_an19::tests::` | 0 | 0.64 s | 32 tests passed; 456 fixed-radius directed-distance families and 456 threshold families match their independent Oracles; symbolic labels, recursive scales, source materializations, source/virtual splits, and descendant fragments survive valid hierarchy operations and uniform scaling while mutations are rejected |
| `cargo test -p rect-graph` | 0 | 1.99 s | 102 tests passed; projection charging preserves hierarchy, contraction, recovery, active-class, source-counter, symbolic-label, and recursive-scale certificates |
| `cargo fmt --all -- --check` | 0 | 0.40 s | clean |
| `python3 tools/check_biclique_bound.py` | 0 | 0.13 s | bound check passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 4.22 s | no warnings |
| `cargo test --workspace` | 0 | 421.41 s | 247 passed and 3 existing release-scale campaigns ignored across 13 suites |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 2.84 s | 7 package documentation sets generated without warnings |
| `cargo build --workspace --release` | 0 | 16.77 s | 6 crates compiled successfully |
| `python3 tools/check_release_consistency.py` | 0 | 2.66 s | 10 runs, 499220 grid comparisons, 174767 polygon rows/components, and 27228 CP-SAT components verified |
| `git diff 2ee099f..HEAD -G '#\[ignore' --stat` | 0 | <0.01 s | no ignored test changed; the same 3 documented release-scale campaigns remain ignored |
| `git diff 2ee099f..HEAD -- results` | 0 | <0.01 s | no stale or regenerated release evidence changed |
| changed-line credential, local-path, fallback, and source-runtime mode scans | 0 | <0.01 s | no credentials, local absolute paths, fallback activation, `SourceMonotone`, or `StructuralSourceBound` selection added |

The fixtures cover an exact path petal and Figure 6 window, rejection of a
nonunit edge, stable choice between equal diamond paths, a fractional radius,
an interior rational portal, Claim 15 differential equality, short-edge tree
expansion, atomic highway interval accounting, bidirectional rational
provenance splitting, dense-to-stable projection, complete-segment recovery,
rejection of partial/cyclic/disconnected recovery, unit Figures 4--5 recursion,
virtual-path suppression, radius/tree/stretch verification, mutation rejection,
exact-Oracle comparison on every connected four-node simple graph, top-level
source preservation across portal splits and quotient recursion, and
source-edge, symbolic-class, recursive-parent, same-scale-contraction, and
per-source scale/materialization/fragment/split audit mutation rejection.
Uniform scaling preserves symbolic, source-scale, and structural projection
charge counts while the priority mode remains
`ReducedLengthMonotone` and `source_runtime_verified()` remains false. The
500-node nonvirtual path additionally demonstrates multiple materialized
projection length classes,
only 2 source-label classes versus 16 materialized classes, 3 virtual-label
classes, nonzero provenance-free projection work even though its first
top-level target is not virtual, and per-source segment repetition beyond
recursion depth.
