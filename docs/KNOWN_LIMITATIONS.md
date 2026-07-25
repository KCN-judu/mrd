# Known limitations

- The implemented geometry adapter accepts finite unions of unit grid cells and
  ordinary nondegenerate holes. Soltan--Gorpinevich ornaments, line-segment
  holes, point holes, isolated formal-boundary points, and arbitrary degenerate
  formal holes are not represented and are not claimed as supported.
- General polygon input is not accepted. All supported geometry must be
  generated from finite grid cells and serialized in the colored-grid JSON
  format.
- CompactOnly enumerates effective chords with the exact grid-specialized
  `GridInteriorRunEnumerator`; the aligned-reflex pair implementation remains
  the differential reference. This does not implement the paper's general
  polygon `O(n log n)` sweep-line enumeration bound.
- The compact biclique implementation follows Cardinal--Yuditsky Theorem 8 but
  uses straightforward sorting in recursive calls. It prioritizes checkable
  construction over optimized constants. Because the embedding has four
  coordinates, the cited general bound specializes to `O(q log^4 q)`.
- `VerificationMode::CompactOnly` avoids explicit conflict edges, pairwise
  chord traversal, Hopcroft--Karp, C0 construction, and the full partition
  audit. Its correctness relies on the same constructive recursion that is
  exhaustively audited in `FullyAudited`; output geometry and minimum-cut
  certificate invariants remain checked in both modes.
- Portable process peak-memory measurement is not implemented. A null
  `peak_memory_bytes` diagnostic means unmeasured, not zero.

- Compact biclique validation is an `O(sigma)` coordinate-extrema proof. It
  intentionally does not expand every `A_k x B_k`; fully audited mode retains
  the explicit edge-multiset audit.

- The v0.3 grid-run path is specialized to finite unit-cell components. It is
  differentially checked against the pairwise reference, but it does not claim
  the paper's general polygon `O(n log n)` enumerator.
- CompactOnly now defaults to the indexed-frontier completion backend after
  exact differential equality on the recorded v0.4 populations. The
  reference-rescan backend remains selectable as an Oracle and FullyAudited
  continues to use it by default.
- The clean-hole-free path-tree representation is an additional specialized
  backend. FullyAudited retains the area-flood-fill dual and both orientations
  as independent oracles. CompactOnly uses the boundary-laminar dual and
  endpoint-only HLD without per-path edge vectors or prepared-grid transpose;
  this finite-grid interval construction does not implement the paper's
  general polygon planar sweep.
- Prepared occupancy, cut arrays, recovery state, and validation are dense in
  the component-local bounding box. Very sparse components with a large local
  box can therefore use `O(A)` memory even when their cell count is much less
  than `A`.
- Dinic is the only max-flow backend. The deterministic almost-linear theoretical
  flow algorithm cited by the paper is intentionally not implemented.
- The exact-cover oracle is exponential and intended for small components. The
  verification harness defaults to a 40-cell cutoff.
- JSON colors are compared as exact `serde_json::Value` values. SVG output is a
  debugging view and not a correctness oracle.
- Exhaustive `4x4` verification is available as an explicit release-mode command
  rather than a default unit test because its runtime is machine-dependent.
- Experimental agreement is finite evidence, not a proof that the practical
  implementation realizes the paper's full `n^(1+o(1))` algorithm.
