# Geometry-Backed Path-Tree Families

Path-tree evidence must come from unit-cell polygons, not from synthetic trees
fed into production code. Each generated instance is checked for clean
hole-free eligibility, exact effective-chord families, and dissection validity.

The v0.7 family suite is organized as follows:

- **laminar-chain**: nested fixed-orientation intervals; the dual is a path and
  opposite chords exercise single edges, prefixes, suffixes, middle subpaths,
  and the whole tree;
- **laminar-star**: disjoint fixed-orientation intervals; the dual is a star
  rooted at the outer region and paths join multiple leaves;
- **balanced-laminar**: recursive nested/disjoint combinations producing a
  roughly balanced dual and paths crossing heavy chains;
- **asymmetric-orientation**: clean components whose two orientations have
  materially different path counts, tree-edge counts, and sigma.

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
