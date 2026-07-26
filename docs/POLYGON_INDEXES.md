# Indexed polygon engine

The v1.0 production path keeps the v0.9 reference algorithms and adds one
prepared context per solve:

`normalize -> validate -> Boundary -> BoundaryIndex -> OrthogonalEdgeIndex -> reflex groups`

`PreparedPolygonContext` owns the normalized polygon, boundary metadata,
half-open/closed orthogonal edge indexes, sorted reflex groups by equal `x` and
`y`, base coordinates, and owned-allocation estimates. Production diagnostics
must report one build and one normalization/validation pass.

`OrthogonalEdgeIndex` stores horizontal and vertical edges in deterministic
coordinate groups and segment-tree stabbing indexes. Strict point location uses
the doubled-coordinate half-open vertical crossing rule `[bottom, top)`, so a
vertex is counted on exactly one incident vertical edge. Boundary contact and
ray shooting use closed intervals. Open segment reporting returns only reported
edge identities; Definition 7 still performs the endpoint and subinterval
conditions exactly.

The indexed polygon chord implementation is an aligned-reflex pair algorithm,
not the paper's general Soltan--Gorpinevich `O(n log n)` sweep. Its useful
intermediate bound is `O(n log n + C polylog n + Z)`, where `C` is the number of
equal-coordinate reflex pairs and `Z` is the number of reported boundary
events. The reference pairwise implementation remains available and reports
full-boundary scan counts.

Completion uses an incremental frontier and a dynamic cut index. Selected cuts
are materialized first, horizontal completion runs before vertical completion,
and only endpoint/intersection candidates are refreshed after insertion. The
indexed backend shares one `PreparedCoordinateArrangement` for occupancy,
barriers, rectangle recovery, and difference-array validation.

Owned-byte diagnostics are estimates of Rust-owned vectors, maps, indexes,
certificates, and arrangement arrays. They are not process peak RSS.

