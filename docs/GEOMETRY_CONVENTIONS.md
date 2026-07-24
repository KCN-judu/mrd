# Geometry conventions

- Correctness-critical coordinates are signed `i64`; graph indices and grid
  ranks are `usize`; dominance coordinates are checked `i128` expressions of
  ranks.
- A grid cell `(x,y)` denotes the closed unit cell geometrically, while coverage
  and output rectangles use the half-open combinatorial convention
  `[x0,x1) x [y0,y1)`.
- Output rectangles have positive integer width and height. The validator checks
  every covered cell, rejects outside coverage and positive-area overlap, and
  requires exact component coverage.
- Effective chords are nonzero closed segments. Endpoint contact counts as a
  conflict. This is why the paper's even/odd rank encoding is used instead of a
  perturbation or floating-point epsilon.
- Cell boundary edges are oriented with component interior on the left. Shared
  opposite edges cancel. Consequently outer loops have positive signed area and
  hole loops have negative signed area.
- Collinear unit boundary edges are simplified to elementary contour segments
  for reflex classification. A right turn is a local-nonconvexity vertex for
  both outer and hole loops because formal interior remains on the left.
- For an ordinary cell-union polygon, the open interval of a candidate
  horizontal chord is interior exactly when every crossed unit interval has a
  component cell immediately above and below; vertical chords use left and
  right cells. This is the grid specialization of Soltan--Gorpinevich Definition
  7(2). There are no ornaments, so its exceptional finite boundary contacts and
  degenerate cases are outside the current supported model.
- SVG generation is diagnostic only. It never feeds coordinates or decisions
  back into a solver.

