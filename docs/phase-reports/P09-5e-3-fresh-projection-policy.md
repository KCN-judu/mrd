# P09.5e.3 - Fresh Compressed Projection Policy Evidence

## Status

**State: in progress.** Implementation commit: `45849b1`.

This report records one bounded, nonterminal part of the P9.5e.3 campaign. It
does not close P9.5e.3, P9.5, or `Backend::require_complete()`.

## Issue Matrix

| Issue | Observed | Required contract | Evidence and acceptance |
| --- | --- | --- | --- |
| Fresh state after an accepted source update | P9.5e.2 constructed one projection and stopped at one update, so it did not demonstrate preparation for a successor snapshot. | `Driver` must request and independently certify a new `Projection` for each nonterminal snapshot before selection. | The one-by-one compressed circulation accepts two updates; its factory prepares two separately snapshot-bound projections, and the two stored records carry unequal certified snapshots. |
| Bounded witness versus termination | The two supported updates do not reach the additive-half boundary. | An iteration limit must remain a failure result, and recovery must not run for a nonterminal successor. | `Circulation::run_source` returns `IterationLimit { maximum_iterations: 2 }`; the final snapshot still rejects additive-half certification. |
| Backend completeness | Terminal recovery and one bounded nonterminal path exist, but no general policy prepares a terminating projection sequence for all supported compressed inputs. | Do not enable a complete backend from bounded evidence. | `Backend::require_complete()` remains `Error::Incomplete`; its no-fallback static audit still passes. |

## Contract and Result

The test-only factory uses the existing exact `1 x 1` compressed biclique
fixture. Its strictly interior initial circulation is nonterminal. For each
factory request it rebuilds a `TerminalTree`, `SpannerSnapshot`, and
`source_flow::iteration::Projection` from the current
`CertifiedIpmSnapshot`, the externally supplied exact rational coordinates,
the checked ledger, and `kappa = 1/2`. `Projection::new` reruns the Theorem
4.3 approximation certificate before the driver selects its direction.

With `maximum_iterations = 2`, both source-selected transitions are accepted.
The factory is called twice, the records are numbered `0` and `1`, their input
snapshot values differ, and both certificates cover every circulation arc. The
driver then returns the explicit iteration-limit error. The successor still
fails `certify_additive_half_termination`, so `run_source` never enters the
terminal matching/cover recovery path.

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
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | 7 focused compressed-source tests passed |
| `cargo test -p graph source_flow -- --nocapture` | 0 | 21 focused source-flow tests passed |
| `python3 tools/check_source_flow_audit.py` | 0 | no reference-flow or recovery fallback dependency |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source tree-chain/core boundary has no Oracle fallback |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace --quiet` | 0 | full workspace suite passed |
| `env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | workspace documentation built without warnings |
| `cargo build --workspace --release` | 0 | release build passed |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

## Remaining Boundary

This is a fresh-projection reconstruction witness, not a general terminating
projection policy. P9.5e.3 still needs a source-supported policy that can
prepare every nonterminal snapshot it claims can reach additive-half
termination, plus combined compressed-MRD differentials for flow, matching,
cover, chord flags, and rectangle decomposition over a broader supported
population. Until that semantic and no-fallback campaign passes,
`Backend::require_complete()` remains unavailable.

P9.3.2d remains separate low-priority P9.6a proof debt. The formal SIAM source
with DOI `10.1137/17M1115575` does not provide the required reduced-event
ordering/counting proof. This bounded campaign makes no `AlmostLinear`,
`an19_runtime_verified: true`, or AN19 asymptotic-runtime claim.
