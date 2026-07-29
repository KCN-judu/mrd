# P09.4a - Source Tree-Chain Contract

## Scope

P9.4a establishes the finite, source-shaped representation beneath the
Theorem 5.1 dynamic minimum-ratio-cycle work. It adds
`source_min_ratio::{model,chain}` and deliberately leaves the older
`dynamic_min_ratio::{oracle,experiment}` namespace as the permanent enumerating
and replay baseline.

The production chain represents ordered logical levels, stable branch IDs,
explicit branch slots, immutable source-edge spanning-tree snapshots, and pure
selector transitions. A shifted state advances one chosen level modulo its
branch count and resets all descendants. The model contains no queue, cycle
enumerator, shortest-path Oracle, mutable link-cut state, or runtime claim.

## Contract And Evidence

| Requirement | Implementation evidence | Verification |
| --- | --- | --- |
| Stable tree-chain identity | `LevelId`, `BranchId`, explicit slots, and ordered levels | duplicate level, branch, and slot mutation tests reject |
| Source-tree integrity | each branch's edge IDs must form a current connected spanning tree of `SourceDynamicGraph` | malformed edge counts and unknown source IDs reject |
| Deterministic shifted selection | `Chain::initial_shifts`, `Chain::shift`, and `Chain::select` are pure transformations | multi-level parent reset and slot-order independence regressions |
| Immutable snapshots | transitions return a new `Shifts` value and retain the prior value | regression compares initial state after child and parent shifts |
| No production fallback | `source_min_ratio` imports neither the P8 enumerating Oracle nor replay path | `python3 tools/check_source_min_ratio_audit.py` |

The checker scans only production prefixes. Tests may continue to use the
exact Oracles for later differential work, but P9.4a has no production cycle
query to compare yet.

## Explicit Boundary

This subphase does not construct the compact cycle of Definitions 5.6--5.8,
does not expose or discover a hidden stability witness, and does not implement
link-cut maintenance, dynamic sparsification, `Update`, `Query`, or `Detect`.
It makes no approximation, recourse, priority-queue, amortized, Theorem 5.1,
or AN19 runtime claim.

P9.3.2d remains faithful implementation with deferred low-priority proof debt.
The formal SIAM paper, DOI `10.1137/17M1115575`, does not provide its missing
reduced-event ordering/counting proof. That debt does not block P9.4 semantic
work, but it continues to prohibit the `AlmostLinear` name,
`an19_runtime_verified: true`, and an AN19 asymptotic runtime claim.

## Audit

Phase baseline: `ba3779e42db2db509e85870adf894bdc2eb93c1f`.
Implementation commit: `4ce313b`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted workspace |
| `python3 tools/check_biclique_bound.py` | 0 | existing structural bound check passed |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | production tree-chain path has no Oracle fallback |
| `cargo clippy -p graph --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_min_ratio -- --nocapture` | 0 | 4 passed, 146 filtered |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | full suite passed; the command proxy emitted no count summary |
| `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps` | 0 | documentation built without warnings |
| `cargo build --workspace --release` | 0 | 6 crates compiled in 17.12 seconds |
| `python3 tools/check_release_consistency.py` | 0 | v1.3.0 release consistency passed |

Final inspection found no ignored-test changes, fallback imports, stale
generated evidence, credentials, private keys, tokens, or local absolute paths.

## Next Action

P9.4b must add compact cycle segments and deterministic exact decoding against
the selected source trees. It may use P7/P8 only as test Oracles and must not
route production decoding through an enumerator.
