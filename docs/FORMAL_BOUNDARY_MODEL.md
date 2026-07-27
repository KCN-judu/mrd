# Formal Boundary Representation

## Normative source and scope

The representation follows Soltan and Gorpinevich, *Minimum Dissection of a
Rectilinear Polygon with Arbitrary Holes into Rectangles*, Discrete &
Computational Geometry 9 (1993), pp. 57--79, DOI `10.1007/BF02189307`.
The model in this phase maps the source's Section 2 definitions exactly over
the repository's already validated ordinary topological region:

- p. 58 defines `Or P` as a finite family of closed horizontal or vertical
  segments and isolated points inside the topological polygon;
- an isolated point belongs to `int P` and to no ornament segment;
- each ornament segment's open interior belongs to `int P`;
- every common point of two ornament segments is a vertex of both segments;
- Definition 1, p. 59 sets `Bd P = Or P union bd P` and
  `Int P = P without Bd P`;
- Definition 3, p. 59 makes contour vertices, ornament segment endpoints, and
  isolated ornament points vertices of the formal polygon; and
- Definition 4, p. 60 defines an elementary segment as a nonzero closed segment
  in `Bd P` containing exactly its two endpoints from `V(P)`.

The original source allows topological contours to share vertices. P2 retains
`RectilinearPolygon` as the topological-region Oracle, so those contours remain
simple and pairwise boundary-disjoint. This does not restrict the P2 features:
ornament endpoints may attach to an outer or hole contour, isolated ornaments
form point holes, connected segment ornaments form one-dimensional formal
holes, and ornament paths may connect a topological hole to the formal exterior.
General contour-contact solving remains outside P2 and must not be inferred.

## Rust model

`rect_core::FormalRectilinearPolygon` owns:

- a normalized `RectilinearPolygon` topological region; and
- a canonical `Ornament` containing sorted isolated points and sorted
  `OrnamentSegment` values with lexicographically ordered endpoints.

The constructor validates before returning. Fields are private, and custom
deserialization calls the constructor, so a deserialized value cannot bypass
normalization. `normalized()` is idempotent. Serialization of a constructed
value is deterministic, and serialization/deserialization is an exact
round-trip.

`RectilinearDomain` is implemented for the formal model. Point and open-segment
predicates first apply the ordinary-region Oracle and then remove every point
or segment contact with `Or P`. The exact doubled area equals the ordinary
region's area because a finite union of points and line segments has zero area.

## Structured validation

`FormalPolygonError` distinguishes:

- malformed ordinary topological regions;
- zero-length and non-axis-aligned ornament segments;
- duplicate points or canonical duplicate segments;
- isolated points outside the strict topological interior or on a segment;
- segment endpoints outside the closed topological region;
- segment interiors not contained in the strict topological interior;
- positive-length segment overlaps; and
- segment intersections that are not endpoints of both segments.

Endpoints may lie on `bd P`, as required by source examples such as an ornament
segment joining a hole contour to the exterior contour. An endpoint on the
interior of a topological edge becomes a formal vertex and splits that edge
into elementary segments.

## Incidence and formal holes

`FormalRectilinearPolygon::incidence()` derives, rather than stores:

1. every stable formal vertex in lexicographic point order;
2. every maximal elementary segment between consecutive formal vertices;
3. exact source provenance for each elementary segment;
4. endpoint incidence lists; and
5. connected components of `Bd P`.

The component containing topological loop 0 is the formal exterior. Every
other component is a formal hole and is classified by dimension:

- `point` for an isolated formal-boundary point;
- `segment` for a connected ornament segment network; or
- `topological` for a component containing an ordinary hole contour.

If an ornament joins a hole contour to the outer contour, both belong to the
formal exterior component and that ordinary topological hole is therefore not
a formal hole, matching the source's example on p. 59.

## Tagged JSON

The CLI accepts this validation-only input in `auto` mode or with
`--input-format formal-polygon`:

```json
{
  "type": "formal-rectilinear-polygon",
  "outer": [[0, 0], [20, 0], [20, 20], [0, 20]],
  "holes": [],
  "ornament": {
    "isolated_points": [[6, 6]],
    "segments": [[[8, 10], [12, 10]]]
  }
}
```

`rect-cli verify` returns the canonical model and complete incidence. P2
deliberately rejects `solve` for this input. Effective-chord enumeration,
merge/delete cases, completion, sparse subdivision, and rectangle validation
for formal holes belong to P3; silently dropping the ornament would violate
the source model.

The permanent fixture is `test-data/polygons/formal-boundary.json`.
