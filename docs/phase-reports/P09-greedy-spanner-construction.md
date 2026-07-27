# P09 Greedy Spanner Construction

## Scope

`DecrementalSpanner` can now construct and rebuild its own certificate with a
stable-order greedy unweighted spanner routine. An active edge is retained only
when no retained-subgraph path within a supplied hop limit joins its endpoints.
Every omitted edge receives the discovered explicit embedding path.

## Boundary

This is a deterministic static/rebuild baseline. It is not the expander-based
decremental construction of Theorem 8.2: it has no quasipolynomial size,
congestion, recourse, or update-time proof. Existing certificate validation and
measurements remain the source of truth for its observable behavior.

## Evidence

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all` | 0 | formatted source |
| `cargo check -p rect-graph` | 0 | passed |
| `cargo clippy -p rect-graph --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test -p rect-graph decremental_spanner -- --nocapture` | 0 | 6 passed, 34 filtered |
| `git diff --check` | 0 | no whitespace errors |

The tests construct a two-hop triangle spanner, validate its omitted-edge
embedding, delete the omitted edge, and rebuild. A zero-hop request is rejected.
P9 remains incomplete.
