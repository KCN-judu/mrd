# P09 Functional Architecture Refactor

- Date: 2026-07-29
- Branch: `codex/full-implementation`
- Baseline: `318fb302363f47515c8082ac273427eb1e81d52e`
- Implementation: `2b0036545cdd2cfe8a26c3655aa4c72fd6b1791f`
- Phase effect: maintenance only; P9.3.2d's runtime proof remains deferred,
  while P9 continues through the source-shaped backend chain

## Scope

The seven workspace packages now use responsibility-bearing names:
`mrd-domain`, `graph`, `exact-cover-oracle`, `sg-oracle`, `dominance`,
`verification`, and `mrd`. Dominance solving is owned by
`dominance::experiment`, SG grid algorithms are owned by `sg_oracle::grid`,
and differential grid campaigns are owned by `verification::grid`. Oracle and
experimental chord enumeration are distinct as
`sg_oracle::grid::oracle::Pairwise` and
`sg_oracle::grid::experiment::InteriorRuns`.

Removed crate-root paths were not re-exported. Namespace-local names were
shortened to `Mode`, `Verification`, `Representation`, `Analysis`, `Geometry`,
`Enumerator`, and `Error`. CLI process startup is isolated from application
dispatch. These boundaries use modules, enums, generics, and static dispatch;
no trait objects, registries, compatibility adapters, or additional runtime
allocations were introduced.

## Audit

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `git diff --check` | passed |
| `python3 tools/check_biclique_bound.py` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` | 261 passed, 3 ignored |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | passed |
| `cargo build --workspace --release` | passed |
| `python3 tools/check_release_consistency.py` | passed |

Historical result manifests and earlier phase reports retain their original
package paths because rewriting them would falsify recorded reproduction
commands. Active documentation, tools, Cargo metadata, and source imports use
the new architecture.

## Runtime Status

No complexity or runtime claim changed. The AN19-shaped implementation remains
exact and source-shaped on its tested domain, while the reduced-event
hierarchy-wide amortization, proof-backed priority queue, and complete AN19
runtime verification remain unproved and deferred to P9.6.
