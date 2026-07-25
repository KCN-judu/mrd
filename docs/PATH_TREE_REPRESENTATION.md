# Clean Hole-Free Path-Tree Representation

For a clean hole-free component, `rect-dominance::path_tree` constructs a
reference region dual by inserting every vertical effective chord as a unit
cut, flood-filling occupied cells across uncut sides, and labeling the two
incident regions of each chord. The implementation checks connectivity,
unique labels, and `|E| + 1 = |V|`; it is deliberately area-sensitive and is a
reference construction, not the paper's planar-sweep runtime.

Every horizontal chord is then mapped to the unique region-tree path between
its endpoint regions. The path is recovered from tree geometry, never from the
explicit conflict graph. FullyAudited verifies that the path edge set equals
the independently materialized geometric neighborhood.

Heavy-light decomposition stores parent, parent edge, depth, subtree size,
heavy child, chain head, chain ID, and edge position. Equal subtree sizes are
broken by the smallest region ID. Each path is decomposed into disjoint
canonical segment-tree intervals. A canonical node becomes one biclique whose
left side is the set of horizontal paths selecting it and whose right side is
the chain-edge range below it. The resulting partition is audited against the
explicit graph only in FullyAudited mode; CompactOnly uses the partition and
flow certificate without building that graph.

The public representation selector is:

```text
dominance-4d  existing four-dimensional representation
path-tree     require a clean-hole-free certificate
auto          path-tree when eligible, otherwise compact 4D fallback
```

The CLI exposes the selector as `--representation`. For an eligible component,
the path-tree backend constructs both orientations on the prepared occupancy
and deterministically keeps the one with smaller biclique vertex-occurrence
size, breaking ties in favor of `vertical-tree-horizontal-paths`. The selected
orientation is recorded in `diagnostics.path_tree_orientation` and in the
certificate. Both orientations reuse the same grid reference dual and HLD
implementation; the dual construction remains area-sensitive rather than the
paper's planar sweep. Dinic is still the exact practical flow backend.
