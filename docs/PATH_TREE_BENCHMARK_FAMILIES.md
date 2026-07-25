# Geometry-Backed Path-Tree Families

Path-tree evidence must come from unit-cell polygons, not from synthetic trees
fed into production code. Each generated instance is checked for clean
hole-free eligibility, exact effective-chord families, and dissection validity.

The v0.7 family suite is organized as follows:

- **laminar-chain**: the corrected clean complete-bipartite grid realization;
  nested fixed-orientation intervals form a path and opposite chords exercise
  long path incidences;
- **laminar-star**: disjoint fixed-orientation intervals; the current clean
  comb realization is retained as a star-shape dual probe;
- **balanced-laminar**: recursive comb geometry produces a high-degree
  branching dual probe; path-bearing mixed witnesses are audited separately
  against the reference population;
- **asymmetric-orientation**: a fixed-mask clean mixed H/V witness whose two
  orientation metrics are recorded explicitly; it exercises the axis-view
  mapping independently of the scale parameter used by the other families.

For every family the benchmark records dual vertices/edges, depth, branching,
heavy-chain count, heavy-chain intervals, canonical segment nodes, bicliques,
sigma, path-length and tree-edge occurrence metrics. The regression bounds are
theorem-shaped guards with generous constants, not formal proofs:

```text
sum_heavy_intervals <= C1 * path_count * ceil_log2(q + 1)
sum_canonical_nodes <= C2 * path_count * ceil_log2(q + 1)^2
tree_edge_occurrences <= C3 * tree_edge_count * ceil_log2(q + 1)
```

Synthetic chain/star/balanced trees remain useful HLD unit tests, but they do
not count as geometry evidence.

The release probe confirms that these are not merely labels: the
`balanced-laminar-9` unit-cell instance reaches dual-tree branching degree 9
in the horizontal-tree axis view. The family CSV records the corresponding
dual vertices, depth, path counts, and sigma for every generated fixture.

An equal-depth four-side-notch candidate was deliberately rejected from the
population: its horizontal and vertical chord endpoints coincide, so the
clean certificate reports `SharedBoundaryEndpoint`. It is not used as
evidence or silently counted as a star.
