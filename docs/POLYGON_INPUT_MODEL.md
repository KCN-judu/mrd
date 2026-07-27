# Boundary-Native Polygon Input Models

The repository has two explicit layers. `RectilinearPolygon` is the ordinary
solvable model below. `FormalRectilinearPolygon` adds source-backed ornaments,
isolated points, degenerate point/segment formal holes, canonical incidence,
and structural validation. See `docs/FORMAL_BOUNDARY_MODEL.md`. Formal input is
validation-only until its P3 chord/completion pipeline is implemented.

Version 1.0 accepts the same v0.9 domain: one connected ordinary rectilinear polygon described by
one outer loop and zero or more ordinary two-dimensional hole loops. A loop is
implicitly closed from its last vertex to its first vertex, and coordinates are
signed `i64` values. The polygon object contains only boundary coordinates; it
does not contain a rasterized cell set.

`RectilinearPolygon::new` performs deterministic normalization before the
strict audit. Repeated closing vertices are removed, consecutive collinear
vertices are merged, the outer loop is oriented counter-clockwise, holes are
oriented clockwise, and every loop is rotated to its lexicographically smallest
vertex. Hole order is then sorted by the normalized vertex sequence.

The accepted model has exactly one nondegenerate outer loop, simple pairwise
boundary-disjoint loops, holes strictly inside the outer loop, mutually
exterior holes, a connected formal interior, and no boundary self-contact.
Every normalized loop has at least four vertices, no zero-length edges, and
only horizontal or vertical edges.

The ordinary model intentionally rejects ornaments, isolated formal-boundary
points, point or segment holes, multiple disconnected outer components, nested
holes, self intersections or touches, overlapping boundary edges, and any
outer/hole or hole/hole point contact. The formal wrapper supports the first
three classes without weakening ordinary-loop validation.

All topology and area checks use integer arithmetic. Signed doubled area is
accumulated exactly in `i128`; doubled-coordinate midpoint and side probes are
used for strict interior predicates. No coordinate-magnitude rasterization is
part of the production polygon model.

Production builds one `PreparedPolygonContext` after normalization. Structural
validation, `Boundary`, `BoundaryIndex`, `OrthogonalEdgeIndex`, reflex groups,
and base coordinate vectors are constructed once and borrowed by downstream
stages. Standalone reference APIs retain their convenience behavior by building
a temporary context internally.
