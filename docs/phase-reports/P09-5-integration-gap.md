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

This narrows the recovery gap but does not close P9.5. There is still no
certified iterative producer for the terminal snapshot, no lower-bound or
augmentation recovery integration, no MRD compressed-network differential,
and no end-to-end no-fallback audit. `Backend::require_complete()` therefore
continues to reject execution and `an19_runtime_verified` remains false.
P9.3.2d's missing AN19 proof is a separate low-priority debt: it does not
block these semantic integration tasks, but must be closed in P9.6a before an
`AlmostLinear` name or AN19 runtime claim.

## Incremental audit

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | no production recovery or reference-flow fallback dependency |
| `cargo test -p graph source_flow` | 0 | 4 focused tests passed, including the test-only exact recovery differential |
