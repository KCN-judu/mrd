# P09.4c - Hidden-Stability Query Boundary

`source_min_ratio::query::decode_candidate` accepts an already validated
`StableMinRatioLedger` and decodes a compact candidate through the P9.4a/b
source chain. Its public result contains only signed circulation arcs and the
ledger coordinate count; `StableWitness` is neither accepted nor returned.

The focused source-min-ratio suite has 8 passing tests. Workspace Clippy,
workspace tests, rustdoc, release build, biclique bound, no-fallback audit, and
release consistency all passed.

This is an exact public contract, not an approximate cycle search, hidden
witness discovery algorithm, dynamic data structure, link-cut implementation,
or Theorem 5.1/AN19 runtime claim.
