# Output-Sensitive Subdivision Sweep

## Contract

The input is the normalized set of horizontal and vertical boundary/cut
segments for an ordinary nondegenerate integer-coordinate rectilinear polygon.
Each source segment owns a split-coordinate set initialized with its two
endpoints. Collinear normalization deduplicates identical segments with the
same provenance and rejects positive-length conflicting overlaps. Endpoint
contacts remain distinct events because they are required by the half-edge
subdivision.

## Closed-endpoint event order

For increasing x, events use the total order

```text
HorizontalStart < VerticalQuery < HorizontalEnd
```

A horizontal segment `[left,right]` is therefore active at both `left` and
`right`. A vertical query `[bottom,top]` range-reports active records whose y
lies in the closed interval. Consequently proper crossings, T-junctions, and
shared endpoints are reported exactly once, by their unique vertical event.
Removing a horizontal before the query would lose right-endpoint contacts;
inserting it after the query would lose left-endpoint contacts.

Every reported point is added directly to both source split lists. Sorting and
deduplicating each list and joining adjacent coordinates creates all and only
positive-length atomic segments. The implementation never reconstructs global
point maps and rescans whole coordinate lines.

With `S` input segments and `J` reported orthogonal intersections, the ordered
event/status implementation takes `O((S + J) log S)` time and `O(S + J)`
algorithmic payload. The exact diagnostic contract for `orthogonal-sweep` is
`subdivision_candidate_pair_tests == 0`.

`reference-range-scan` remains selectable as the v1.2 Oracle. Differential
verification compares junction sets, atomic segments, vertices, half-edges,
faces, canonical rectangles, and validation results, not only rectangle counts.
