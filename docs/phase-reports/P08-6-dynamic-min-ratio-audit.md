# P08.6 - Dynamic Min-Ratio Audit

## Contract

P8.6 adds `DynamicMinRatioAudit`, the integration boundary for P8.1 through
P8.5. It retains the checked stable replay, validates every submitted compact
cycle through P7's exact circulation Oracle, and records exact counts for
cycle checks and rejected operations. Insertion, directed-edge, and arbitrary
topology requests are explicitly rejected.

This is an audit/replay component. It does not discover an approximate cycle,
maintain the source's dynamic graphs, use a fallback backend, or claim the
Theorem 5.1 approximation, update, or amortized-work bounds.

## Evidence

- The focused integration regression validates a compact two-arc circulation,
  verifies the P7 static Oracle is invoked, rejects an edge insertion, and
  checks both work counters.
- Existing P8.1--P8.5 tests continue to cover stable updates, rooted forests,
  spanner certificates, deterministic forest collection, compact cycle decode,
  and tree-chain replay.
- No unsupported operation is silently mapped onto a supported one.

## Full audit

Phase baseline: `0032ffc76a0574b416b64643abfd42c8338ef60c`.

The complete required audit exited 0 in 23.9 seconds:

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

## Remaining limitation

P8 supplies checked, deterministic baseline components only. P9 cannot use
their existence as evidence for an almost-linear flow claim without the full
source assumptions, genuine query algorithm, and end-to-end recovery gates.
