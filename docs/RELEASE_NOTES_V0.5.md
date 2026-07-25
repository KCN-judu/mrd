# v0.5.0 - Prepared Grid Pipeline and Dense Recovery

Release tag: `v0.5.0-prepared-grid-pipeline`

- builds one prepared component context per CompactOnly solve;
- consumes prepared interior runs without production cell hash sets;
- uses dense cut arrays as the indexed backend's only mutable cut state;
- recovers rectangles with dense visited storage and an integer queue;
- reuses prepared occupancy in final validation;
- preserves pairwise enumeration, ordered completion, and hash BFS as Oracles;
- records separate conflict-heavy, completion-heavy, and area-heavy evidence.

The release does not implement general polygon enumeration, formal degenerate
holes, an optimized Theorem 8 recursion, or an almost-linear exact-flow
backend.
