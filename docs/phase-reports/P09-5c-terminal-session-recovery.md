# P09.5c - Terminal Source-Session Recovery

## Status

**State: complete as a terminal-session handoff.** Implementation commit:
`3527d70`.

This subphase composes an already certified, additive-half terminal
`source_flow::iteration::Session` with compressed circulation recovery. It
does not run IPM iterations, infer a terminal state, select a reference flow,
or enable `Backend::require_complete()`.

## Contract

`CertifiedIpmSnapshot` now stores an immutable identity for the certified
circulation: node count, demands, and each ordered arc's endpoints, capacity,
and cost. `verify_network` compares that identity before an update or an
additive-half termination certificate is accepted. Consequently every
source-flow recovery path rejects a same-sized but different network before
rounding or cover reconstruction begins.

`dominance::compressed_flow::experiment::source::Circulation::recover_source_session`
accepts a `Session`, invokes only `Backend::recover_terminated` on its own
circulation, and then uses `recover_certified` to return the exact matching and
Konig cover. The terminal snapshot therefore proves the same exact circulation
that is recovered. The source-flow static audit requires this public handoff and
continues to prohibit reference-flow, min-cost, dynamic-min-ratio, and
rounding-Oracle dependencies in production modules.

## Focused Evidence

- `cargo test -p graph source_flow`: 16 tests passed. A new regression changes
  one cost on a same-sized circulation and receives
  `CertifiedIpmError::NetworkMismatch` before terminal recovery.
- `cargo test -p dominance compressed_flow::experiment::source`: 4 tests
  passed. The compressed two-by-two source fixture now recovers its matching
  and cover through `recover_source_session`, rather than manually composing
  recovery and cover conversion.

## Audit

Phase baseline: `3728886d13009ac3455649e08a67b2bd0f68499a`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | finite source candidate boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | source handoff has no reference-flow or recovery fallback |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_flow` | 0 | 16 focused tests passed |
| `cargo test -p dominance compressed_flow::experiment::source` | 0 | 4 focused tests passed |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | documentation built without warnings |
| `cargo build --workspace --release` | 0 | six workspace crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Remaining Boundary

P9.5 still lacks a complete source iteration driver that prepares each next
exact source projection from an explicitly certified approximation, drives the
session to additive-half termination, and then runs this handoff over the full
compressed MRD flow/cut/cover/chord/rectangle differential population.
`Backend::require_complete()` remains unavailable until that work and its
no-fallback campaign pass.

P9.3.2d remains deferred low-priority P9.6a proof debt. It does not block this
semantic work, but it continues to prohibit the `AlmostLinear` name,
`an19_runtime_verified: true`, and any AN19 asymptotic runtime claim.
