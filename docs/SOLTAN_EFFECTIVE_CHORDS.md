# Soltan--Gorpinevich Definition 7 Mapping

The source is Soltan and Gorpinevich, *Minimum Dissection of a Rectilinear
Polygon with Arbitrary Holes into Rectangles*, Discrete & Computational
Geometry 9 (1993), pp. 57--79, Definition 7 on p. 62. The implementation uses
the following exact conditions for the ordinary nondegenerate polygon subset.

| Paper condition | Code predicate | Tests |
| --- | --- | --- |
| (1) `[v,w]` is horizontal or vertical | endpoint coordinates share exactly one coordinate and `v != w` | `polygon_chords_accept_only_axis_aligned_pairs` |
| (2) the open interval is in `Int P` except for finitely many points | split at every boundary intersection; every open subinterval midpoint is strict interior | `polygon_chords_reject_hole_interiors` |
| (3) endpoints are local-nonconvex vertices and each is isolated in `Bd P` or a vertex of an elementary boundary segment collinear with the chord | reflex endpoint classification plus incident-edge orientation check | `polygon_chords_require_reflex_collinear_endpoints` |
| (4) every boundary point in the open interval is a vertex of a unique orthogonal elementary segment | every interior boundary intersection is a normalized vertex with exactly one incident edge orthogonal to the chord; collinear overlaps and edge-interior crossings reject | `polygon_chords_reject_nonvertex_boundary_crossings` |

The phrase “visible aligned reflex vertices” is therefore only a candidate
filter. It does not establish condition (2), the endpoint provenance in (3),
or the unique orthogonal elementary-segment requirement in (4).

The grid pairwise implementation remains the independent unit-cell Oracle. The
general polygon implementation is an exact reference enumerator, not the
paper's `O(n log n)` sweep. Any grid/polygon chord-set disagreement is a
correctness failure and keeps the grid path as the production default for grid
input until minimized and explained.

The paper's completion order is also normative here: selected effective chords
are inserted first, then horizontal simple chords, then vertical simple chords.
