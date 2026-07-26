# Path-Tree Orientation Policies

The clean path-tree representation has two symmetric choices:

- `vertical-tree-horizontal-paths`: vertical effective chords are dual-tree
  edges and horizontal chords are tree paths;
- `horizontal-tree-vertical-paths`: the roles are exchanged.

`BuildBothExact` constructs both and selects the smaller actual biclique sigma,
breaking ties in favor of the vertical tree. This remains the FullyAudited
Oracle. CompactOnly defaults to exact `BuildBothExact` on the production path.
`BoundEstimate` remains an explicit heuristic benchmark policy and computes

```text
L = ceil(log2(q + 1))
vertical = |H| L^2 + |V| L
horizontal = |V| L^2 + |H| L
```

before constructing a partition and selects the smaller estimate, again with a
vertical tie break. `VerticalTree` and `HorizontalTree` are deterministic
debug/benchmark controls.

The CLI spelling is:

```text
--path-tree-orientation build-both|bound-estimate|vertical-tree|horizontal-tree
```

The policy and selected orientation are serialized in diagnostics. A bound
estimate is a dispatch heuristic, not a proof that its chosen sigma is optimal;
the v0.7/v0.8 campaigns compare it against `BuildBothExact` and record absolute
and relative regret. The expanded v0.8 population includes positive-regret
counterexamples in stored mixed-branching witnesses, so the heuristic is not
the production default. This is finite selector evidence, not a general
theorem.

For `BoundaryLaminar`, CompactOnly uses an axis-generic boundary view for the
horizontal-tree orientation. FullyAudited retains the historical physical
transpose as an independent Oracle until the axis-view equality campaign is
complete.

The v0.7 audit reports all recorded rows, not only optimum counts. Across
160,443 historical clean components, `BoundEstimate` selected the exact minimum
sigma on every row; the v0.8 witness audit deliberately adds counterexamples
that invalidate extrapolating that result.
The full per-instance CSV is `results/v0.7-path-tree-orientation-audit.csv`,
with a compact summary in the adjacent JSON and Markdown files. This is finite
evidence for the selector, not a proof of a general polygon bound.
