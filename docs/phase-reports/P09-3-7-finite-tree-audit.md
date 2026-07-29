# P09.3.7 - Finite Tree Traceability and Update Audit

## Scope

This audit closes the finite Section 9.1 path before P9.4 begins. It verifies
source traceability, immutable-update evidence, exact certificate accounting,
and explicit finite-domain rejection for `source_lsst`. It does not prove
Theorem 1.2's general stretch, update-time, recourse, or deamortization bounds.

## Traceability

| Source-facing operation | Production module | Checked evidence |
| --- | --- | --- |
| `H_i = G_i/F_i` and scaled cross-edge length | `source_lsst::level` | stable source endpoints, discarded loops, and exact `stretch_tilde(e) * length_Gi(e)` recompute from the partial-forest certificate |
| Exact dyadic stretch/length partition and finite embedding initialization | `source_lsst::bucket` | integer-only exponents, stable source translation, and finite `source_spanner::dynamic::rebuild` replay |
| `T_0 = F_0 union F_1` | `source_lsst::chain` | exact source-tree, stretch, embedding-hop, congestion, and encoded-length certificates; the terminal tree is constructed with `source_an19` |
| Insert/delete/smaller-side split replay | `source_lsst::replay` | immutable history reconstruction, full-snapshot rebuild count, scheduled-rebuild flags, and stable source-edge recourse sets |

`tools/check_source_lsst_audit.py` examines only the production prefix of the
root, level, bucket, chain, and replay modules. It rejects a production
reference to either enumerating Oracle, greedy spanner construction,
`simple_paths`, or the legacy decremental-spanner path, while requiring the
finite source-shaped calls listed above.

## Adversarial History

The new deterministic replay starts from a four-node cycle with diagonal and
exact nonuniform weights `2, 3, 5, 7, 11`. It then inserts stable source edge
`5` with weight `13`, splits vertex `0` by moving source edge `3`, and deletes
source edge `5`. Each connected snapshot is rebuilt from immutable history and
checked against the bounded exhaustive LSST Oracle. The trace records four
snapshots, three full-snapshot rebuilds, one scheduled rebuild after the second
batch, one insertion, one smaller-side split, one deletion, and a final source
tree with four active stable IDs. A rejected repeat deletion leaves the prior
immutable state unchanged; mutating `full_snapshot_rebuilds` causes replay
verification to reject.

## Finite-Domain Boundary

The chain now has direct regression evidence for all central explicit rejects:

- a nonintegral `1/2` source length;
- integral source length `9` above the configured maximum `8`;
- a scaled dyadic bucket outside the configured exponent range;
- parallel edges in one finite bucket component; and
- a mutated source-tree certificate.

These tests supplement the existing invalid-batch and smaller-side encoding
tests. Exact positive nonuniform weights are audited in the adversarial replay;
the accepted production domain remains connected graphs with positive integral
lengths, finite dyadic keys, simple finite bucket components, and the existing
bounded spanner domain. Every snapshot is deliberately rebuilt, so the observed
work and recourse counters are not a dynamic source bound.

## Audit

Phase baseline: `6b3bb73c8b5b6a09adf290df8bc7ce68904a461d`.
Implementation SHA: `66d7920`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_source_lsst_audit.py` | 0 | finite production trace and no-fallback boundary verified |
| `cargo test -p graph source_lsst -- --nocapture` | 0 | 14 focused tests passed |
| `cargo test -p graph source_spanner -- --nocapture` | 0 | 24 focused tests passed |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | 297 passed, 3 existing ignored, 13 suites, 389.21 s |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | six crates built |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and deferred AN19 proof status are consistent |

P9.3.2d remains low-priority proof debt. DOI `10.1137/17M1115575` does not
provide the reduced-event ordering/counting proof. This audit permits P9.4
implementation work, but it does not permit `AlmostLinear`,
`an19_runtime_verified: true`, or an AN19 asymptotic runtime claim.
