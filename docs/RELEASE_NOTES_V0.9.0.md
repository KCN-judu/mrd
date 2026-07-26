# v0.9.0 Boundary-Native Ordinary Rectilinear Polygons

This release adds an exact boundary-native frontend for one connected ordinary
rectilinear polygon with integer coordinates, one nondegenerate outer loop, and
zero or more nondegenerate ordinary holes.

The polygon pipeline normalizes and validates loops, constructs compact
boundary metadata, enumerates Soltan--Gorpinevich Definition 7 effective chords
with an exact pairwise reference algorithm, reuses the verified 4D and clean
path-tree compact matching backends, completes the dissection with coordinate
compression, and validates coordinate rectangles without rasterizing by
coordinate magnitude.

The release evidence records exact grid/polygon chord, selection, cut, optimum,
and rectangle equality on all supported `3x3` and `4x4` components plus free
polyomino, adversarial, complete-bipartite, deterministic random, ordinary-hole,
and native nonuniform-coordinate fixtures.

Ornaments, point or segment holes, arbitrary degenerate formal holes, boundary
self-contact, disconnected outer components, the general `O(n log n)` polygon
enumerator/completion algorithms, and the theoretical almost-linear flow
backend remain outside this release.
