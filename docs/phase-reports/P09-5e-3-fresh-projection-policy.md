# P09.5e.3 - Fresh Compressed Projection Policy Evidence

## Status

**State: in progress.** Implementation commits: `45849b1`, `c902c37`,
`5391ada`, `20f8a18`.

This report records one bounded, nonterminal part of the P9.5e.3 campaign. It
does not close P9.5e.3, P9.5, or `Backend::require_complete()`.

## Issue Matrix

| Issue | Observed | Required contract | Evidence and acceptance |
| --- | --- | --- | --- |
| Fresh state after an accepted source update | P9.5e.2 constructed one projection and stopped at one update, so it did not demonstrate preparation for a successor snapshot. | `Driver` must request and independently certify a new `Projection` for each nonterminal snapshot before selection. | `FixedProjectionFactory` rebuilds two separately snapshot-bound projections for the one-by-one compressed circulation, and the two stored records carry unequal certified snapshots. |
| Exact coordinate update and reuse | `FixedProjectionFactory` can recertify unchanged coordinates only until they become stale. | A finite source fixture may supply distinct exact coordinates per successor snapshot, but it must consume them in order, reject exhaustion, and certify each entry before selection. | `ScheduledProjectionFactory` owns a caller-supplied immutable sequence of identity-compatible `Input` values. The compressed `1 x 1` fixture uses distinct first and successor coordinates, consumes both, and returns the explicit iteration limit instead of reusing either. |
| Bounded witness versus termination | The two supported updates do not reach the additive-half boundary. | An iteration limit must remain a failure result, and recovery must not run for a nonterminal successor. | `Circulation::run_source` returns `IterationLimit { maximum_iterations: 2 }`; the final snapshot still rejects additive-half certification. |
| Backend completeness | Terminal recovery and one bounded nonterminal path exist, but no general policy prepares a terminating projection sequence for all supported compressed inputs. | Do not enable a complete backend from bounded evidence. | `Backend::require_complete()` remains `Error::Incomplete`; its no-fallback static audit still passes. |

## Contract and Result

`source_flow::iteration::FixedProjectionFactory` is the production
implementation of this restricted policy. It owns externally supplied,
immutable exact `Input` coordinates, a checked stable ledger, finite spanner
parameters, and `kappa`; it never derives values from fixed-point intervals.
For every request it rebuilds a `TerminalTree`, `SpannerSnapshot`, and
`source_flow::iteration::Projection` for the current
`CertifiedIpmSnapshot`. `Projection::new` reruns the Theorem 4.3
approximation certificate before the driver selects its direction. If the
unchanged exact coordinates no longer certify the new snapshot, preparation
fails explicitly before any new source selection or state mutation.

The exact `1 x 1` compressed biclique fixture supplies the policy's immutable
coordinates. Its strictly interior initial circulation is nonterminal.

With `maximum_iterations = 2`, both source-selected transitions are accepted.
The factory records two successful preparations, the records are numbered `0`
and `1`, their input snapshot values differ, and both certificates cover every
circulation arc. The driver then returns the explicit iteration-limit error.
The successor still fails `certify_additive_half_termination`, so `run_source`
never enters the terminal matching/cover recovery path.

A second bounded source-flow regression probes the policy beyond those two
updates. On the existing 5-node exact source fixture, fixed coordinates certify
25 successive preparations and updates. The following preparation rejects with
`GradientApproximation { edge: 0 }` before a 26th source selection. The driver
retains exactly 25 records and the policy's preparation counter is 25. This
proves a transparent coordinate-staleness boundary for the fixed policy; it is
not a bound on the number of valid updates for arbitrary inputs.

`ScheduledProjectionFactory` now supplies the complementary finite policy. It
accepts a nonempty, caller-owned sequence of immutable exact `Input` values
with one stable source/circulation identity, rebuilds and certifies a projection
from exactly one entry per request, and advances only after that preparation
succeeds. An empty schedule, an identity mismatch, or exhaustion rejects
explicitly; neither an older coordinate set nor a fixed-point interval endpoint
can be substituted. The compressed `1 x 1` fixture supplies a literal successor
return-arc gradient of `-399999997/3000000` rather than reusing its initial
`-400/3`. Both entries certify their distinct successive snapshots and lead to
two accepted source updates before the deliberate iteration limit. The final
snapshot remains nonterminal and recovery is not called.

The exact source coordinates remain the P9.5e.2 fixture values:

```text
lengths:  11/4, 4, 8, 5, 8
gradients: 0, 0, 0, 0, -400/3
kappa: 1/2
```

The structural normalization is still used only by the finite source-tree and
spanner construction. Candidate scoring and Theorem 4.3 certification retain
the unscaled rational coordinates.

## Verification

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | 8 focused compressed-source tests passed |
| `cargo test -p graph source_flow -- --nocapture` | 0 | 24 focused source-flow tests passed |
| `python3 tools/check_source_flow_audit.py` | 0 | no reference-flow or recovery fallback dependency |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source tree-chain/core boundary has no Oracle fallback |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace --quiet` | 0 | full workspace suite passed |
| `env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | workspace documentation built without warnings |
| `cargo build --workspace --release` | 0 | release build passed |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Remaining Boundary

`FixedProjectionFactory` is a fresh-projection reconstruction policy, not a
general dynamic coordinate-maintenance or termination policy. It supports only
snapshots for which its caller-supplied fixed coordinates, ledger, and finite
source structures continue to certify; the 25-update source-flow regression
shows the exact coordinate-staleness failure when that condition ends.

`ScheduledProjectionFactory` proves only that a finite, independently supplied
coordinate trace can be consumed without stale reuse. It is not the missing
source-supported construction of that trace and it does not prove that any
nonterminal compressed instance reaches additive-half termination. P9.5e.3
still needs a source-supported policy that updates exact coordinates for every
nonterminal snapshot it claims can reach additive-half termination, plus
combined compressed-MRD differentials for flow, matching, cover, chord flags,
and rectangle decomposition over a broader supported population. Until that
semantic and no-fallback campaign passes, `Backend::require_complete()` remains
unavailable.

P9.3.2d remains separate low-priority P9.6a proof debt. The formal SIAM source
with DOI `10.1137/17M1115575` does not provide the required reduced-event
ordering/counting proof. This bounded campaign makes no `AlmostLinear`,
`an19_runtime_verified: true`, or AN19 asymptotic-runtime claim.
