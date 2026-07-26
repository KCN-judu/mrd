# Ordinary Polygon Validation

The v0.9 validator remains a readable exact reference implementation. It audits
each loop with an explicit `O(n^2)` segment-pair pass; this is deliberately not
described as a sweep-line algorithm.

v1.0 adds `OrthogonalSweepValidator`, which is the production default. It uses
deterministic exact integer events and the prepared orthogonal edge index.
Accepted normalized polygons and broad negative error categories are compared
against `ReferenceQuadraticValidator`; the committed negative campaign also
requires deterministic first-failure agreement for its fixtures.

The structured error surface distinguishes non-axis-aligned and zero-length
edges, too few vertices, self intersections, non-adjacent touches, duplicate
vertices or edges, wrong orientation, area overflow, unsupported degenerate
boundaries, and invalid hole placement or nesting. A normalized outer loop has
positive signed doubled area, every hole has negative signed doubled area, and
the formal area is the outer area plus the signed hole areas.

For ordinary loops, a hole is valid only when it is strictly inside the outer
loop and its boundary is disjoint from every other loop. Pairwise disjoint
holes are automatically nonnested after the containment audit; a nested pair is
reported explicitly. These conditions imply a connected formal interior for
the supported single-outer model.

The validator is independent of grid dimensions. It uses exact segment
intersection tests and doubled-coordinate ray casting, so coordinates such as
`0` and `1_000_000_000` are handled without allocating intermediate cells.
