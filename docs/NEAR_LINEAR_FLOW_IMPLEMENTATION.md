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
| Lemma 4.1 (KP15) | Deterministically round a feasible fractional integral-capacity min-cost flow in `O(m log m)` without increasing cost | `rect-graph::min_cost::rounding` | exact feasible/cost-preserving differential against a rational oracle |
| Definitions 4.2--4.5 | Exact incidence matrix, signed circulations, positive lengths, gradients, hidden stable witness, `Update`/`Query`/`Detect` semantics | `rect-graph::min_ratio_cycle` | P8.1 checked contract ledger: conservation, positivity, valid-pair, factor-two stability, and replay invariant tests; no dynamic cycle query claim |
| Theorem 4.6 (CKLPPS22) | IPM reduction, additive-half fractional target, quasipolynomial capacity/cost domain, total update/detect accounting | `rect-graph::interior_point` | checked bounds, exact rational arithmetic, potential decrease, and recovery tests |
| Lemma 5.4 | Dynamic low-stretch rooted forest with decremental forest edges, vertex splits, stretch upper bounds, recourse | `rect-graph::rooted_forest` | P8.2 exact rooted-forest baseline: deletion/split preconditions, static Definition 5.3 certificate recomputation, root-path trace, and recourse counters; no Lemma 5.4 bound claim |
| Lemma 5.5 | Deterministic multiplicative-weights collection of low-stretch forests | `rect-graph::lsf_mwu` | per-edge average-stretch and deterministic ordering tests |
| Definitions 5.6--5.8, Theorem 5.1 | d-level tree chain, shifted single branch, compact cycle representation, hidden-stability data structure | `rect-graph::dynamic_min_ratio` | encoded-cycle, update/query/detect, shift/rebuild, and amortized-accounting tests |
| Theorem 8.2 | Decremental sparse spanner with short low-congestion embeddings under deletions and vertex splits | `rect-graph::decremental_spanner` | embedding path, congestion, deletion/split, and recourse certificates |
| Theorem 1.2 and Section 9 | Dynamic low-stretch spanning tree using contracted forests and embedded spanners | `rect-graph::dynamic_lsst` | edge-count range, stretch, rebuild, and update-work counters |
| Theorem 1.1 | Exact min-cost/max-flow only after all preceding assumptions and exact recovery hold | `rect-graph::almost_linear` | no fallback, complete source-assumption gate, differential and complexity evidence |

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

## Next action

Retrieve and archive the exact cited predecessor versions, especially CKLPPS22
for the IPM/data-structure interface, KP15 Section 4 for deterministic
rounding, and CS21 for decremental expander shortest paths. Then split P8 into
source-backed subphases before implementing the dynamic structures.
