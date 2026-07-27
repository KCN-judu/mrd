# P08.5 - Compact Cycle Chain

## Contract

P8.5 adds `rect-graph::dynamic_min_ratio`. `CompactCycle` encodes signed
off-tree edges and signed tree-path segments, then decodes them through P7's
exact circulation validator. `ShiftedTreeChain` deterministically implements
the shift/reset and rebuild/reset trace of Definitions 5.9--5.10. The
`DynamicMinRatioReplay` composition delegates `Update`, `Query`, and `Detect`
to the P8.1 stable ledger.

This is a checked replay baseline. It does not construct core graphs or
sparsifiers, search for an approximate min-ratio cycle, hide a witness, or
claim Theorem 5.1's dynamic data structure or amortized bound.

## Evidence

- Three focused tests cover compact cycle decoding to a P7-validated
  circulation, deterministic descendant reset under shift/rebuild, and an
  integrated stable-ledger update/query/detect replay.
- Signed arc occurrences reject empty, invalid-direction, invalid-ID, and
  nonconserving sequences before a compact cycle is accepted.
- There are no performance artifacts, fallback paths, or automatic backend
  selection claims.

## Full audit

Phase baseline: `c1ba81e1af0998975ef65ad006cc1fb80ff717ff`.

The complete required audit exited 0 in 21.7 seconds:

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

P8.6 must integrate the checked P8.1--P8.5 parts only as a deterministic
replay/audit component and reject unsupported operations explicitly.
