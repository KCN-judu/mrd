# Known limitations

- The implemented geometry adapter accepts finite unions of unit grid cells and
  ordinary nondegenerate holes. Soltan--Gorpinevich ornaments, line-segment
  holes, point holes, isolated formal-boundary points, and arbitrary degenerate
  formal holes are not represented and are not claimed as supported.
- General polygon input is not accepted. All supported geometry must be
  generated from finite grid cells and serialized in the colored-grid JSON
  format.
- Effective chords are enumerated by exact aligned-reflex pair testing. This is
  semantically faithful for the supported grid model but does not implement the
  paper's `O(n log n)` sweep-line enumeration bound.
- The compact biclique implementation follows Cardinal--Yuditsky Theorem 8 but
  uses straightforward sorting in recursive calls. It prioritizes checkable
  construction over optimized constants. Because the embedding has four
  coordinates, the cited general bound specializes to `O(q log^4 q)`.
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
