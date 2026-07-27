# Fact Check Register

All narration and visible factual text must map to an entry here before final
render. Wording in the permitted column is intentionally narrower than source
claims where the implementation has a smaller domain.

| ID | Statement | Source | Class | Permitted on-screen wording | Prohibited overclaim | Status |
| --- | --- | --- | --- | --- | --- | --- |
| F001 | Soltan and Gorpinevich published the source paper in 1993. | Local scan, title page; `docs/REFERENCES.md` | publication fact | `Soltan - Gorpinevich, 1993` | implying this repository or its later compression appeared in 1993 | verified |
| F002 | The source paper is titled *Minimum Dissection of a Rectilinear Polygon with Arbitrary Holes into Rectangles*. | Local scan, p. 57 | publication fact | exact title | shortened wording that changes the problem domain | verified |
| F003 | The source proves an `O(n^(3/2) log n)` algorithm for arbitrary, possibly degenerate holes. | Local scan, abstract and introduction | theorem | `The 1993 source proves an O(n^(3/2) log n) algorithm in its formal model.` | claiming the current Rust implementation realizes this full scope or bound | verified |
| F004 | Definition 7 identifies effective chords using local nonconvexity, interior, endpoint, and boundary-contact conditions. | Local scan, p. 62; `docs/SOLTAN_EFFECTIVE_CHORDS.md` | theorem definition | `Effective chords begin and end at local nonconvexity and satisfy the formal interior conditions.` | reducing the general definition to ordinary reflex-to-reflex visibility without stating the restricted model | verified |
| F005 | The accepted current ordinary-polygon model has one outer loop and ordinary nondegenerate holes. | `README.md`; `docs/KNOWN_LIMITATIONS.md` | implementation fact | `The current solver accepts ordinary rectilinear polygons with ordinary holes.` | arbitrary formal boundary ornaments or disconnected outer components | verified |
| F006 | Formal ornaments, point holes, segment holes, and isolated formal-boundary points have a representation and validator but are not accepted by solving stages. | `README.md`; `docs/KNOWN_LIMITATIONS.md`; `docs/FORMAL_BOUNDARY_MODEL.md` | implementation fact | `Formal boundary data can be represented and checked; its solving pipeline remains unfinished.` | `The solver handles point and segment holes.` | verified |
| F007 | The ordinary-polygon effective-chord sweep is source-mapped and uses `O(n log n + q)` construction on the accepted model. | `docs/SOLTAN_SWEEP_IMPLEMENTATION.md`; `docs/ALGORITHMS.md` | implementation fact | `For accepted ordinary loops, the sweep constructs chords in O(n log n + q).` | applying this implementation claim to formal ornaments and degeneracies | verified |
| F008 | Horizontal and vertical effective chords conflict when their closed segments intersect. | `docs/ALGORITHMS.md`; `docs/SOLTAN_EFFECTIVE_CHORDS.md` | implementation and mathematical fact | `A horizontal choice can conflict with a crossing vertical choice.` | calling every visual graph edge a conflict without exported crossing evidence | verified |
| F009 | The implemented general-input embedding uses ranked coordinates with endpoint-safe parity. | `docs/ALGORITHMS.md`; `rect-dominance` embedding documentation | implementation fact | `Ranks are doubled; opposite sides occupy even and odd positions.` | implying raw direct coordinates are the current general-input implementation | verified |
| F010 | The exact displayed embedding is `alpha(h)=(2 rank(l),-2 rank(r),2 rank(y),-2 rank(y))` and `beta(v)=(2 rank(x)+1,-2 rank(x)+1,2 rank(t)+1,-2 rank(b)+1)`. | `docs/ALGORITHMS.md` | implementation fact | display exact formulas with `rank` | omitting `rank` when discussing current general polygon input | verified |
| F011 | Closed chord intersection is equivalent to strict coordinate-wise dominance under the parity embedding. | `docs/ALGORITHMS.md`; embedding tests | theorem and implementation fact | `A crossing becomes strict dominance in four ordered coordinates.` | claiming to literally show four-dimensional Euclidean space | verified |
| F012 | Theorem 8 gives a biclique representation; this implementation audits a partition. | `docs/ALGORITHMS.md`; `docs/REFERENCES.md` | theorem and implementation fact | `The dense conflict relation is partitioned into complete bipartite blocks.` | invented compression ratios or universal practical speedup | verified |
| F013 | The four-dimensional representation size bound is `O(q log^4 q)`. | `README.md`; `docs/ALGORITHMS.md` | theorem specialization | `The four-coordinate representation has O(q log^4 q) size.` | calling it linear | verified |
| F014 | The compact flow network uses source to horizontal chords, biclique nodes, vertical chords, and sink. | `docs/ALGORITHMS.md` | implementation fact | `The blocks become a smaller exact flow network.` | showing arcs or capacities not present in exported data | verified |
| F015 | The current implementation uses Dinic and recovers an integral residual minimum cut and vertex cover. | `docs/ALGORITHMS.md`; `docs/KNOWN_LIMITATIONS.md` | implementation fact | `Dinic returns the flow; the residual cut recovers a minimum vertex cover.` | deterministic almost-linear max flow | verified |
| F016 | The clean hole-free path-tree backend turns one chord family into a dual tree and the other into paths. | `docs/PATH_TREE_REPRESENTATION.md`; `docs/BOUNDARY_DUAL_CONSTRUCTION.md` | implementation fact | `On eligible clean hole-free inputs, one family becomes a tree and the other becomes paths.` | presenting it as the universal polygon backend | verified |
| F017 | v1.3 uses output-sensitive orthogonal intersection sweep, sparse subdivision, and event-driven sparse validation. | `README.md`; `docs/OUTPUT_SENSITIVE_SUBDIVISION_SWEEP.md`; release tag | implementation fact | `v1.3 adds output-sensitive sparse geometry paths.` | claiming every backend and input is almost-linear | verified |
| F018 | The v1.3 release evidence reports zero disagreements on its recorded finite campaigns. | `results/paper-tables.md`; `results/release-index.json` | experiment | `Recorded v1.3 campaigns report zero disagreements.` | proof for all possible inputs | verified |
| F019 | The implementation keeps slower reference paths and independent Oracles for differential checking. | `README.md`; `docs/ALGORITHMS.md`; `docs/EXPERIMENTS.md` | implementation fact | `Slower paths remain as Oracles and differential references.` | describing all checks as formally verified proofs | verified |
| F020 | `comb.json` is a committed ordinary, hole-free rectilinear polygon fixture. | `test-data/polygons/comb.json` | implementation fact | source-data ID `polygon/comb@093961f` | calling it the final selected hero dissection fixture | verified |
| F021 | Remotion 4.0.499 and matching `@remotion/three` are current on 2026-07-27. | npm registry; official Remotion docs checked 2026-07-27 | toolchain fact | `Remotion 4.0.499` in production metadata | implying future compatibility without verification | verified |

## Required source review

- Local source reviewed: `tmp/pdfs/soltan-gorpinevich-1993.pdf`, 23 pages.
- Visual inspection reviewed the title page and p. 62 effective-chord definition.
- Text extraction reviewed the abstract, Definitions 1-9, theorem context, and algorithm outline.
- Repository documentation refers to a supplied Version 4.1 current paper, but no corresponding local file was located. Do not quote or attribute claims uniquely to that paper until it is supplied again.

## Claims held for later data export

The following may not enter narration or graphics until exact production JSON
exists: fixture-specific chord counts, conflict-edge counts, biclique counts,
flow values, cover membership, rectangle optimum, compression ratios, path-tree
interval counts, and per-scene benchmark timings.
