# Clean Hole-Free Path-Tree Representation

For a clean hole-free component, `rect-dominance::path_tree` provides two dual
backends. `ReferenceAreaFloodFill` inserts every vertical effective chord as a
unit cut, flood-fills occupied cells across uncut sides, and labels the two
incident regions of each chord. `BoundaryLaminar` is the CompactOnly
construction: it cuts the normalized outer boundary at a deterministic
endpoint-free gap, builds the interval-containment tree, and labels boundary
gaps without occupancy flood fill. The formal rule is in
`docs/BOUNDARY_DUAL_CONSTRUCTION.md`.

Every horizontal chord is mapped to a `CompactTreePath` containing only its two
endpoint regions. Endpoint-only HLD emits `O(log q)` heavy-chain intervals and
feeds the canonical segment decomposition directly; the path length is
computed from endpoint depths without enumerating tree edges. The legacy
`ChordTreePath` edge vector and per-path BFS remain an independent
FullyAudited oracle.

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

The CLI exposes the selector as `--representation` and `--region-dual
reference-area|boundary-laminar`. FullyAudited constructs both orientations
with the area reference and compares their explicit paths and partitions.
CompactOnly uses the boundary-laminar vertical orientation without transposing
the prepared occupancy; its trace records zero area visits, unit-cut records,
per-path BFS calls, and explicit path-edge materialization. Dinic is still the
exact practical flow backend.
