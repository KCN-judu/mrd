# P13 - Constant-Factor Performance Hardening

## Status

**State: P13.1-P13.2 complete; P13.3-P13.5 remain.** This phase starts with a
reproducible baseline, not an optimization claim. All timings below are local
development-profile observations for the exact 3x3 finite-grid population; they
are only valid for comparison against later runs with the same recorded
environment and command.

## P13.1 - Reproducible Hot-Path Baseline

`mrd benchmark --suite direct-grid-parity --output <path>` now serializes phase
totals separately for `fully-audited` and `compact-only` direct and ranked
coordinate paths. Each mode covers 897 nonempty connected components generated
from the 511 nonzero 3x3 masks. The report requires 1,794 exact
direct-versus-ranked output/certificate comparisons before it succeeds.

The direct path retained zero rank sorts, rank-map entries, and rank-map-owned
bytes. The ranked Oracle recorded 3,588 sorts, 624 entries, and 18,240
estimated owned bytes. The local direct/ranked embedding totals were 607/4,553
microseconds. The full per-mode phase maps are intentionally machine-readable
in the benchmark JSON so later P13 subphases can compare geometry, embedding,
biclique, compressed-flow, completion, validation, and end-to-end-adjacent
phase totals independently.

This is a baseline only. It neither promotes the local timings to a portable
end-to-end result nor establishes a speedup outside the measured population.

## P13.2 - Geometry and Embedding Storage

`sg-oracle::grid::experiment::InteriorRuns` previously inserted every
canonical horizontal and vertical chord record into a `BTreeSet`. Its traversal
already walks ascending reflex coordinates, ascending interior runs, and
ascending aligned endpoint pairs; the records are therefore unique and
lexicographically canonical by construction. The implementation now emits
directly into `Vec`, removing per-record tree-node allocation and balancing
work while preserving the exact record order.

The reference `Pairwise` Oracle deliberately retains its `BTreeSet`. Existing
3x3 and 4x4 exhaustive chord-family differentials, P11's full embedding and
rectangle campaign, and the workspace audit all remain the acceptance gates.
The post-change single baseline observation was not used as a speed claim: its
microsecond variance is too small and environment-sensitive for that purpose.

This is a local storage/traversal reduction with an explicit permanent Oracle,
not a complexity-bound change.

## Audit

Phase baseline: `3bcf4a284d947f1d2cce015d79711135fc9daaa1`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo test -p verification` | passed |
| `cargo clippy -p verification -p mrd --all-targets --all-features -- -D warnings` | passed |
| `cargo run -p mrd -- benchmark --suite direct-grid-parity --output <temporary>.json` | 897 components, 1,794 comparisons, zero mismatches/errors, and per-mode phase maps |
| `cargo test -p sg-oracle` | passed, including exhaustive 3x3/4x4 chord-family and completion differentials |
| `python3 tools/check_biclique_bound.py` | passed |
| `python3 tools/check_source_flow_audit.py` | passed |
| `python3 tools/check_release_consistency.py` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` | passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | passed |
| `cargo build --workspace --release` | passed |

The complete workspace audit remains required before this checkpoint is pushed.
