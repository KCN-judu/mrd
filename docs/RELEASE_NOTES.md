# v0.2.0 - Adversarial Validation and Compression Evidence

Release tag: `v0.2.0-adversarial-validation`

This release hardens the finite colored-grid verification artifact with:

- all 65,536 binary `4x4` grids and 337,058 monochromatic components;
- 10,000 deterministic random `8x8` grids, seed 42, and 162,162 components;
- all 87,146 canonical free polyominoes through 12 cells plus two explicit
  ordinary-hole fixtures;
- independent CP-SAT comparisons on 6,998 inputs and 27,228 components: all
  binary `3x3` grids, all free polyominoes through 10 cells, and 13 bounded
  endpoint/topology/dense adversarial fixtures;
- exact biclique-partition audits for missing, fabricated, and duplicate edges;
- geometry-backed dense-conflict benchmarks through requested size 128;
- generated correctness, compression, and supported-scope tables;
- the corrected four-dimensional Cardinal--Yuditsky bound `O(q log^4 q)`.

The CP-SAT campaign completed all 27,228 selected component comparisons with
0 timeouts, 0 unsupported components, 0 solver errors, and 0 disagreements;
11 larger adversarial grids were explicitly excluded by the configured input
limit. Across the geometry-backed dense family from size 4 through 128,
explicit conflict edges grew from 32 to 16,896, biclique incidence size from
40 to 1,619, and compact-flow arc reduction from 30.00% to 93.79%. Every
tested compact decomposition passed the exact edge-multiplicity audit.

The supported model is finite colored unit-cell grids with ordinary
nondegenerate holes. Ornaments, isolated formal-boundary points, line-segment
holes, point holes, arbitrary degenerate formal holes, and general polygon
input remain unsupported.

This artifact does not implement the classical `O(n log n)`
Soltan--Gorpinevich chord-enumeration sweep or the cited deterministic
almost-linear exact-flow backend. It uses exact aligned-reflex pair tests and
Dinic in the practical implementation, and must not be described as an
end-to-end implementation of every `n^(1+o(1))` black box.
