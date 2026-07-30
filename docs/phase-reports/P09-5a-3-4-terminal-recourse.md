# P09.5a.3.4 - Terminal Candidate Recourse

## State

**Complete for supported immutable same-network snapshots.** Implementation
commit `b73b0fa` closes P9.5a.3.4. It does not provide a general dynamic tree,
an end-to-end source-flow backend, or an AN19 runtime claim.

## Implementation

`source_min_ratio::input::Input::has_same_source_identity` is a pure shared
check for stable source IDs, circulation IDs, directed endpoints, and node
count. It deliberately excludes current gradients, lengths, and structural
weights, which are allowed to change across a recourse transition.

`terminal::Tree::transition` rebuilds the exact successor terminal tree with
the same root, rejects changed identities, and returns immutable `inserted`,
`refreshed`, `retired`, and `reembedded` sets. Re-embedding compares the exact
decoded candidate direction through each snapshot's own chain and bindings.
`Transition::apply` requires the supplied registry to exactly equal its prior
candidate population, retires absent declarations, and re-evaluates every
retained candidate in the successor context even if the compact declaration did
not change.

The finite core transition uses the same `Input` identity predicate. A
successor-snapshot test passes the independently rebuilt terminal and core
snapshots through `Step::from_maintained_candidates`; neither path enumerates
cycles or imports a reference flow backend.

## Evidence

- Gradient-only terminal changes refresh all retained IDs with no false
  re-embedding.
- A terminal-tree length change causes candidate insert/retire/re-embedding
  recourse and leaves the registry exactly equal to the successor population.
- Applying a terminal transition to a manually altered registry rejects.
- Updated terminal and core snapshots are accepted together by the complete
  candidate-to-`Step` selector.

## Audit

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source minimum-ratio no-fallback boundary accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | no production reference-flow or recovery fallback |
| `cargo test -p graph source_min_ratio --no-fail-fast` | 0 | 28 focused tests passed |
| `cargo test -p graph source_flow::iteration --no-fail-fast` | 0 | 6 focused tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized workspace build passed |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Limits And Next Action

The transition reconstructs finite static terminal snapshots; it does not
provide the source's general dynamic maintenance or any amortized guarantee.
P9.5 remains in progress until the selected direction drives a complete
certified iteration and recovery through compressed MRD networks with flow,
cut, cover, chord, and rectangle differentials. P9.3.2d remains separate,
low-priority P9.6a proof debt. DOI `10.1137/17M1115575` does not supply the
required reduced-event ordering/counting conversion, but that debt does not
block this semantic integration work.
