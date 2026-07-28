# P09.3.1 Source Structure Contracts

## Scope

This subphase turns the source assumptions consumed by Lemmas 5.4--5.5,
Theorem 8.2, and Theorem 1.2 into checked data contracts. It does not construct
a low-stretch forest or dynamic spanner and makes no runtime claim.

## Implemented contracts

- Positive exact rational edge lengths and weights with a checked numerator/
  denominator coordinate bound.
- Atomic update batches containing insertions, deletions, and vertex splits,
  with stable edge identifiers and exact batch/operation counters.
- Separate source `|U|` and `Enc(U)` counters. A split must list the smaller
  incident side, contributes `max(1, moved_edges)` encoding units, and is
  rejected atomically when the larger side is supplied.
- Explicit `k`, `L`, update-budget, split-budget, and
  encoding-budget plus `ceil(log2(n+1))^exponent` coordinate-bit gates.
- Rooted spanning-forest validation with one root per component.
- Definition 5.3 exact stretch recomputation and supplied overestimate checks.
- The Lemma 5.4 requirement that every post-initialization inserted edge has
  stretch overestimate exactly one.
- Edge-disjoint forest-piece partitioning, connected-subtree validation, full
  vertex coverage, at most one shared boundary vertex per piece, and exact
  `vol_G(W \\ R)` checks.
- Explicit spanner-subgraph path embeddings, simple contiguous path checks,
  maximum path length, vertex congestion, encoded embedding length, and
  re-embedded-spanner-edge counters.

## Evidence

- A weighted triangle fixture certifies stretches `2, 2, 3`, weighted initial
  stretch `7`, two overlapping pieces with one boundary vertex, and maximum
  piece volume four.
- An insertion between two singleton roots certifies source stretch exactly
  one.
- A split-plus-insertion batch updates stable endpoints and counters; a later
  duplicate deletion is rejected atomically without mutating the graph.
- A degree-three split rejects a two-edge moved side, accepts the one-edge
  smaller side, and records one operation and one encoding unit.
- A two-edge path spanner embeds the third triangle edge with path length two,
  maximum vertex congestion three, and encoded embedding length four.
- A coordinate bound of eight on three vertices passes an explicit
  `log2(n+1)^2 = 4`-bit gate.

## Source boundary

The contracts follow Definitions 5.2--5.3 and the enumerated guarantees in
Lemma 5.4, plus the explicit path/recourse quantities of Theorem 8.2 in
arXiv:2309.16629v1. Big-O constants are not invented: later constructors must
provide concrete observed certificates and only P9.3.7 may decide whether the
source asymptotic hypotheses are established.

## Audit

Phase baseline: `22e83712fccc484e0d56bac2dd3e4349f726c2f3`.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo test -p rect-graph source_lsst` | 0 | 4 focused tests passed |
| `cargo clippy -p rect-graph --all-targets --all-features -- -D warnings` | 0 | no warnings |

The permanent P8 rooted-forest and greedy-spanner implementations remain
available as independent small-instance Oracles.
