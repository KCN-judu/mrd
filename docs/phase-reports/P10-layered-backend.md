# P10 - Layered Public Backend Architecture

## Status

**State: P10.1-P10.4 complete (solver mode, reference solver, source-with-target
solver, certificate verification) with CLI integration (P10.5) and static
audit coverage (P10.6).** Direct-grid parity (P11), benchmark separation
(P10.8), and documentation closeout (P10.9) remain. `Backend::require_complete()`
stays `Error::Incomplete`; no AN19 runtime claim is made.

Implementation commits: `e265e01`, `aa0a618`, `6905b93`, `ab11586`.

## P10.1 - Solver mode and provenance model

`crates/mrd/src/layered.rs` defines the explicit layered model:

- `SolverMode { Reference, SourceWithTarget { target, source_config } }` - no
  `AutomaticSource` variant exists.
- `SolverProvenance { ReferenceExact, SourceCertifiedAtMost { target } }` -
  recorded on every result.
- `SourceConfig`, `FixedPointConfigSpec`, `RatioSpec` - serialization-friendly
  source parameters.
- `LayeredResult` - a stable result model with objective, matching, cover,
  selected chords, rectangles, provenance, verification summary, and a target
  certificate.
- `LayeredError` - separates `UnsupportedOrUndetermined` from `Source` and
  `Reference` failures.

A unit test `solver_mode_has_no_automatic_source_variant` asserts there is no
automatic-source constructor.

## P10.2 - Reference-backed public solver

`solve_reference(polygon)` runs the permanent formal-polygon reference path
(`complete_formal_polygon`) and returns a `LayeredResult` with
`SolverProvenance::ReferenceExact`, exact matching/cover/chords/rectangles, and
a fully verified summary. Tested on the Figure 3 fixture, including
deterministic serialization with provenance.

## P10.3 - Source-with-target public solver

`solve_source_with_target(polygon, config)` runs only the source-shaped
production path under a caller-supplied inclusive target: it builds the
compressed `Circulation` from the formal family partition, runs the Appendix
B.1 `begin_with_target` / `TargetDriver` path, recovers the original flow and
matching/cover, selects chords, and completes rectangles. A certified result is
returned only when the recovered original cost is at most the target; any other
source failure is reported as an explicit `Source` or
`UnsupportedOrUndetermined` error and is never classified as target
infeasibility.

The positive path on the full Figure 3 fixture is slow (the Appendix B.1
initial point is far from terminal), so the corresponding unit test is
`#[ignore]`d and the fast honest-failure contract test is active.

## P10.4 - Certificate verification

- `verify_source_infeasible_below(network, target, dual)` verifies a
  `DualLowerBoundCertificate` exactly and requires a strictly greater dual
  objective.
- `verify_cover_below(circulation, cover, target)` verifies a compressed
  `CoverBelowProof`.
- `verify_source_feasible_at_most(network, solution, target)` verifies exact
  feasibility and `cost <= target`.
- `CirculationNetworkSpec` and `DualLowerBoundCertificateJson` provide
  deterministic serialization.

## P10.5 - CLI integration

- `mrd solve --backend reference|source-with-target --target <integer>`.
  Reference is the default and requires no target. Source mode requires
  `--target`, supports formal-polygon input only, never silently falls back to
  a reference backend, and reports `UnsupportedOrUndetermined` honestly.
- `mrd verify-negative-certificate --network <json> --certificate <json>
  --target <integer>` verifies a serialized dual lower-bound certificate.

CLI tests cover default reference, source-with-target target parsing, missing
target rejection, explicit grid-input unsupported, and certificate
verification success and strict-bound rejection.

## P10.6 - Static audit

`tools/check_source_flow_audit.py` now also scans `crates/mrd/src/layered.rs`:
it requires the layered API surface (`SolverMode`, `SolverProvenance`,
`LayeredResult`, `solve_reference`, `solve_source_with_target`,
`verify_source_infeasible_below`, `UnsupportedOrUndetermined`, provenance
fields) and forbids an `AutomaticSource` mode, an automatic `solve_source`
entry, and binary-search wrappers.

## Audit

Phase baseline: `c5c0e687ac6693a3f85ecaaea7f0fa27818930e0`. The following
commands exit `0`.

| Command | Result |
| --- | --- |
| `git status --short` | clean after commits |
| `git diff --check` | no whitespace errors |
| `cargo fmt --all -- --check` | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | compact biclique bound accepted |
| `python3 tools/check_source_flow_audit.py` | layered API + no-fallback + no-automatic-mode audit accepted |
| `python3 tools/check_source_lsst_audit.py` | low-stretch-tree audit accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | no workspace warnings |
| `cargo test --workspace` | full suite passes |
| `env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | documentation built without warnings |
| `cargo build --workspace --release` | release build passed |
| `python3 tools/check_release_consistency.py` | release provenance accepted |

## Remaining work

- P11 (renumbered): direct grid parity embedding.
- P10.8: benchmark and evidence separation (reference-provided-target labels,
  separate target-provider/source/recovery/verification/total timings).
- P10.9: documentation closeout across README, ARCHITECTURE, ALGORITHMS,
  KNOWN_LIMITATIONS, NEAR_LINEAR_FLOW_IMPLEMENTATION, and TESTING.
- Automatic `F*` search remains blocked (P9.5e.3g.3); `Backend::require_complete()`
  remains `Error::Incomplete`; no AN19 runtime claim is made.
