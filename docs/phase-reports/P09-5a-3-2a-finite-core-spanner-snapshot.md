# P09.5a.3.2a - Finite Core/Spanner Snapshot

## State

**Complete for one immutable finite snapshot.** Implementation commit
`cdb2ce9` closes the declaration prerequisite that previously had no
rejected-core population. It does not close P9.5a.3.2b, P9.5a.3.3b, P9.5a, or
P9.5.

## Semantic Basis

Algorithm 4 of van den Brand et al., arXiv:2309.16629v1, pp. 41--42, has
separate Task 2 `W -> J` and Task 3 `J -> W` embeddings. Algorithm 1 and the
Lemma 5.11 discussion use a rejected core edge together with its maintained
spanner embedding to form a fundamental spanner candidate. The finite replay
therefore cannot infer Task 3 from a same-endpoint witness edge.

## Implementation

- Positive decomposition levels use a canonical circulant witness whose exact
  degree vector satisfies `deg_{J_i[X]}(v) / (phi * 2^i)`'s degree sandwich and
  whose positive expansion is exhaustively certified.
- `algorithm4::second_embedding` independently routes every input `J` edge in
  `W`, preserving bounded hop, edge-congestion, deletion, and round evidence.
- Finalization composes each oriented Task 3 path through Task 2 paths and
  loop-erases only closed local walks required by the existing simple-path
  embedding representation.
- `source_min_ratio::spanner::Snapshot` constructs a finite singleton-forest
  Section 9.1 chain from one exact IPM/source projection. Every rejected core
  edge is declared as one explicit contiguous `SpannerPath` plus the same edge
  as its opposite-oriented anchor.
- `candidate::Registry` rejects a `TreePath` substituted for a spanner path and
  rejects a noncontiguous or anchor-containing explicit path before decoding or
  scoring it.

## Evidence

- The K5 Algorithm 4 fixture has ten input edges, a five-edge positive-level
  circulant witness, an independently embedded `J -> W` trace, and a strict
  image subgraph.
- The K5 source snapshot emits five rejected-core declarations. Each one
  decodes to a nonempty exact circulation and the registry selects a nonzero
  candidate under nonzero gradients.
- `tools/check_source_min_ratio_audit.py` includes the `spanner` module and
  rejects production Oracle, greedy, and enumerating-cycle fallbacks.

## Audit

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | tree-chain and sparsified-core boundary accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | no production reference-flow or recovery fallback |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_min_ratio` | 0 | 21 focused tests passed |
| `cargo test -p graph source_spanner` | 0 | 28 focused tests passed |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized workspace build passed |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Limits And Next Action

The accepted domain is an explicit connected finite graph with integral source
lengths and finite witness certificates. The snapshot rebuilds from immutable
input and makes no recourse, cross-snapshot maintenance, general CGLNPS20,
Theorem 8.1, Theorem 8.2, Theorem 5.1, or runtime claim.

P9.5a.3.2b must next retain stable candidate identities while source updates
replace or retire embeddings across snapshots. P9.5a.3.3b must then merge the
terminal and spanner populations into the no-fallback `Step` differential.
P9.3.2d remains distinct, low-priority P9.6a proof debt: the formal SIAM
version of Abraham--Neiman (DOI `10.1137/17M1115575`) does not establish the
reduced-event ordering/counting conversion, so this implementation does not
claim the AN19 runtime.
