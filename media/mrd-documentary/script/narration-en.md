# English Narration - V1 Provisional

Status: first animatic pass. Timing remains provisional until guide recording.
Bracketed pauses are editorial breathing space, not spoken text.

## 00:00-00:18 - Cold open

A shape can be divided in countless ways. [pause]

But somewhere inside it, one division is the smallest.

The question is not only how to draw the rectangles. It is how to know that no
smaller answer is hiding somewhere else.

## 00:18-00:48 - The 1993 clue

In 1993, Valeriu Soltan and Alexei Gorpinevich wrote down a structural answer.

Not a brute-force search through every possible dissection. A map from geometry
to combinatorics, built to include holes that could even collapse to points or
segments.

The present implementation begins from a narrower, ordinary polygon model. But
the trail still starts here, with the paper's decisive geometric objects.

## 00:48-01:18 - Effective chords

The boundary hides a finite set of effective chords.

They begin and end at local nonconvexity. Between those endpoints, the formal
interior and boundary conditions decide whether a chord truly exists.

On the ordinary polygons accepted today, an exact sweep finds these chords from
the first boundary hit of each interior reflex ray. Blocked candidates disappear.
The surviving cuts become the first reliable marks on the map.

## 01:18-01:46 - Geometry becomes conflict

Not every surviving chord can be chosen together.

A horizontal chord may cross a vertical one. Choosing both would make the family
inadmissible. So every horizontal chord belongs on one side, every vertical chord
on the other, and every crossing becomes an edge.

The geometry has become a bipartite conflict graph.

## 01:46-02:18 - Four ordered coordinates

The endpoints now enter four ordered coordinates.

For general polygon input, the implementation ranks each relevant coordinate.
Horizontal chords occupy even positions. Vertical chords occupy odd positions.

One comparison preserves the left endpoint. Another preserves the right. Two
more preserve the vertical span. [pause]

A closed crossing becomes strict dominance in all four coordinates. This is not
a picture of literal four-dimensional space. It is an exact ordered test.

## 02:18-02:44 - Compact blocks

The conflict graph may be dense. But its order is not arbitrary.

Repeated neighborhoods align into complete bipartite blocks. The constructive
partition used here follows the four-coordinate dominance relation, and the
audited path checks that every explicit conflict appears exactly once.

The edge cloud contracts. The same relation remains.

## 02:44-03:09 - Compressed flow

Each block becomes a chamber in a smaller flow network.

The source reaches horizontal chords. Horizontal chords reach biclique nodes.
Those nodes reach vertical chords, and vertical chords reach the sink.

The implementation uses integral capacities and Dinic. It does not materialize
every conflict edge on the compact path. Yet the network still remembers the
same matching value.

## 03:09-03:26 - Minimum cut

Then the final pulse stops. [pause]

The residual network leaves a minimum cut. Its outer arcs recover a minimum
vertex cover: the precise set of conflicting choices that must be removed.

What remains is a maximum admissible family of effective chords.

## 03:26-03:48 - A second map

For clean, hole-free polygons, another structure can appear.

Cut with one chord orientation and the resulting regions form a dual tree. The
opposite chords become paths through that tree. Heavy-light chains turn those
paths into canonical intervals, feeding the same compact matching machinery.

It is a specialized route, not a replacement for every polygon.

## 03:48-04:05 - Executable artifact

Over successive releases, the theorem became an executable artifact: exact
Oracles, endpoint-safe dominance, compact execution, indexed completion,
path-tree representation, boundary-native polygons, the source-mapped sweep,
and output-sensitive sparse geometry.

Slower paths stayed in the repository as independent references. Compact
structures kept certificates.

## 04:05-04:25 - Final dissection

Now return to the opening shape.

The selected effective chords enter first. Simple completion chords follow.
The sparse subdivision closes, and its interior faces settle into rectangles.

An independent validator checks containment, overlap, holes, union, area, and
the declared count. For this fixture, the answer is not merely plausible. It is
the exact optimum recorded by the solver.

## 04:25-04:40 - The unfinished passage

But the map is not complete.

Formal boundary ornaments can now be represented and checked, but their solving
pipeline remains unfinished. Dinic is practical and exact, but it is not the
deterministic almost-linear flow algorithm from the theoretical frontier.

The evidence is extensive. It is still finite evidence, not a proof of the full
end-to-end bound.

## 04:40-04:48 - End card

Compact matching for minimum rectangular dissection. [pause]

From a 1993 structural theorem to an exact, auditable implementation.
