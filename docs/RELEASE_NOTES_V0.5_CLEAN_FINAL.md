# v0.5 Clean Geometry Completion

This follow-up closes the finite-grid structural gaps left by the prepared
pipeline release without moving either existing v0.5 tag.

- The clean complete-bipartite family is generated on an integer grid for every
  positive `t` whose dimensions fit `usize`.
- The reference and grid-run chord enumerators verify `|H|=|V|=2t` and
  `G=K_{2t,2t}` for the stored family campaign.
- The path-tree backend constructs both tree/path orientations, selects the
  smaller HLD biclique partition deterministically, and records the selected
  orientation in diagnostics and certificates.
- CompactOnly path-tree execution remains edge-free: no explicit conflict
  graph, Hopcroft--Karp, C0 partition, or full edge-partition audit is built.
- Benchmark CSVs now expose conflict representation, orientation, dual-region
  count, path count, path-edge incidences, canonical-node count, sigma, and
  four-dimensional sigma.

The implementation remains finite-grid and practical. General polygon input,
formal degenerate holes, the classical general-polygon `O(n log n)` sweep, and
the cited almost-linear exact-flow backend remain outside this artifact's
implementation scope.
