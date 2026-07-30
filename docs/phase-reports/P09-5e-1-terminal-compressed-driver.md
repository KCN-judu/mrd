# P09.5e.1 - Terminal Compressed Source-Driver Differential

## Status

**State: complete as a terminal-session composition.** Implementation commit:
`90f51ae`.

This subphase connects the P9.5d bounded driver to compressed circulation
recovery. It proves that a source session already certified at additive-half
termination can pass through one no-fallback operation to a matching, Konig
cover, selected chords, and formal rectangle completion. It does not construct
or select a projection for a nonterminal snapshot, so it does not close P9.5e
or enable `Backend::require_complete()`.

## Production Contract

`compressed_flow::experiment::source::Circulation::run_source` accepts a
`source_flow::iteration::Driver`, calls its exact `run` method for this exact
circulation, and then calls `recover_source_session` on the retained terminal
session. Its `Run` result retains both the exact driver completion and the
recovered compressed solution. Iteration failures are represented separately
from source recovery failures, so a wrong snapshot/network cannot be
misreported as a rounding or cover failure.

The production module does not import Dinic, Push--Relabel, min-cost Oracle
code, dynamic-min-ratio code, or permanent recovery paths. The static
source-flow audit now requires the composed entry point and its distinct
iteration failure boundary.

## Differential Fixtures

The test module alone obtains an expected integral optimum from the permanent
reference solver. It also constructs an explicit strictly interior circulation
by assigning a positive balanced flow through every biclique block and its
outer arcs. A checked convex interpolation between that circulation and the
reference optimum creates a strictly interior additive-half snapshot; exact
feasibility and additive-half termination are certified before production
recovery is called.

The composed production path is differentially checked for:

- an explicit complete two-by-two biclique partition against the exact matching
  and cover Oracle;
- a Theorem 8 compact partition of an MRD chord conflict graph against its
  explicit matching and cover;
- the formal Figure 3 polygon, including conversion of the recovered cover to
  chord flags and coordinate-compressed rectangle completion;
- rejection of an additive-half snapshot that belongs to a different
  circulation before any recovery path is entered.

All three accepted terminal fixtures have zero driver records by construction:
this verifies that a terminal snapshot reaches recovery without requesting a
factory projection. It is deliberately not evidence for nonterminal source
projection preparation.

## Audit

Phase baseline: `b068bea42d2438f0035ca60b4486f0ba2461f8e7`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source candidate boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | composed driver/recovery boundary has no reference-flow fallback |
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | 5 compressed source-flow tests passed |
| `cargo test -p graph source_flow -- --nocapture` | 0 | 21 source-flow tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace` | 0 | full workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | documentation built without warnings |
| `cargo build --workspace --release` | 0 | six workspace crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Remaining Boundary

P9.5e.2 must prepare a fresh `Projection` for each nonterminal compressed
snapshot, prove its exact coordinates satisfy Theorem 4.3, and exercise at
least one source-selected transition per applicable fixture. No endpoint of a
`DyadicInterval` can be silently adopted as an exact coordinate. P9.5e.3 then
must perform the full combined campaign before the backend completeness gate
can be reconsidered.

P9.3.2d remains independent low-priority P9.6a proof debt. No AN19 runtime
claim, `AlmostLinear` name, or `an19_runtime_verified: true` result is made.
