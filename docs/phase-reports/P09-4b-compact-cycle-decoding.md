# P09.4b - Compact Cycle Decoding

`source_min_ratio::cycle` now decodes compact source-edge segments directly.
An immutable binding checks that each source edge and circulation arc have the
same declared endpoints. Selected tree paths are expanded by the current
`Chain` branch; the decoded signed arcs then pass
`CirculationNetwork::validate_signed_circulation`.

The production path imports no enumerating cycle Oracle. Regressions cover a
tree-path plus off-tree cycle, endpoint-mismatched bindings, missing bindings,
degenerate paths, and nonconserving output. The source-min-ratio focused suite
has 7 passing tests. Workspace format, Clippy, tests, rustdoc, release build,
the biclique check, no-fallback audit, and release-consistency check passed.

This establishes finite exact decoding only. It does not construct an
approximate cycle, hidden-stability query, dynamic update path, link-cut tree,
or Theorem 5.1/AN19 runtime bound.
