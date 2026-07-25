# v0.6.0 - True Compact Path-Tree Construction

Release tag: `v0.6.0-true-compact-path-tree`

This milestone makes the clean path-tree CompactOnly route compact in its
actual data structures:

- `CompactTreePath` stores endpoint regions only; explicit edge vectors and
  per-path BFS remain independent FullyAudited oracles.
- Endpoint-only heavy-light decomposition emits canonical segment intervals
  directly and computes path lengths from tree depths.
- `BoundaryLaminar` builds the clean region dual from normalized boundary
  interval containment, without unit-chord expansion, area flood fill, or
  prepared-grid transposition.
- Execution traces and diagnostics record every nonmaterialization contract,
  dual backend, interval count, owned estimates, and explicit path records.
- The clean complete-bipartite campaign covers `t=1,2,4,8,16,32,64,128`.

The finite-grid scope is unchanged: general polygon sweeps, ornaments,
degenerate holes, optimized Theorem 8 sorting, and almost-linear exact flow
remain outside the implementation.
