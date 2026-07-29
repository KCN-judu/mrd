# P09.5a.3.1 - Terminal-Tree Projection And Declarations

## Status

**State: complete for one immutable source/IPM snapshot.** Implementation SHA:
`abb77ac`.

## Scope

`graph::source_min_ratio::terminal::Tree` accepts a checked exact
`input::Input`, a matching circulation network, and a source root. It
materializes the source graph and orientation-preserving arc bindings, invokes
the existing exact AN19-shaped static hierarchy, and retains its tree
certificate. The resulting tree is represented as one checked terminal
`source_min_ratio::chain::Chain` branch with deterministic initial shifts.

For every source edge outside the source tree, the module emits exactly one
`FundamentalTree` declaration: the unique selected tree path from its first
endpoint to its second endpoint followed by that edge in reverse. Candidate IDs
are the stable source-edge IDs. `candidate::Registry` can then revalidate and
score that supplied declaration using the same exact `Input` provenance.

This is direct source-derived fundamental-cycle construction. It scans the
fixed source edge set and never searches for simple, residual, or minimum-ratio
cycles.

## Evidence

Two focused tests cover:

- a triangle projection, where the source hierarchy returns a checked tree,
  the sole non-tree edge becomes one terminal declaration, the declaration
  decodes to an exact circulation, and the registry selects a nonzero ratio;
- a source graph that is already a tree, where the declaration population and
  registry result stay empty instead of invoking an enumerating fallback.

`Tree::verify` rechecks the retained hierarchy certificate and rebuilds the
full exact snapshot, including materialization, branch, shifts, and
declarations.

## Audit

Implementation SHA `abb77ac` passed:

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | terminal module has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | source-flow no-fallback boundary accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_min_ratio::terminal` | 0 | 2 focused tests passed |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | rustdoc accepted with warnings denied |
| `cargo build --workspace --release` | 0 | release build accepted |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Non-claims And Next Action

This module has one terminal tree only. It does not represent source
core/spanner embeddings, produce fundamental spanner declarations, apply
candidate replacement or retirement across snapshots, use the selected heap
choice to form a `Step`, expose the stability witness, or claim an amortized,
Theorem 5.1, AN19, or `AlmostLinear` runtime bound.

P9.5a.3.2 must attach core/spanner embedding provenance and source-driven
candidate updates. P9.5a.3.3 must then certify the selected compact direction
through `Step::from_compact_candidate` and run the no-fallback differential.
