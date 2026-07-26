# Boundary Gap Backend Differential

The v0.8 frontend has two interchangeable boundary-gap labelers:

- `ReferenceNested`, which scans every interval for every boundary gap;
- `EventSweep`, which processes sorted half-open interval start/end events.

The production path uses `EventSweep`. `ReferenceNested` remains an independent
correctness oracle and is selectable through the backend-aware solver API.

The complete reproducible campaign is:

```text
target/release/rect-cli benchmark \
  --suite path-tree-gap-differential \
  --max-cells 12 --random-cases 100000 \
  --sizes 1,2,4,8,16,32,64,128 \
  --output results/v0.8-gap-backend-differential.csv
```

It compares indexed boundary lookup against the preserved linear lookup,
shared endpoint metadata against coordinate endpoint recovery, the indexed
clean classifier against the pairwise classifier, and both gap labelers. For
every clean component it compares both path-tree orientations, dual
chord-labeled edges, boundary-gap regions, endpoint regions, compact paths,
HLD arrays, biclique partitions, final rectangles, optimum counts, and both
cell-exact validators.

The population contains all nonempty binary 3x3 and 4x4 masks, all
free-polyomino levels through 12 cells under every dihedral transform plus a
translation replay, 100,000 seed-42 connected regions, all stored path-tree
regressions, the geometry families, and complete-bipartite instances through
`t=128`. A disagreement is delta-minimized and persisted with its original and
minimized grid and exact difference labels.

CompactOnly production diagnostics continue to require zero linear boundary
lookups and zero pairwise endpoint comparisons. Event rows require zero
interval-membership tests and exactly one push/pop per interval. The frozen
counts are in `results/v0.8-gap-backend-differential.{csv,json,md}`.

The frozen report contains 950,557 inputs and 1,053,939 components, including
385,947 clean components. It records 16,530,980 boundary-index comparisons,
3,368,464 endpoint-metadata comparisons, 1,053,939 classifier comparisons, and
771,894 orientation comparisons. All 385,947 clean components verify, with zero
mismatches and zero solver errors. The reference backend performs 52,388,678
interval-membership tests; the event backend performs 409,593 pushes and the
same number of pops.
