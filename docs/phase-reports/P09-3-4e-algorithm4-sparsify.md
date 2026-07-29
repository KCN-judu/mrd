# P09.3.4e - Certified Algorithm 4 Replay

## Scope

P9.3.4e implements the currently certified finite-domain subset of Algorithm 4,
`Sparsify(H', J, Pi(J -> H'))`, from arXiv:2309.16629v1, pp. 41--42. It wires
the P9.3.4b--d finite contracts together without claiming the general theorem
or any of its asymptotic bounds.

## Task 1

`source_spanner::algorithm4::witness` accepts only the existing certified
single-level, single-component decomposition. It verifies that the component
covers every source vertex and edge, constructs the deterministic finite
witness, and records the exact layer weight `phi * 2^level`, vertices, source
edges, and witness-edge provenance. General multi-level decompositions reject.

## Task 2

`algorithm4::first_embedding` replays the `W -> J` loop. It uses direct input
edges where possible and the production stable BFS otherwise. For every round
it records embedded witness edges, vertices exceeding the exact composed
`J -> H'` congestion threshold, and the corresponding `J` deletions. It
returns explicit unembedded witness-edge IDs when the supplied finite limits do
not complete the loop; there is no Oracle fallback.

## Task 3 And Image

`algorithm4::finalize` handles the current finite witness's direct `J -> W`
branch, composes it with the stored `W -> J` paths, constructs the image
subgraph, and runs the existing independent `Audit::verify` for the direct and
composed embeddings. It rejects a missing witness edge or unembedded Task 2
path.

## Limits

This is not a general `Sparsify` implementation. The accepted decomposition is
one level and one component on the at-most-20-node exhaustive domain; its
witness is complete, so the Task 3 path loop currently takes only direct
witness edges. General multi-level loop scheduling, source expander pruning,
and Theorem 8.1 congestion, sparsity, length, and runtime guarantees remain
unimplemented and unclaimed.

## Focused Evidence

- A four-cycle produces a finite complete witness with exact layer weight one.
- Task 2 embeds all witness edges with direct and two-hop paths under an ample
  threshold, and separately records threshold-induced source-edge deletions.
- Task 3 reconstructs the image subgraph and verifies a composed maximum path
  length of one for the identity host fixture.

## Audit

Phase baseline: `e396484ffb9c48ff105b18c81ae578970dbf44e3`.
Implementation SHAs: `93a0aa2`, `08a854c`, `3a637ac`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatted |
| `python3 tools/check_biclique_bound.py` | 0 | bound checker passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_spanner::algorithm4 -- --nocapture` | 0 | 4 passed |
| `cargo test --workspace` | 0 | 280 passed, 3 existing ignored |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized build passed |
| `python3 tools/check_release_consistency.py` | 0 | release metadata and baseline evidence passed |

The final source-spanner inspection found no simple-path Oracle call in the
Algorithm 4 production modules, no ignored P9.3.4e tests, stale generated
evidence, credentials, tokens, private keys, or local absolute paths.
