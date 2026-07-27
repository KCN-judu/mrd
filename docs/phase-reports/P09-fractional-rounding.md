# P09 Fractional Flow Rounding

## Scope

This is the first corrective subphase of P9, following the failed source gate
in `P09-integration-gate-audit.md`. It implements an exact rational
feasible-flow representation and a deterministic costed flow-rounding Oracle.
It does not implement an interior-point method, a dynamic minimum-ratio-cycle
query, a low-stretch construction, or an almost-linear backend.

## Source Contract

Kang and Payor, *Flow Rounding* (arXiv:1507.08139), Section 3 establishes:

- with integral capacities, a fractional circulation whose fractional-edge
  subgraph has no cycle is integral;
- cancelling a fractional cycle by its directional availability reaches an
  integral edge without violating capacities; and
- for costed rounding, one of the two directions has non-increasing cost.

`CirculationNetwork::round_fractional_costed` is a deterministic, exact
implementation of that reduction. It scans fractional arcs in stable arc-ID
order, uses BFS over previously scanned fractional arcs to obtain a cycle, and
chooses the non-increasing-cost direction (the forward direction on a zero
cost tie). The search is an audit Oracle and intentionally has no near-linear
complexity claim.

## Delivered Evidence

- `FractionalCirculation` stores exact reduced `ExactRatio` coordinates and
  exact rational cost.
- `verify_fractional_solution` checks dimension, rational capacity bounds,
  exact node balances, and exact objective equality.
- Every rounding step records its signed cycle, rational augmentation, and
  before/after exact costs. The implementation revalidates fractional
  feasibility after every update and rejects a stalled fractional state.
- The differential test enumerates all quarter-integral circulations on a
  two-arc cycle for every pair of costs in `[-2, 2]`; it proves each result is
  integral and feasible and has cost no greater than the fractional input.
- A three-edge example exercises the strictly decreasing choice, and an
  invalid nonconserving fractional input is rejected.

## Commands

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all` | 0 | formatted source |
| `cargo check -p rect-graph` | 0 | type and borrow checks passed |
| `cargo clippy -p rect-graph --all-targets -- -D warnings` | 0 | no warnings |
| `cargo test -p rect-graph costed_rounding -- --nocapture` | 0 | 2 passed, 34 filtered |
| `cargo test -p rect-graph fractional -- --nocapture` | 0 | 2 passed, 34 filtered |
| `git diff --check` | 0 | no whitespace errors |

The full P9 release gate has not run and P9 is not complete. Existing
concurrent `rect_graph` full-test processes from prior sessions are not used as
evidence for this subphase.

## Remaining P9 Gates

1. Add bounded-domain exact IPM state and Theorem 4.6 potential/update
   accounting, retaining this rounding Oracle for integral recovery.
2. Replace replay-only P8 querying with a source-matching approximate dynamic
   minimum-ratio-cycle query and auditable counters.
3. Integrate the selected backend with MRD compressed-flow parity and run the
   full P9 audit before any almost-linear claim is considered.
