# P09.5a.3.2b - Finite Core Recourse

## State

**Complete for supported immutable same-network snapshots.** Implementation
commit `9238b37` closes P9.5a.3.2b. At that commit it did not implement a
general dynamic source graph, terminal/core candidate merge, or a complete
`Step` selector. Commit `98a7d0e` subsequently adds the matching-immutable-
snapshot merge and `Step` bridge; terminal cross-snapshot maintenance remains
outside both substeps at that point. Commit `b73b0fa` subsequently adds finite
same-network terminal recourse.

## Implementation

`source_min_ratio::spanner::Snapshot::transition` rebuilds the next exact
projection with the same source/circulation identities and returns pure stable
candidate sets:

- `inserted` for newly rejected core edges;
- `retired` for no-longer-rejected edges;
- `refreshed` for all retained IDs whose exact gradient or length score must be
  re-evaluated; and
- `reembedded` for retained IDs whose explicit compact embedding changed.

`Transition::apply` first requires the supplied registry to equal the exact
prior declaration population. It then retires removed candidates, replaces all
retained candidates using the next exact coordinate context, and inserts new
candidates. This prevents a stale or mixed source population from silently
accepting a transition.

## Evidence

- A gradient-only K5 transition refreshes every candidate but records no false
  re-embedding.
- A one-edge length-bucket change triggers source-declared insertion or
  retirement, then produces exactly the new snapshot's registry population.
- Applying the same transition to a registry with one manually retired
  candidate fails explicitly.

## Audit

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | finite tree-chain and sparsified-core boundary accepted |
| `cargo test -p graph source_min_ratio` | 0 | 24 focused tests passed |
| `cargo test -p graph source_spanner` | 0 | 28 focused tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | warning-free documentation |
| `cargo build --workspace --release` | 0 | optimized workspace build passed |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Limits

The transition rejects a changed network shape or source endpoint identity. It
does not consume `SourceUpdateBatch`, merge terminal declarations, offer an
Oracle fallback, prove amortized recourse, or support an AN19 runtime claim.
The P9.3.2d proof debt remains low-priority P9.6a work; DOI
`10.1137/17M1115575` does not establish the required reduced-event conversion.

## Next Action

P9.5a.3.3b now combines the terminal and core candidate populations for
matching immutable snapshots, bridges the best source-declared choice to
`Step`, and supplies a K5 no-fallback differential. The next action is live
terminal/core recourse is now complete for the finite same-network domain; the
next action is full backend integration.
