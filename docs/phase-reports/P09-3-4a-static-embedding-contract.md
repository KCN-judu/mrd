# P09.3.4a - Static Embedding Composition Contract

## Scope

P9.3.4a implements the exact semantic boundary required before Theorem 8.1's
Algorithm 4 can be constructed. It does not implement an expander, expander
decomposition, decremental expander paths, witness graph, `Sparsify`, or a
Theorem 8.1 sparsity/congestion/runtime claim.

## Implementation

`graph::source_spanner::model` defines independent, simple unweighted graphs
with stable edge identifiers, explicit simple-path embeddings, selected
subgraphs, and a compositional audit. Given `Pi(J -> H')` and an embedding of
`J` into selected `J~ subset J`, the audit substitutes each `J` edge in the
second path with its exact `H'` path and then revalidates the result. It records
the direct, subgraph, and composed maximum path lengths, edge congestion,
vertex congestion, total encoded path length, selected edge count, and maximum
subgraph degree.

`graph::source_spanner::oracle::simple_paths` is an intentionally bounded,
deterministic DFS enumerator of every simple path in lexicographic edge-id
order. It is separate from the model and cannot be reached by any production
embedding API.

## Evidence

- A composition fixture verifies a triangle `J`, a two-edge `J~`, a host
  embedding, exact composed routing, selected-edge count, and composed path
  length.
- Negative tests reject target edges outside `J~` and a repeated-vertex path.
- The Oracle test enumerates both bounded simple routes in deterministic order.

## Source Boundary

Theorem 8.1 requires `H'`, `J`, and `Pi(J -> H')` plus the composed embedding
after Algorithm 4 creates `J~`. This phase establishes those definitions only.
P9.3.4b must construct Theorem 8.4's witness expander; P9.3.4c and P9.3.4d
must establish the Theorem 8.5/8.6 prerequisites before P9.3.4e may implement
Algorithm 4.

## Audit

Phase baseline: `91a3e3cfbe54b132c86f4fa351394fc2141cc527`.
Implementation SHA: `e0b7bc1`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_spanner -- --nocapture` | 0 | 3 passed |
| `cargo test --workspace` | 0 | 267 passed, 3 existing ignored, 547.77s |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

Final diff inspection found no fallback use, ignored P9.3.4a tests, stale
generated evidence, credentials, tokens, private keys, or local absolute paths.
