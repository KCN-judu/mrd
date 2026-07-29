# P09.5 - Source-Shaped Flow Integration Gap

The certified IPM exposes public snapshots, update metrics, additive-half
termination, and recovery APIs. Inspection established that the older
snapshot-recovery route invokes the permanent exact rounding implementation.
The residual refinement experiment likewise depends on an enumerating
residual-cycle implementation. Neither may be called by `source_flow::Backend`.

Commit `094a289` adds `source_flow::recovery::round`, an independent,
deterministic exact fractional-cycle cancellation. `Backend::recover_terminated`
first certifies the additive-half boundary, invokes only that local recovery,
checks equality with the snapshot's retained integral optimum, and validates
the final integral circulation. It retains the complete cancellation trace.
The test-only differential compares the complete trace on a shared fractional
cycle with the permanent recovery implementation; production code has no such
dependency. `tools/check_source_flow_audit.py` rejects reference max-flow,
enumerating min-cost, legacy snapshot-recovery, and dynamic-cycle dependencies
from the production P9.5 boundary.

Commit `b34be66` adds `source_flow::iteration::Session`. It owns a certified
snapshot and its `IpmDetectLedger`; an immutable `Step` supplies exact
direction, approximations, and `kappa`. The session applies the existing
Lemma 4.4 transition, records the accepted step, and exposes exact Detect
threshold accounting. This connects update semantics to recovery without
using a reference flow path. It deliberately does not select `Step.direction`:
the P9.4 query boundary validates candidate cycles but does not construct the
minimum-ratio update required by the source iteration.

Commit `1a95a59` adds `Step::from_compact_candidate`. It calls the P9.4 query
boundary only to decode an externally selected compact cycle, then sums signed
arc occurrences into a complete exact direction vector and revalidates its
circulation. This binds compact source-tree semantics to the P9.5 step input
without selecting a candidate or using an enumerating cycle implementation.

Commit `8d7975b` adds source-flow terminal recovery through both initial-point
augmentation and lower-bound normalization. It first recovers the augmented
integral circulation, rejects any surviving artificial arc, restores the
normalized network, then restores original lower bounds and objective offset.
The focused lower-bound fixture includes a fixed edge, a negative lower bound,
and artificial root arcs; all are checked without a reference recovery path.

Commit `6179f22` removes an implicit residual-cycle Oracle from this recovery
boundary. `source_flow` now uses a narrow exact feasibility-and-objective
validator after the additive-half certificate and exact snapshot-optimum
equality establish optimality. Augmentation and lower-bound restoration use
corresponding feasibility-only maps, while the permanent full optimality
validators remain available for reference code. The static audit now rejects a
direct optimality-validator call from production `source_flow`; a regression
test demonstrates that feasibility validation does not itself claim
optimality.

P9.5 remains open. Source candidate selection, MRD compressed-network
differential evidence, and an end-to-end no-fallback audit are still absent.
`Backend::require_complete()` therefore continues to reject execution and
`an19_runtime_verified` remains false. P9.3.2d's missing AN19 proof is a
separate low-priority debt: it does not block these semantic integration tasks,
but must be closed in P9.6a before an `AlmostLinear` name or AN19 runtime
claim.

## Incremental audit

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact-biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source minimum-ratio no-fallback boundary accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | no production recovery or reference-flow fallback dependency |
| `cargo test -p graph source_flow` | 0 | 8 focused tests passed, including augmented and lower-bound recovery |
| `cargo test -p graph` | 0 | 164 graph tests passed, including feasibility-versus-optimality regression |
| `cargo clippy -p graph --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace` | 0 | workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | documentation built without warnings |
| `cargo build --workspace --release` | 0 | six crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |
