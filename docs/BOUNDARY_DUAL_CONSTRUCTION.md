# Boundary-Laminar Region Dual

This document specifies the finite-grid construction used by the v0.8
CompactOnly path-tree backend. It applies only after the clean hole-free
certificate has established one normalized outer boundary loop, proper chord
endpoints, and distinct endpoints.

## Intervals and the root gap

`Boundary::from_component` stores the outer loop counter-clockwise with stable
`BoundaryVertexId` values. The builder chooses the lowest-index boundary gap
(the gap after a vertex) that is not incident to any fixed-orientation tree
chord endpoint. The vertex immediately after that gap becomes cyclic origin.
After rotation, the root gap is the wrap gap between positions `n-1` and `0`.

Each tree chord is represented by the closed endpoint interval `[a,b]` with
`a < b`; this is exactly the boundary arc that does not contain the root gap.
Proper noncrossing arcs in a disk have laminar intervals: two intervals are
disjoint or one contains the other. Sorting by `(a,-b,chord_id)` and checking
the active stack rejects a crossing pair before any tree is emitted.

The containment stack creates an outer region `0` and one inner region for
each tree chord. The edge labelled by chord `c` joins the containing region to
the new inner region. Consequently the dual has `|T|+1` regions, `|T|` edges,
is connected, and is acyclic.

## Boundary-gap labels and endpoint regions

For every boundary gap, the label is the deepest active interval containing
that gap. At a proper reflex endpoint of a horizontal path chord, exactly one
incident boundary edge is horizontal. Its interior sector is the sector
adjacent to the chord endpoint; the corresponding incoming or outgoing gap
label is therefore the incident dual region. This rule uses the normalized
orientation, the two incident boundary edges, and the chord's horizontal
direction. It does not infer a region from a cyclic index alone and does not
look up occupied cells.

The unique tree path between the two endpoint regions is the set of fixed
orientation chords whose arcs separate the two boundary sectors. By the disk
arc separation lemma, this is exactly the set of geometric crossing chords.
Production gap labeling uses `GapBackend::Experiment`: starts are
processed outer-to-inner, end events are popped before labeling the gap at the
end coordinate, and every push/pop must match the active stack. The preserved
`GapBackend::Oracle` backend performs the old gap-by-interval membership scan for
differential testing. Event diagnostics record zero membership tests and one
push/pop per interval; the reference records its exact `n * |T|` test count.

FullyAudited independently reconstructs the same path with area flood-fill and
per-path BFS, then compares the edge sets and the endpoint regions.

## Scope

The implementation is a grid-specialized realization. It does not claim the
paper's general polygon sweep, ornaments, point/segment holes, or degenerate
formal holes. `ReferenceAreaFloodFill` remains available as the independent
correctness oracle and is the default for FullyAudited. CompactOnly uses
`BoundaryLaminar` and endpoint-only HLD after the differential tests pass. The
event sweep is `O(n + |T| log |T|)` including interval sorting; no general
polygon sweep-line complexity is claimed.
