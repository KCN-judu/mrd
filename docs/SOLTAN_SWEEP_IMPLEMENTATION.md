# Soltan--Gorpinevich Sweep Implementation

## Scope and sources

This document specifies the v1.1 effective-chord enumerator for the repository's
accepted boundary-native polygon model: one simple outer orthogonal loop, zero
or more pairwise boundary-disjoint ordinary two-dimensional hole loops, and
integer `i64` coordinates. It does not support ornaments, isolated boundary
points, segment holes, point holes, boundary contacts, or disconnected outer
components.

The normative source statements are Soltan and Gorpinevich, *Minimum
Dissection of a Rectilinear Polygon with Arbitrary Holes into Rectangles*,
Discrete & Computational Geometry 9 (1993), pp. 57--79:

- pp. 57--61 define the formal boundary, elementary segments, and local
  nonconvexity;
- p. 62, Definition 7 defines effective chords and Observation 2 states that
  different effective chords have at most one common point and no proper part
  is effective;
- pp. 76--77, Section 10 Step 1 gives the horizontal procedure (a)--(d), says
  to construct the vertical family symmetrically, and states an `O(n log n)`
  sweep-line bound; and
- p. 76 states the source input representation: two linear arrays of
  elementary segments, one for the topological boundary and one for the
  ornament.

The supplied Version 4.1 paper uses those pages only to justify the explicit
output bound `q = O(n log n)`; it does not add an event-level description of
Step 1. In particular, the original paper does **not** prescribe the Rust event
ordering, status tree, or certificate below. Those are engineering choices that
realize the source procedure on the narrower ordinary-loop model and are kept
separate from source claims.

## Source procedure and ordinary-loop specialization

For horizontal chords, Section 10 Step 1 says to:

1. construct every chord `[x,z]` with endpoints in `V(P)` and open interval in
   `Int P`;
2. delete a chord whose endpoint has two vertical elementary boundary segments;
3. repeatedly merge adjacent chords through an eligible non-isolated boundary
   vertex; and
4. delete chords whose endpoints fail the isolated-or-collinear-horizontal
   requirement.

The vertical procedure is symmetric. The source allows formal boundary
features, so steps 2 and 3 are necessary there.

For the v1.1 accepted model, each boundary vertex has exactly one horizontal
and one vertical incident elementary segment. There are no isolated points,
overlapping segments, or contacts. If a boundary point lay in the open interval
of a horizontal Definition 7 chord, condition (4) would make it a vertex with
one vertical elementary segment; its unavoidable horizontal elementary segment
would contain a nonempty part of the chord's open interval, contradicting
condition (2). The vertical case is symmetric. Consequently:

- every supported effective chord is a proper open-interior segment;
- source steps 2 and 3 have no applicable ordinary-loop configuration;
- condition (3) reduces to both endpoints being reflex vertices, because every
  such endpoint already has the required collinear elementary segment; and
- a chord is effective exactly when the unique axis ray leaving a reflex vertex
  on the non-boundary side first reaches another reflex vertex.

This is the central specialization lemma. It is tested against both preserved
pairwise Definition 7 implementations on ordinary holes, endpoint/topological
fixtures, native polygons, grid-derived polygons, and metamorphic variants.
It must not be applied to a rejected formal-boundary input.

## Axis-generic event sweep

`SoltanGorpinevichSweepEnumerator` uses one `SweepAxis` implementation twice.
For `Horizontal`, the scan coordinate is `y`, status objects are vertical
boundary elementary segments keyed by `x`, and queries emit horizontal chords.
For `Vertical`, swap `x/y` and horizontal/vertical throughout. No copied
orientation-specific algorithm is permitted.

For every axis, each status segment contributes two events. At one scan
coordinate the deterministic order is:

1. insert segments whose lower scan endpoint is that coordinate;
2. query reflex vertices at that coordinate in increasing transverse coordinate
   and stable boundary-vertex-ID order;
3. remove segments whose upper scan endpoint is that coordinate.

The closed-at-event convention makes an elementary segment incident to a query
vertex visible to the status. The query excludes the source transverse
coordinate, so the source's incident orthogonal segment is not selected as its
own blocker. A `BTreeSet` status provides predecessor/successor ray shooting.
The source reflex vertex's incident same-axis boundary segment determines its
only strict-interior direction; this is local boundary incidence, not a
candidate Definition 7 test. The first status segment in that direction is the
candidate blocker. It emits exactly when the hit point is another reflex vertex
and the source point is canonical before the target point. The canonical owner
rule prevents duplicate records without a post-hoc candidate list.

The implementation uses checked `i128` conversion only where doubled
coordinates are needed by an audit. Primary construction uses native integer
coordinates, stable loop/vertex IDs, and canonical chord sorting. The sweep
never enumerates all reflex pairs, equal-coordinate pairs, or a quadratic
candidate list.

## Invariants and certificates

The following invariants are checked in focused tests and, where inexpensive,
in the production backend:

| Invariant | Rust surface | Test class |
| --- | --- | --- |
| Status contains exactly segments closed over the current scan coordinate | `SweepStatus` event phases | event-order and endpoint fixtures |
| A reflex query uses only the non-boundary axis ray | `interior_direction` | all four reflex orientations |
| The selected blocker is the nearest closed orthogonal segment | status predecessor/successor | holes and nested-coordinate fixtures |
| An output has two reflex endpoints and open strict interior | `SweepOutputRecord` audit | three-backend differential |
| Every output has one canonical owner and no duplicate ID | canonical output insertion | duplicate suppression unit test |
| No pairwise or full-boundary fallback ran | `SweepMetrics` | CompactOnly contract tests |

`SweepCertificate` stores aggregate per-axis counters in every production
result. A debug invocation can retain ordered event summaries and output
provenance, bounded by a fixed trace limit; the normal serialized solver result
contains only the bounded summary and aggregate counters. Fully audited tests
may independently call the existing Definition 7 predicate and the two
pairwise enumerators *after* construction.

## Complexity and unsupported source features

For an accepted ordinary polygon, there are `O(n)` elementary segments and
reflex vertices. Each segment is inserted and removed once per applicable axis,
and each reflex is queried once per axis. `BTreeSet` status operations cost
`O(log n)`, so construction costs `O(n log n)` plus `O(q)` canonical output
writing. The proof depends on the specialization lemma above; it does not
claim that this Rust status layout is the original paper's undocumented data
structure.

The bound and the implementation do not cover the source's ornament array,
isolated points, segment/point formal holes, or the repeated merge/delete cases
that those features require. Completion remains the existing indexed
coordinate-arrangement implementation, which separately does not claim the
full classical completion bound. Dinic remains the practical flow backend.
