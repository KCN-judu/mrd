# Polygon backend differential evidence

Every production polygon backend is compared with two preserved pairwise
Oracles before a default is changed. v1.1 compares
`ReferencePolygonPairwiseEnumerator`, `IndexedPolygonPairwiseEnumerator`, and
`SoltanGorpinevichSweepEnumerator`. Equality is structural, not only an optimum
count:

- normalized polygon and reflex vertices;
- complete horizontal and vertical Definition 7 chord families, endpoint
  identities, and deterministic IDs;
- endpoint metadata and clean eligibility certificate;
- selected representation, flow value, and minimum vertex-cover size;
- selected horizontal/vertical cuts;
- added horizontal/vertical simple cuts;
- canonical coordinate rectangles and optimum count;
- reference quadratic and indexed arrangement validation.

The committed v1.0 populations are:

| Population | Inputs | Components | Supported | Verified | Disagreements |
| --- | ---: | ---: | ---: | ---: | ---: |
| binary 3 x 3 grid-derived polygons | 511 | 897 | 893 | 893 | 0 |
| binary 4 x 4 grid-derived polygons | 65,535 | 168,529 | 166,189 | 166,189 | 0 |
| extended polyomino/hole/adversarial/random/native/metamorphic | 7,657 | 7,659 | 7,394 | 7,394 | 0 |
| native A-H fixtures | 40 | 40 | 40 | 40 | 0 |

The extended campaign includes all free polyominoes through ten cells, ordinary
hole fixtures, existing endpoint/topology/path-tree witnesses, complete-
bipartite and dense families, 1,000 deterministic random connected regions,
and affine/translation/reflection variants. Small bounded members also run the
raster Oracle. Full JSON reports, counterexample bundles, and producing commit
metadata are stored under `results/v1.0-*`.

On disagreement the report stores the original and currently minimized polygon,
the reason, and all three solver outputs. Sweep failures additionally retain
the bounded event certificate and output provenance; a disagreement blocks the
default. The v1.1 campaign writes separate reports under `results/v1.1-*` and
also checks that sweep pair-iteration, Definition 7 fallback, full-boundary
scan, and duplicate-output counters remain zero.

v1.2 extends each feasible completion comparison to four exact paths:
`CoordinateCompressedCompletion`; indexed completion with the reference
line-map and dense recovery; indexed completion with dynamic stabbing and dense
recovery; and dynamic stabbing with sparse face recovery and slab validation.
All paths must agree on selected cuts, added-cut order, canonical cut unions,
rectangles, area, and validator result category.  Dynamic production diagnostics
must report zero coordinate-line and interval scans; sparse CompactOnly traces
must report no dense arrangement allocations.
