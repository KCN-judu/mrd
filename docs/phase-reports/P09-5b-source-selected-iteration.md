# P09.5b - Source-Selected Certified Iteration

## Status

**State: complete as one exact source-selected transition.** Implementation
commit: `4043a85`.

This subphase joins the completed immutable terminal/core candidate selector to
the certified IPM transition. It is not a complete backend driver and does not
change `Backend::require_complete()` or `an19_runtime_verified: false`.

## Contract

`source_flow::iteration::SourceSelected` is an explicit command carrying:

- the current `CertifiedIpmSnapshot`;
- the exact current `source_min_ratio::input::Input`;
- the checked `StableMinRatioLedger`;
- matching immutable terminal and rejected-core snapshots; and
- the exact source parameter `kappa`.

`Session::apply_source_selected` first requires the command's certified
snapshot to equal the session's current snapshot. It then requires the supplied
`Input` to exactly equal both maintained candidate inputs. Exact gradient and
length vectors are copied only from that `Input`; no rational coordinate is
derived from a fixed-point interval. The existing complete-population selector
chooses and decodes the winning source declaration, and the existing
`Session::apply` path re-certifies the approximation, records Detect accounting,
and commits the successor only after all checks pass.

The operation rejects a stale snapshot, unequal input, absent nonzero source
candidate, or any existing selection/IPM error before mutating the session.
`SelectedOutcome` retains both the exact selected `Step` and the certified
transition outcome for auditability.

## Focused Evidence

`cargo test -p graph source_flow::iteration` passes 8 tests. The new finite
directed simple-graph fixture has a strict interior circulation and supported
finite core/spanner state. It verifies one source-selected certified update,
including the exact source step size. A second regression reuses the old source
command after the session advances and supplies an unequal `Input`; both calls
reject and leave the relevant session snapshot unchanged.

`tools/check_source_flow_audit.py` now requires the source-selected command,
entry point, and stale-snapshot rejection while continuing to reject reference
flow, min-cost, dynamic-min-ratio, and rounding-oracle dependencies from the
production source-flow modules.

## Audit

Phase baseline: `9be04dc87c93b199ceff9604920b3ec1e8d9ab96`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | finite source candidate boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | source-selected path has no reference-flow or recovery fallback |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p graph source_flow::iteration` | 0 | 8 focused tests passed |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | documentation built without warnings |
| `cargo build --workspace --release` | 0 | six workspace crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Remaining Boundary

The next source projection must be explicitly prepared from the next certified
snapshot; this subphase does not infer it from intervals or maintain a general
dynamic candidate structure. P9.5 still needs a complete termination/recovery
driver and the full compressed MRD flow, cut, cover, chord, and rectangle
differential campaign. Until those semantic and no-fallback gates pass,
`Backend::require_complete()` continues to reject execution.

P9.3.2d remains separate low-priority P9.6a proof debt. DOI
`10.1137/17M1115575` does not provide the reduced-event ordering/counting
conversion; this does not block P9.5 semantics, but it continues to prohibit
the `AlmostLinear` name, `an19_runtime_verified: true`, and any AN19 runtime
claim.
