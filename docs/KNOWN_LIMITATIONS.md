# Known limitations

- The geometry adapters solve finite unit-cell grids and one boundary-native
  ordinary integer-coordinate rectilinear polygon with ordinary nondegenerate
  two-dimensional holes. Soltan--Gorpinevich ornaments, line-segment holes,
  point holes, and isolated formal-boundary points now have a canonical,
  source-mapped representation and incidence validator, but are not yet
  accepted by chord enumeration, completion, or dissection solvers. Boundary
  self-contact and multiple disconnected outer components remain rejected.
- General polygon production uses the source-mapped `sg-sweep` event/status
  construction and incremental indexed completion. The sweep's `O(n log n + q)`
  claim is limited to the accepted ordinary-loop model; it does not implement
  the source's formal-boundary ornaments, isolated points, or degenerate-hole
  merge/delete cases. `chord::oracle::Pairwise` and
  `chord::oracle::Indexed` remain exact reference backends.
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
  v1.3 reports retained payload, excess collection capacity, container
  estimates, and peak temporary payload separately; allocator/node overhead
  remains explicitly unmeasured, so these fields are not process RSS.

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
  backend. FullyAudited retains the area-flood-fill dual, physical transpose,
  and both orientations as independent oracles. CompactOnly uses the
  boundary-laminar dual and endpoint-only HLD without per-path edge vectors or
  production transposition; its orientation policy remains exact `build-both`
  after the v0.8 regret audit. This finite-grid
  interval construction does not implement the paper's general polygon planar
  sweep.
- The v0.7 audit found zero positive sigma regret for `BoundEstimate` on its
  historical finite population, but the v0.8 stored mixed-branching witness
  audit found five positive-regret rows. `BoundEstimate` is therefore not the
  CompactOnly default; it remains a diagnostic benchmark policy.
- Prepared occupancy, cut arrays, recovery state, and validation are dense in
  the component-local bounding box. Very sparse components with a large local
  box can therefore use `O(A)` memory even when their cell count is much less
  than `A`.
- CompactOnly polygon completion now uses a static-coordinate dynamic stabbing
  index with only populated tree nodes materialized, output-sensitive sparse
  half-edge recovery, and event-tree slab validation, so it does not
  materialize an `O(|X||Y|)` arrangement. The dense coordinate arrangement is
  intentionally retained for FullyAudited and differential oracle paths.
  Sparse construction is specialized to ordinary nondegenerate rectilinear
  loops; it does not extend support to ornaments, point/segment holes, or other
  formal degeneracies.
- The optional polygon recovery `auto` policy is evidence-backed only for the
  committed boundary-native scaling population and remains opt-in. No universal
  dense/sparse crossover is claimed. The complete size-256 reference campaign
  exceeded the practical release time budget.
- Dinic and highest-label push-relabel are practical exact max-flow backends.
  The deterministic almost-linear theoretical flow algorithm cited by the paper
  is intentionally not implemented.
- The AN19 hierarchy's workspace scans, counters, invariants, and finite
  differential/regression tests are implemented and audited. The formal SIAM
  journal source (DOI `10.1137/17M1115575`) was checked but does not establish
  the required ordering/counting conversion for exact reduced costs
  `ell(u,v) + d(x,u) - d(x,v)`. This missing lemma blocks P9.3.2d and the AN19
  asymptotic runtime chain; empirical counts do not close the proof. An isolated
  exact all-radii engine and definition-level Oracle now agree on the bounded
  A--H campaign and emit complete charge traces. The fixed-snapshot semantic
  event and queue-item cardinalities are structurally certified as `O(n+m)`,
  and the reduced engine's stable exact binary heap has a separately certified
  `O((n+m) log(n+m))` counted-comparison bound. The source-equivalent
  `O(m+n log log n)` priority-queue bound, global amortization, and production
  AN19 runtime remain unproved.
- The exact-cover oracle is exponential and intended for small components. The
  verification harness defaults to a 40-cell cutoff.
- JSON colors are compared as exact `serde_json::Value` values. SVG output is a
  debugging view and not a correctness oracle.
- Exhaustive `4x4` verification is available as an explicit release-mode command
  rather than a default unit test because its runtime is machine-dependent.
- Experimental agreement is finite evidence, not a proof that the practical
  implementation realizes the paper's full `n^(1+o(1))` algorithm.
