# Boundary-Native Polygon Completion

`CoordinateCompressedCompletion` is the v0.9 exact reference backend for an
ordinary polygon. It does not allocate cells according to coordinate
magnitude.

Selected effective chords are inserted first. The simple-chord phases then run
in the Soltan--Gorpinevich order: horizontal followed by vertical. Candidate
points are normalized boundary vertices, cut endpoints, and orthogonal cut
intersections. Candidates are ordered by `(y,x)` and direction order
`East,North,West,South`, filtered by phase. Four exact half-integer probes
classify the local quadrants. Existing cuts and original boundary rays split
the local angle components exactly as in the grid reference policy.

A ray stops at the first original boundary segment or existing perpendicular
or collinear cut. The emitted certificate stores canonical unions of selected
and added horizontal and vertical segments; simple-chord counts remain separate
metrics.

Rectangle recovery collects all x and y coordinates from boundary vertices
and cuts. Atomic open rectangles are the Cartesian products of adjacent unique
coordinates. Exact doubled-coordinate midpoints classify atomic rectangles;
cuts are adjacency barriers; flood fill recovers regions; every region must be
a full coordinate rectangle.

If `|X|` and `|Y|` are the coordinate counts, recovery uses
`O(|X||Y|)` atomic storage and time plus cut lookup costs. This is arrangement
sensitive, not coordinate-magnitude sensitive, and is not claimed to implement
the paper's general `O(n log n)` completion algorithm.

`IndexedPolygonCompletion` is the v1.0 production backend. It preserves the
same selected-cut, horizontal-then-vertical, `(y,x)`, and direction ordering.
Its frontier is incremental: inserted cuts add endpoints and newly reported
opposite-orientation intersections, stale heap entries are revalidated lazily,
and boundary/cut ray stops use static and dynamic indexes. Production counters
require zero global candidate rebuilds, zero full boundary scans, and zero full
cut scans.

The final cut family builds one `PreparedCoordinateArrangement`. Scanline
parity fills occupied spans, dense barrier arrays provide constant-time recovery
adjacency, and a two-dimensional difference array validates output coverage.
The reference and indexed backends must return identical selected cuts, added
cuts, and canonical rectangles.
