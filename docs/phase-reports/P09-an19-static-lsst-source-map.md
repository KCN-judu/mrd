# P09.3.2 AN19 Static LSST Source Map

## Status

**P9.3.2d's faithful implementation is complete; its runtime proof is deferred
at low priority.** The formal SIAM journal version of Abraham and Neiman was
checked, but it does not establish the reduced-event ordering/counting
conversion needed here. Consequently, the AN19 runtime chain remains
unverified. P9.3.3 through P9.5 may proceed with the source-shaped backend;
Low-priority P9.6a must close this proof debt before an AN19 runtime claim is
made.

Source recovery, exact single-petal and symbolic weighted-portal gates, compact
weighted hierarchy, recursive contraction, and fast membership-event processing
are implemented. This report maps the complete 2012 AN19 manuscript and the
complete 22-page 2019 SIAM journal text (DOI `10.1137/17M1115575`) to
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
Projection node-slot and incident-scan charges are `0fc48a1`.
The complete nonprojection workspace-scan ledger is `d17a6cd`.
The power-of-two chord-family reduced-class lower bound is `0b3b704`.

| Category | Item | Status | Evidence or limitation |
| --- | --- | --- | --- |
| Implemented and empirically audited | workspace scan counting, implementation counters/invariants, differential and regression tests | Complete | 247 tests passed and 3 existing tests were ignored; local and remote repository state was clean at `8f9ab06ce00c1e80a58e5b6302c14a408fefabd7` |
| Source-checked but unsupported | formal SIAM source identification | Complete | Abraham--Neiman, SIAM J. Comput. 48(2), 2019, pp. 227--248, DOI `10.1137/17M1115575`; the required conversion is not present |
| Refuted conversion branch | `O(log n)` reduced-cost classes from `O(log n)` original power-of-two lengths | Refuted | the `N=2^q` chord family has only `q+1` original classes but at least `N/2-1` distinct forward reduced costs |
| Still unproved | exact reduced-event ordering replacement and corresponding AN19 asymptotic runtime | Deferred, low priority | the event stream must bypass the linear reduced-cost class count with a proved exact ordering structure before P9.6a can certify complexity |
| Downstream gate | P9.3.2d proof debt | nonblocking implementation gate | P9.3.3 through P9.5 may continue; `AlmostLinear`, `an19_runtime_verified: true`, and AN19 runtime claims wait for P9.6a proof closure |

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
   certificates, differentially checked against `source_lsf::oracle::Lsst` on its
   bounded domain.
4. Region-growing event implementation and source-shaped counters establishing
   the near-linear bound without graph expansion or an Oracle fallback.

Gates 1--3 are implemented and empirically audited. Gate 4's runtime proof is
deferred because the reduced-event conversion is missing. This does not block
P9.3.3 through P9.5, but no AN19, Lemma 5.4, or production runtime claim is
made before P9.6a closes the proof debt.

## Linear reduced-class lower bound and remaining proof obligation

The missing conversion cannot be an `O(log n)` bound on exact reduced-cost
classes. For `N=2^q`, take the unit path `v_0,...,v_(N-1)` and, for every
`i <= N-3`, add the chord `{v_i,v_(N-1)}` of length
`2^ceil(log2(N-1-i))`. The path remains shortest from center `v_0`, so
`d(v_i)=i`. The graph has exactly `q+1` original power-of-two length classes.
For every `r` with `N/2 < r < N`, the chord from `v_(N-1-r)` has length `N`
and forward reduced cost

`N + d(v_(N-1-r)) - d(v_(N-1)) = N-r`.

These `N/2-1` values are distinct. Thus the exact reduced graph has
`Omega(N)` length classes even though the original graph has `O(log N)`
classes. Commit `0b3b704` checks the construction at five powers of two,
verifies every center distance, witnesses every derived cost (doubled by the
implementation's integral normalization), and retains the detailed 128-node
Figure 6 differential. The algebraic family, not the finite executions, proves
the asymptotic lower bound.

Potential reweighting still computes the correct multi-source labels using the
original length classes: ordinary transformed arcs have length `2 ell(u,v)`.
It does not preserve event order, because the required threshold for vertex
`v` is the transformed label minus the vertex-dependent potential `2 d(x,v)`.
The remaining obligation is therefore to produce a stable exact ordering of
those de-potentialized thresholds over every relevant radius/highway interval
in `O(m + n log log n)` work per cluster, or to prove a source-equivalent
aggregate bound yielding `O(m log n log log n)`. The proof must include the
exact rational/word representation after window division and recursive portal
splits, charge every ordering operation, and preserve the existing Figure 6
differentials. Comparison sorting, a fixed-`i128` radix argument, or an
unstated bounded-integer assumption is insufficient.

## Implemented single-petal gate

`source_an19::petal::UnweightedPetal` implements Figure 6 on the original unit-length vertex
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

- `source_an19::petal::WeightedPetalAtRadius` evaluates Claim 15 with exact rational reduced
  directed lengths. It represents a radius inside an edge as the original edge
  ID, orientation, and exact offset, without unit subdivision.
- Each highway records the exact original-edge portion whose forward directed
  cost is halved. The directed region-growing result is differentially equal
  to the cone-union baseline on their shared unit-length domain.
- `source_an19::projection::HighwayLedger` stores canonical exact intervals on original edges,
  atomically rejects positive overlap, merges touching intervals, and computes
  the effective endpoint-to-endpoint length after every portion is halved once.
- `source_an19::projection::ShortEdgeContraction` computes the cluster radius, contracts exactly the
  edges shorter than `rad(X)/n^2`, retains original IDs on quotient edges, and
  expands a certified quotient tree with deterministic internal forests into
  an original-edge spanning tree.

These pieces established P9.3.2c's symbolic representation and recovery
contracts. Subsequent P9.3.2d commits compose Figure 5 recursively and audit
the hierarchy, radius/stretch certificates, and workspace scans. The remaining
P9.3.2d proof debt is the reduced-event obligation stated above.

P9.3.2d now also has an exact weighted Figure 6 baseline.
`source_an19::petal::WeightedPetal` treats each successive highway edge as one parametric
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

`source_an19::experiment::hierarchy::Lsst` now completes Figures 4--5 on the exact unit-length
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
`source_lsf::oracle::Lsst`.

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
`source_an19::projection::Audit::verify` cross-checks their maxima against hierarchy
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

Every full projection now also records one materialization and classifies every
stable adjacency reference it visits as active internal, active boundary, or
inactive. The verifier requires `projection calls = cache hits + full
materializations`; internal references equal exactly twice the materialized
edge slots; and connected projected node slots are at most materialized edges
plus one root slot per full projection. Active boundary references are charged
to the two endpoints of each original, virtual-leaf, or split-created segment
lineage across the certified scale-entry bound. Inactive references use the
same two-endpoint charge for every portal-split lineage. The three classes must
sum to the recorded projection incident total, which remains a checked subset
of all workspace scans.

The 500-node unit fixture makes 165 projection calls: 83 cache hits and 82 full
materializations. Those builds contain 6,056 node and 5,974 edge slots, with
11,948 active-internal, 172 active-boundary, and 332 inactive incident scans;
their classified total is 12,452 of 18,290 workspace scans. The alternating
rational fixture makes 118 calls, 55 hits, and 63 builds, with 4,319 node and
4,256 edge slots, 8,512 internal, 113 boundary, and 203 inactive scans. Uniform
scaling preserves all counts, and mutations of builds, each scan class, and
their aggregate are rejected.

This closed projection-side node and incident-index work. At that commit, the
remaining structural ledger concerned nonprojection workspace scans and the
independent all-radii candidate-event work; the following workspace audit
closes the former while `StructuralSourceBound` remains unselected.

Every nonprojection workspace edge visit is now also classified as a radius
certificate scan, contraction-input scan, retained quotient-edge scan,
contraction-recovery scan, or final augmented-tree recovery scan. The verifier
requires these five classes plus the three projection incident classes to sum
exactly to all workspace scans. It independently rebuilds radius scans as
twice the stored radius-certificate edge total, contraction-input scans from
every nonbase compact-weighted certificate, retained scans from quotient-edge
counts, and contraction recovery from contracted edges plus twice the checked
quotient-tree size. Final recovery is reconstructed from the exact stable-edge
and augmented-tree lineage formulas using input edges and nodes, virtual
leaves, and portal splits. Overflow is checked at every sum and product.

The 500-node nonvirtual unit fixture's 5,838 nonprojection scans are all radius
scans. The alternating rational fixture decomposes its 7,215 nonprojection
scans into 4,032 radius scans, 1,496 contraction-input scans, and 1,687 final
recovery scans. A dedicated recursive-contraction fixture records 12 radius,
4 contraction-input, 2 retained, 4 contraction-recovery, and 11 final-recovery
scans. Uniform scaling preserves these counts, and every class is rejected
when mutated even if the aggregate workspace total is changed with it.

This closes the remaining workspace-scan ledger. Inspection of the complete
`hierarchy_work_units` sum leaves the all-radii candidate-event processing as
the independent unproved structural interface. It does not resolve the exact
reduced-event ordering gap, so `StructuralSourceBound` and `SourceMonotone`
remain unselected and `source_runtime_verified()` remains false.

## Exact all-radii event implementation evidence

Commits `7ea13da`, `28f9ff7`, `6c8cfac`, `98bb615`, `5e771d8`,
`a25ac08`, `d4dda8f`, and `02c8385` isolate and certify the fixed-projection event
contract from the existing hierarchy production path. `source_an19::oracle::event::Engine` uses
definition-level exact threshold and Figure 6 selection logic.
`source_an19::experiment::event::Engine` independently runs the integral-normalized exact
reduced costs `2(ell(u,v) + d(x,u) - d(x,v))`, records each insertion, pop,
replacement, tie, and stale item, and rejects disagreement with the existing
source-shaped threshold path rather than falling back. `source_an19::event::backend::Unavailable`
remains unavailable and returns an explicit error.

The canonical trace contains projection/recursion identity, stable source and
segment lineage, exact materialized and symbolic length, halving and structural
generations, reduced cost and event radius numerator/denominator pairs, queue
sequence numbers, stale reasons, before/after semantic state, endpoints,
directed incidences, deterministic tie fields, and six candidate charge keys.
Verification reruns the selected backend and rejects reduced-cost, radius,
source, depth, lineage, halving, tie, stale, duplicate-event, and state
mutations.

The bounded release campaign in `results/an19-event-adversarial.json` covers 31
fixed snapshots at sizes 16 and 32 across all A--H families. Oracle and reduced
outputs agree in every case. Family A records 20 reduced costs from 5 original
classes and then 40 from 6; this is a finite witness consistent with the
separately proved linear lower-bound family, not a new asymptotic proof. Family
G uses paired snapshots in which highway halving changes the reverse-key order
from 4/3 to 2/3. Families B, C, and F emulate recursive observation contexts;
they are not a hierarchy amortization argument.

This initially completed P9.3.2d-impl, -oracle, -trace, and -differential. The
follow-up `source_an19::event::certificate::LocalBound` also completes P9.3.2d-local-proof:
each fixed snapshot has at most `3n + 4m + 2` semantic events and
`n + 2m + 2` queue insertions/pops. The counterexample activity remains
empirical/proof-discovery evidence. P9.3.2d-global-proof, -pq-proof, and
-runtime remain planned low-priority proof work.
The reduced engine now also uses a stable exact binary min-heap and carries an
`An19PracticalQueueBoundCertificate`. For `I` insertions it reconstructs the
fixed-snapshot comparison bound `3 I ceil(log2(max(I,1))) + 2m`, separates
push, pop, and relaxation-label observations, and rejects strategy, scope, and
formula mutations. Oracle runs retain their independent explicit sort and do
not carry this implementation certificate. Across the 31 release cases, the
largest observed total is 473 and the largest conservative bound is 1,112.
This is an `O((n+m) log(n+m))` practical implementation bound, not the missing
source-equivalent `O(m+n log log n)` proof.
`source_an19::experiment::hierarchy::AmortizationMode` remains `AggregateRegressionOnly`, the priority-queue
mode remains `ReducedLengthMonotone`, and `source_runtime_verified()` remains
false.

## Complete event-engine phase audit

The implementation baseline for this closeout was `89b5ea3`; the pre-closeout
working HEAD was `4413e94`. All required commands completed successfully:

| Command | Exit | Result |
| --- | ---: | --- |
| `git status --short` | 0 | clean before closeout |
| `git diff --check` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `python3 tools/check_biclique_bound.py` | 0 | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 254 passed, 3 existing ignored, 13 suites, 395.92 s |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 7 package documentation sets generated without warnings |
| `cargo build --workspace --release` | 0 | release workspace current |
| `python3 tools/check_release_consistency.py` | 0 | P1 baseline and AN19 evidence checks passed; 30 reachable manifest commits |
| `python3 -m py_compile tools/generate_paper_tables.py tools/check_release_consistency.py` | 0 | both tools compile |
| bounded event CLI twice | 0 | stable JSON SHA-256 `53782128`; Markdown SHA-256 `6ae987a5` |
| paper-table generator twice | 0 | stable scope SHA-256 `539d98ed`; paper SHA-256 `031edcd4`; manifest SHA-256 `7eada86b` |
| `git diff 89b5ea3..HEAD -G '#\\[ignore' --stat` | 0 | no ignored tests changed |
| changed-line credential/local-path scan | 0 | no credentials, private keys, or absolute local paths |

The full campaign result is `results/an19-event-adversarial.json` with its
generated summary in `results/an19-event-adversarial.md`. The release checker
also verifies all eight families, 31 cases, Oracle agreement, five true
implementation/local statuses, three false global/PQ/runtime statuses, every
local-bound formula, every reduced-engine practical heap formula, absence of
that certificate on Oracle runs, and the retained Family A reduced-class witness. No
downstream runtime-dependent phase was started.

### Local-event-proof closeout audit

The local proof started from synchronized SHA `0dba080` and closes only the
fixed-snapshot cardinality substatus. The pre-closeout implementation HEAD was
`b4358a9`. The complete mandatory audit passed:

| Command | Exit | Result |
| --- | ---: | --- |
| `git status --short` | 0 | clean; branch ahead only by intended local-proof commits |
| `git diff --check` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `python3 tools/check_biclique_bound.py` | 0 | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 254 passed, 3 existing ignored, 13 suites, 407.52 s |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 7 package documentation sets generated without warnings |
| `cargo build --workspace --release` | 0 | release workspace current |
| `python3 tools/check_release_consistency.py` | 0 | all 62 Oracle/reduced certificates and conservative status boundaries verified |
| bounded event CLI twice | 0 | stable JSON SHA-256 `53782128`; Markdown SHA-256 `6ae987a5` |
| paper-table generator twice | 0 | stable scope SHA-256 `539d98ed`; paper SHA-256 `031edcd4`; manifest SHA-256 `7eada86b` |

No ignored test, production fallback, source-runtime mode, credential, private
key, or absolute local path was added. At this local-event-proof closeout, the
trace queue still used exact linear minimum scans and could perform
quadratically many comparisons in its certified item count. The later
practical heap-bound closeout below supersedes that implementation fact with a
stable binary heap, but does not resolve P9.3.2d's source proof debt. P9.3.3
may proceed while the resulting runtime claim remains prohibited.

## Runtime acceptance audit

| Issue | Observed | Source requirement | Current acceptance |
| --- | --- | --- | --- |
| global workspace scans | Recursive projections formerly scanned every augmented edge, including unrelated sibling clusters. | Section 7 charges work to the current cluster and then uses logarithmic edge participation. | `720f0cb` maintains stable incident-edge indexes and projects only the current cluster. Verified by hierarchy differentials and mutation tests. |
| repeated Figure 5 distances | The implementation formerly recomputed `d_X(x0, .)` for every later petal and located a target using a new path in `Y`. | Figure 5 fixes `d_X` and `P_x0,t(X)` for the entire decomposition; Claim 1 proves the fixed path remains in `Y`. | `720f0cb` rebuilds once after the imaginary first path and reuses fixed `X` distances for later target selection. |
| omitted heap work | Push/pop counts treated a binary-heap operation as one unit and did not count its comparisons. | A runtime certificate must charge the actual priority-queue implementation. | `720f0cb` counts every heap comparison. The certificate reports `BinaryHeap`, so it cannot claim the Section 7 runtime. |
| weighted length classes | Exact rational input could contain `m` distinct lengths. | Section 7 rounds down to powers of two so only `O(log n)` active length classes remain after scale restriction. | `27d5773` rounds production workspace lengths to `base * 2^j`, preserves original lengths for provenance/stretch, proves the factor-two interval, and is invariant under uniform scaling. |
| source priority queue | Production shortest paths and fixed-radius Claim 15 runs now use original edge-length classes. Potential reweighting changes every ordinary reduced arc `l+d(x,u)-d(x,v)` back to `l`; ordered highway source labels represent the half-length path and an interior portal exactly. The all-radii Figure 6 event stream still groups by reduced directed cost, and a 128-node power-of-two chord fixture produces 162 classes. | OMSW10 Sections 5--6 bounds its queue by the original edge-length set `L`; KMPb Corollary 5.5 states its fast `ConeCut` bound for `k` distinct cone distances. EEST05 Definition 4.4 charges an original edge length only when a path leaves the forward-edge ideal, but AN19 explicitly uses the different excess metric. Final AN19 Section 6, p. 245, repeats the jump from original power-of-two weights to improved Dijkstra on the reduced graph without bounding the reduced cost classes. | **Blocked.** `ece2722` closes the fixed-radius subproblem with 456 directed-distance differentials and a source-class counterexample audit. Neither the public manuscript nor the final journal text proves that AN19's exact event order has `O(log n)` classes; KMPb Lemma 5.6 also moves from distinct graph lengths to its `ConeCut` call without giving the missing conversion. Keep `ReducedLengthMonotone` until an authoritative correction or an independently proved exact rational event-order structure is available. |
| recursive amortization | The aggregate certificate still uses a fixed `1024` factor. Recursive projections remap every noncontiguous augmented cluster to exactly `0..|X|`. `e4f54af` also keeps top-level source IDs independent of quotient-local recovery provenance through portal splits and nested contractions, and stores only per-source and aggregate projection counters. On the deterministic 500-node unit path, one original length class becomes as many as 16 active projection classes. Before fixed-path reuse, 16,948 projected edge occurrences included 4,332 provenance-free segments and one original edge reached 111 occurrences. `bc61592` and `14e9abb` reduce materialized edge occurrences to 12,274 and workspace scans to 31,498. `8d68d59` then applies 39 exact portal splits in place across 83 cache hits, reducing those figures to 5,974 and 18,290. Its length-class multiset keeps the active maximum at 16 rather than hiding cached segment classes; one source edge still has 33 materialized occurrences. `5cc49f0` independently propagates unsplit source labels through portal splits and quotient recursion: the 16 active classes reduce to 2 symbolic source classes and 3 symbolic virtual classes. `b050625` independently reconstructs 2,256 logical source-scale participations from radius certificates; their per-source maximum is 9 across maximum partition depth 8, and same-scale contractions do not create a false level. `60fdfe4` decomposes the 5,974 occurrences into 4,533 source materializations, 61 extra source fragments, and 1,380 provenance-free occurrences, and structurally charges each class to certified scales and 49 attributed splits. `0fc48a1` decomposes 12,452 projection incident scans into 11,948 internal, 172 boundary, and 332 inactive references and charges node slots and all three scan classes. `d17a6cd` closes the remaining 5,838 unit and 7,215 rational nonprojection scans with exact radius, contraction, quotient, and recovery reconstruction. | The proof charges every edge to `O(log n)` recursive scales and obtains `O((m+n log log n) log n)`. | **Open.** Logical source scales, projection/source/fragment charges, node and incident scans, and the complete workspace-scan ledger are structurally certified. The exact all-radii candidate-event order remains outside a source-backed structural work bound. Keep `AggregateRegressionOnly` and `ReducedLengthMonotone` until that interface is proved. |

The fixed `1024 * m * ceil(log n) * ceil(log log n)` ceiling is therefore only
a regression guard for observed counters. The source-edge audit exposes where
work accumulates but does not convert the observed totals into an asymptotic
bound. It is not accepted as a proof. P9.3.2d is implementation-complete with
deferred proof debt; `source_an19::experiment::hierarchy::AmortizationMode`
remains `AggregateRegressionOnly`, and no AN19 production runtime or full
Lemma 5.4 completion is claimed.

## Persisted source proof debt

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

## External authoritative-source search ledger

The following read-only search was completed on 2026-07-28 UTC after the final
SIAM PDF became available locally. Failed or rate-limited requests are recorded
only as coverage limitations, not as evidence that a source does not exist.

| Surface | Query or record | Result relevant to P9.3.2d |
| --- | --- | --- |
| Local SIAM version of record | PDF metadata, rendered title page, and extracted complete text | Confirms the 22-page 2019 SIAM article, authors, pages 227--248, and DOI `10.1137/17M1115575`; no supplement or corrected event-order argument appears in the document |
| Crossref work and Unixref records | DOI `10.1137/17M1115575` | The deposited record identifies only the journal article and version-of-record PDF; its relation set is empty and contains no correction, erratum, or supplement relation |
| Crossref title search | Exact article title, publication dates from 2012 onward | Returns the 2012 STOC paper and 2019 SIAM article as the matching versions; no corrected article or supplement is identified |
| DBLP title search | Exact article title | Returns exactly the 2012 STOC and 2019 SIAM records |
| OpenAlex article record | Work `W2924819758` | Identifies one published version, no repository full text, no retraction, and 18 citing works; no correction or replacement version is linked |
| OpenAlex citation graph | All 18 works citing `W2924819758` | No citing title is an erratum, corrigendum, correction, or proof of the exact reduced-event conversion; the citations are applications or later metric/flow work |
| OpenAlex author/title searches | Ittai Abraham and Ofer Neiman works matching `petal` | Both catalogs identify the 2012 and 2019 versions and no later corrected petal-decomposition paper; an unrelated corrigendum in Neiman's catalog concerns a different paper |
| Focused web search | Exact DOI with `erratum`, `correction`, and `supplement` | No relevant result was returned |
| SIAM landing page and Semantic Scholar API | Direct article lookups | Automated access was respectively rejected and rate-limited; these attempts add no negative evidence |

No formal correction, author clarification, supplement, revised manuscript, or
later paper proving the exact conversion was found. This is a bounded search
result, not a proof of nonexistence. It leaves the exact reduced-event lemma
unproved and scheduled as P9.6a low-priority work. Closing that task requires
an authoritative corrected construction or an independently proved replacement
satisfying the exact rational event-order and `O(m log n log log n)` work
obligations.

## Repository-wide proof-debt consistency audit

The original documentation audit started from clean, synchronized local and
remote HEAD `8f9ab06ce00c1e80a58e5b6302c14a408fefabd7`. This lower-bound audit started
from local implementation HEAD `0b3b70499864a509f0e6622d6cd2779c0b876d95`
and remote HEAD `88a89b2c28ef3872bc32c3d0cd0d9d98d97c2eeb`. The SIAM PDF title page was
rendered and its metadata/text were checked again: 22 pages, Ittai Abraham and
Ofer Neiman, SIAM J. Comput. 48(2), pp. 227--248, DOI
`10.1137/17M1115575`. The repository contains no LaTeX or Typst paper source;
the available paper build is the deterministic `paper-tables.md` generation.

The generator produced stable SHA-256 values on two consecutive runs. The
status source, generated scope CSV, paper tables, README, limitations,
algorithm traceability, experiments, testing QA, references, near-linear flow
map, master plan, and P9 reports now separate implemented evidence,
source-checked-but-unsupported evidence, and unproved obligations. The release
consistency checker enforces the deferred-proof boundary and rejects explicit
runtime/P9.3.2d complexity-completion overclaims.

| Command | Exit | Result |
| --- | ---: | --- |
| `python3 -m py_compile tools/generate_paper_tables.py tools/check_release_consistency.py` | 0 | both documentation tools compile |
| paper-table generator, run twice | 0 | stable SHA-256: `fc68e6bb` for `paper-tables.md`, `350d60cc` for `scope-table.csv`, and `c7e0c7d8` for unchanged `manifest.json` |
| `cargo test -p rect-graph source_an19::tests::` | 0 | 32 passed |
| `cargo fmt --all -- --check` | 0 | clean |
| `python3 tools/check_biclique_bound.py` | 0 | bound check passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 247 passed, 3 existing ignored, 13 suites, 397.72 s |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 7 package documentation sets generated without warnings |
| `cargo build --workspace --release` | 0 | release workspace is current |
| `python3 tools/check_release_consistency.py` | 0 | release evidence and AN19 deferred-proof status are consistent |

## Focused evidence

| Command | Exit | Duration | Result |
| --- | ---: | ---: | --- |
| `git status --short` | 0 | <0.01 s | only the intended proof-debt documentation, generator, checker, and generated tables changed before closeout |
| `git diff --check` | 0 | <0.01 s | clean |
| `cargo test -p rect-graph source_an19::tests::` | 0 | 0.49 s | 32 tests passed; 456 fixed-radius directed-distance families and 456 threshold families match their independent Oracles; source/virtual projection charges and every workspace scan class survive valid hierarchy operations and uniform scaling while synchronized class/aggregate mutations are rejected |
| `cargo test -p rect-graph` | 0 | 0.50 s | 102 tests passed; projection and workspace scan charging preserve hierarchy, contraction, recovery, active-class, source-counter, symbolic-label, and recursive-scale certificates |
| `cargo fmt --all -- --check` | 0 | 0.43 s | clean |
| `python3 tools/check_biclique_bound.py` | 0 | 0.08 s | bound check passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 5.67 s | no warnings |
| `cargo test --workspace` | 0 | 397.72 s | 247 passed and 3 existing release-scale campaigns ignored across 13 suites |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 2.50 s | 7 package documentation sets generated without warnings |
| `cargo build --workspace --release` | 0 | 14.65 s | 6 crates compiled successfully |
| `python3 tools/check_release_consistency.py` | 0 | 2.40 s | 10 runs, 499220 grid comparisons, 174767 polygon rows/components, and 27228 CP-SAT components verified |
| `git diff af46e74..HEAD -G '#\[ignore' --stat` | 0 | <0.01 s | no ignored test changed; the same 3 documented release-scale campaigns remain ignored |
| generated-evidence diff and deterministic regeneration | 0 | <0.01 s | only the AN19 scope rows changed, and two consecutive generations produced identical hashes |
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
Uniform scaling preserves symbolic, source-scale, structural projection, and
workspace-scan charge counts; projection-build, node-slot, internal, boundary,
inactive, radius, contraction, and recovery mutations are rejected while the
priority mode remains `ReducedLengthMonotone` and `source_runtime_verified()`
remains false. The
500-node nonvirtual path additionally demonstrates multiple materialized
projection length classes,
only 2 source-label classes versus 16 materialized classes, 3 virtual-label
classes, nonzero provenance-free projection work even though its first
top-level target is not virtual, and per-source segment repetition beyond
recursion depth.

## Practical heap-bound closeout audit

The practical fixed-snapshot queue work starts from synchronized SHA
`ebde003f8907447dadb06d6551ccb4a95dae3db6`. Implementation commit `02c8385`
replaces the traced reduced engine's linear minimum scan with the stable exact
binary min-heap and introduces the comparison certificate. Commit `fbc869e`
adds release reconstruction and conservative documentation, and `bbf13b3`
records deterministic A--H evidence generated from the committed verifier.

All 31 reduced runs carry a valid `3 I ceil(log2(max(I,1))) + 2m` certificate;
all 31 Oracle runs carry none. The maximum observed/bound totals are 473 and
1,112. Exact semantic agreement remains true in every case. The source target
is not upgraded: `priority_queue_bound_proved`, `global_amortization_proved`,
and `an19_runtime_verified` remain false, P9.3.2d-pq-proof remains planned
low-priority work, and P9.3.3 through P9.5 may proceed. P9.6a remains
responsible for proof closure before an `AlmostLinear` claim.

| Command | Exit | Result |
| --- | ---: | --- |
| `git status --short --branch` | 0 | clean; branch ahead only by the three intended practical-heap commits before closeout |
| `git diff --check` | 0 | clean |
| `cargo fmt --all -- --check` | 0 | clean |
| `python3 tools/check_biclique_bound.py` | 0 | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 256 passed, 3 existing ignored, 13 suites, 392.76 s |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 7 package documentation sets generated without warnings |
| `cargo build --workspace --release` | 0 | release workspace current |
| `python3 tools/check_release_consistency.py` | 0 | all 31 reduced practical certificates reconstructed; all 31 Oracle certificates absent; conservative proof flags preserved |
| bounded event CLI twice | 0 | stable JSON SHA-256 `c89db530`; Markdown SHA-256 `c4fe7d1d` |
| paper-table generator twice | 0 | stable scope SHA-256 `539d98ed`; paper SHA-256 `031edcd4`; manifest SHA-256 `7eada86b` |
| source diff searched for `ignore` | 0 | no ignored test changed; the same 3 release-scale campaigns remain ignored |
| changed-file credential and local-path scan | 1 | no matches, as required |

The first attempted ignored-test audit used a malformed regular expression and
Git rejected it before scanning. The corrected source-diff query above ran
successfully and returned no changes. No failed product check was suppressed.
