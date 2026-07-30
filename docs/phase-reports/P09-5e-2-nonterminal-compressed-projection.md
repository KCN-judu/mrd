# P09.5e.2 - Nonterminal Compressed Projection Campaign

## Status

**State: complete for one supported nonterminal compressed fixture.**
Implementation commit: `58bf417`.

This subphase closes the rational-coordinate and nonterminal-driver gap left
by P9.5e.1. It does not establish additive-half termination, recover a
nonterminal flow, run a reference backend in production, complete the broad
chord/rectangle population, or enable `Backend::require_complete()`.

## Exact Coordinate Boundary

`source_min_ratio::input::Input::normalize_structure` is a pure structural
transformation. It computes the checked LCM of exact length denominators and
uniformly scales structural lengths and tree weights for the finite integral
tree/spanner construction. It preserves source IDs and endpoints. The original
`Input` remains the sole coordinate source for compact-cycle decoding,
candidate scoring, and the Theorem 4.3 approximation certificate.

The regression covers both a general rational input with denominators `2`,
`4`, and `6` (common scale `12`) and a full `SpannerSnapshot` with length
`1/2`. The latter proves that the finite chain uses the normalized length while
the candidate registry retains and scores the original `1/2` coordinate.

## Nonterminal Compressed Fixture

The applicable compressed fixture is an explicit one-by-one biclique: five
circulation arcs, unit outer and return capacities, and capacity two on the
two block arcs. Its strictly interior circulation assigns `1/4` to every arc,
has exact cost `-1/4`, and retains the known integral optimum `-1`; the
additive-half certificate rejects this state before a factory is invoked.

The test factory independently provides the exact rational source vector

```text
lengths:       11/4, 4, 8, 5, 8
gradients:     0, 0, 0, 0, -400/3
kappa:         1/2
```

These values are defined in test data rather than derived from a
`DyadicInterval`. `Projection::new` independently reruns the exact Theorem
4.3 checks for each arc. The structural scale is four, giving integral lengths
`11, 16, 32, 20, 32`; equal-length components are disjoint single-edge buckets
in the currently certified finite source-spanner subset.

The source driver accepts one nonzero compact-cycle direction and records its
complete projection/certificate/update record. With an explicit one-update
limit, it returns `SourceIteration(IterationLimit { maximum_iterations: 1 })`.
The retained successor still fails additive-half termination. This is the
required bounded nontermination witness; recovery is deliberately not called.

## Audit

Phase baseline: `bba146cb545b28ad4fb8735ef79fce307710d1a8`.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source tree-chain/core boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | source driver/recovery boundary has no reference-flow fallback |
| `cargo test -p graph source_min_ratio::input -- --nocapture` | 0 | 4 focused input tests passed |
| `cargo test -p graph source_min_ratio::spanner -- --nocapture` | 0 | 5 focused spanner tests passed |
| `cargo test -p graph source_flow -- --nocapture` | 0 | 21 source-flow tests passed |
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | 6 compressed source-flow tests passed |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace` | 0 | full workspace suite passed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` | 0 | workspace documentation built without warnings |
| `cargo build --workspace --release` | 0 | six workspace crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Remaining Boundary

P9.5e.3 must run a full combined compressed MRD driver/recovery campaign
across matching, cover, chord, and rectangle outputs. This report neither
upgrades the finite source-spanner domain nor claims general nonterminal
coverage. `Backend::require_complete()` remains unavailable.

P9.3.2d remains deferred low-priority P9.6a proof debt. The formal SIAM
source with DOI `10.1137/17M1115575` does not provide the required reduced
event ordering/counting proof. Nothing here permits the `AlmostLinear` name,
`an19_runtime_verified: true`, or an AN19 asymptotic runtime claim.
