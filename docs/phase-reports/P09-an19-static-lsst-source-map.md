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

## Runtime acceptance audit

| Issue | Observed | Source requirement | Current acceptance |
| --- | --- | --- | --- |
| global workspace scans | Recursive projections formerly scanned every augmented edge, including unrelated sibling clusters. | Section 7 charges work to the current cluster and then uses logarithmic edge participation. | `720f0cb` maintains stable incident-edge indexes and projects only the current cluster. Verified by hierarchy differentials and mutation tests. |
| repeated Figure 5 distances | The implementation formerly recomputed `d_X(x0, .)` for every later petal and located a target using a new path in `Y`. | Figure 5 fixes `d_X` and `P_x0,t(X)` for the entire decomposition; Claim 1 proves the fixed path remains in `Y`. | `720f0cb` rebuilds once after the imaginary first path and reuses fixed `X` distances for later target selection. |
| omitted heap work | Push/pop counts treated a binary-heap operation as one unit and did not count its comparisons. | A runtime certificate must charge the actual priority-queue implementation. | `720f0cb` counts every heap comparison. The certificate reports `BinaryHeap`, so it cannot claim the Section 7 runtime. |
| weighted length classes | Exact rational input could contain `m` distinct lengths. | Section 7 rounds down to powers of two so only `O(log n)` active length classes remain after scale restriction. | `27d5773` rounds production workspace lengths to `base * 2^j`, preserves original lengths for provenance/stretch, proves the factor-two interval, and is invariant under uniform scaling. |
| source priority queue | Production shortest paths and fixed-radius Claim 15 runs now use original edge-length classes. Potential reweighting changes every ordinary reduced arc `l+d(x,u)-d(x,v)` back to `l`; ordered highway source labels represent the half-length path and an interior portal exactly. The all-radii Figure 6 event stream still groups by reduced directed cost, and a 128-node power-of-two chord fixture produces 162 classes. | OMSW10 Sections 5--6 bounds its queue by the original edge-length set `L`; KMPb Corollary 5.5 states its fast `ConeCut` bound for `k` distinct cone distances. EEST05 Definition 4.4 charges an original edge length only when a path leaves the forward-edge ideal, but AN19 explicitly uses the different excess metric. Final AN19 Section 6, p. 245, repeats the jump from original power-of-two weights to improved Dijkstra on the reduced graph without bounding the reduced cost classes. | **Blocked.** `ece2722` closes the fixed-radius subproblem with 456 directed-distance differentials and a source-class counterexample audit. Neither the public manuscript nor the final journal text proves that AN19's exact event order has `O(log n)` classes; KMPb Lemma 5.6 also moves from distinct graph lengths to its `ConeCut` call without giving the missing conversion. Keep `ReducedLengthMonotone` until an authoritative correction or an independently proved exact rational event-order structure is available. |
| recursive amortization | The aggregate certificate still uses a fixed `1024` factor. Recursive projections remap every noncontiguous augmented cluster to exactly `0..|X|`. `e4f54af` also keeps top-level source IDs independent of quotient-local recovery provenance through portal splits and nested contractions, and stores only `O(m)` per-source and aggregate projection counters. On the deterministic 500-node unit path, one original length class becomes as many as 16 active projection classes. Before fixed-path reuse, 16,948 projected edge occurrences included 4,332 provenance-free segments and one original edge reached 111 occurrences. `bc61592` reuses seven already computed first-target paths, reducing those figures to 15,856, 4,126, and 104 respectively, with projection calls falling from 172 to 165. `14e9abb` reuses 26 unchanged-cluster snapshots by `Rc`, reducing materialized edge occurrences to 12,274 and workspace edge scans from 38,894 to 31,498; exact mutations atomically invalidate the cache. | The proof charges every edge to `O(log n)` recursive scales and obtains `O((m+n log log n) log n)`. | **Open.** Duplicate unchanged-state materialization is removed and counted, but portal splits still require full re-materialization and materialized portal-segment classes are not structurally bounded. Implement incremental split updates or symbolic portal labels, then prove per-edge recursive-scale participation. Do not select `StructuralSourceBound` or `SourceMonotone` from these observations. |

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

| Command | Exit | Result |
| --- | ---: | --- |
| `git status --short` | 0 | only AN19 source-gate implementation and P9 documentation changed |
| `git diff --check` | 0 | clean |
| `cargo test -p rect-graph source_an19` | 0 | 31 tests passed; 456 fixed-radius directed-distance families and 456 threshold families match their independent Oracles; shared snapshots are reused only without mutation and portal splits force an exact rebuild |
| `cargo test -p rect-graph --lib` | 0 | 101 tests passed; projection caching preserves all hierarchy, contraction, recovery, and source-counter certificates |
| `cargo fmt --all -- --check` | 0 | clean |
| `python3 tools/check_biclique_bound.py` | 0 | bound check passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 245 passed and 3 existing release-scale campaigns ignored across 13 suites; 405.23 seconds |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 7 package documentation sets generated without warnings; 3.84 seconds |
| `cargo build --workspace --release` | 0 | 6 crates compiled successfully; 15.87 seconds |
| `python3 tools/check_release_consistency.py` | 0 | 10 runs, 499220 grid comparisons, 174767 polygon rows/components, and 27228 CP-SAT components verified |

The fixtures cover an exact path petal and Figure 6 window, rejection of a
nonunit edge, stable choice between equal diamond paths, a fractional radius,
an interior rational portal, Claim 15 differential equality, short-edge tree
expansion, atomic highway interval accounting, bidirectional rational
provenance splitting, dense-to-stable projection, complete-segment recovery,
rejection of partial/cyclic/disconnected recovery, unit Figures 4--5 recursion,
virtual-path suppression, radius/tree/stretch verification, mutation rejection,
exact-Oracle comparison on every connected four-node simple graph, top-level
source preservation across portal splits and quotient recursion, and
source-edge projection aggregate mutation rejection. The 500-node nonvirtual
path additionally demonstrates multiple materialized projection length classes,
nonzero provenance-free projection work even though its first top-level target
is not virtual, and per-source segment repetition beyond recursion depth.
