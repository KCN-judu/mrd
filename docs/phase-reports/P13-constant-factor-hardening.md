# P13 - Constant-Factor Performance Hardening

## Status

**State: P13.1-P13.3 complete; P13.4 audited with closeout pending; P13.5
remains.** This phase starts with a reproducible baseline, not an optimization
claim. All timings below are local development-profile observations for the
exact 3x3 finite-grid population; they are only valid for comparison against
later runs with the same recorded environment and command.

## P13.1 - Reproducible Hot-Path Baseline

`mrd benchmark --suite direct-grid-parity --output <path>` now serializes phase
totals separately for `fully-audited` and `compact-only` direct and ranked
coordinate paths. Each mode covers 897 nonempty connected components generated
from the 511 nonzero 3x3 masks. The report requires 1,794 exact
direct-versus-ranked output/certificate comparisons before it succeeds.

The direct path retained zero rank sorts, rank-map entries, and rank-map-owned
bytes. The ranked Oracle recorded 3,588 sorts, 624 entries, and 18,240
estimated owned bytes. The local direct/ranked embedding totals were 607/4,553
microseconds. The full per-mode phase maps are intentionally machine-readable
in the benchmark JSON so later P13 subphases can compare geometry, embedding,
biclique, compressed-flow, completion, validation, and end-to-end-adjacent
phase totals independently.

This is a baseline only. It neither promotes the local timings to a portable
end-to-end result nor establishes a speedup outside the measured population.

## P13.2 - Geometry and Embedding Storage

`sg-oracle::grid::experiment::InteriorRuns` previously inserted every
canonical horizontal and vertical chord record into a `BTreeSet`. Its traversal
already walks ascending reflex coordinates, ascending interior runs, and
ascending aligned endpoint pairs; the records are therefore unique and
lexicographically canonical by construction. The implementation now emits
directly into `Vec`, removing per-record tree-node allocation and balancing
work while preserving the exact record order.

The reference `Pairwise` Oracle deliberately retains its `BTreeSet`. Existing
3x3 and 4x4 exhaustive chord-family differentials, P11's full embedding and
rectangle campaign, and the workspace audit all remain the acceptance gates.
The post-change single baseline observation was not used as a speed claim: its
microsecond variance is too small and environment-sensitive for that purpose.

This is a local storage/traversal reduction with an explicit permanent Oracle,
not a complexity-bound change.

## P13.3 - Biclique and Flow Layout

The compressed-flow layout previously allocated three vectors containing only
contiguous `FlowNodeId` values for horizontal endpoints, biclique blocks, and
vertical endpoints. The layout now stores the three starts and two side counts;
all node IDs are derived directly. This removes three allocations and their
stored ID words on every compressed-flow solve without changing node numbering
or arc insertion order.

P11's exact network snapshot differential verifies the materialized node/source/
sink/ordered-arc topology, flow, cut, cover, and rectangles under both
coordinate backends. A new compressed-flow regression additionally proves an
out-of-bounds biclique endpoint still returns `BicliqueEndpointOutOfBounds`
before flow execution. The local baseline command remained correct; its timing
is retained as an observation only, not a portable speed claim.

## P13.4 - Deterministic Execution Policy

`verification::execution` now owns the only P13 component scheduler. Its
`ComponentExecutionPolicy` is explicit: one requested worker is the serial
path, while any larger positive count creates a fixed-size scoped-worker pool.
The main thread owns all input/output effects and returns completed component
results in original component order. It also returns the first failure in that
order, even when a later task finished first.

The implementation uses bounded synchronous task and result channels plus a
canonical `BTreeMap` reorder buffer. At most the chosen worker count is
submitted without a result, and at most that count can remain in the reorder
buffer; the report serializes the observed maxima. The requested output vector
remains proportional to the component count by contract. There is no automatic
hardware-concurrency selection.

`mrd verify --component-workers <positive>` exposes this policy for grid input
only. Polygon and formal-polygon verification reject a parallel worker request,
and solve/benchmark paths remain sequential. The serial-versus-two-worker grid
differential compares component order, every solver optimum, all rectangles,
and complete certificates; time measurements are intentionally excluded because
they are locally nondeterministic instrumentation rather than solver semantics.
The generic scheduler also regresses output ordering, earliest-input failure
selection, the worker/reorder bounds, and zero-worker rejection.

The local CLI observation on `test-data/example.json` with two components and
`--component-workers 2` recorded `deterministic-parallel`, two maximum
in-flight components, and one maximum reorder-buffered component. The latter
is a local scheduling observation; the contract is the worker-count upper
bound, not a fixed reorder count. This is an execution-boundary check only,
not a throughput or speedup claim.

## Audit

Phase baseline: `3bcf4a284d947f1d2cce015d79711135fc9daaa1`.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | passed |
| `cargo test -p verification` | passed |
| `cargo clippy -p verification -p mrd --all-targets --all-features -- -D warnings` | passed |
| `cargo run -p mrd -- benchmark --suite direct-grid-parity --output <temporary>.json` | 897 components, 1,794 comparisons, zero mismatches/errors, and per-mode phase maps |
| `cargo test -p sg-oracle` | passed, including exhaustive 3x3/4x4 chord-family and completion differentials |
| `cargo test -p dominance compressed_flow` | passed, including invalid biclique endpoint rejection |
| `cargo test -p verification execution::` | passed: canonical output/failure order and bounded scheduler counters |
| `cargo test -p verification grid::` | passed: serial-versus-bounded-parallel grid semantic differential |
| `cargo test -p mrd verify_cli_exposes_explicit_component_worker_bound` | passed: explicit CLI worker policy |
| `cargo clippy -p verification -p mrd --all-targets --all-features -- -D warnings` | passed |
| `python3 tools/check_biclique_bound.py` | passed |
| `python3 tools/check_source_flow_audit.py` | passed |
| `python3 tools/check_release_consistency.py` | passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | passed |
| `cargo test --workspace` | passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | passed |
| `cargo build --workspace --release` | passed |

P13.3's complete workspace audit passed before implementation commit `d5fcd67`
was pushed. P13.4 then reran `git diff --check`, format checking, the
component-policy regressions, the 3x3 direct-grid differential, both repository
audit scripts, workspace clippy/test/doc/release build, and release-consistency
checking. Every command returned zero. The direct-grid benchmark again recorded
897 components, 1,794 comparisons, zero mismatches/errors, and zero direct
rank counters. P13.4 has no performance claim beyond its bounded execution
contract; P13.5 remains responsible for consolidated release evidence.
