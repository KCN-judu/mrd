# P09.5a.3.3b - Complete Candidate Step

## State

**Complete for matching immutable snapshots.** Implementation commit
`98a7d0e` closes P9.5a.3.3b. It does not maintain terminal candidates across
updates, run a complete source-flow iteration, or establish an AN19 runtime.

## Implementation

`source_flow::iteration::Step::from_maintained_candidates` accepts one checked
terminal tree, one checked rejected-core snapshot, the current network, and
the caller's exact approximation coordinates. It:

1. requires both snapshots to own the same exact `Input`;
2. requires the caller gradient and length vectors to equal that input;
3. rebuild-verifies both immutable snapshots against the current network;
4. creates and independently evaluates their two source-declared registries;
5. rejects an overlapping `CandidateId` population;
6. selects the greater exact quality, breaking equality by stable ID; and
7. decodes the winning compact cycle only with its producing terminal or
   spanner chain, shifts, materialization, and arc bindings.

No graph cycle is enumerated. The implementation has no production dependency
on `dynamic_min_ratio`, a min-cost Oracle, Dinic, or Push--Relabel. The
source-flow static audit checks that boundary.

## Evidence

- A K5 fixture constructs nonempty terminal and rejected-core populations from
  the same exact input. It independently scores the two registry choices and
  directly decodes the expected winner; the combined entry point produces the
  identical exact `Step`.
- A terminal/core pair constructed from different gradients rejects before any
  selection.
- Equal qualities resolve to the lower stable candidate ID.
- The former terminal-only coordinate check remains covered under its generic
  mismatch error.

This is a finite semantic differential, not a minimum-ratio optimality proof
or a runtime result.

## Audit

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source minimum-ratio no-fallback boundary accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | no production reference-flow or recovery fallback |
| `cargo test -p graph source_flow::iteration --no-fail-fast` | 0 | 5 focused tests passed |
| `cargo test -p graph source_min_ratio --no-fail-fast` | 0 | 24 focused tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized workspace build passed |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Limits And Next Action

At this commit the terminal snapshot had no cross-snapshot transition. Commit
`b73b0fa` subsequently adds finite same-network terminal recourse. `Backend`
still rejects complete execution, so P9.5 remains in progress pending the full
compressed MRD flow/cut/cover/chord/rectangle differential and end-to-end
no-fallback integration. P9.3.2d
remains separate low-priority P9.6a proof debt: DOI `10.1137/17M1115575` does
not provide the reduced-event ordering/counting conversion. That debt does not
block this semantic work, but it still prohibits `AlmostLinear`,
`an19_runtime_verified: true`, and an AN19 runtime claim.
