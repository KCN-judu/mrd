# P09.3.6 - Finite Dynamic Low-Stretch Tree

## Scope

Section 9.1 of arXiv:2309.16629v1 contracts a partial forest as `H_i = G_i/F_i`, rescales each cross edge to `stretch_tilde(e) * length_Gi(e)`, partitions exact dyadic ranges, and defines `T_i = F_i union F_{i+1} union ... union F_d`.

This phase implements one finite depth-one instance of that data flow. It does not claim the theorem's subpolynomial stretch, update time, amortized recourse, deamortization, or arbitrary-depth hierarchy.

## Implementation

`source_lsst::level` consumes a checked partial-forest certificate, preserves source endpoints and exact scaled lengths for cross edges, and explicitly records contracted loops. A direct component-enumerating Oracle differentially checks components, crossings, loops, and scaled lengths.

`source_lsst::bucket` computes exact `floor(log2(r))` for positive rationals using integer arithmetic. It splits each range into connected components, initializes the finite dynamic-spanner replay, and translates selected edges and embedding paths back to `SourceEdgeId`. Unsupported components return errors; no path or greedy Oracle is used by the production path.

`source_lsst::chain` combines a certified `F_0` with an AN19-shaped static terminal tree `F_1` on the selected contracted spanner. It verifies `T_0 = F_0 union F_1` is a source tree and recomputes exact weighted/max stretch plus per-level embedding hops, congestion, and encoded length. The bounded path fixture agrees with the exhaustive LSST Oracle.

`source_lsst::replay` models a state as an initial graph plus immutable batch history. It reconstructs the graph, finite chain, scheduled-rebuild flags, full-snapshot rebuild count, and tree recourse. Tests cover a connected deletion, a smaller-side stable-ID split, and an insertion. Its `F_0` is the certified empty forest, so this finite path does not claim dynamic-LSF maintenance or amortized work. The LSST Oracle is test-only and never a fallback.

## Limits

The accepted domain is connected positive integral lengths within the supplied finite bound, simple finite spanner bucket components, and the existing at-most-eight-node spanner fixture domain. Every replayed snapshot is rebuilt. All counters are exact observations, not a dynamic runtime claim.

P9.3.2d remains low-priority proof debt. DOI `10.1137/17M1115575` does not provide the reduced-event ordering/counting proof. This does not block work through P9.5, but still forbids `AlmostLinear`, `an19_runtime_verified: true`, and AN19 runtime claims.

## Audit

Phase baseline: `4714ee3311ac85e1407aa3ae047e6ce5f2558697`.
Implementation SHAs: `8a69733`, `a9ac727`, `6985234`, `4a3ad34`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo test -p graph source_lsst -- --nocapture` | 0 | 12 focused tests passed |
| `cargo test -p graph source_spanner -- --nocapture` | 0 | 24 focused tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

The source scan found the exhaustive LSST Oracle only inside `#[cfg(test)]` blocks. No `simple_paths`, greedy-rebuild, or legacy decremental-spanner fallback is reachable from the finite tree-chain production modules. No ignored P9.3.6 tests, stale evidence, credentials, keys, or local absolute paths were introduced.
