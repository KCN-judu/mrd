# P09.5a.3.3a - Terminal-Candidate Step Bridge

## Status

**State: complete for terminal declarations in one immutable source/IPM
snapshot.** Implementation SHA: `5afa4c7`.

## Scope

`graph::source_flow::iteration::Step::from_terminal_candidate` is the narrow
pure boundary from a checked `source_min_ratio::terminal::Tree` to a Lemma 4.4
`Step`. It reconstructs the exact gradient and length vectors stored in the
terminal's immutable `source_min_ratio::input::Input` and rejects caller
vectors that differ in value or order. It then creates the terminal declaration
registry, takes its best nonzero candidate, and delegates exact direction
decoding to `Step::from_compact_candidate`.

An empty terminal declaration population, or a population whose candidates
have zero quality, yields `Ok(None)`. No graph search, reference implementation,
or recovery backend is consulted. The registry contains only source-derived
terminal fundamental tree declarations from `terminal::Tree`; the bridge does
not create or infer core/spanner candidates.

## Evidence

Focused tests cover a nonzero terminal declaration that decodes to an exact
circulation direction, coordinate mismatch rejection, and an already-tree
source graph whose empty population produces no step. The static source-flow
audit now requires `from_terminal_candidate` and its coordinate-mismatch error,
while continuing to reject production dependencies on `dynamic_min_ratio`,
min-cost Oracles, Dinic, and Push--Relabel.

## Audit

Implementation SHA `5afa4c7` passed:

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | terminal/source boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | source-flow no-fallback boundary accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_flow::iteration` | 0 | focused iteration tests passed |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | rustdoc accepted with warnings denied |
| `cargo build --workspace --release` | 0 | release build accepted |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Non-claims And Next Action

This bridge neither creates core/spanner embedding provenance nor manages
candidate replacements across snapshots. The existing finite Algorithm 4
replay retains all direct input edges, so it currently produces no rejected
core edges from which to declare fundamental spanner candidates. It must not
be presented as a complete selector.

P9.5a.3.2a therefore remains blocked on a source-supported sparsifier, or an
equivalent source-supported construction, that can produce rejected-core
declarations with exact provenance. P9.3.2d's reduced-event proof debt remains
the separate low-priority P9.6a task: it does not block this semantic work, but
it continues to prohibit an `AlmostLinear` name or AN19 runtime claim.
