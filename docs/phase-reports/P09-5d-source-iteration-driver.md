# P09.5d - Certified Multi-Step Source Iteration Driver

## Status

**State: complete as a bounded exact iteration orchestrator.** Implementation
commit: `0410b79`.

This subphase closes the missing multi-step control boundary between one
source-selected update and an additive-half terminal session. It does not
choose exact values from fixed-point intervals, construct a general dynamic
source data structure, use a reference flow backend, recover a compressed MRD
instance, or enable `Backend::require_complete()`.

## Contract

`source_flow::iteration::Projection` owns the exact source state for one
specific `CertifiedIpmSnapshot`: the exact `Input`, terminal and rejected-core
candidate populations, checked ledger, and `kappa`. Its constructor first
checks the exact network identity and the equality of all three source inputs,
then reconstructs the Theorem 4.3 factor-two-length and scaled-gradient
certificate from the exact coordinate vector. It also rebuilds and verifies
both candidate populations before a source direction can be selected.

`source_flow::iteration::Factory` is the only effect boundary for preparing
the next projection. A factory receives the current certified snapshot and
must return a new `Projection`; its coordinate selection stays explicit and
outside the driver. A stale projection is rejected by the existing session
snapshot equality check before any state changes.

`source_flow::iteration::Driver` checks additive-half termination before every
factory request. On a nonterminal snapshot it requests one projection, applies
one selected direction through the existing certified Lemma 4.4 transition,
and records the pre-update snapshot, exact input, approximation certificate,
direction, and outcome. An explicit maximum update count returns
`IterationLimit` rather than making an unsupported termination claim. The
driver retains its terminal session, available through `session` or
`into_session`, for the already established local recovery handoff.

`Backend::begin_source_iterations` exposes this bounded driver without
changing `Backend::require_complete()`: the latter remains an explicit
`Incomplete` error and `an19_runtime_verified` remains false.

## Focused Evidence

- A prepared projection exposes one Theorem 4.3 certificate with one
  factor-two length and scaled-gradient check per circulation arc.
- A two-update run requests and records two separately snapshot-bound
  projections before returning its explicit iteration limit.
- Reusing the first projection after an accepted update returns
  `StaleCertifiedSnapshot`; the session retains exactly its first accepted
  state and one trace record.
- A snapshot already at additive-half termination completes without asking the
  factory for a projection, including when its allowed update count is zero.
- The public backend entry point starts that same terminal driver but still
  rejects `require_complete()`.

## Audit

Phase baseline: `ae7a626a401e92259661cff8f63cf6227207c123`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source candidate boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | driver and recovery boundary have no reference-flow fallback |
| `cargo test -p graph source_flow::iteration -- --nocapture` | 0 | 12 focused iteration tests passed |
| `cargo test -p graph source_flow -- --nocapture` | 0 | 21 source-flow tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace` | 0 | full workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | documentation built without warnings |
| `cargo build --workspace --release` | 0 | six workspace crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Remaining Boundary

P9.5e must drive this interface over compressed MRD instances and compare the
resulting matching, cover, chord flags, and rectangle decomposition with the
bounded permanent references. The projection factory must remain explicit and
snapshot-bound; it may not infer exact coordinates from `DyadicInterval`s or
use an Oracle to select, update, recover, or substitute production flow.

P9.3.2d remains deferred low-priority P9.6a proof debt. This work changes no
AN19 asymptotic claim and does not permit `AlmostLinear` or
`an19_runtime_verified: true`.
