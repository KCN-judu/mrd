# P09.2.4 Initial, Termination, and Recovery Boundary

## Scope and issue matrix

| Field | Evidence |
| --- | --- |
| Classification | partial source-semantic implementation |
| Implemented | CKLPPS22 Appendix B.1's root-vertex augmentation for arbitrary demands in the normalized zero-lower-bound model; the `200m log(mU)` initial-potential check; the `20m log(1/2)` additive-half boundary; exact costed-flow rounding; and recovery from a verified augmented optimum. |
| Not implemented | General nonzero-lower-bound shifting and BLNPSSSW20 Lemma 8.10's independent random perturbation-cost construction and probability certificate. |
| Reason | The current checked network is normalized to lower bound zero and has no perturbation seed/probability contract. Unsupported domains remain explicit rather than being silently treated as source-complete. |
| Status | In progress; the implemented boundary is usable and audited, but P9.2.4 cannot close until the source augmentation and perturbation interfaces are added. |

## Implemented contract

`CirculationNetwork::initial_point_augmentation` adds one root vertex and at
most one root arc per original vertex, sets every original arc to `u_e/2`,
routes the remaining exact imbalance at half artificial capacity, and assigns
artificial cost `4mU^2`. The augmented optimum is validated before truncation;
any positive artificial flow produces an infeasibility result. The certified
snapshot then proves `Phi <= 200m log(mU)` under the configured word bound.
`certify_additive_half_termination` proves both the potential boundary from
Lemma 4.1 and an enclosed objective gap at most one half. `recover_additive_half`
then invokes the permanent exact cycle-cancelling rounding Oracle and checks
that its integral cost equals the supplied integral `F*`.

No `f64`, unchecked transcendental, Dinic, Push--Relabel, or enumerating
minimum-ratio-cycle call is used by these checks.

## Evidence

- Zero-demand and nonzero-demand two-edge instances accept the midpoint/root
  initializer and certify the initial bound.
- The nonzero-demand augmented optimum uses no artificial edge and recovers
  the original exact optimum after truncation.
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

The implementation remains below the source-complete P9.2.4 bar until lower
bounds and the Lemma 4.11 perturbation contract are represented.
