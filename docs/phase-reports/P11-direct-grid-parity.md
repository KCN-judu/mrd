# P11 - Direct Grid Parity Embedding

## Status

**State: P11.1-P11.2 complete; P11.3-P11.5 remain.** The permanent
`RankedCoordinates` Oracle remains the default construction. The new direct
encoder is exact and independently testable, but it is not yet routed through
the grid solver; that end-to-end work is P11.3.

## P11.1 - Backend Contract and Counters

`dominance::embedding` now exposes `EmbeddingCoordinateBackend` with two
explicit variants:

- `RankedCoordinates` is the current general-coordinate construction and the
  permanent Oracle.
- `DirectGridParity` is the finite integer grid construction. It is selected
  only through `DominanceEmbedding::new_with_backend`; the existing
  `DominanceEmbedding::new` remains ranked by default.

Every `DominanceEmbedding` records its backend and `EmbeddingMetrics`.
`rank_sort_count`, `rank_map_entry_count`, and `rank_map_owned_bytes` describe
only ranked-coordinate construction. The direct branch reports exactly zero for
all three counters rather than an unavailable estimate. The ranked byte metric
is a capacity-based owned-entry estimate; it is not a global allocator claim.

## P11.2 - Exact Direct Parity Encoder

The direct branch materializes only the required checked integer formulas:

```text
horizontal: (2*l, -2*r, 2*y, -2*y)
vertical:   (2*x+1, -2*x+1, 2*t+1, -2*b+1)
```

It never calls the rank-set/rank-map construction and never creates a sorted
coordinate vector. Checked `i128` arithmetic preserves the existing overflow
error contract. Even horizontal coordinates and odd vertical coordinates also
make cross-side coordinate equality impossible on the direct path.

## Evidence

The focused embedding test exhaustively constructs the existing small chord
population through both backends. It checks direct geometry/dominance
equivalence, exact explicit-graph equality with the ranked Oracle, ranked
counter presence, and all three zero direct counters. A separate fixture checks
the literal formula coordinates.

P11.3 must still prove end-to-end equality of biclique partitions, networks,
flows, covers, selected cuts, and rectangles on grid inputs. No direct-grid
performance claim is made in this checkpoint.

## Audit

Phase baseline: `feef7e71fdc873e6bb48bb71bfe3dfde543f46f8`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo test -p dominance embedding` | 2 passed, 58 filtered out |
| `cargo clippy -p dominance --all-targets --all-features -- -D warnings` | passed |
| `cargo test -p dominance` | 58 passed, 2 ignored (143.72s) |
| `git diff --check` | passed |
| `python3 tools/check_biclique_bound.py` | passed |
| `python3 tools/check_source_flow_audit.py` | passed |
| `python3 tools/check_release_consistency.py` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` | 420 passed, 4 ignored (15 suites, 534.15s) |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | passed |
| `cargo build --workspace --release` | passed |

The final staged-diff inspection must also reject accidental fallback,
credentials, private paths, stale generated evidence, and ignored direct-grid
tests before the closeout is pushed.
