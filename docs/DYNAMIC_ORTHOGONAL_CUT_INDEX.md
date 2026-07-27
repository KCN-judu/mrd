# Dynamic Orthogonal Cut Index

## Purpose

`DynamicPolygonCutIndex` remains the line-map reference implementation.
`DynamicStabbingCutIndex` is the CompactOnly production index for the
boundary-native ordinary-polygon completion policy.

## Finite universe

The index is initialized from the static coordinate closure proved in
[POLYGON_COMPLETION_COORDINATE_CLOSURE.md](POLYGON_COMPLETION_COORDINATE_CLOSURE.md).
Coordinates are exact signed integers.  Inserts, frontier candidates,
intersections, and ray stops are rejected if they leave that universe.

## Data structures and bounds

For every axis, a per-coordinate `BTreeSet` stores the canonical union of
non-overlapping collinear intervals.  Membership and nearest collinear endpoint
queries use one predecessor or successor operation, not an interval scan.

Perpendicular segments are decomposed into canonical nodes of an insert-only
segment tree over the opposite-axis coordinate universe.  Each node stores an
ordered `(fixed_coordinate, segment_id)` set.  A point-stabbing query visits a
root-to-leaf path; nearest predecessor/successor queries take the best result
over that path.  Orthogonal intersection reporting performs key-range reports
on the same path and deduplicates actual intersection points.

For `M` universe coordinates and `k` reported intersections, the documented
targets are `O(log^2 M)` insertion and nearest blocker lookup, and
`O(log^2 M + k)` reporting.  The implementation is insert-only because
completion never removes a cut.  Diagnostics expose canonical-node insertions,
tree visits, ordered-set queries, reports, and owned-byte estimates.  Production
dynamic runs require both `cut_index_coordinate_line_scans == 0` and
`cut_index_interval_scans == 0`.

## Semantic preservation

Only cut membership, cut ray shooting, and orthogonal intersection reporting
changed.  Selected cuts are inserted first; horizontal completion precedes
vertical completion; candidates retain `(y, x, East, North, West, South)`
ordering; invalid frontier entries are rechecked lazily; and boundary/cut stops
are compared exactly.  Differential verification requires equality of selected
cuts, added-cut order, canonical final unions, and rectangles with the line-map
backend.
