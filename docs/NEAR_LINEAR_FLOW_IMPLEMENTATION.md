# Deterministic Almost-Linear Flow Specification

## Authoritative source

Jan van den Brand et al., "A Deterministic Almost-Linear Time Algorithm for
Minimum-Cost Flow," arXiv:2309.16629v1, 2023. This document maps its stated
contracts to intended code; it does not claim that those components exist.

Retrieved predecessor sources: Chen et al., "Maximum Flow and Minimum-Cost
Flow in Almost-Linear Time," arXiv:2203.00671v2, 2022; and Kang--Payor,
"Flow Rounding," arXiv:1507.08139v1, 2015; and Chuzhoy--Saranurak,
"Deterministic Algorithms for Decremental Shortest Paths via Layered Core
Decomposition," arXiv:2009.08479v1, 2020 (SODA 2021).
Dynamic-tree operations are pinned to Sleator--Tarjan, "A Data Structure for
Dynamic Trees," STOC 1981, DOI 10.1145/800076.802464. KP15 uses this family
for cycle cancellation; Section 6.2 of the primary source uses link-cut trees
to apply compact min-ratio-cycle updates.

## Theorem-to-module map

| Source | Required contract | Intended Rust module | Required evidence |
| --- | --- | --- | --- |
| Lemma 4.1 (KP15) | Deterministically round a feasible fractional integral-capacity min-cost flow in `O(m log m)` without increasing cost | `graph::min_cost::rounding` | exact feasible/cost-preserving differential against a rational oracle |
| Definitions 4.2--4.5 | Exact incidence matrix, signed circulations, positive lengths, gradients, hidden stable witness, `Update`/`Query`/`Detect` semantics | `graph::min_ratio_cycle` | P8.1 checked contract ledger: conservation, positivity, valid-pair, factor-two stability, and replay invariant tests; no dynamic cycle query claim |
| Theorem 4.6 of the primary source, encapsulating CKLPPS22 Theorem 4.3 | IPM reduction, additive-half fractional target, quasipolynomial capacity/cost domain, total update/detect accounting | `graph::interior_point` | certified fixed-point Equation (9), checked approximation bounds, potential decrease, update/detect accounting, and recovery tests |
| Lemma 5.4 | Dynamic low-stretch rooted forest with decremental forest edges, vertex splits, stretch upper bounds, recourse | `graph::rooted_forest`, `graph::source_an19` | P8.2 forest contracts and P9.3.2 AN19 hierarchy/workspace counters are implemented and empirically audited; the checked SIAM source does not establish the reduced-event conversion needed for the AN19 runtime. This is deferred proof debt, so no Lemma 5.4 bound is claimed, but P9.3.3 through P9.5 may build the source-shaped backend. |
| Lemma 5.5 | Deterministic multiplicative-weights collection of low-stretch forests | `graph::source_lsf::experiment::mwu` | P9.3.3 constructs exactly `k` source-shaped LSFs by weighted-copy expansion, AN19 static trees, and the P9.3.2 forest initializer. It records an exact finite-instance MWU certificate with every round's `W` envelope and uniform per-edge average-stretch bound. The P8.4 weighted-Kruskal baseline remains an Oracle; no `O(log^7 n)` or runtime claim is made until a uniform source Lemma 5.4 `W = O(log^4 n)` and word-bound audit pass. |
| Theorem 8.1, P9.3.4a | Exact static embedding-composition contract | `graph::source_spanner::{model,oracle}` | exact simple `H'`/`J` graphs, `J~ subset J`, explicit `J -> H'` and `J -> J~` paths, exact composed-path/path-length/edge-congestion/vertex-congestion/maximum-degree audit, and bounded enumerating simple-path Oracle; no expander, sparsity, or Theorem 8.1 asymptotic claim |
| Theorem 8.4, P9.3.4b | Certified finite-domain witness expander | `graph::source_spanner::experiment::{circulant,domain}` | positive levels select the first canonical circulant satisfying every exact degree sandwich and an exhaustive positive expansion certificate; level zero uses `J_0` directly. The older complete witness remains an experiment fixture. All unsupported inputs reject, so this is not CGLNPS20's general construction or a runtime claim. |
| Theorem 8.5, P9.3.4c | Certified finite-domain one-level expander decomposition | `graph::source_spanner::experiment::{decomposition,domain}` | accepts a connected finite graph only when its full edge set forms one level that passes capacity, exact minimum-degree floor, and exhaustive cut-expansion checks; it selects the greatest source-valid level through `ceil(log2(n))`, records the explicit sole component and every source edge exactly once, then recomputes the complete certificate. Disconnected, multi-level, and out-of-domain cases reject. This is not the general deterministic decomposition or a source runtime claim. |
| Theorem 8.6, P9.3.4d | Exact decremental-path semantics and bounded certificate | `graph::source_spanner::decremental::{state,query,certificate}` | immutable deletion snapshots, stable edge IDs, replayed accepted/rejected request traces, isolated-vertex monotonic pruning, and stable-ID BFS paths. A separate bounded simple-path enumerator differentially verifies the semantic response. This intentionally constrained exact model does not implement Theorem 8.6's expander pruning rule, general decremental structure, or source work/depth bounds. |
| Theorem 8.1, P9.3.4e | Certified finite Algorithm 4 replay | `graph::source_spanner::algorithm4::{witness,first_embedding,second_embedding,finalize}` | source Algorithm 4 (arXiv:2309.16629v1, pp. 41--42) is replayed for the certified single-level, single-component finite domain: source degree weights, `W -> J` bounded path/threshold/deletion traces, an independent `J -> W` bounded path/deletion loop, oriented composition with local loop erasure, image subgraph, and exact composition audit. Multi-level components, Theorem 8.6's general pruning, and Theorem 8.1 bounds remain unimplemented and unclaimed. |
| Definitions 5.6--5.8, Theorem 5.1 | d-level tree chain, shifted single branch, compact cycle representation, hidden-stability data structure | `graph::source_min_ratio::{input,model,chain,cycle,candidate,spanner,terminal,query,execution}` | P9.4 provides immutable source-tree snapshots, stable branch IDs, deterministic shifted selection, compact-cycle decoding, and finite ledger transitions. P9.5a has exact IPM/source/circulation provenance through `input::Input`, an exact heap over explicitly declared fundamental spanner/tree candidates through `candidate::Registry`, a source-shaped terminal tree, and an immutable finite `spanner::Snapshot` that emits every rejected core edge as an explicit contiguous `SpannerPath` plus anchor edge. `source_flow::iteration::Step::from_terminal_candidate` still bridges only the terminal heap after exact coordinate equality. Cross-snapshot candidate maintenance and the complete-candidate differential remain absent. P8's `dynamic_min_ratio::{oracle,experiment}` remains the permanent enumerating/replay baseline and cannot be imported by production. No Theorem 5.1 selection or bound claim is made. |
| Theorem 8.2 | Decremental sparse spanner with short low-congestion embeddings under deletions and vertex splits | `graph::decremental_spanner` | P8.3 simple-undirected certificate verifier: explicit embedding paths, congestion, deletion/split validity, and recourse; no Theorem 8.2 construction or bound claim |
| Theorem 1.2 and Section 9 | Finite source-shaped dynamic low-stretch tree semantics using contracted forests and embedded spanners | `graph::source_lsst::{level,bucket,chain,replay}` | exact provenance, scaled-length, embedding, tree, rebuild, and recourse certificates on an explicit finite integral connected domain; no source stretch/runtime bound |
| Theorem 1.1 | Exact min-cost/max-flow only after all preceding assumptions and exact recovery hold | `graph::almost_linear` | no fallback, complete source-assumption gate, differential and complexity evidence |

## Representation and precision rules

The existing `u64`-capacity `FlowNetwork` is insufficient: it has no costs,
demands, fractional flow, signed circulation, incidence representation, or
exact rational potential arithmetic. The proposed circulation layer therefore
needs signed integer costs/demands/capacities and an exact rational internal
representation. It must reject values outside the source's checked
quasipolynomial-domain gate before an almost-linear complexity claim.

The source's dynamic data structure is not a generic adversarial min-ratio
cycle API. Definition 4.4 requires a hidden stable witness, valid-pair bounds,
factor-two stability for non-explicitly-updated edges, and quasipolynomial
bounds on lengths and witness upper bounds. These are runtime assumptions that
must be represented as checked certificates, not comments.

## P6 blocker

arXiv:2309.16629 explicitly delegates essential constructive details to
CKLPPS22, KP15, CS21, and dynamic-tree machinery. The three arXiv sources are
version-pinned. CS21 is limited to decremental simple undirected graphs, so the
spanner layer must not claim a directed or arbitrary-update implementation.
The source map is complete for P6. P7 must implement only the superlinear exact
circulation and rounding Oracle first; P8 must split the dynamic structures
into source-backed subphases before claiming their amortized guarantees.

## Current P9.3.2d deferred proof debt

The formal SIAM version of Abraham--Neiman, *Using Petal-Decompositions to
Build a Low Stretch Spanning Tree*, SIAM J. Comput. 48(2), 2019, pp. 227--248,
DOI `10.1137/17M1115575`, has been obtained and checked. It does not establish
the conversion from the implementation's exact reduced costs
`ell(u,v) + d(x,u) - d(x,v)` to an explicitly bounded ordered set of
reduced-event equivalence classes. Workspace scan counting and finite tests are
complete implementation evidence, but they do not prove this asymptotic
obligation. P9.3.2d's faithful implementation and exact Oracle differential are
complete. The proof obligation is explicitly deferred at low priority while
P9.3.3 through P9.5 build the complete source-shaped flow backend. P9.6 must
return to it before approving the `AlmostLinear` name, a true
`an19_runtime_verified` report, P9 complexity closeout, or any AN19 runtime
claim.

The implementation side of this boundary is now explicit. `source_an19::oracle::event::Engine`
enumerates definition-level fixed-snapshot events, while
`source_an19::experiment::event::Engine` runs the separate exact reduced-cost queue and rejects
any semantic disagreement. `source_an19::event::backend::Unavailable` is a replaceable placeholder
that returns an explicit unsupported error. The canonical trace and six charge
maps support future proof or counterexample work, but their bounded A--H
campaign is not itself a proof. A separate structural certificate now proves
the fixed-snapshot semantic-event and queue-item cardinalities. Reduced runs
also certify the stable binary heap's counted comparisons by
`3 I ceil(log2(max(I,1))) + 2m`. That practical
`O((n+m) log(n+m))` result does not supply the missing source-equivalent
priority-queue comparison or global amortization proof.

## Historical P6 next action

Retrieve and archive the exact cited predecessor versions, especially CKLPPS22
for the IPM/data-structure interface, KP15 Section 4 for deterministic
rounding, and CS21 for decremental expander shortest paths. Then split P8 into
source-backed subphases before implementing the dynamic structures.
