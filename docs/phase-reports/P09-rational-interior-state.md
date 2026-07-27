# P09 Rational Interior State

## Scope

This P9 subphase adds an exact-rational, bounded-domain state machine for
checking potential-reduction experiments. It is not an almost-linear min-cost
flow backend and does not claim to instantiate the source's fixed-point IPM.

## Source Boundary

The relevant source is CKLPPS22, arXiv:2203.00671, Equation (9), Definition
4.2, Lemma 4.4, Theorem 4.3, and Lemma 4.6. The source potential uses
`log(c^T f - F*)` and `x^-alpha` for nonintegral alpha. Consequently it cannot
be evaluated as an exact rational number. The original P9 audit's phrase
"Theorem 4.6" was imprecise for this source version: Lemma 4.6 is the Taylor
bound for `log`, while Theorem 4.3 states the potential-reduction IPM.

`RationalInteriorPointState` deliberately uses the separately defined exact
surrogate

```text
(cost - objective_lower_bound) + barrier_weight * sum_e(1 / lower_slack_e + 1 / upper_slack_e)
```

It validates only what that formula proves: integral input-domain bounds,
strict rational feasibility, rational circulation updates, strict observed
surrogate-potential decrease, and exact iteration/changed-coordinate totals.
It never reports the surrogate as Equation (9), a source potential decrease,
or a near-linear iteration bound.

## Evidence

- `CirculationNetwork` now exposes exact rational objective, circulation,
  capacity-slack, and bounded-input validators used by the state machine.
- Input capacities, costs, and demands must lie within an explicit positive
  absolute bound before construction or update.
- Every accepted update has zero rational divergence, stays strictly inside
  every capacity interval, and strictly decreases the recomputed rational
  surrogate.
- The test accepts an exact decreasing two-edge update and records its two
  changed coordinates; it rejects an out-of-domain network and an update that
  raises the surrogate.

## Commands

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all` | 0 | formatted source |
| `cargo check -p rect-graph` | 0 | passed |
| `cargo clippy -p rect-graph --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test -p rect-graph interior_point -- --nocapture` | 0 | 2 passed, 36 filtered |
| `git diff --check` | 0 | no whitespace errors |

## Remaining P9 Gate

The source-grade fixed-point potential, its approximation/error bounds, its
initial-point construction, and its termination/recovery proof are still
unimplemented. This subphase supplies exact audit instrumentation only; it
does not close P9.
