# Algorithms and acceptance matrix

## Solver A: independent exact cover

`rect-oracle-exact-cover` enumerates every positive-area integer-grid rectangle
whose cells all belong to one component. A dynamic bitset represents each
option. The recursive search chooses an uncovered cell with the fewest currently
disjoint options, rejects overlap, and prunes with
`ceil(uncovered cells / largest available option area)`. The initial incumbent
is the valid all-singleton cover. It returns the selected rectangles, not only a
count.

This solver does not call boundary extraction, effective-chord generation,
matching, dominance, bicliques, or flow.

## Solver B: explicit Soltan--Gorpinevich

For ordinary polygons formed by unit grid cells, `rect-oracle-sg` performs:

1. cancel shared oriented cell edges and trace boundary loops;
2. identify local-nonconvexity vertices by right turns with formal interior on
   the left;
3. enumerate aligned reflex-vertex pairs satisfying Definition 7 in this grid
   model: every open unit subinterval has component cells on both sides;
4. build every closed horizontal--vertical intersection edge explicitly;
5. run Hopcroft--Karp and alternating-reachability minimum-cover recovery;
6. select the complement independent chord family;
7. add selected cuts, then horizontal and vertical simple chords as in Section
   10, Steps 3--4, and recover rectangular cell regions.

The implementation checks the formula
`r + 1 - holes - effective_chords + matching` and validates the matching,
vertex cover, independent family, completion count, and explicit rectangles.

## Solver C: paper algorithm

`rect-dominance` ranks all relevant coordinates and implements the paper's
parity encoding exactly:

```text
alpha(h) = (2 rank(l), -2 rank(r), 2 rank(y), -2 rank(y))
beta(v)  = (2 rank(x)+1, -2 rank(x)+1,
            2 rank(t)+1, -2 rank(b)+1)
```

Every horizontal--vertical pair is checked against closed geometric
intersection before graph algorithms run.

Stage C0 creates one biclique per explicit dominance edge. Stage C1 implements
the proof recursion of Cardinal--Yuditsky Theorem 8: split points by the current
coordinate, recurse on low-left/high-right after dropping that coordinate, and
recurse within each half without dropping it. The implementation verifies both
edge-set equality and multiplicity one, so the output is an edge partition.

Each biclique becomes one internal flow node. Outer arcs have capacity one and
internal arcs use `min(horizontal_count, vertical_count) + 1`. Dinic returns an
exact integral max flow and residual minimum cut. The implementation rejects a
cut that crosses an internal arc, recovers the vertex cover, and compares its
size and flow value to independent Hopcroft--Karp.

## Acceptance matrix

| Requirement | Evidence | Acceptance |
| --- | --- | --- |
| Grid components use four-connectivity | `rect-core::grid` unit test | corner-only contact is split |
| Boundary area and holes are exact | boundary invariant and ring test | signed doubled area equals twice the cell count |
| Exact-cover output is optimal and explicit | exhaustive differential tests | matches SG/C0/C1 through all `3x3` grids |
| Effective chord conflicts are explicit | SG certificate checks | every edge is a closed intersection |
| 4D embedding preserves endpoints | independent exhaustive segment test | every pair satisfies intersection iff strict dominance |
| C0 flow reduction is correct | graph unit test and differential suite | Dinic value equals Hopcroft--Karp |
| C1 is a partition, not only a cover | `verify_exact_partition` | every edge has multiplicity exactly one |
| Geometric output is valid | solver-independent validator | positive rectangles cover each component cell exactly once |

