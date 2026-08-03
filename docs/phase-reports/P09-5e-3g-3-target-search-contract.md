# P09.5e.3g.3 - Target-Search Decision Contract

## Status

**State: blocked for automatic `F*` search with direct source audit evidence,
but with exact negative-certificate *types* implemented and verifiable.** No
automatic binary-search wrapper is implemented. `Backend::require_complete()`
remains `Error::Incomplete`, P9.5e.3 and P9.5 remain in progress, and no AN19
runtime claim is made.

## Primary goal

Determine whether the cited source material provides a valid, implementable,
source-backed decision invariant for an incorrect target guess, so that a
target query can soundly distinguish:

1. `FeasibleAtMost(T)`: an explicit integral feasible solution of cost `<= T`;
2. `ProvenInfeasibleBelow(T)`: a mathematically valid certificate that
   `F_opt > T`;
3. `UnsupportedOrUndetermined(error)`: implementation, precision, structural,
   or source-domain failure.

This report records the source audit, the required mathematical analysis, the
exact source passages inspected, what they state and omit, the interpretations
rejected, the API that remains valid, and the theorem still needed.

## Source identification

The repository pins the predecessor source as:

- Li Chen, Rasmus Kyng, Yang P. Liu, Richard Peng, Maximilian Probst Gutenberg,
  and Sushant Sachdeva, "Maximum Flow and Minimum-Cost Flow in Almost-Linear
  Time," arXiv:2203.00671v2 (2022), DOI 10.48550/arXiv.2203.00671.

The primary source is:

- Jan van den Brand et al., "A Deterministic Almost-Linear Time Algorithm for
  Minimum-Cost Flow," arXiv:2309.16629v1 (2023), which delegates the
  potential-reduction method to CKLPPS22.

`docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md` maps both sources to intended modules.
The paper text of arXiv:2203.00671v2 was retrieved and inspected directly for
this audit (Section 4, Equation (9), Theorem 4.3, Lemma 4.4, Lemma 4.12,
Algorithm 7, and Appendix B.1 / C).

The source registry was rechecked on 2026-08-03. arXiv still identifies
`2203.00671v2` (last revised 2022-04-22) as the latest CKLPPS version and
`2309.16629v1` (submitted 2023-09-28) as the only version of the deterministic
primary source. No later revision or erratum supplies the missing target
decision invariant.

## Source passages inspected

### Section 4 / Equation (9) and the binary-search remark

The potential (Equation (9)) is defined only with respect to the true optimal
value `F* = c^T f*`:

```text
Phi(f) = 20m log(c^T f - F*) + sum_e [(u+_e - f_e)^(-alpha) + (f_e - u-_e)^(-alpha)]
```

The source states (Section 4, around p. 24):

> "assume that we know `F*`, as running our algorithm allows us to binary
> search for `F*`."

This is the sole place the paper mentions binary search. It is a remark, not a
theorem, lemma, or algorithm. The paper gives no decision invariant for an
incorrect guess, no certificate for `F_opt > T`, and no monotone-predicate
proof. It does not define the observable behavior when the supplied guess is
below `F_opt` (non-termination) or above `F_opt` (potential undefined), and it
provides no way to certify either case.

### Theorem 4.3

The potential-reduction method (Theorem 4.3) is stated and proved under the
hypothesis that the IPM is given the exact optimum `F*`:

```text
At the end of O~(m kappa^2) iterations, we have c^T f(t) <= c^T f* + (mU)^-10.
```

The theorem requires an initial flow `f(0)` with `Phi(f(0)) <= 200m log mU`
(Lemma 4.12). The potential `log(c^T f - F*)` forces `c^T f > F*` throughout;
if the caller-supplied target exceeds `F_opt`, the flow would need to pass below
the target and the potential term is not defined there. The theorem therefore
certifies convergence only when the supplied value equals the true optimum.

### Lemma 4.4 (per-update potential decrease)

Lemma 4.4 gives a certified `kappa^2 / 500` potential decrease per accepted
update under the same exact-`F*` assumption. The repository's `PotentialBudget`
implements the interval-safe form of this bound and additionally proves a
conditional finite update count. Exhausting that budget is a certificate that
the potential did not cross the additive-half threshold after the certified
number of updates **given that every requested projection is prepared and
accepted**. It is not by itself a certificate that `F_opt > T`: budget
exhaustion can also result from a projection failure, a stale snapshot, a
changed `kappa`, a coordinate-certificate failure, or a structural-domain
rejection, none of which is a mathematical statement about `F_opt`.

### Algorithm 7 (MinCostFlow)

Algorithm 7 takes `(G, d, c, u+, u-, f(0), F*)` where `F*` is described as a
"guess of the optimal flow". Its termination test is

```text
while c^T f(t) - F* >= (mU)^-10:
```

- If `F* = F_opt`, the flow converges to `F*` and the loop terminates.
- If `F* < F_opt`, then `c^T f(t) - F* >= F_opt - F* >= 1` for every feasible
  `f(t)` (integral costs), so the additive-half boundary `(mU)^-10` is never
  reached and the loop cannot terminate on its own. The source provides no
  "guess too low" detection, no decision output, and no infeasibility
  certificate in this branch.
- If `F* > F_opt`, the potential term `log(c^T f - F*)` is undefined in the
  region below `F*`, and the source does not analyze or certify this case.

Algorithm 7 therefore implements only the positive direction: a completed run
under a valid target returns a feasible flow whose cost is at most the target.
It does not implement a decision procedure for an incorrect guess.

### Lemma 4.12 and Appendix B.1 (Initial Point)

Lemma 4.12 and Appendix B.1 construct the O(m+n)-edge augmentation used by
`begin_with_target`. They certify the strict initial snapshot and the
`200m log mU` potential bound. Lemma 4.12 also states that, given an optimal
flow for the augmented instance, the algorithm can either recover an optimal
flow for the original instance or conclude that the original instance admits no
feasible flow. This is a feasibility statement for the *instance*, not a
decision certificate for an arbitrary caller-supplied integral target; it does
not establish that a target query with `F*` below `F_opt` can certify
`F_opt > T`.

### Appendix C (cost/capacity scaling)

Appendix C (Algorithm 9) reduces min-cost flow to `O(log C)` polynomially
bounded instances and, in the process, extracts dual potentials `y` and uses
`epsilon`-optimality (Definition C.4, Lemmas C.5, C.8, C.9). These are
certificates of exact optimality of a *constructed flow*, not a target-decision
invariant for an incorrect guess. The paper explicitly states it does not
maintain dual variables in the main IPM:

> "We do not maintain any dual variables, so such a guarantee does not hold for
> our algorithm." (around p. 23)

Consequently the repository cannot obtain a dual-potential certificate from the
implemented source path without reintroducing a reference solver, which is
forbidden by the static audit.

### Appendix C.9 cannot bootstrap target decisions

The original PDF was reread directly on 2026-08-03, including the rendered
page containing Lemma C.9. That lemma is a **dual-extraction reduction after
exact solution**, not an algorithm for deciding an unchecked target. Its proof
first computes the optimal primal flow in `T_MCC(m, C, U)` time. Only because
that flow is already optimal can it assert that the residual graph has no
negative cycle. It then computes a residual distance label by another
un-capacitated min-cost-flow call in `O(T_MCC(m, C, U))` time before constructing
the dual slacks.

Algorithm 9 invokes this lemma only after it asks to solve each rounded
residual instance exactly. Consequently, neither Algorithm 9 nor Lemma C.9 can
construct `DualLowerBoundCertificate` from a `TargetDriver` failure: doing so
would either assume the exact minimizer that target discovery is meant to
obtain, or call a general exact min-cost-flow solver as a fallback. Both are
outside the source-shaped production contract. Appendix C therefore does not
turn an unclassified failed target run into `ProvenInfeasibleBelow(T)`.

## Required mathematical analysis

### 1. What problem is solved for T below / equal / above F_opt

For the target-augmented Appendix B.1 formulation:

- `T = F_opt`: the potential is well defined, the additive-half boundary is
  reachable, recovery returns an integral flow of cost exactly `F_opt`, and
  `F_opt <= T` is proven by construction.
- `T > F_opt`: `F_opt <= T` is trivially true. A completed run still returns a
  feasible integral flow of cost at most `T`, but the run cannot be certified
  to terminate in general because the potential `log(c^T f - T)` is undefined
  below the target; a run may fail for this reason without proving anything
  about `F_opt`.
- `T < F_opt`: `c^T f - T >= 1` for every feasible integral flow, so the
  additive-half boundary is unreachable and the run cannot terminate at the
  certificate. A non-terminating run does **not** certify `F_opt > T`.

### 2. Augmented circulation: feasibility, strict interiority, boundedness

Appendix B.1's augmentation preserves feasibility of the original instance and
adds artificial arcs of cost `4mU^2` that carry zero flow in any optimum when
the original instance is feasible (Lemma 4.12 proof). The midpoint initial flow
is strictly interior. These properties are certified by the repository's
`initial_point_augmentation` and `CertifiedIpmSnapshot::evaluate`.

### 3. Monotone predicate P(T) = [F_opt <= T]

`P` is monotone, but the source provides **no observable certificate for
`P(T) = false`**. The negative direction is not established by any theorem,
lemma, or algorithm in the inspected source.

### 4. Observable certificate for P(T) = false

None is provided by the source. The candidates considered and rejected:

- Budget exhaustion: conditional on every projection succeeding; does not
  distinguish `F_opt > T` from implementation failure.
- Non-termination: indistinguishable from a valid target whose source session
  simply has not yet reached the boundary under the finite/conditional
  projection policy.
- Non-strict initial point: occurs when `T` equals the initial-flow cost; this
  is a domain failure, not a certificate about `F_opt`.
- Recovery `TargetNotMet`: fires only after a run *did* terminate at
  additive-half and the rounded original cost exceeds `T`; the source does not
  prove this situation is reachable for `T < F_opt`, so it is not a sound
  negative decision.

### 5. Negative certificate type

If a negative certificate were implementable, it would be a feasible dual
solution (vertex potentials `y`, `s-`, `s+`) with objective value greater than
`T` under the dual (59) / an `epsilon`-optimality witness. The repository has
no such type, and the source's main IPM explicitly does not maintain dual
variables. Building one would require a reference min-cost solver or an
enumerating residual-cycle implementation, both forbidden in production by the
static audit.

### 6. Implementation failures cannot be mistaken for mathematical infeasibility

The existing source path already keeps them separate: a run returns an explicit
`iteration::Error` (projection, candidate, coordinate, stale snapshot, budget,
kappa) or a recovery error, and `recover_terminated_at_most` returns
`TargetNotMet` only on a terminated snapshot. However, **the absence of a
positive decision does not imply infeasibility**, and the current API does not
attempt to claim it does. This is the exact separation the task requires, and
it is preserved.

### 7. Integer target search interval

No search wrapper is implemented. If one were, the interval would need:
- a lower bound (e.g. `-m*U*C` from cost/capacity bounds),
- an upper bound (e.g. `m*U*C`),
- integrality of all costs/capacities,
- an overflow-safe midpoint `lo + (hi - lo) / 2`,
- termination only after a certified decision in both directions.

None of the negative-decision preconditions holds, so no wrapper is added.

### 8. Number of target probes

Not applicable: no search is implemented because the negative direction is not
source-backed.

## Why the omission prevents a verified binary-search wrapper

A binary-search wrapper needs, for every tested `T`, a sound answer to
`P(T) = [F_opt <= T]` in both directions. The source:

- proves the positive direction under an exact `F*` assumption;
- does not define or certify the negative direction for an incorrect guess;
- does not maintain dual variables that would supply a negative certificate;
- makes the binary-search claim only as a remark (p. 24), with no theorem.

Without a certified negative direction, any search that treated a failed run as
"target too low" would collapse `UnsupportedOrUndetermined` into
`ProvenInfeasibleBelow`, which is unsound.

## Interpretations considered and rejected

- "Non-termination implies `F_opt > T`": rejected. Non-termination also
  occurs for implementation, precision, projection, and structural-domain
  reasons.
- "Budget exhaustion implies `F_opt > T`": rejected. The budget is conditional
  on every projection succeeding and every update being accepted; exhaustion
  is not a statement about `F_opt`.
- "A lower bound can replace `F*`": rejected. The potential and Lemma 4.1/4.4
  are proved only for the exact optimum; substituting a bound changes the
  termination threshold without a certificate.
- "A successful target run proves `F_opt = T`": rejected. It proves only
  `F_opt <= T`.
- "The dual of (59) can be reconstructed from the IPM": rejected. The paper
  states it does not maintain dual variables.
- "A single recovered cost gives a search direction for all failed guesses":
  rejected. There is no failure-implies-infeasible certificate.

## API that remains valid

- `Backend::begin_with_target(network, target, ...)` constructs the strict
  Appendix B.1 initial point for one integral inclusive target.
- `TargetDriver::run()` recovers an integral original flow only when the
  terminal snapshot certifies additive-half and the rounded original cost is at
  most the target (`recover_augmented_terminated_at_most`).
- `Circulation::run_with_target(...)` decodes the recovered original flow into
  a matching/Konig-cover certificate.
- A completed run at `T` proves `F_opt <= T` (one-sided positive certificate).
- A failed run returns an explicit error; it is not classified as
  "target too low".
- **Negative-certificate verifiers (new):** a caller may *prove* `F_opt > T`
  by supplying an exactly verified certificate. No reference solver constructs
  or selects it, and a missing or failed certificate is never an infeasibility
  decision:
  - `source_flow::certificate::DualLowerBoundCertificate` (vertex potentials
    `y` and slacks `s-`, `s+`) with `Backend::prove_infeasible_below`, which
    verifies exact dual feasibility and requires the dual objective to be
    strictly greater than `T`.
  - `Circulation::certify_cover_below` for the compressed MRD, which verifies a
    caller-supplied vertex cover and requires `cover_size < -T`; by Konig's
    theorem this certifies `max_matching < -T`, hence `F_opt > T`.

## Remaining theorem needed

A theorem that, for any incorrect guess `T`, either produces a certified dual /
`epsilon`-optimality witness proving `F_opt > T`, or certifies that the failure
is an implementation/domain failure, is required before any binary-search
wrapper may be implemented. The certificate *types* above let a caller verify a
supplied negative certificate exactly, but they do not automatically *find*
one; automatic search remains blocked because the source does not construct the
certificate.

## Code audit

- Naming is inclusive-target throughout: `TargetDriver`, `run_with_target`,
  `begin_with_target`, `TargetRun`, `TargetNotMet`. No `ExactTargetDriver`,
  `ExactTargetRun`, or `run_source_with_exact_target` remains.
- Recovery checks use `recovered cost <= target` (inclusive), not
  `== target`.
- `Error::RecoveryNotOptimal` remains only for the strict
  `recover_terminated` path that knows the exact optimum; it is not used by the
  inclusive-target path.
- `tools/check_source_flow_audit.py` now additionally requires the
  negative-unclassified contract wording in both the root and compressed
  modules, the inclusive-target wording of `InvalidTarget`, and the new
  certificate verifiers (`prove_infeasible_below`, `certify_cover_below`,
  `DualLowerBoundCertificate`, `InfeasibilityProof`, `CoverBelowProof`).
- The negative-certificate verifiers perform only exact feasibility/objective
  arithmetic over immutable network and partition data; the static audit's
  forbidden list rejects any reference-flow or Oracle dependency in them.
- No automatic binary-search or target-inference code exists in production.

## Tests

The one-sided positive certificate and its explicit failure boundary are
covered by:

- `target_recovery_accepts_an_original_cost_below_the_target` (graph):
  strict recovery cost `0`, at-most under target `1` accepts cost `0`, target
  equal to recovered cost `0` accepts (inclusive equality), and target `-1`
  returns `TargetNotMet { target: -1, actual: 0 }`.
- `binds_an_augmented_source_driver_to_a_caller_supplied_target` (graph):
  the source factory is invoked once with the augmented network and the same
  target; a `NoSourceCandidate` failure propagates as an explicit iteration
  error, not as an infeasibility decision.
- `rejects_a_target_that_does_not_leave_the_augmented_initial_point_strict`
  (graph): a target equal to the initial-flow cost fails strict initialization
  before the factory executes.
- `target_entry_starts_the_augmented_source_path_for_a_supplied_optimum`
  (compressed `1 x 1`): the inclusive-target compressed entry invokes the
  factory once with the augmented network and target `-1` preserved in the
  certified snapshot.
- `target_entry_rejects_a_non_strict_initial_point_before_factory_execution`
  (compressed `2 x 2`): a target equal to the integral initial-flow cost
  rejects before factory execution.

The negative-certificate verifiers are covered by `source_flow::certificate`
tests (graph) and `cover_certificate_*` tests (compressed):

- `dual_certificate_certifies_a_strict_lower_bound_on_the_optimum` (graph):
  `prove_infeasible_below` returns a proof for `T = -1` on a zero-flow optimum.
- `dual_certificate_rejects_a_target_that_is_not_exceeded` (graph): a dual
  objective equal to `T` is rejected as `CertificateInsufficient`.
- `dual_certificate_rejects_an_infeasible_slack_assignment` and
  `dual_certificate_rejects_negative_slack` (graph): exact per-arc dual
  feasibility and nonnegativity are enforced.
- `dual_certificate_uses_demands_in_the_objective` (graph): demands contribute
  to the dual objective, and an infeasible reference circulation is rejected.
- `dual_certificate_dimension_mismatch_rejects` and
  `from_potentials_never_constructs_an_infeasible_certificate` (graph):
  dimension checks and the constructive `from_potentials` invariant.
- `cover_certificate_proves_optimum_above_a_supplied_target` (compressed):
  a size-`1` cover certifies `F_opt > -2` on the `1 x 1` fixture.
- `cover_certificate_rejects_a_target_that_is_not_exceeded` (compressed):
  a cover matching `-T` is rejected as `CoverCertificateInsufficient`.
- `cover_certificate_rejects_a_cover_that_omits_a_conflict_edge` (compressed):
  an uncovered compressed biclique edge rejects.
- `cover_certificate_rejects_a_wrong_declared_size` and
  `cover_certificate_agrees_with_the_recovered_minimum_cover` (compressed):
  declared-size recomputation and agreement with the recovered minimum cover.

No test claims `T < F_opt` returns certified infeasible automatically from a
failed run: that still requires a caller-supplied certificate, and automatic
search remains blocked. No binary-search wrapper or target-inference test is
added.

## Audit

Phase baseline: `ae1352aa5faadf01c9f08c1c93b5ede658a4b0c6`. The following
commands exit `0`.

| Command | Result |
| --- | --- |
| `git status --short` | clean after commit |
| `git diff --check` | no whitespace errors |
| `cargo fmt --all -- --check` | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | compact biclique bound accepted |
| `python3 tools/check_source_flow_audit.py` | source-flow boundary, inclusive-target, and negative-unclassified contract accepted |
| `python3 tools/check_source_lsst_audit.py` | low-stretch-tree audit accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | no workspace warnings |
| `cargo test --workspace` | full suite passes |
| `env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | documentation built without warnings |
| `cargo build --workspace --release` | release build passed |
| `python3 tools/check_release_consistency.py` | release provenance accepted |

## Remaining blockers

1. **P9.5e.3g.3 remains blocked for automatic search**: exact negative
   certificate *types* are implemented and independently verifiable
   (`DualLowerBoundCertificate` / `prove_infeasible_below`,
   `certify_cover_below`), but the source provides no automatic construction of
   those certificates, so a failed run still cannot be classified as
   "target too low" and no binary-search / automatic `F*` solving is allowed.
2. **P9.5e.3 and P9.5 remain in progress**: `Backend::require_complete()`
   returns `Error::Incomplete`.
3. **P9.3.2d remains deferred proof debt**: no AN19 runtime claim.
