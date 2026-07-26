# v0.8.1 Boundary-Indexed Adaptive Path-Tree

This patch release supersedes the immutable v0.8.0 tag without moving it.

- CompactOnly and FullyAudited both default to exact `BuildBothExact` path-tree
  orientation selection. `BoundEstimate` remains an explicit benchmark policy;
  five committed counterexamples have positive sigma regret.
- The complete indexed-frontend differential verifies 950,557 inputs and
  1,053,939 components with zero mismatches and zero solver errors.
- The committed witness search is byte-for-byte idempotent. Its 16 minimized
  witnesses span 47--115 cells, and derived connected-sum witnesses cannot seed
  their own family generation.
- The path-tree advantage corpus has 27 eligible mixed-orientation rows, 14
  strict path-tree sigma advantages, and zero strict 4D advantages. Every row
  preserves the exact optimum and canonical rectangles.
- Specialized path-tree benchmark commands now record their run provenance in
  the manifest. The high-q representation comparison reaches q=2,052 with zero
  counterexamples.
- Full formatting, biclique-bound, Clippy, workspace-test, rustdoc, release
  consistency, exhaustive, random, polyomino, adversarial, CP-SAT, dense,
  dual-differential, gap-differential, witness, and stored-regression gates pass.

The implementation still targets ordinary finite unit-cell regions. General
polygon enumeration, formal degenerate holes, optimized Theorem 8 constants,
and an almost-linear exact-flow backend remain outside this release.
