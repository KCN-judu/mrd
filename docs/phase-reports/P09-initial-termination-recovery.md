# P09.2.4 Initial, Termination, and Recovery Boundary

## Scope and issue matrix

| Field | Evidence |
| --- | --- |
| Classification | source-semantic implementation |
| Implemented | Exact nonzero-lower-bound normalization and recovery; CKLPPS22 Appendix B.1 root augmentation for arbitrary demands; initial-potential and additive-half certificates; Lemma 4.11 isolation perturbation with exact source constants; nearest-integer recovery; KP15 costed rounding; and P7 exact verification. |
| Randomness boundary | The deterministic constructor validates a realized rank vector in `1..=2mU`. The theorem's probability `>=1/2` is recorded only under the explicit external assumption that ranks were sampled independently and uniformly. No pseudorandom seed is represented as mathematical independence. |
| Status | Complete for the current integral min-cost circulation model. Source-grade dynamic cycle selection and running-time claims remain P9.3--P9.6. |

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

`LowerBoundCirculationNetwork` applies `x_e=f_e-u^-_e`, shifts both endpoint
demands, records the exact objective offset, removes fixed-flow arcs, and
verifies the inverse mapping. `IsolationPerturbation` implements the TeX source
constants exactly: ranks in `1..=2mU`, denominator `4m^2U^2`, scaled integral
costs, original tolerance `1/(12m^3U^3)`, and scaled tolerance `1/(3mU)`.
Coordinatewise nearest-integer recovery is accepted only after the P7 exact
Oracle verifies the original integral optimum.

No `f64`, unchecked transcendental, Dinic, Push--Relabel, or enumerating
minimum-ratio-cycle call is used by these checks.

## Evidence

- Zero-demand and nonzero-demand two-edge instances accept the midpoint/root
  initializer and certify the initial bound.
- The nonzero-demand augmented optimum uses no artificial edge and recovers
  the original exact optimum after truncation.
- A negative-lower-bound fixture verifies demand shifting, exact objective
  offset, fixed-flow arc elimination, source initialization, and inverse
  recovery.
- A realized two-edge isolation perturbation verifies the exact denominator,
  support, scaled tolerance, near-flow rounding, structured invalid-rank
  rejection, and exact optimality certificate.
- A quarter-flow with integral optimum zero certifies additive-half termination
  and rounds exactly to the zero-cost integral circulation.
- Unsupported empty/zero-capacity domains remain rejected by structured errors.

## Audit

Phase baseline: `f9a78302b7df825695d4a952edc37c192d6601ce`.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `git diff --check` | 0 | no whitespace errors |
| `cargo test -p rect-graph interior_point` | 0 | 11 focused tests passed |
| `cargo test -p rect-graph min_cost` | 0 | 16 focused tests passed |
| `python3 tools/check_biclique_bound.py` | 0 | passed |
| `cargo test --workspace` | 0 | all workspace suites passed |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | passed |
| `cargo build --workspace --release` | 0 | passed |
| `python3 tools/check_release_consistency.py` | 0 | baseline and 30 reachable manifest commits verified |

No almost-linear backend or running-time claim is introduced by this subphase.
