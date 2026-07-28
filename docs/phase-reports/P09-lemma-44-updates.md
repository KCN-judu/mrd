# P09.2.3 Certified Lemma 4.4 Updates

## Scope and issue matrix

| Field | Evidence |
| --- | --- |
| Classification | source-semantic implementation gap |
| Observed | P9.2.2 certified Equation (9) quantities and approximation hypotheses, but no transition could update the flow or account for Detect. |
| Expected | CKLPPS22 Theorem 4.3 item 2 requires a circulation direction satisfying `g_tilde^T Delta + kappa ||L_tilde Delta||_1 <= 0`, `eta = kappa^2/(50 |g_tilde^T Delta|)`, strict feasibility, and an additive potential decrease. Dynamic interaction also requires certified weighted coordinate-update and Detect accounting. |
| Change | `CertifiedIpmSnapshot::apply_lemma_44_update` checks the exact circulation and ratio conditions, constructs the exact rational successor, re-evaluates its bounded fixed-point snapshot, and certifies a drop of at least `kappa^2/500`. `IpmDetectLedger` accumulates per-edge intervals for `ell_e |eta Delta_e|` and reports only lower-bound-certified thresholds. |
| Verification | Focused Lemma 4.4 and Detect regressions, workspace tests, strict Clippy, formatting, and diff checks. |
| Acceptance | Certified transition and accounting layer complete. Initial-point construction, additive-half termination, source-grade dynamic structures, and exact recovery remain later P9.2.4--P9.6 work. |

## Semantic basis

The primary source is CKLPPS22 arXiv:2203.00671v2, Theorem 4.3 and Lemma 4.4,
as delegated by arXiv:2309.16629v1 Section 4. The implementation uses exact
`ExactRatio` arithmetic for the direction, dot product, norm, and step size.
The fixed-point intervals are used only for the source quantities and the
potential comparison; no floating-point value, Dinic call, Push--Relabel call,
or enumerating cycle Oracle participates in an update.

## Invariants enforced

- `Delta` has the graph dimension and zero divergence under the network's arc
  incidence matrix.
- Every supplied approximate length and gradient passes the P9.2.2 factor-two
  and `kappa/8` certificates before the ratio is evaluated.
- `g_tilde^T Delta < 0` and `g_tilde^T Delta + kappa ||L_tilde Delta||_1 <= 0`.
- The exact step is positive and follows the source constant
  `kappa^2/(50 |g_tilde^T Delta|)`.
- The successor is a validated strict-interior fractional circulation and is
  re-evaluated under the identical fixed-point configuration.
- The potential interval proves `Phi(f) - Phi(f+eta Delta) >= kappa^2/500`.
- Detect reports an edge only when its accumulated interval lower endpoint is
  at least the certified epsilon interval's upper endpoint, then resets that
  edge accumulator.

## Evidence

- A two-edge unit-capacity-slack circulation with `kappa=1/2`, direction
  `(-1,-1)`, and exact approximations `(g,ell)=((40,0),(2,2))` yields
  `eta=1/8000`, a strict feasible successor, and one counted iteration with
  two changed coordinates.
- The Detect regression verifies that an epsilon above the certified lower
  bound reports no edge, while a lower epsilon reports both edges and resets
  both accumulators. Detect-call and detected-edge counters are exact.
- P9.2.2's existing malformed approximation, boundary, and precision-mismatch
  tests remain green.

## Audit

Phase baseline: `bba500e8dfef7e90dbddd077bd23d0df0922137a`.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo fmt --all -- --check` | 0 | formatting clean |
| `git diff --check` | 0 | no whitespace errors |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no warnings |
| `cargo test -p rect-graph interior_point` | 0 | 7 focused tests passed |
| `cargo test --workspace` | 0 | passed; full output retained in session audit |

The staged-diff review found no fallback path, ignored test, stale generated
evidence, credential, token, private key, or local absolute path. This report
does not claim the deterministic almost-linear flow theorem.

## Remaining gate

P9.2.4 must implement the source initial-point and additive-half termination
boundary, deterministic KP15 rounding, and exact recovery differential before
the P9.2 family can be closed.
