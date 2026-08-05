# MRD Full Implementation Master Plan

- Plan schema: 2
- Current branch: codex/full-implementation
- Baseline local SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Baseline origin/main SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Current evidence SHA: 211308a4981c09ccd549bd0ed322db847f427ce3
- Current phase: P9.5e.3g.3 automatic target-decision blocker
- Current phase state: blocked
- Last completed phase: P17 geometry-phase-diagnostic
- Plan last updated: 2026-08-05
- Overall target: complete source-traceable geometry, an automatic exact
  source-flow solver only when its decision contract is established, and a
  proof-backed AN19 runtime claim only when its missing reduction is proved.

## Scope

This is the active plan, not a process archive. Completed phase narratives,
release notes, and superseded next actions are summarized in
[`HISTORY.md`](HISTORY.md). Current implementation scope is described in
[`IMPLEMENTATION.md`](IMPLEMENTATION.md), including its finite campaigns and
their boundaries.

The permanent reference solvers remain the complete production surface on the
accepted domain. The source backend remains a research interface until its
target-decision and runtime proof obligations are closed.

## Current Evidence

- Exact grid, polygon, formal-fixture, and external-oracle campaigns are
  retained in `results/final-campaigns/` and summarized by the final reports.
- The direct-grid parity census verifies 511 nonzero 3x3 masks, 897 foreground
  components, and 1,794 paired comparisons with zero mismatches and zero
  direct rank counters.
- The repeated direct-grid campaign contains three predeclared warm-ups and 31
  measured fresh processes. It is local descriptive evidence only.
- The AN19 event campaign contains 31 fixed A--H snapshots with exact
  Oracle/reduced semantic agreement and local certificates. It is not a global
  amortization or runtime proof.
- The P15 paper-scaling full campaign is complete: seven deterministic
  families, eight target sizes, three warm-ups, and the predeclared 31/15
  measured repetition schedule produced 5,824 terminal rows. It retains 4,522
  successes and 1,302 unsupported exact-cover rows, with zero timeout, error,
  invalid row, or paired objective mismatch. The evidence source commit is
  `252d01f08c6ba64b17b8fe22ce7317d7c2d58c76`; raw and analyzed artifacts are
  committed separately after that source run.

## P16 Paper-Kernel-Scaling Campaign

The in-process `paper-kernel-scaling` campaign is complete for its declared
finite population. One release process owns each family/size partition,
reuses one canonical component, counterbalances the three timed production
paths, and records Scope A (solve from canonical input) and Scope B
(representation-and-solver kernel) separately. The plan contains six families
and eight target levels. It produced 45 complete points, three predeclared
dense-conflict stop states, 1,070,372 retained measured iterations, 138
production correctness checks, zero mismatches, zero duplicate sample
identities, and zero missing planned points. The source commit is
`103700eaa2b55de14daab010a82556efdf16fb84`; binary and config hashes are
recorded in the full summary and archive manifest.

The dense-conflict target 2048 exceeded the five-second per-iteration limit
during preflight. Targets 4096 and 8192 are explicitly retained as propagated
stop states, not inferred measurements. This satisfies the predeclared stop
rule and does not justify extrapolation beyond complete levels.

P16 does not replace P15. P15 remains fresh-process evidence; Scope A removes
process startup and CLI/config/serialization overhead while retaining the full
solve; Scope B additionally excludes common geometry, completion, and final
verification. Their ratios must not be pooled, and P16 does not prove an
asymptotic runtime bound, AN19 runtime, universal speedup, or a crossover
outside the measured families and host.

## P17 Geometry-Phase Diagnostic and Boundary Discovery

**State.** Complete for the declared finite campaign. This phase diagnoses the
shared geometry cost exposed by P16 Scope A measurements and compares an
internal reference boundary path with a prepared-occupancy path. It is finite,
host-specific evidence; it does not alter the P15 schema or create an
asymptotic claim. The detailed audit is
[`phase-reports/P17-geometry-phase-diagnostic.md`](phase-reports/P17-geometry-phase-diagnostic.md).

**Measured fact before implementation.** The historical
`Boundary::from_component` path probes four directed edges per occupied cell,
uses hash-set cancellation for shared edges, then builds the same adjacency,
ordered trace, normalization, reflex, and sorting structures. For the
representation-crossover family at target 8192, the deterministic generator
has `t=91`, `N=390131`, `q=364`, and `U=69944`; the reference path therefore
probes `4N=1,560,524` candidate edges and retains only `U` exposed edges. Prior
measurements attributed roughly 97% of geometry preprocessing to this boundary
extraction. These are implementation measurements, not a proof of asymptotic
complexity.

**Implemented optimization.** `Boundary::from_prepared_component` probes the
prepared occupancy once per cell and inserts only exposed edges. Loop tracing,
normalization, area conservation, reflex identification, endpoint identities,
and deterministic ordering remain shared with the reference implementation.
The reference edge-toggle path remains available under
`mrd_domain::context::oracle` for differential verification. The optimization
reduces cancellation work and transient internal-edge storage; both paths
retain the same output-sensitive boundary, adjacency, and sorting terms, so no
new asymptotic bound is claimed.

**Protocol.** Schema-v2 kernel rows carry setup, boundary, chord, conflict,
representation, flow/matching, cover, completion, reconstruction, and output
validation timings as monotonic integer nanoseconds. Parent/child accounting is
checked for boundary, geometry, chord, completion, output validation, Scope A,
and Scope B. `A` is the foreground component bounding-box area and is checked
as `N <= A <= width*height`; `M` is compact network nodes plus arcs. Reference
and optimized campaigns use identical seeds, families, sizes, repetition rules,
and binary provenance. The comparison config is consumed by the analyzer and
requires equal protocol fields, source commit, release binary, generator
identity, and canonical instances.

**Evidence boundary.** The six-family, ten-level reference and optimized
campaigns contain 60 planned and observed points per backend, 57 complete
points, three stopped points, 9,826 reference iterations, and 9,954 prepared
iterations. The paired run has 57 complete canonical-instance pairs, 174
correctness checks per backend, zero structural/objective mismatches, and a
successful `--resume` audit for both backends. The source provenance is
`211308a4981c09ccd549bd0ed322db847f427ce3`; the release binary hash is
`59ae735a1c99726cb9d298aba1aafc71d5aae4da6e26683d248f6dddb6821ba3`. Fits
require at least six valid levels and are empirical descriptors only. Stopped
points remain retained and censored; they are never treated as timing
observations. RSS is not claimed: the executable budget applies to estimated
retained structural bytes, while `max_rss_delta_bytes` remains explicitly
unavailable.

**Analysis erratum.** The first P17 phase report selected only leaf timings and
then queried coarse geometry/completion parents that were absent from its phase
table. Its clone/representation dominance paragraph was not supported by the
accepted raw artifact and is corrected in the phase report without rewriting
the historical raw files. The largest Scope A parent is family-dependent:
completion dominates comb-staircase, dense-conflict, random-connected, and
representation-crossover, while geometry dominates sparse-conflict and
supported-holes. Representation remains a candidate for a separately measured
experiment, not a proven global bottleneck.

P9.3.2d remains implementation-path complete but proof-path deferred and low
priority; this phase cannot close the reduced-event conversion or automatic
target-decision obligations.

## P18 Canonical Sharing and Representation Follow-up

**State.** The P18 implementation separates immutable canonical input from
algorithm-local mutable workspace at the benchmark Scope A boundary. The
`clone-canonical-reference` path remains an internal executable reference;
`borrowed-canonical` uses ordinary Rust borrowing and removes the deep
`GridComponent.cells` copy. There is deliberately no executable
`shared-prepared-context` identity: a future prepared-context reuse experiment
must have distinct semantics and its own evidence.

**Evidence rule.** P18 is a finite ownership/measurement-harness optimization,
not a new production asymptotic algorithm. Scope A records clone, borrow/share,
release, and workspace preparation separately; Scope B does not construct the
Scope-A selection workspace. Structural byte values are capacity-based payload
estimates, not allocator or RSS measurements. Clean source/binary/config
provenance, exact paired sample order, adaptive repetition census, and zero
structural/objective/witness mismatches are hard gates. A Scope A confidence
interval crossing 1.0 is a valid negative result, especially for the primary
`representation-crossover` family.

**Next experiment.** Only after clone closeout may one implementation change be
measured: a per-algorithm, per-campaign-lane reusable compact representation
workspace, deterministically reset before each solve. No selector, hybrid policy,
zero-conflict shortcut, or representation rewrite belongs in P18.

## P15 Paper-Scaling Campaign

**Status.** Complete for the predeclared finite campaign. The full run and
analysis artifacts are committed under the `paper-scaling-full` names. This
phase is independent of the P9 source-flow blocker and cannot close an AN19
proof obligation.

**Implementation.** `mrd benchmark --suite paper-scaling` executes one
versioned deterministic request. `verification::paper_scaling` exposes compact
MRD, explicit Hopcroft--Karp, explicit C0 flow, and bounded exact-cover Oracle
paths. The process runner uses release binaries, a fixed seed, fresh processes,
per-pair Fisher--Yates order, configurable censoring, and raw-row retention.
The analyzer emits robust summaries, paired bootstrap intervals, predeclared
size-level fits, booktabs tables, and SVG figures without notebook state.

**Acceptance boundary.** The full configuration specifies all seven families,
eight target sizes, three warm-ups, 31 measured runs for target sizes through
27, 15 thereafter, a 60-second timeout, and a six-size-level fit minimum.
No exponent is emitted below that minimum. `exact-cover-oracle` is explicitly
unsupported over its cell limit; that state is retained and never counted as a
compact win. The completed run establishes the reproducible chain, output
schema, correctness gate, censoring, and generated artifacts over the declared
finite population. It reports empirical fits only where six valid size levels
exist; the bounded exact-cover Oracle has no valid six-level timing fit. A
crossover is reported only for `representation-crossover` at target 60, and is
not a universal backend policy.

## P9.3.2d Runtime-Proof Deferral

**Implementation path: complete and nonblocking. Proof path: deferred and low
priority.** The formal SIAM journal version of Abraham--Neiman, DOI
`10.1137/17M1115575`, was checked. It does not provide the required conversion
from original power-of-two edge-length classes to a bounded number or ordering
of exact reduced-event classes for
`c_x(u,v) = ell(u,v) + d(x,u) - d(x,v)`.

The missing obligation is: given the event objects generated by the cited AN19
construction, prove an explicit upper bound on reduced-event equivalence
classes and justify the exact ordering transformation used by the
implementation, sufficiently to derive the claimed runtime.

Until P9.6a closes this obligation, the backend must not be named
`AlmostLinear`, report `an19_runtime_verified: true`, or make an AN19
asymptotic runtime claim. Finite tests, source scans, trace audits, and local
event certificates establish implementation evidence only. This proof debt
does not invalidate the faithful implementation or its exact Oracle
differential.

## Active Blocker: P9.5e.3g.3

**Goal.** Provide an automatic, source-backed decision procedure for the
unknown optimum `F*`, or establish an evidence-backed reason that no such
procedure is available from the cited route.

**Status.** Blocked. The public source path accepts a caller-supplied inclusive
target and can certify a completed feasible run or verify a supplied negative
certificate. It cannot discover `F*`. The checked Appendix C.9 route assumes
an exact primal optimum and invokes an exact solver again; it cannot bootstrap
an independent negative decision.

**Allowed behavior.** Preserve the `SourceWithTarget` interface, accurate
provenance, and `UnsupportedOrUndetermined` outcomes. Never convert execution
failure into target infeasibility and never insert a reference-solver fallback.

**Exit evidence.** A new source must provide a valid constructive target
decision and a proof/implementation audit that is independent of a known
optimum. Until then, no automatic source solver entry point or binary search
over `F*` is permitted.

## Deferred Proof Debt: P9.6a

**Goal.** Resolve the reduced-event conversion, ordering, hierarchy-wide
amortization, and source-equivalent priority-queue bound needed for the AN19
runtime chain.

**Priority.** Low. It becomes eligible only after a complete source-flow
backend has a valid automatic target contract. Its completion requires source
traceability, formal argument review, implementation counters that match the
argument, and an independent audit. Empirical event counts cannot close it.

## Global Rules

- Read this plan and the relevant current design document before changing a
  phase or claim.
- Preserve user work; never force-push or push directly to `main`.
- Retain independent reference backends permanently.
- Do not weaken an assertion merely to pass a test or silently skip an
  unsupported source theorem.
- Never claim a complexity bound unless the code, assumptions, counters, and
  proof all match.
- Minimize every correctness disagreement into a permanent regression.
- Keep exact inputs, seeds, filters, and outcomes in machine-readable results.
- Distinguish finite verification, local measurement, implemented semantics,
  and proved asymptotic statements in every report.
- Do not call the source backend automatic, `AlmostLinear`, or AN19-runtime
  verified while either active obligation remains open.
- Update this plan and `HISTORY.md` only for durable state changes; keep
  command transcripts and generated data in their proper artifacts.

## Audit Before a Future Phase Closeout

Run the applicable commands and record their outputs, versions, durations, and
result files:

```text
git status --short
git diff --check
cargo fmt --all -- --check
python3 tools/check_biclique_bound.py
python3 tools/check_source_flow_audit.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo build --workspace --release
python3 tools/check_release_consistency.py
```

Also inspect staged changes, ignored-test status, fallback boundaries, result
provenance, and machine-local paths. A phase-specific campaign is mandatory
when an acceptance condition requires it. Before a push, fetch `origin`, check
for divergence, push only `codex/full-implementation`, and verify the remote
SHA matches local `HEAD`.
