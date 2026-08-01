# P09.5e.3 - Fresh Compressed Projection Policy Evidence

## Status

**P9.5e.3c state: complete for the declared nine-point isolated-lattice
population. P9.5e.3d state: complete for a snapshot-bound conditional
potential budget. P9.5e.3e state: complete for independently recomputed
Definition 4.2 coordinates in the checked fixed-point domain. P9.5e.3f state:
complete for execution-state decoupling from hidden-stability auditing.
P9.5e.3g.1 state: complete for derived finite source configuration.
P9.5e.3g.2 state: complete for the caller-supplied inclusive-target initial-point
entry. P9.5e.3g.3 state: blocked with source-audit evidence in
`docs/phase-reports/P09-5e-3g-3-target-search-contract.md`. P9.5e.3 state:
in progress.** Implementation commits: `45849b1`,
`c902c37`, `5391ada`, `20f8a18`, `8668461`, `6be878a`, `2802323`, `1c58cee`,
and `ae1352a`.

This report records the completed P9.5e.3b terminating-range evidence and the
completed P9.5e.3c isolated-lattice output differential, together with the
P9.5e.3d conditional potential-reduction budget, the P9.5e.3e complete
Definition 4.2 coordinate policy, and the P9.5e.3f execution-state decoupling.
It also records P9.5e.3g.1's exact per-input structural configuration
derivation and P9.5e.3g.2's target-preserving Appendix B.1 source entry. It
does not close P9.5e.3, P9.5, or `Backend::require_complete()`.

## Issue Matrix

| Issue | Observed | Required contract | Evidence and acceptance |
| --- | --- | --- | --- |
| Fresh state after an accepted source update | P9.5e.2 constructed one projection and stopped at one update, so it did not demonstrate preparation for a successor snapshot. | `Driver` must request and independently certify a new `Projection` for each nonterminal snapshot before selection. | `FixedProjectionFactory` rebuilds two separately snapshot-bound projections for the one-by-one compressed circulation, and the two stored records carry unequal certified snapshots. |
| Exact coordinate update and reuse | `FixedProjectionFactory` can recertify unchanged coordinates only until they become stale. | A finite source fixture may supply distinct exact coordinates per successor snapshot, but it must consume them in order, reject exhaustion, and certify each entry before selection. | `ScheduledProjectionFactory` owns a caller-supplied immutable sequence of identity-compatible `Input` values. The compressed `1 x 1` fixture uses distinct first and successor coordinates, consumes both, and returns the explicit iteration limit instead of reusing either. |
| Independent current-snapshot coordinates | A scheduled fixture demonstrates consumption but not reconstruction from the current exact flow. | Rebuild source coordinates from independently reconstructible exact data without selecting a fixed-point interval endpoint. | `ReciprocalSlackProjectionFactory` derives only from the retained exact flow, exact optimum, and immutable network capacities/costs. `Projection::new` independently proves its Theorem 4.3 error contract before selection. A static audit rejects accesses to `snapshot.lengths()` and `snapshot.gradients()` in the coordinate module. |
| Complete Definition 4.2 coordinates | The reciprocal-slack reconstruction intentionally omits the alpha-weighted barrier gradient, so it can certify only the snapshots on which that omission remains within the Theorem 4.3 error allowance. | Construct a fresh exact-rational source input from all Definition 4.2 terms without reading a retained snapshot coordinate interval. | `DefinitionProjectionFactory` independently reruns the checked fixed-point Definition 4.2 calculation from exact flow/objective/network/configuration data, serializes lower dyadic representatives, then relies on `Projection::new` to independently certify Theorem 4.3. The compressed source suite's `2 x 2`, chord, Figure 3, and exhaustive 410-mask population now use this factory; a 64-successor nonterminal regression verifies every input and snapshot changes. |
| Supported termination range | The 64-update fixture remains intentionally nonterminal, but a distinct strictly interior `1 x 1` source fixture crosses the additive-half boundary after one selected source update. | A terminating run must rebuild exact coordinates from its current snapshot, select a nonzero source direction, certify additive-half termination, then recover without a reference backend. | At uniform arc flow `547590/1000000`, the initial snapshot rejects additive-half. `ReciprocalSlackProjectionFactory` prepares one fresh source projection, the selected step is nonzero, the successor certifies additive-half, and `Circulation::run_source` recovers matching `(0, 0)` and a size-one cover. |
| Arbitrary cap versus source potential progress | `Driver::run` accepts a caller-selected update cap, which is necessary for a nontermination witness but does not record why a terminating run has sufficient fuel. | A source-facing terminating entry may use only a snapshot-bound cap derived from the Equation (9)/Lemma 4.1 potential threshold and the separately certified Lemma 4.4 per-update decrease. It must reject a changed snapshot or `kappa`, a failed projection, or budget exhaustion; it must not recover a nonterminal session. | `PotentialBudget` records one exact initial snapshot, one exact `kappa`, the conservative dyadic lower potential endpoint, termination threshold, and per-update decrease. `run_source_with_potential_budget` recovers the nonterminal `1 x 1` fixture after one source update. A changed-`kappa` regression rejects before the session mutates. |
| Cyclic bucket construction | The finite Algorithm 4 decomposition subset rejects the formal Figure 3 cyclic bucket with `Decomposition(InvalidCertificate)`. | A chosen finite source construction must provide stable selected source edges and exact paths for every bucket edge without an Oracle or a retry after failure. | `bucket::Construction::CanonicalTree` is selected explicitly before construction. Stable-source-order union-find selects a spanning tree and stable BFS reconstructs every rejected-edge tree path; the certificate is revalidated from the immutable bucket. A parallel-edge cycle regression preserves source IDs `0` and `2` and embeds source `1` through source `0`. |
| Finite source configuration | Projection factories accepted fixture-specific roots and dyadic bounds, including an exponent `64` unrelated to the snapshot's exact structural graph. | Derive the canonical root, minimal accepted finite dyadic bound, and explicit cyclic-bucket construction from every exact `Input`; no factory constructor may retain those values. | `spanner::Parameters::derive` contracts the exact singleton forest and derives `FlowNodeId(0)`, the current maximum absolute exponent, and `CanonicalTree`. A focused exponent-four test and the 64-successor Definition 4.2 regression pass through that derivation. The source-flow audit requires the production call. |
| Caller-supplied inclusive target | The source driver accepted a certified snapshot but had no public composition from a caller-provided integral `F*` through Appendix B.1 recovery into the compressed matching/cover decoder. | Build the strict augmented initial point and its one-snapshot potential budget for the supplied target; preserve it through recovery, reject a non-strict initial point, and never query or infer an optimum. A completed run may recover an original integral cost at most the target. | `Backend::begin_with_target` constructs `TargetDriver`; `run` recovers only through `recover_augmented_terminated_at_most`, accepting an original cost at most the supplied target and rejecting one that exceeds it. `Circulation::run_with_target` decodes only that recovered flow. The `1 x 1` target `-1` fixture invokes its factory once with the augmented network; a `2 x 2` target equal to the integral initial-flow cost rejects before the factory executes. A graph regression accepts original cost `0` under target `1` and rejects target `-1` with `TargetNotMet`. |
| Backend completeness | The checked fixed-point coordinate policy and finite configuration derivation are general within their stated domains, but callers cannot yet enter through an independently constructed strict initial point with a checked inclusive target. | Do not enable a complete backend from coordinate reconstruction and configuration derivation alone. | `Backend::require_complete()` remains `Error::Incomplete`; its no-fallback static audit still passes. |
| Execution-state dependency | Source execution previously required `StableMinRatioLedger` solely because `decode_candidate` returned its edge count; selection and source certificates did not consume its witness. | Decode compact cycles through the source graph, chain, shifts, bindings, and circulation only; retain the ledger audit boundary separately. | **Complete:** pure `query::decode` serves source execution. `decode_candidate` remains the P8/P9.4 audit adapter, and source-flow plus compressed-MRD regressions construct no `StableWitness`. |

## Contract and Result

`source_flow::iteration::FixedProjectionFactory` is the production
implementation of this restricted policy. It owns externally supplied,
immutable exact `Input` coordinates and `kappa`; it derives finite spanner
parameters from each input and never derives values from fixed-point intervals.
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
never enters the terminal matching/cover recovery path. A separate
`maximum_iterations = 3` regression now accepts three consecutive
reciprocal-slack updates. Its three records have distinct inputs and snapshots,
and its final snapshot is still nonterminal.

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

`ReciprocalSlackProjectionFactory` removes the finite fixture schedule from the
independently reconstructed coordinate path. For current exact flow `f`, exact
optimum `F*`, and `m` arcs, it rebuilds the rational input

```text
length_tilde(e) = 1 / f_e + 1 / (u_e - f_e)
gradient_tilde(e) = 20 m c_e / (c^T f - F*)
```

from `CertifiedIpmSnapshot::flow`, `optimal_cost`, and the immutable
circulation network only. It neither reads the snapshot's fixed-point length
or gradient intervals nor chooses an interval endpoint. The omitted barrier
gradient is not assumed negligible: `Projection::new` must certify the exact
factor-two and scaled-gradient inequalities before a source candidate can be
selected. The general source-flow regression accepts two distinct successor
inputs constructed this way; the compressed `1 x 1` regression now accepts
three. The previous compressed fixture supplied a finite source-structure
exponent limit of `64`. P9.5e.3g.1 removes that input-independent
configuration: every fresh `Input` now derives its own minimum accepting bound
from the exact singleton contraction. The same regression's `64` now denotes
only its explicit number of nonterminal updates.

`DefinitionProjectionFactory` closes the remaining coordinate-construction
gap within the checked fixed-point domain. From the same exact flow, objective,
network, and fixed-point configuration, it independently recomputes
Definition 4.2's `alpha`, both `slack^-(1 + alpha)` length terms, and the
alpha-weighted barrier gradient. The factory serializes lower endpoints from
that fresh calculation as exact dyadic rationals. It does not access
`CertifiedIpmSnapshot::lengths`, `gradients`, or any retained coordinate
interval. `Projection::new` then separately compares the dyadic input with the
snapshot's certified Definition 4.2 intervals under Theorem 4.3, so a changed
formula, insufficient precision, or an invalid representative rejects before
candidate selection. This is a general coordinate constructor for the checked
fixed-point domain, not a claim that any arbitrary compressed input has already
supplied its ledger, strict initial source state, and inclusive target.

The exact source coordinates remain the P9.5e.2 fixture values:

```text
lengths:  11/4, 4, 8, 5, 8
gradients: 0, 0, 0, 0, -400/3
kappa: 1/2
```

The structural normalization is used only by the finite source-tree and
spanner construction. It rounds each snapshot's lengths and tree weights
relative to their own positive minimum into power-of-two classes, then divides
by that minimum. This produces a dimensionless, scale-relative structural
topology without global common-denominator LCM growth. Candidate scoring and
Theorem 4.3 certification retain the unscaled rational coordinates.

The former fourth preparation no longer rejects in candidate scoring.
`ExactRatio` now carries normalized arbitrary-precision integer components, so
the source candidate quality remains exact when repeated certified updates grow
its numerator or denominator beyond `i128`. The compact-cycle materialization
uses only the scale-relative structural graph for topology and bindings; raw
exact coordinates remain in `Input` and are used directly for candidate scoring
and Theorem 4.3 certification. This prevents raw-coordinate magnitude from
becoming a structural graph encoding bound.

The same `1 x 1` compressed fixture accepts 64 consecutive reciprocal-slack
preparations and source-selected updates. Every adjacent pair of records has a
different exact input and certified snapshot. The run then returns only its
explicit `IterationLimit { maximum_iterations: 64 }`, and its final snapshot
still fails additive-half termination. This is regression evidence that the
former fourth-update overflow was representational, not a valid stopping
condition; it is not evidence that arbitrary supported inputs terminate or
that the backend is complete.

The same 64-successor fixture also accepts 64 independent
`DefinitionProjectionFactory` preparations and source-selected updates. Every
adjacent record has both a distinct exact dyadic `Input` and a distinct
certified snapshot; it remains deliberately nonterminal at the explicit limit.
The `2 x 2`, chord, Figure 3, and 410-mask population use the complete
Definition 4.2 factory rather than the reciprocal-slack approximation.

The same compressed topology now also has a deliberately narrow terminating
range. The strict interior circulation with every arc at `547590/1000000` is
nonterminal under the certified additive-half check. Its one
`DefinitionProjectionFactory` preparation is reconstructed from the live
snapshot rather than from fixed-point intervals; the selected source direction
is nonzero, its successor certifies additive-half termination, and the normal
`Circulation::run_source` handoff recovers matching `(0, 0)` and a size-one
Konig cover. This establishes a genuine nonterminal-to-terminal source session
for the supported `1 x 1` range. It does not establish a termination policy for
the broader compressed-MRD population.

`PotentialBudget` now makes the progress component of that source session
explicit. For a nonterminal certified snapshot and fixed `kappa`, it records
the exact snapshot identity and derives a conservative integer number of
updates from the dyadic lower endpoint of the initial potential, the dyadic
lower endpoint of `20m log(1/2)`, and the dyadic upper endpoint enclosing
`kappa^2 / 500`. This is the interval-safe form of the Equation (9)/Lemma 4.1
and Lemma 4.4 implication: every accepted matching-`kappa` update is already
checked to decrease the potential by at least that amount. A terminal starting
snapshot has budget zero; a nonterminal snapshot receives at least one update.

`Driver::run_with_potential_budget` validates that its session still equals the
budget's initial snapshot and that every fresh `Projection` uses the recorded
`kappa` before `Session::apply_source_selected` can mutate state. It otherwise
retains every existing projection, candidate, Lemma 4.3, Lemma 4.4, and
additive-half check. A failed factory, stale snapshot, changed `kappa`, or
budget exhaustion returns an error and cannot call recovery. The `1 x 1`
compressed entry `Circulation::run_source_with_potential_budget` therefore
demonstrates a budgeted nonterminal-to-terminal source session without relying
on the fixture's manual iteration limit.

The additive-half certificate now compares its retained exact rational
objective gap directly with `1/2`, while it still returns the independently
enclosed gap for audit. This avoids treating an outward-rounded gap enclosure
that straddles the exact boundary as a semantic failure; it does not weaken the
potential threshold or the exact optimality/recovery checks.

The explicit `2 x 2` biclique differential uses the same bounded construction:
a test-only exact reference establishes the strict interior interpolation point,
but the sole prepared projection, source candidate selection, certified update,
additive-half check, and matching/cover recovery all use the production source
path. The resulting matching and cover still agree with the retained exact
reference. The MRD chord differential also follows this one-update path.

The source core now makes its cyclic-bucket construction an explicit immutable
policy. `Construction::CanonicalTree` does not call Algorithm 4 and does not
retry Algorithm 4 if a witness fails. It sorts stable `SourceEdgeId` values,
uses local union-find state to select exactly one spanning tree for the
connected bucket, then uses stable breadth-first tree paths to embed every
rejected edge. The stored selected set and paths are verified against exact
contracted endpoints. This is a finite source-graph certificate, not an Oracle,
and it makes no sparse-spanner, congestion, stretch, dynamic-recourse,
termination, or asymptotic-runtime claim. `Construction::Algorithm4` remains
available as a distinct requested construction.

With that explicit policy, the formal Figure 3 compressed circulation no
longer uses a terminal-only handoff. It begins at the exact nonterminal boundary
snapshot, prepares one fresh Definition 4.2 projection, selects a nonzero
source direction, and reaches additive-half termination after that accepted
update. The recovered flow value, matching, vertex cover, selected horizontal
and vertical chord flags, and rectangle vector are exactly equal to the formal
reference analysis and reference completion for the fixture.

The same production path now exhausts all 511 nonempty masks of the nine-point
isolated-point lattice. The 101 masks with an empty horizontal or vertical
family, or with no explicit conflict edge, are outside the compressed-flow
domain. Every one of the remaining 410 masks starts from an independently
certified nonterminal snapshot, prepares a fresh Definition 4.2 projection
for each accepted update, reaches additive-half termination within the explicit
eight-update limit, and recovers a maximum matching, minimum vertex cover,
independent chord family, and optimum rectangle count. The observed maximum is
two accepted updates. A lower-precision rational-interval search only locates
the test fixture's near-boundary candidate; the returned source snapshot always
uses the normal source precision and independently certifies nontermination,
with a normal-precision search fallback if the candidate disagrees.

The population comparison uses exact certificate equivalence rather than
requiring one arbitrary matching tie-break: mask `255`, for example, has a
different valid maximum matching from the retained Dinic reference while
preserving every certificate invariant and the optimum rectangle count. A
separate `2 x 3` compressed-circulation regression proves that pruning isolated
outer endpoints leaves a strictly interior active circulation and still returns
the original `2`-by-`3` cover dimensions, with every isolated endpoint
uncovered.

### P9.5e.3g.2 Explicit Inclusive-Target Entry

`Backend::begin_with_target` accepts one integral target `F*` as a checked
domain value. It creates the existing Appendix B.1 augmentation, evaluates the
certified strict initial snapshot against that same target, derives the
`PotentialBudget` from that augmented snapshot and network, and starts a source
driver with no arbitrary iteration cap. `TargetDriver` retains the target and
the immutable augmentation. Its `run` method recovers only through
`recover_augmented_terminated_at_most`, so a completed run accepts an original
integral cost at most the supplied target and rejects one that exceeds it; it
does not require the target to equal the recovered optimum.

`Circulation::run_with_target` is the compressed-flow boundary. It creates that
driver, runs it, and passes only `recovered.original` into `recover_certified`;
it has no Oracle dependency and no alternate flow path. The `1 x 1` compressed
fixture supplies the exact optimum `-1` and observes one factory call with the
Appendix B.1 augmented network and the same target in the certified snapshot.
Its factory then returns `NoSourceCandidate`, which is propagated as a source
failure: this fixture verifies initialization and target preservation, not
terminal source completion. The `2 x 2` fixture supplies a target equal to the
augmentation's integral initial-flow cost. Strict initialization rejects it
before the factory is called. A graph-level regression builds a valid
additive-half terminal snapshot on the two-arc cycle, recovers original cost
`0` through both the strict `recover_augmented_terminated` and the inclusive
`recover_augmented_terminated_at_most` under target `1`, and returns
`TargetNotMet` for target `-1`.

This is not an incorrect-target decision procedure. In particular, the source
does not establish whether an arbitrary failure means the supplied target is
too low, too high, or unavailable for another reason. P9.5e.3g.3 therefore
remains blocked, no binary-search wrapper exists, and
`Backend::require_complete()` remains `Error::Incomplete`.

## Verification

### P9.5e.3g.2 Full Audit

Phase baseline: `059f3b68d33816a6a94aea4dd90cefb9bf1493d2`. The following
commands exited `0` on 2026-07-31 after the target-entry tests were added.
The complete workspace audit also passed with 385 tests passed and 3 existing
ignored campaigns.

| Command | Exit | Result |
| --- | ---: | --- |
| `cargo test -p dominance target_entry -- --nocapture` | 0 | 2 inclusive-target compressed-entry tests passed |
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | 18 compressed-source tests passed, including the target-entry regressions |
| `cargo test -p graph source_flow -- --nocapture` | 0 | 31 source-flow tests passed, including direct target-driver checks and the at-most recovery boundary |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_source_flow_audit.py` | 0 | audit requires both inclusive-target entry boundaries and found no fallback |
| `git diff --check` | 0 | no whitespace errors |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | finite tree-chain/core boundary has no Oracle fallback |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace` | 0 | 385 passed, 3 existing ignored across 13 suites |
| `env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | workspace documentation built with warnings denied |
| `cargo build --workspace --release` | 0 | six workspace crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

### P9.5e.3g.1 Full Audit

Phase baseline: `07018227e003154d8aad891df1537384126ff4f1`. The following
closeout commands all exited `0` on 2026-07-31. The source-flow audit now
reports the derived-configuration boundary; no generated result artifact is
required for this focused pure-configuration change.

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | finite tree-chain/core boundary has no Oracle fallback |
| `python3 tools/check_source_flow_audit.py` | 0 | production execution derives its finite source configuration and has no forbidden fallback or hidden-stability dependency |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace` | 0 | full workspace suite passed |
| `env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | workspace documentation built with warnings denied |
| `cargo build --workspace --release` | 0 | six workspace crates built in release mode |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |

| Command | Exit | Result |
| --- | ---: | --- |
| `git diff --check` | 0 | no whitespace errors |
| `cargo fmt --all -- --check` | 0 | Rust formatting accepted |
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | focused compressed-source suite passed, including the full Definition 4.2 `2 x 2`, chord, Figure 3, 410-member isolated-lattice, 64-successor, and budgeted nonterminal session regressions |
| `cargo test -p graph source_flow -- --nocapture` | 0 | 28 focused source-flow tests passed, including coordinate reconstruction and changed-`kappa` rejection before session mutation |
| `cargo test -p graph source_flow::coordinates -- --nocapture` | 0 | 2 coordinate tests passed, including independently recomputed Definition 4.2 dyadics accepted by Theorem 4.3 |
| `cargo test -p graph source_min_ratio::spanner -- --nocapture` | 0 | 5 canonical-tree source-core tests passed |
| `cargo test -p graph source_lsst::bucket -- --nocapture` | 0 | 4 bucket tests passed, including the cyclic parallel-edge certificate and the no-retry Algorithm 4 rejection |
| `python3 tools/check_source_flow_audit.py` | 0 | no reference-flow, recovery fallback, or hidden-stability execution dependency |
| `python3 tools/check_source_min_ratio_audit.py` | 0 | source tree-chain/core boundary has no Oracle fallback |
| `python3 tools/check_biclique_bound.py` | 0 | compact biclique bound accepted |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no workspace warnings |
| `cargo test --workspace --quiet` | 0 | full workspace suite passed |
| `env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | workspace documentation built without warnings |
| `cargo build --workspace --release` | 0 | release build passed |
| `python3 tools/check_release_consistency.py` | 0 | release provenance accepted |
| `cargo check --workspace` | 0 | arbitrary-precision migration compiles across all dependent packages |
| `cargo test -p graph` | 0 | 210 graph tests and documentation tests passed |
| `cargo test -p dominance reciprocal_slack_coordinates_continue_past_the_former_exact_scoring_overflow` | 0 | the compressed source session accepted 64 fresh nonterminal updates |
| `cargo test -p dominance recovers_a_single_edge_compressed_solution_after_one_nonterminal_source_update` | 0 | a nonterminal source-selected run prepared one fresh projection, certified termination, and recovered the compressed matching/cover |
| `cargo test --workspace` | 0 | current full workspace suite passed; the 3 existing release-scale campaigns remain ignored (2 dominance, 1 verification) |
| `cargo test -p dominance definition_coordinates_rebuild_across_multiple_nonterminal_successors -- --nocapture` | 0 | 64 independently recomputed Definition 4.2 successor preparations passed |
| `cargo test -p dominance potential_budget_recovers_a_nonterminal_compressed_solution_without_a_manual_limit -- --nocapture` | 0 | one budgeted nonterminal-to-terminal Definition 4.2 recovery passed |
| `cargo test -p graph source_flow::iteration` | 0 | 17 focused execution tests passed after removal of hidden-stability ledger state |
| `cargo test -p graph source_min_ratio::query` | 0 | 2 query tests passed, including pure compact decoding without a ledger or witness |
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | compressed source execution passed without constructing a `StableWitness` |
| `cargo test -p graph source_min_ratio::spanner -- --nocapture` | 0 | 6 focused tests passed, including exact root/exponent derivation and snapshots built from derived parameters |
| `cargo test -p graph source_flow::iteration -- --nocapture` | 0 | 17 focused tests passed with test snapshots and every production factory deriving their configuration |
| `cargo test -p dominance compressed_flow::experiment::source -- --nocapture` | 0 | compressed source suite passed; its 64-successor Definition 4.2 regression uses no fixture structural parameter |
| `python3 tools/check_source_flow_audit.py` | 0 | production factories require `SpannerParameters::derive(&input)` and retain no forbidden fallback or hidden-stability dependency |

## Remaining Boundary

**P9.5e.3f is complete for execution-state decoupling.** The source-flow
production path does not semantically consume `StableMinRatioLedger`: compact
candidate selection uses exact `Input` quality, and `Projection`/`Session`
independently certify the supplied source coordinates. The sole ledger-derived
field was the audit-only `stable_edge_count` returned by
`source_min_ratio::query::decode_candidate`. `query::decode` now exposes the
same exact compact-cycle transformation without ledger state, and all
source-flow projections, candidates, selected state, and factory constructors
use it. The existing ledger adapter remains for P8/P9.4 auditing; this change
does not expose a hidden witness or claim its construction.

P9.5e.3g.2 completes the inclusive-target boundary only: for a caller-supplied
integral `F*` within the checked source domain, it constructs and validates the
strict Appendix B.1 starting point, runs the budgeted source path, and verifies
at-most-target-preserving recovery before compressed decoding. A completed run
may recover an original integral cost at most the target; it does not require
equality. It neither discovers `F*` nor supplies the missing interpretation of
failures for a wrong target. That source gap is P9.5e.3g.3, remains blocked,
and does not enable `Backend::require_complete()`. The blocked status is
supported by a direct source audit of arXiv:2203.00671v2 recorded in
`docs/phase-reports/P09-5e-3g-3-target-search-contract.md`.

`FixedProjectionFactory` is a fresh-projection reconstruction policy, not a
general dynamic coordinate-maintenance or termination policy. It supports only
snapshots for which its caller-supplied fixed coordinates and finite source
structures continue to certify; the 25-update source-flow regression
shows the exact coordinate-staleness failure when that condition ends.

P9.5e.3d closes only the conditional potential-accounting side of termination:
given the initial certified snapshot, unchanged `kappa`, and a sequence of
successfully certified fresh projections, its update budget has no arbitrary
manual cap. It does not construct that projection sequence or prove that a
factory remains available at every nonterminal successor. Consequently it does
not itself construct source coordinates or source state, even though
`DefinitionProjectionFactory` now provides the coordinate component. It does
not enable `Backend::require_complete()`.

`ScheduledProjectionFactory` proves only that a finite, independently supplied
coordinate trace can be consumed without stale reuse. It is not the missing
source-supported construction of that trace and it does not prove that any
nonterminal compressed instance reaches additive-half termination.
`DefinitionProjectionFactory` now supplies the missing current-snapshot
coordinate construction for every snapshot in its checked fixed-point domain.
P9.5e.3 still needs a source-backed target-search decision invariant and a
broader public compressed-MRD acceptance campaign. CKLPPS22 p.24 and Algorithm
7 do not yet establish a decision invariant for an incorrect target; therefore
no binary-search wrapper may infer or replace `F*`, and
`Backend::require_complete()` remains unavailable.

`ReciprocalSlackProjectionFactory` remains a deliberately incomplete
approximation policy with a 64-update nonterminal regression. Arbitrary-
precision candidate arithmetic and structural-only topology remove the former
global-LCM, hierarchy, and fourth-step raw-coordinate boundaries for that
separate regression. `DefinitionProjectionFactory` supplies the supported
nonterminal-to-terminal `1 x 1` range and the current declared population
differential, but neither factory establishes public all-input source state or
a runtime bound.

P9.5e.3c recognizes connected tree buckets as an explicit `TreeIdentity`
construction: selecting every edge with its own one-edge path is an exact
zero-stretch embedding, so no positive-level expander witness is needed.
`CanonicalTree` adds the corresponding explicit finite construction for cyclic
buckets. The nonterminal chord and formal-polygon differentials each prepare
one source projection, take one selected source update, certify termination,
and match their retained references. The latter compares flow, matching, cover,
chord flags, and full rectangle decomposition. The declared nine-point lattice
population extends this to all 410 compressed-flow masks and observes at most
two updates per run. This completes P9.5e.3c's population differential, but it
does not supply a general termination policy; `Backend::require_complete()`
remains unavailable while the P9.5e.3 parent obligation remains in progress.

P9.3.2d remains separate low-priority P9.6a proof debt. The formal SIAM source
with DOI `10.1137/17M1115575` does not provide the required reduced-event
ordering/counting proof. This bounded campaign makes no `AlmostLinear`,
`an19_runtime_verified: true`, or AN19 asymptotic-runtime claim.
