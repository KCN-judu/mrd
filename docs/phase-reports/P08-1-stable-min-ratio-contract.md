# P08.1 - Stable Min-Ratio Contract

## Contract

`rect-graph::min_ratio_cycle::StableMinRatioLedger` implements an exact,
auditor-facing representation of Definitions 4.2--4.5 in van den Brand et al.,
arXiv:2309.16629v1. It validates signed incidence circulations, positive
lengths, valid-pair bounds, negative witness quality, factor-two witness
stability, and a checked approximate update direction. Its exact rational flow
coordinates and append-only `Update`/`Query`/`Detect` log make traces
deterministically replayable.

The witness is supplied only to this verifier. The ledger neither discovers a
cycle nor claims a hidden-witness, dynamic, amortized, or almost-linear
algorithm. It accepts no graph insertion, deletion, or vertex split operation;
those are deferred to separately audited P8 subphases.

## Evidence

- Six `rect-graph` min-ratio tests cover exact update/query/detect replay,
  invalid non-circulation rejection, explicit witness-bound reset, immediate
  factor-two violation, and a decrease followed by an invalid rebound against
  every earlier non-explicit bound.
- All comparisons use checked `i128` arithmetic or reduced exact ratios; no
  floating-point decision is used.
- There are zero disagreements or generated performance artifacts. This phase
  is a contract verifier, not a benchmarked backend.

## Full audit

Phase baseline: `29ad3613ca2c180cb5f0dd4cae672c51bf8322e8`.

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

Final diff inspection found no ignored-test changes, fallback use, stale
evidence, credentials, private keys, tokens, or local absolute paths.

## Next action

P8.2 must provide only the source-mapped rooted-forest primitive and retain a
static forest Oracle. No P8 or P9 complexity claim is unlocked by P8.1.
