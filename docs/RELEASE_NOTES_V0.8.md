# v0.8.0 Boundary-Indexed Adaptive Path-Tree

This release hardens the supported finite-grid path-tree frontend.

- `BoundaryIndex` and shared chord endpoint metadata remove repeated linear
  boundary scans from CompactOnly production paths.
- Clean endpoint ownership uses a deterministic map; the pairwise classifier
  remains an independent reference.
- BoundaryLaminar gap labels use a checked event sweep; nested labeling remains
  available for differential tests.
- Mixed-branching geometry witnesses are searched and persisted with dual,
  compact-path, HLD, biclique, diagnostics, and SVG artifacts.
- CompactOnly uses indexed frontier completion and exact `BuildBothExact`
  orientation selection. `BoundEstimate` remains an explicit heuristic policy
  after five positive-regret mixed-witness rows were found. FullyAudited keeps
  reference rescan and `BuildBothExact`.

The release still targets ordinary finite unit-cell regions. General polygon
Soltan--Gorpinevich enumeration, formal degenerate holes, optimized Theorem 8
constants, and an almost-linear exact-flow backend remain outside this release.
