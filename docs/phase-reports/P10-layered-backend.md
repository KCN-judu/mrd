# P10 - Layered Public Backend Architecture

## Status

**State: P10.1-P10.9 complete (solver mode, reference solver, source-with-target
solver, certificate verification, CLI/static audit, separated benchmark
evidence, and public-status audit).** Direct-grid parity is P11.
`Backend::require_complete()` stays `Error::Incomplete`; no AN19 runtime claim
is made.

Implementation commits: `e265e01`, `aa0a618`, `6905b93`, `ab11586`,
`332a5c5`.

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

## P10.8 - Benchmark and evidence separation

`mrd::layered::experiment` is a dedicated measurement namespace, and
`mrd benchmark --suite layered --output <path>` emits deterministic categories
instead of one opaque hybrid timing. The standard run records independent
polygon-derived measurements for:

- complete reference solving;
- formal-family geometry;
- compact dominance/circulation representation;
- recovery-only rectangle completion;
- independent dual-certificate verification.

The report also carries an explicit direct-grid `unavailable` row: P11 is the
only phase allowed to make a direct-grid measurement. This prevents a
polygon-derived result from being relabelled as direct-grid evidence.

Source rows are opt-in. `--source-target <integer>` records
`caller-supplied`; `--reference-provided-target` first measures a reference
solve and records `reference-exact` separately from the source call. Neither
mode is exposed by the source solver itself and neither performs automatic
`F*` search. A source failure is `source-undetermined`, never a fallback or an
infeasibility conclusion. Source targets serialize as decimal strings so every
valid `i128` target is preserved exactly in JSON.

Completed source runs obtain separate geometry, compressed-representation,
source-driver, recovery, verification, and total-hybrid timings from the
private `layered::execution` boundary. When a source run fails before a
completed result, only its total source-entry time is recorded; unexecuted
later stages stay absent rather than receiving invented timings.

The standard run was checked with a deliberately impossible caller target
`-85070591730234615865843651857942052864`. It produced a
`caller-supplied` / `source-undetermined` record, retained the exact decimal
target, and did not fall back. This validates reporting semantics only; it is
not source-backend success evidence and does not change P9.5e.3g.3.

## P10.9 - Documentation and release audit

The public README, architecture, algorithm, limitations, near-linear-flow, and
testing documents now use the exact public names and status boundaries:

- the CLI accepts `--backend reference|source-with-target`, not `source`;
- `solve_source_with_target(polygon, config)` obtains its inclusive target from
  `config.target`, rather than taking a separate target parameter;
- no document turns reference-provided benchmark input into automatic `F*`
  search, fallback, target infeasibility, `AlmostLinear`, or an AN19 runtime
  claim;
- P10 benchmark evidence is polygon-derived, while its explicit direct-grid
  `unavailable` record reserves direct-grid parity for P11.

This audit preserves the production/reference contract and documents the
source path as research-only, target-bound execution. The source target-search
and AN19-runtime proof obligations remain separate blockers.

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

P10.8 incremental checks at code commit `332a5c5` all exited `0`:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Rust formatting accepted |
| `cargo clippy -p mrd --all-targets --all-features -- -D warnings` | no warnings in the public/experiment boundary |
| `cargo test -p mrd` | 27 passed, 1 existing ignored |
| `python3 tools/check_source_flow_audit.py` | scans both the public layered API and private source execution module; no reference fallback or automatic target discovery |
| `mrd benchmark --suite layered --output <temporary file>` | five verified polygon-derived rows plus explicit direct-grid unavailable row |
| `mrd benchmark --suite layered --source-target -85070591730234615865843651857942052864 --output <temporary file>` | exact decimal target and honest `source-undetermined` outcome; no fallback |

The closeout audit reran the mandatory workspace gates after the code commit:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `python3 tools/check_biclique_bound.py` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed with no warnings |
| `cargo test --workspace` | 419 passed, 4 existing ignored; exit `0` (15 suites, 540.07s) |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | passed |
| `cargo build --workspace --release` | passed |
| `python3 tools/check_source_flow_audit.py` | passed |
| `python3 tools/check_release_consistency.py` | passed |
| `git diff --check` | passed |

P10.9 command-level documentation checks at the P10.8 closeout checkpoint all
exited `0`:

| Command | Result |
| --- | --- |
| `mrd solve --help` | exposes only `reference` and `source-with-target` backend values |
| `mrd benchmark --help` | exposes `layered`, `--source-target`, and `--reference-provided-target` |
| `mrd benchmark --suite layered --output <temporary file>` | five verified polygon-derived rows plus one direct-grid unavailable row |
| `mrd benchmark --suite layered --source-target -85070591730234615865843651857942052864 --output <temporary file>` | exact decimal target and `caller-supplied` / `source-undetermined`; no fallback |

The P10.9 documentation closeout then reran the complete workspace audit. All
commands exited `0`:

| Command | Result |
| --- | --- |
| `git diff --check` | no whitespace errors |
| `cargo fmt --all -- --check` | passed |
| `python3 tools/check_biclique_bound.py` | passed |
| `python3 tools/check_source_flow_audit.py` | passed |
| `python3 tools/check_source_lsst_audit.py` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed with no warnings |
| `cargo test --workspace` | 419 passed, 4 existing ignored (15 suites, 530.88s) |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | passed |
| `cargo build --workspace --release` | passed |
| `python3 tools/check_release_consistency.py` | passed |

## Remaining work

- P11 (renumbered): direct grid parity embedding.
- Automatic `F*` search remains blocked (P9.5e.3g.3); `Backend::require_complete()`
  remains `Error::Incomplete`; no AN19 runtime claim is made.
