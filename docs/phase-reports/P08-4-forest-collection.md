# P08.4 - Forest Collection

## Contract

P8.4 adds `rect-graph::lsf_mwu::ForestCollection`, an exact deterministic
small-instance baseline for the forest-collection shape in Lemma 5.5 of
arXiv:2309.16629v1. Every round performs checked weighted Kruskal selection,
uses the P8.2 rooted-forest primitive to measure every Definition 5.3 stretch,
and deterministically doubles the penalty for a stretched edge. It returns the
selected trees, exact per-edge average-stretch certificates, and update counts.

This is not the paper's low-stretch-forest construction and makes no
`O(log^7 n)` average-stretch, initialization-time, or production-scale claim.
The P8.2 static stretch computation remains the small-instance Oracle.

## Evidence

- Two focused tests prove deterministic repeated construction and exact
  average-certificate availability on a triangle, plus rejection of zero
  collection size and disconnected input.
- Weighted ordering, penalty growth, rational addition, and average
  normalization all use checked arithmetic. Overflow is reported rather than
  changing a tree choice.
- The collection records one round per requested tree and every penalty update.

## Full audit

Phase baseline: `b0d0b227290d859620354744149a2c3630a830da`.

The complete required audit exited 0 in 25.4 seconds:

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

Final diff inspection found no ignored tests, stale generated evidence,
fallbacks, credentials, tokens, private keys, or local absolute paths.

## Next action

P8.5 must encode compact cycles and tree-chain replay only after preserving
agreement with P7's exact static circulation Oracle on small instances.
