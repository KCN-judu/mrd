# Polygon backend differential evidence

Every indexed backend is compared with the preserved reference backend before
production defaults are changed. Equality is structural, not only an optimum
count:

- normalized polygon and reflex vertices;
- complete horizontal and vertical Definition 7 chord families;
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
the reason, and both solver outputs. The current release population contains no
disagreements, so no minimized regression bundle was added.

