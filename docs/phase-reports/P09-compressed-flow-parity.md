# P09 Compressed Flow Parity

## Scope

`audit_biclique_flow_parity` proves the present compressed-flow boundary is
consistent across independent exact references. It first verifies that a
biclique partition represents the explicit graph exactly, then compares the
explicit Hopcroft--Karp matching cardinality with compressed Dinic and
compressed Push--Relabel flow/cut recovery.

## Evidence

The differential test enumerates all 16 two-by-two explicit bipartite graphs.
For each graph it builds the singleton-edge partition and requires all three
exact paths to agree on matching and recovered-cover cardinality. An invalid
partition is rejected before a compressed network is built.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all` | 0 | formatted source |
| `cargo check -p rect-dominance` | 0 | passed |
| `cargo clippy -p rect-dominance --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test -p rect-dominance compressed_flow -- --nocapture` | 0 | passed |
| `git diff --check` | 0 | no whitespace errors |

## Boundary

This establishes integration parity only for permanent reference backends. It
does not select or emulate an almost-linear backend, so P9 remains incomplete.
