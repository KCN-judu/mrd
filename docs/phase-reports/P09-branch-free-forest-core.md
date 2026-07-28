# P09.3.2 Branch-Free Forest Core

## Status

P9.3.2 remains in progress. This report covers the completed Appendix B.3 tree
core; Lemma B.7 decomposition, global stretch overestimates, weighted LSST
construction, and dynamic batch integration remain required before Lemma 5.4
can close.

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
- Verifies the monotonicity required by Lemma 5.4: enlarging the ancestor-closed
  root set produces a forest edge subset.

## Evidence

A five-vertex branched tree with one off-tree edge validates the auxiliary
height, branch-free closure, exact congestion order, one-root-per-component
edge count, and decremental forest property. A separate fork fixture rejects
the non-branch-free root set containing both children without their LCA. The
same fixture checks all five active edges against the fixed global stretch
certificate after the root set changes.

## Audit

Baseline: `2a553013554db1b6623f82cf15c3392ea2206f63`.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo test -p rect-graph source_lsf` | 0 | 2 focused tests passed |
| `cargo fmt --all -- --check` | 0 | clean |
| `cargo clippy -p rect-graph --all-targets --all-features -- -D warnings` | 0 | no warnings |

No low-stretch-tree, Lemma 5.4, or runtime claim is made by this partial core.
