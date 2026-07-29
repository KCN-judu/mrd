# Completion Coordinate Closure

## Scope

This lemma applies to the repository's supported input model: one ordinary
nondegenerate rectilinear outer loop, zero or more ordinary nondegenerate
two-dimensional holes, integer coordinates, and Definition 7 effective
chords.  It does not apply to formal boundary ornaments, point holes, segment
holes, or other degenerate formal-boundary inputs rejected by
`PreparedPolygonContext`.

## Theorem

Let `U` be the union of every boundary vertex coordinate and every selected
effective-chord endpoint coordinate, considered independently on both axes.
During the deterministic completion policy (selected cuts first; horizontal
phase before vertical; `(y, x)` and then East, North, West, South order), every
candidate point, simple-cut endpoint, and ray blocker has both coordinates in
`U`.

## Proof

Initially, candidates are boundary vertices.  Selected effective-chord
endpoints are normalized boundary vertices, so their endpoints are in `U`.
Assume inductively that all already inserted cuts have endpoints in `U`.

The completion frontier may add only a cut endpoint or an intersection of one
horizontal and one vertical cut.  Endpoints satisfy the induction hypothesis.
At a cut intersection the x coordinate is the vertical cut's x coordinate and
the y coordinate is the horizontal cut's y coordinate, so both are in `U`.

For a ray from a candidate, the first blocker is either a boundary segment or
an inserted cut.  A boundary blocker has its fixed boundary coordinate and
shares the ray coordinate with the source.  A perpendicular cut blocker has
its fixed coordinate and shares the source coordinate; a collinear cut blocker
is one of that cut's endpoints.  In all cases both coordinates are in `U`.
The next simple-cut endpoint is therefore in `U`, closing the induction.

The selected-chord term is retained explicitly although Definition 7 endpoints
are boundary vertices.  It makes the finite universe contract clear at the
completion API boundary and remains correct if selection provenance is supplied
directly by a future compatible frontend.

## Runtime Contract

`polygon_cut_index::experiment::Index` is initialized from this finite universe. In debug
and audited paths, every inserted cut, generated candidate, reported
intersection, and ray stop is checked against it.  A violation is a semantic
error, not a request to extend the universe: it invalidates this proof's
preconditions and must be minimized and preserved as a regression.

The reference line-map completion remains the differential oracle.  Tests
compare selected cuts, added-cut order, canonical cut unions, and rectangles
between the two indexes on every supported completion population.
