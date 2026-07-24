# v0.2.0 - Adversarial Validation and Compression Evidence

Release tag: `v0.2.0-adversarial-validation`

This release hardens the finite colored-grid verification artifact with:

- all 65,536 binary `4x4` grids and 337,058 monochromatic components;
- 10,000 deterministic random `8x8` grids, seed 42, and 162,162 components;
- all 87,146 canonical free polyominoes through 12 cells plus two explicit
  ordinary-hole fixtures;
- independent CP-SAT comparisons for all binary `3x3` grids, all free
  polyominoes through 10 cells, and bounded endpoint/topology/dense adversarial
  fixtures;
- exact biclique-partition audits for missing, fabricated, and duplicate edges;
- geometry-backed dense-conflict benchmarks through requested size 128;
- generated correctness, compression, and supported-scope tables;
- the corrected four-dimensional Cardinal--Yuditsky bound `O(q log^4 q)`.

The supported model is finite colored unit-cell grids with ordinary
nondegenerate holes. Ornaments, isolated formal-boundary points, line-segment
holes, point holes, arbitrary degenerate formal holes, and general polygon
input remain unsupported.

This artifact does not implement the classical `O(n log n)`
Soltan--Gorpinevich chord-enumeration sweep or the cited deterministic
almost-linear exact-flow backend. It uses exact aligned-reflex pair tests and
Dinic in the practical implementation, and must not be described as an
end-to-end implementation of every `n^(1+o(1))` black box.
