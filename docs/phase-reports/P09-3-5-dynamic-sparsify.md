# P09.3.5 - Finite Dynamic Sparsify Replay

## Scope

P9.3.5 connects the certified P9.3.4 Algorithm 4 subset to the source-shaped
decremental update domain used by Theorem 8.2: edge deletions and smaller-side
vertex splits. It is a finite, immutable replay with stable original edge IDs;
it does not claim Theorem 8.2's general sparsity, recourse, or runtime bounds.

## Batch Domain

`source_spanner::dynamic::batch` records atomically validated deletion/split
batches. A split may move only a nonempty active incident side no larger than
the side left behind. Replaying persistent batches rebuilds the active simple
graph and maps each relative edge ID back to its stable original source ID.

## Finite Rebuild

`dynamic::rebuild` runs the already certified finite Algorithm 4 construction
on each active graph. Its snapshot maps the selected image and every input
embedding path to stable IDs. Exact set difference records selected-edge
additions and removals. The re-embedding set is derived from each surviving
source edge's old and new image paths, rather than from coincident stable edge
identifiers. Every snapshot is replayed through the exact static embedding
contract before it is accepted.

`Accounting` records initialization selected-edge count, batches, source update
and encoded sizes, deletion/split totals, selected-edge recourse, and
re-embedding totals. These are finite observations only and are not used as an
amortized complexity claim.

## Independent Oracle

`dynamic::oracle::greedy` is a deliberately slow verification implementation.
For each active source edge it exhaustively enumerates all bounded simple paths
in the current selected subgraph before deciding whether to retain the edge.
It does not invoke the dynamic Algorithm 4 construction. Its certificate is
also checked after mapping stable IDs back to the active graph.

The differential update fixture permits the finite Algorithm 4 path and the
greedy Oracle to select different subgraphs. It requires both certificates to
cover exactly the active source-edge set and forbids either certificate from
referencing a deleted stable edge.

## Limits And Proof Boundary

The accepted Algorithm 4 input remains a connected, one-level finite domain.
Operations which leave that domain return an explicit error; there is no Oracle
fallback. This phase does not implement general multi-level decomposition,
general source pruning, Theorem 8.2 recourse/sparsity bounds, or its runtime.

P9.3.2d remains separate, low-priority proof debt. The formal SIAM paper,
DOI `10.1137/17M1115575`, does not provide the required reduced-event
ordering/counting proof. That fact does not block implementation through P9.5,
but it continues to prohibit the `AlmostLinear` name, a true
`an19_runtime_verified` result, and any AN19 asymptotic runtime claim.

## Focused Evidence

- Deleting an edge from a four-cycle removes its stable ID from the finite
  selected snapshot and records one selected-edge removal.
- A degree-two smaller-side split preserves all original stable IDs, reports
  the new vertex and exact encoded size, and replays a valid finite embedding.
- The path-level re-embedding test distinguishes a changed source path from a
  stable edge with the same identifier.
- The Oracle retains two edges of a triangle under a two-hop bound and embeds
  the third edge through the canonical two-edge path.
- The deletion differential validates both the finite rebuild certificate and
  the independent greedy certificate on exactly the active source-edge set.

## Audit

Phase baseline: `e396484ffb9c48ff105b18c81ae578970dbf44e3`.
Implementation SHAs: `1d18dee`, `7282e92`, `9d7bed7`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo test -p graph source_spanner::dynamic -- --nocapture` | 0 | 8 focused tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

The final source inspection found `simple_paths` only in
`source_spanner::dynamic::oracle`; the finite Algorithm 4 rebuild does not use
it. There are no ignored P9.3.5 tests, stale generated evidence, credentials,
tokens, private keys, or local absolute paths in the phase changes.
