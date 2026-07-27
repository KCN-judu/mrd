# P08.3 - Decremental Spanner Certificates

## Contract

P8.3 adds `rect-graph::decremental_spanner::DecrementalSpanner`, a checked
certificate layer constrained to the simple unweighted undirected deletion and
vertex-split domain of Theorem 8.2 in arXiv:2309.16629v1. Every current
certificate specifies a subgraph and an explicit simple spanner path for every
active input edge. Validation checks path endpoints, active membership,
connectivity, path length, vertex congestion, and re-embedding recourse.

The module deliberately does not construct an expander, a spanner, or a
decremental shortest-path structure. It has no insertion or directed-edge API,
and makes no Theorem 8.2 sparsity, congestion, recourse, or runtime claim.

## Evidence

- Four focused tests cover a valid triangle embedding, congestion/path-length
  accounting, certificate replacement after deletion, vertex splitting with
  revalidated paths, repeated-deletion rejection, and parallel-edge rejection.
- Certificate validation rejects inactive paths, non-simple paths, missing
  endpoint paths, and any edge outside the current spanner subgraph.
- No automatic fallback or performance selection exists.

## Full audit

Phase baseline: `51603691f6ca8b60cc178f422d397de24a8aa514`.

The complete required audit exited 0 in 23.4 seconds:

```text
git diff --check
cargo fmt --all -- --check
python3 tools/check_biclique_bound.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo build --workspace --release
python3 tools/check_release_consistency.py
```

Final diff inspection found no ignored tests, stale evidence, fallback use,
credentials, tokens, private keys, or local absolute paths.

## Next action

P8.4 must build a deterministic forest collection over P8.2/P8.3 certificates
while retaining exhaustive small-instance forest comparison as its Oracle.
