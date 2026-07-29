# Grid/Polygon Differential Verification

The existing unit-cell pipeline remains the independent geometry Oracle. A
supported grid component is converted from its normalized boundary loops into
`RectilinearPolygon`; components with boundary self-contact or another v0.9
unsupported degeneracy are rejected explicitly rather than reinterpreted.

For every accepted component the differential requires equality of:

- canonical boundary loop and reflex-vertex semantics;
- horizontal and vertical effective-chord families;
- minimum-cover chord selection;
- canonical selected horizontal/vertical cut unions;
- canonical added horizontal/vertical cut unions;
- optimum rectangle count and coordinate rectangles.

Both native validators run. Equal optimum values alone are insufficient.

The permanent default test covers 893 ordinary components from all 511
nonempty binary `3x3` masks. The explicit release-mode test covers 166,189
ordinary components from all 65,535 nonempty binary `4x4` masks. Both
populations have zero chord, cut, or rectangle disagreements. Unsupported
grid-derived formal degeneracies remain grid-only inputs and are counted
outside the ordinary-polygon population.

The independent bounded raster adapter in `verification::polygon` is available
only for small integer-coordinate differential tests. Width, height, and total
cell limits are mandatory. Production polygon solving records
`raster_oracle_used=false` and never calls the adapter.

v1.0 extends the equality contract to reference versus indexed polygon
geometry, structural validation, chord enumeration, completion, arrangement
recovery, and output validation. The CLI suites persist producing commit,
command, input population, supported/rejected counts, errors, timeouts,
disagreements, and counterexample records. See
`docs/POLYGON_BACKEND_DIFFERENTIAL.md`.
