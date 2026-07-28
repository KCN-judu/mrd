# P09.2.4 Initial, Termination, and Recovery Boundary

## Scope and issue matrix

| Field | Evidence |
| --- | --- |
| Classification | partial source-semantic implementation |
| Implemented | A certified midpoint initializer for the normalized zero-demand, zero-lower-bound model; the `200m log(mU)` initial-potential check; the `20m log(1/2)` additive-half boundary; and exact costed-flow rounding after that boundary. |
| Not implemented | CKLPPS22 Lemma 4.12's arbitrary-demand/lower-bound O(m)-edge augmentation and BLNPSSSW20 Lemma 8.10's independent random perturbation-cost construction and probability certificate. |
| Reason | The current checked network owns only integral capacities/costs and demands, with no source-compatible augmented-instance/mapping or perturbation seed contract. The restricted API refuses unsupported domains rather than silently treating a midpoint as a source initial point. |
| Status | In progress; the implemented boundary is usable and audited, but P9.2.4 cannot close until the source augmentation and perturbation interfaces are added. |

## Implemented contract

`CertifiedIpmSnapshot::initial_point_zero_demand` constructs `u_e/2` for every
positive-capacity arc, validates the resulting zero-demand circulation, and
certifies `Phi <= 200m log(mU)` under the configured fixed-point word bound.
`certify_additive_half_termination` proves both the potential boundary from
Lemma 4.1 and an enclosed objective gap at most one half. `recover_additive_half`
then invokes the permanent exact cycle-cancelling rounding Oracle and checks
that its integral cost equals the supplied integral `F*`.

No `f64`, unchecked transcendental, Dinic, Push--Relabel, or enumerating
minimum-ratio-cycle call is used by these checks.

## Evidence

- The two-edge zero-demand instance with capacity two accepts the midpoint
  initializer and certifies the initial bound.
- A quarter-flow with integral optimum zero certifies additive-half termination
  and rounds exactly to the zero-cost integral circulation.
- Unsupported empty/zero-capacity domains remain rejected by structured errors.

## Audit

Phase baseline: `f9a78302b7df825695d4a952edc37c192d6601ce`.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p rect-graph interior_point` | 0 | 8 focused tests passed |

The implementation remains explicitly below the source-complete P9.2.4 bar.
