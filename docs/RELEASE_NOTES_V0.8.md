# v0.8.0 Boundary-Indexed Adaptive Path-Tree

This release hardens the supported finite-grid path-tree frontend.

- `BoundaryIndex` and shared chord endpoint metadata remove repeated linear
  boundary scans from CompactOnly production paths.
- Clean endpoint ownership uses a deterministic map; the pairwise classifier
  remains an independent reference.
- BoundaryLaminar gap labels use a checked event sweep; nested labeling remains
  available for differential tests.
- The complete indexed-frontend differential covers 950,557 inputs and
  1,053,939 components with zero mismatches or solver errors.
- Sixteen mixed-branching geometry witnesses are delta-minimized to 47--115
  cells and persisted with dual, compact-path, HLD, biclique, diagnostics, and
  SVG artifacts. The connected-sum family grows all required structural
  quantities through eight modules.
- CompactOnly uses indexed frontier completion and exact `BuildBothExact`
  orientation selection. `BoundEstimate` remains an explicit heuristic policy
  after five positive-regret mixed-witness rows were found. FullyAudited keeps
  reference rescan and `BuildBothExact`.
- The committed advantage corpus contains 14 strict path-tree sigma advantages
  and zero strict 4D advantages; every retained row has equal optimum and
  canonical rectangles.

The release still targets ordinary finite unit-cell regions. General polygon
Soltan--Gorpinevich enumeration, formal degenerate holes, optimized Theorem 8
constants, and an almost-linear exact-flow backend remain outside this release.
