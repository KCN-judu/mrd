# P09.3.2 Branch-Free Forest Core

## Status

P9.3.2d is blocked. This report covers the completed Appendix B.3 tree
core, full ST04 decomposition, global stretch overestimates, weighted-copy
reduction, and dynamic batch integration. The AN19 petal constructor,
weighted symbolic portals, hierarchy certificates, and complete workspace
scan ledger are implemented and empirically audited. The formal SIAM source
does not establish the reduced-event ordering/counting conversion mapped in
`P09-an19-static-lsst-source-map.md`; therefore the AN19 runtime and Lemma 5.4
remain unverified.

## Implemented source mechanics

- Validates a caller-supplied spanning tree against the exact source graph.
- Roots the tree and computes subtree sizes and deterministic heavy children.
- Builds Lemma B.9's auxiliary `T_H` by replacing every heavy chain with a
  minimum-depth-rooted balanced BST and retaining light parent links.
- Computes `R upward T_H` and independently checks branch freedom by original
  tree LCAs.
- Computes Definition B.5 congestion exactly as the sum of reciprocal graph
  lengths over every tree path.
- Produces the increasing congestion permutation `pi`, breaking exact ties by
  stable edge identifier.
- Constructs `F_T(R,pi)` by finding adjacent branch-free root pairs and deleting
  the minimum-`pi` edge on each root path.
- Constructs every auxiliary-depth prefix `B_i`, recomputes exact
  `str^{F_T(B_i,pi),ell}`, and evaluates Equation (56) as twice their sum.
- Independently proves the fixed Equation (56) vector bounds every active edge
  under an arbitrary later ancestor-closed root set.
- Applies insertion/deletion batches atomically, adds both updated endpoints'
  auxiliary ancestors, extends inserted-edge bounds with exactly one, and
  rejects any update whose new forest is not a subset of the old forest.
- Rechecks every active edge after each batch and records root additions,
  forest-edge removals, batches, and stretch checks.
- Decouples the initialized abstract tree topology/lengths from later dynamic
  graph endpoint changes. A vertex split adds `u upward T_H` and an isolated
  `u_NEW` root exactly as Appendix A.1 specifies, while moved graph edges are
  checked against the unchanged global stretch certificate.
- Verifies a delegated ST03/ST04 decomposition: connected edge-disjoint pieces
  cover the tree, shared vertices form a branch-free boundary, piece count is
  explicitly bounded, and adjacent non-boundary weight satisfies the exact
  published constant `40 ||w||_1 k / m`.
- Implements the complete Spielman--Teng `decompose/sub` DFS from
  arXiv:`cs/0607105`: exact `phi=2 sum(eta)/t`, all four piece-emission
  branches, edge-to-piece map `rho`, `h<=t`, pairwise piece intersection, and
  nonsingleton assigned weight at most `4 sum(eta)/t`.
- Correctly distinguishes the generic ST04 boundary from CKLPPS branch freedom:
  the initializer takes the ST04 boundary and then applies `upward T_H`.
- Initializes terminals from the certified decomposition boundary and every
  edge above an explicit large-stretch threshold before taking `T_H` closure.
- Constructs the weighted-copy graph `G_v` with
  `ceil(m v_e / ||v||_1)` unit-weight copies per active edge, exact copy maps,
  retained lengths, and a checked total of at most `2m` copies.
- Provides `source_lsf::oracle::Lsst` for differential testing on bounded small
  graphs (`n <= 12`, `m <= 24`). It exhaustively enumerates spanning trees,
  evaluates exact weighted stretch with the certified forest routines, and
  chooses the exact minimum with a stable edge-ID tie break. This exponential
  Oracle is not the AN19 constructor and establishes no production runtime
  claim.
- Verifies the monotonicity required by Lemma 5.4: enlarging the ancestor-closed
  root set produces a forest edge subset.

## Evidence

A five-vertex branched tree with one off-tree edge validates the auxiliary
height, branch-free closure, exact congestion order, one-root-per-component
edge count, and decremental forest property. A separate fork fixture rejects
the non-branch-free root set containing both children without their LCA. The
same fixture checks all five active edges against the fixed global stretch
certificate after the root set changes. A separate dynamic fixture inserts an
edge, deletes an original tree edge, proves forest subset monotonicity after
both batches, and verifies the deleted tree edge is absent. The same trace
splits an endpoint, moves the inserted edge to the new vertex, checks the new
isolated root, and preserves its unit stretch certificate. Additional fixtures
validate a two-piece branch-free decomposition and a `1,2,3` weighted graph
whose copy multiplicities are `1,1,2`. The five-edge fixture also executes the
ST04 DFS with `t=2`, verifies `phi=5`, checks every `rho(e)` has size one or
two, and consumes the resulting boundary in the dynamic LSF initializer. A
weighted triangle with edge weights `10,1,1` checks all three spanning-tree
candidates and proves that the exact Oracle selects a tree containing the
weight-10 edge, with weighted stretch `25` and total graph weight `12`.

## Audit

Baseline: `2a553013554db1b6623f82cf15c3392ea2206f63`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git status --short` | 0 | only the Oracle implementation, export, and this report were modified |
| `git diff --check` | 0 | clean |
| `cargo test -p rect-graph source_lsf` | 0 | 7 focused tests passed |
| `cargo fmt --all -- --check` | 0 | clean |
| `python3 tools/check_biclique_bound.py` | 0 | bound check passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 215 passed, 3 ignored across 13 suites |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | 7 package documentation sets generated without warnings |
| `cargo build --workspace --release` | 0 | 6 crates compiled successfully |
| `python3 tools/check_release_consistency.py` | 0 | 10 runs, 499220 grid comparisons, 174767 polygon rows/components, and 27228 CP-SAT components verified |

The implemented AN19 candidate output has exact tree/stretch and workspace-scan
verifiers. Those implementation checks do not prove the missing reduced-event
class bound. P9.3.2d remains a hard blocker; no complete Lemma 5.4 or runtime
claim is made by this partial core.
