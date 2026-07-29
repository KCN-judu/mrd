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

P9.5 remains open. Source direction selection, lower-bound and augmentation
recovery integration, MRD compressed-network differential evidence, and an
end-to-end no-fallback audit are still absent. `Backend::require_complete()`
therefore continues to reject execution and `an19_runtime_verified` remains
false. P9.3.2d's missing AN19 proof is a separate low-priority debt: it does
not block these semantic integration tasks, but must be closed in P9.6a before
an `AlmostLinear` name or AN19 runtime claim.

## Incremental audit

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | no production recovery or reference-flow fallback dependency |
| `cargo test -p graph source_flow` | 0 | 5 focused tests passed, including recovery differential and supplied-step accounting |
