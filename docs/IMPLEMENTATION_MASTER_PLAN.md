# MRD Full Implementation Master Plan

- Plan schema: 1
- Current branch: codex/full-implementation
- Baseline local SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Baseline origin/main SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Current phase: P11
- Current phase state: in_progress
- Last completed phase: P10
- Last pushed SHA: 06a030d84fcd9b86b451c580f72be818e885c0e7
- Plan last updated: 2026-08-03T11:19:20Z
- Overall target: complete source-traceable geometry, deterministic
  almost-linear exact flow, direct grid parity embedding, constant-factor
  hardening, and final reproducible evidence.

## Verified baseline and gaps

The baseline is workspace version `1.3.0` at `72ce32a`; release tag
`v1.3.0-output-sensitive-sparse-geometry` peels to `533e37a`. It implements
independent exact-cover and ordinary-polygon Soltan--Gorpinevich Oracles,
ordinary-loop `sg-sweep`, indexed completion, output-sensitive subdivision,
event-driven validation, ranked 4D parity embedding, Cardinal--Yuditsky
Theorem 8 biclique partition, compressed Dinic flow, and grid path-tree
specializations. The permanent reference backends remain part of the contract.

Known remaining gaps, established by current code and
`docs/KNOWN_LIMITATIONS.md`, are recursive re-sorting in biclique
construction; only Dinic for flow; no
source-backed deterministic almost-linear exact flow; no direct grid parity
embedding; and unprofiled constant factors. No phase may upgrade a claim until
its implementation, assumptions, counters, and evidence establish it.

## P9 architecture maintenance

The repository-wide functional architecture refactor is recorded by
implementation commit `2b0036545cdd2cfe8a26c3655aa4c72fd6b1791f` and
`docs/phase-reports/P09-functional-architecture-refactor.md`. The workspace now
uses responsibility-bearing package names and explicit `oracle` and
`experiment` namespaces without compatibility re-exports. This maintenance
changes ownership paths and names only. P9.3.2d's implementation is now
accepted as a faithful source-shaped implementation with explicitly deferred
proof debt; the refactor itself does not verify the AN19 runtime.

## P9.3.2d Runtime-Proof Deferral

**Implementation path: nonblocking. Proof path: deferred, low priority.** The
formal SIAM journal version of Abraham--Neiman, DOI `10.1137/17M1115575`, was
checked. It does not provide the required proof converting original
power-of-two edge-length classes into a bounded number or order of exact
reduced-event classes for
`c_x(u,v) = ell(u,v) + d(x,u) - d(x,v)`. The repository therefore first
implements and differentially audits the complete source-shaped flow-solver
chain through P9.5, including its backend-completeness gate. After that chain
is complete, P9.6a returns to the missing reduced-event ordering and
hierarchy-wide amortization proof as explicitly deferred proof debt.

This deferral is deliberately not a license for a complexity claim. Until
P9.6a is completed, the backend must not be named `AlmostLinear`, report
`an19_runtime_verified: true`, or claim the AN19 asymptotic runtime. Finite
tests, exact Oracles, workspace-scan ledgers, local event certificates, and
the full P9.5 semantic flow campaign establish implementation evidence only;
they are not a proof of the missing conversion or downstream runtime bound.

## Global Rules

- Before any phase, reread this entire Global Rules section and that phase.
- Never rely on memory from a previous Codex context.
- Preserve user work.
- Never force-push.
- Never weaken an assertion merely to pass a test.
- Never silently skip an unsupported source theorem.
- Never claim a complexity bound unless code, assumptions, and counters match it.
- Reference backends remain available permanently.
- Every correctness disagreement is minimized and committed as a regression.
- Every phase must update this Markdown before being considered complete.
- Every phase must commit and push automatically after a complete audit.
- Push only to `codex/full-implementation`.
- Verify the remote branch SHA after every push.
- Tags may be pushed only for an explicitly named release phase after all gates.
- Do not create a GitHub Release.
- A blocked phase must persist the blocker, evidence, and next action in this
  Markdown and push the blocker report when possible.
- If a phase is too large, split it into numbered subphases in this same file
  before coding.
- After every successful phase push, reopen this Markdown, reread Global Rules,
  reread the complete next phase, set it to `in_progress`, and continue
  automatically.
- Stop only for a hard source gap, failed correctness audit, remote divergence,
  unavailable credentials, or exhausted execution context. Persist state first.

## Mandatory audit protocol

Every phase records the exact command, exit status, duration, result files,
phase baseline, and phase-specific differential/benchmark gates in its report.
Run, unless a report records an evidence-backed blocker:

```text
git status --short
git diff --check
cargo fmt --all -- --check
python3 tools/check_biclique_bound.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo build --workspace --release
python3 tools/check_release_consistency.py
```

Also inspect every staged diff; accidental ignored tests; fallback use; stale
generated evidence; credentials, tokens, private keys, and local absolute
paths; and comparison against the phase baseline. A phase whose acceptance
requires a release campaign cannot be pushed after unit tests alone.

## Commit and push protocol

1. Make logical implementation commits.
2. Run the complete phase audit.
3. Generate `docs/phase-reports/PXX-<slug>.md`.
4. Update this plan with status, implementation commits, audit commands,
   result files, disagreements, and remaining limitations.
5. Create a phase-closeout commit.
6. Fetch `origin` and confirm no remote divergence.
7. Push `codex/full-implementation` only.
8. Verify the remote SHA equals local `HEAD`.
9. Reopen this file and reread the Global Rules and complete next phase.

## P0 - Persistent plan and baseline

**State:** complete. **Start SHA:** `72ce32a`. **Goal:** persist this plan,
verify branch policy, run the complete baseline quality gate, commit, push,
verify the remote SHA, and stop. Do not implement code in P0.

**Acceptance:** this file is the complete source of truth; the dedicated branch
was created from freshly fetched `origin/main`; the baseline and release tag
were verified; audit output is preserved in `docs/phase-reports/P00-persistent-plan-and-baseline.md`;
only plan/report documentation changes are committed.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P1 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and
   `Last pushed SHA`.
7. Begin P1 automatically unless a persisted hard blocker exists.

## P1 - v1.3 baseline freeze and full audit

**State:** complete. **Start SHA:** `deee489`. Regenerate and archive complete v1.3 correctness,
external-oracle, sparse geometry, release-provenance, fallback, ignored-test,
and benchmark baseline. Require no unexplained disagreement or stale
provenance; commit the baseline used by later phases. Suggested release:
`v1.3.1-baseline-freeze`, only if repository conventions justify a patch tag.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P2 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P2 automatically unless a persisted hard blocker exists.

## P2 - Formal boundary representation

**State:** complete. **Start SHA:** `fe1be92`. **Implementation SHAs:**
`95abbcf`, `b8c2d15`. Implement ornaments, point holes, segment holes, isolated
formal-boundary points, formal incidence, canonical normalization, exact
serialization, and structured validation. Preserve the ordinary polygon model
as an Oracle. Suggested release: `v1.4.0-formal-boundary-model`.

### P2 subphases

1. **P2.1 - Source contract and formal model.** Map Soltan--Gorpinevich
   Definitions 1, 3, and 4 (pp. 58--60) to explicit Rust types, invariants, and
   structured errors. Preserve `RectilinearPolygon` unchanged as the ordinary
   topological-region Oracle.
2. **P2.2 - Canonical normalization and incidence.** Normalize isolated points
   and ornament segments deterministically; validate exact containment and
   intersection rules; derive formal vertices, elementary segments, incidence,
   and connected formal-boundary components with stable IDs.
3. **P2.3 - Serialization, integration, and audit.** Add the tagged JSON model,
   round-trip/canonical/metamorphic/negative fixtures, public documentation,
   complete differential agreement on empty ornaments, and the full phase
   audit. P3 solver behavior must remain explicitly unsupported until P3.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P3 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P3 automatically unless a persisted hard blocker exists.

## P3 - Formal-hole SG sweep and completion

**State:** complete. **Start SHA:** `521f82d`. Implement the original Soltan--Gorpinevich formal-boundary
event model, merge/delete cases, effective chords, completion, sparse
subdivision, and validation for newly supported degeneracies. Require exact
ordinary-input agreement, source-mapped invariants, and dedicated formal-hole
fixtures. Suggested release: `v1.5.0-formal-hole-geometry`.

**Acceptance:** complete. Implementation commits `093961f`, `fd1bbc6`,
`3d94851`, `996ad44`, `659d7fb`, and `6cd0845` implement all five
subphases. The full audit is recorded in
`docs/phase-reports/P03-formal-hole-geometry.md`; permanent evidence is
`results/p3-formal-fixtures.json` and
`results/p3-polygon-differential.json` with its empty counterexample file.
There were no correctness disagreements or unresolved P3 limitations within
the accepted one-component formal input model. Closeout and verified remote
SHA are `3b8347aa1b288a19ec9e07a8474fc591f2281598`.

### P3 subphases

1. **P3.1 - Local nonconvexity and Definition 7 Oracle.** Derive inner-angle
   sectors and the source measure at every formal vertex, including isolated
   points and vertices with coincident or multiple incident elementary
   segments. Implement an exact pairwise Definition 7 predicate/enumerator as
   the permanent formal-boundary Oracle and verify the paper's Fig. 3 family.
2. **P3.2 - Source merge/delete effective-chord construction.** Implement the
   axis-generic Section 10 Step 1(a)--(d) construction, including finite formal
   boundary contacts, repeated endpoint deletion, repeated non-isolated-vertex
   merging, canonical fixed-point filtering, provenance, and proof-matching
   counters. Require exact equality with the P3.1 Oracle and the existing
   ordinary `sg-sweep` on empty ornaments.
3. **P3.3 - Formal admissible-family and optimum integration.** Feed formal
   chord families, including collinear chords sharing isolated endpoints,
   through the permanent explicit and compact conflict/matching Oracles without
   changing orthogonal-intersection semantics. Verify identical maximum
   admissible families, cuts, covers, and optimum counts across backends.
4. **P3.4 - Formal completion, subdivision, and validation.** Add selected
   effective chords to the formal boundary, repeatedly add source-valid simple
   chords that remove remaining local nonconvexity, recover rectangles from the
   resulting formal subdivision, and validate Definition 2 coverage without
   raster assumptions. Preserve ordinary completion and sparse validation as
   differential Oracles.
5. **P3.5 - CLI integration and phase audit.** Enable formal `solve`, add
   dedicated point-hole, segment-hole, attached-hole, shared-endpoint, and
   source-example fixtures, document theorem-to-code invariants, run ordinary
   and formal differential campaigns, and complete the mandatory audit.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P4 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P4 automatically unless a persisted hard blocker exists.

## P4 - Presorted compact biclique construction

**State:** complete. **Start SHA:** `3b8347a`. Remove recursive re-sorting from Cardinal--Yuditsky using
presorted coordinate orders, stable partitioning, reusable scratch arenas, and
structural complexity counters. Require exact edge partition, matching, cut,
cover, and rectangle equality with the current implementation. Suggested
release: `v1.6.0-presorted-biclique-construction`.

### P4 subphases

1. **P4.1 - Dual backends and structural counters.** Preserve the recursive
   re-sorting construction as a permanent reference backend. Add a presorted
   backend that performs one initial sort per coordinate, maintains every
   coordinate order through stable filtering, reuses recursion scratch arenas,
   and records initial sorts, recursive sorts, stable-partition visits,
   scratch growth, recursive nodes, and emitted vertex occurrences.
2. **P4.2 - Production integration and exact differential.** Switch every
   general 4D grid, ordinary-polygon, and formal-polygon call path to the
   presorted backend. Require canonical block equality where construction order
   is specified and, in all cases, exact edge partition, matching value,
   minimum-cover certificate, selected chords, optimum count, and canonical
   rectangle equality with the reference backend. Keep path-tree construction
   independent and unchanged.
3. **P4.3 - Scaling evidence and phase audit.** Run exhaustive, random,
   polyomino, polygon, formal-hole, adversarial, and dense-conflict
   differentials. Archive structural counters and before/after construction
   timings, prove production recursive sort count is zero, complete the
   mandatory audit, report, closeout commit, push, and remote-SHA verification.

**Acceptance:** complete. Implementation commits `85c1083`, `bfa5a94`,
`4cf8250`, `9238066`, and `f5c387e` preserve the recursive-sort construction
as a permanent reference backend and make the presorted construction the
production path. `biclique::Metrics` proves four initial production
sorts and zero recursive sorts. The audited construction requires canonical
partition equality and equal emitted occurrences before downstream flow is
used. P4 evidence is recorded in
`docs/phase-reports/P04-presorted-biclique-construction.md` and the listed
`results/p4-*` artifacts. All campaign disagreement and solver-error counts
are zero. The measured presorted construction is sometimes slower at this
small scale because stable filtering and audit comparisons add work; P4 claims
the eliminated recursive sorts, not a universal timing win.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P5 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P5 automatically unless a persisted hard blocker exists.

## P5 - Exact flow backend framework

**State:** complete. Generalize flow selection and add robust practical exact
backends, including highest-label push-relabel with global relabel and gap
heuristics; preserve Dinic. Require exact value and canonical source-cut
equality where applicable, valid certificates otherwise, generic-network
differential fuzzing, and compressed-network benchmarks. Suggested release:
`v1.7.0-exact-flow-backends`.

**Acceptance:** complete. Implementation commits `1636b75`, `986026d`, and
`90acf15` retain `DinicBackend`, add the permanent `PushRelabelBackend`, and
expose `FlowBackendKind` plus `solve_with_flow_backend` for fully audited grid
solves. The latter preserves the existing Dinic default. Generic differential
tests cover 1,024 deterministic directed integral networks and validate each
push-relabel cut capacity; compressed evidence records exact value equality on
seven dense sizes through 512-by-512 chords. Report and persistent evidence:
`docs/phase-reports/P05-exact-flow-backends.md`,
`results/p5-flow-backends.csv`, and `results/p5-flow-backends.json`. There are
zero counterexamples and solver errors. The new backend is intentionally not
selected adaptively or described as almost-linear.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P6 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P6 automatically unless a persisted hard blocker exists.

## P6 - Source-backed almost-linear flow specification

**State:** complete. Read the FOCS 2023 / arXiv `2309.16629` source and required
predecessors. Create `docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md`, mapping every
theorem, data structure, assumption, precision rule, and recovery step to an
intended Rust module. Do not claim implementation from an interface skeleton.
Mark blocked, rather than inventing a component, if authoritative source is
unavailable.

**Acceptance:** complete. `docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md` pins the
primary source and CKLPPS22, KP15, CS21, and ST83 predecessors; maps every
required theorem/data structure/assumption/recovery step to intended Rust
modules; and records exact arithmetic, hidden-stability, undirected-spanner,
and no-fallback constraints. No advanced implementation is claimed.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P7 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P7 automatically unless a persisted hard blocker exists.

## P7 - Exact min-cost circulation and iterative-refinement core

**State:** complete. **Start SHA:** `9f12445`. **Implementation SHAs:**
`3184ad5`, `4e8326c`, `21fbbd9`. Generic circulation now has exact signed
`i128` capacities, costs, demands, residual operations, static and signed
residual minimum-ratio-cycle Oracles, iterative refinement traces, and exact
recovery verification. Superlinear work is deliberate: no IPM, stability,
rounding, dynamic structure, or almost-linear claim is made. The bounded-flow
differential covers all 125 three-cost assignments and reports zero
disagreements. Full-audit evidence and remaining limitations are recorded in
`docs/phase-reports/P07-exact-circulation-refinement.md`.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P8 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P8 automatically unless a persisted hard blocker exists.

## P8 - Deterministic dynamic minimum-ratio-cycle structures

**State:** complete. **Start SHA:** `237d55b`. P8 is deliberately split
before implementation. No subphase may claim the primary source's amortized
bound unless its own checked operation domain, source-shaped accounting, and
acceptance evidence are present. Each subphase receives an audit, report,
commit, push, and reread transition.

### P8.1 - Checked stable min-ratio state contract

**State:** complete. **Implementation SHA:** `37088c9`. Definitions
4.2--4.5 are now mapped to exact Rust state: signed incidence, positive
lengths, gradients, valid-pair bounds, checked witness stability, and replayable
`Update`/`Query`/`Detect` logs. The six focused invariants have zero
disagreements; full evidence is in
`docs/phase-reports/P08-1-stable-min-ratio-contract.md`. This remains only a
checked representation and static invariant layer, with no dynamic or
amortized claim.

### P8.2 - Dynamic rooted-forest primitive

**State:** complete. A deterministic exact baseline now enforces decremental
forest-edge removal, permitted vertex splits, rooted path update/query, static
Definition 5.3 stretch certificates, and recourse counters. The static BFS
recomputation remains the forest Oracle. Full evidence is in
`docs/phase-reports/P08-2-rooted-forest-primitive.md`; no Lemma 5.4 runtime
or construction claim is made.

### P8.3 - Scope-limited decremental spanner certificates

**State:** complete. The P8.3 certificate layer enforces the simple
undirected deletion/vertex-split domain, explicit embedding paths, congestion,
deletion/split validity, and recourse accounting. It rejects directed,
insertion, arbitrary-update, and generic-spanner semantics. Full evidence is
in `docs/phase-reports/P08-3-decremental-spanner-certificates.md`; no Theorem
8.2 construction or bound claim is made.

### P8.4 - Deterministic low-stretch forest collection

**State:** complete. A deterministic checked reweighting baseline now builds
weighted Kruskal forest candidates, records exact per-edge average-stretch
certificates and operation counts, and retains P8.2 static stretch computation
as the Oracle. Full evidence is in
`docs/phase-reports/P08-4-forest-collection.md`; no Lemma 5.5 bound or
production construction claim is made.

### P8.5 - Compact cycle tree chain and hidden-stability query

**State:** complete. The P8.5 baseline now decodes compact signed cycles with
P7 exact conservation validation, replays deterministic shift/rebuild traces,
and composes P8.1 `Update`/`Query`/`Detect` replay. Full evidence is in
`docs/phase-reports/P08-5-compact-cycle-chain.md`; it makes no Theorem 5.1
query, dynamic data-structure, or amortized-bound claim.

### P8.6 - Dynamic minimum-ratio integration audit

**State:** complete. The P8.6 audit integrates checked P8 replay, P7 static
cycle validation, explicit unsupported-operation rejection, and exact replay
work counters. Full evidence is in
`docs/phase-reports/P08-6-dynamic-min-ratio-audit.md`. P8 supplies baseline
components only; P9 retains every end-to-end and almost-linear source gate.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P9 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P9 automatically unless a persisted hard blocker exists.

## P9 - Integrated deterministic almost-linear exact flow

**State:** in_progress. The source-assumption audit is recorded in
`docs/phase-reports/P09-integration-gate-audit.md`. The first corrective
subphase is recorded in `docs/phase-reports/P09-fractional-rounding.md`:
exact rational feasible-flow validation, deterministic costed cycle rounding,
and a bounded-domain rational surrogate-potential verifier now exist. These
are explicitly superlinear/auditing components, not the fixed-point IPM from
the source. Source-grade IPM accounting, a source-grade Theorem 5.1 query,
source-grade P8 constructions, and MRD integration evidence remain missing. No `AlmostLinear`
backend or fallback has been introduced. Implementation may proceed through
the complete experimental flow backend while missing source complexity proofs
remain explicit proof debt. Runtime naming and release claims stay gated.
Suggested release:
`v2.0.0-deterministic-almost-linear-flow`.

P9 is split into implementation and proof gates. Completion of an Oracle or
source-shaped implementation subphase proves only its stated semantic audit
contract. Missing source complexity arguments do not block construction of the
remaining pipeline, but they do block the `AlmostLinear` name, the AN19 runtime
claim, P9 complexity closeout, and a runtime-claiming release.

### P9.1 - Exact fractional and integration Oracles

**State:** complete as an Oracle baseline. Commits `0abf2a1`, `d1cefdd`,
`48250f5`, `959ed00`, `4e66be4`, and `ffb371f` provide exact rational
fractional-flow validation and rounding, a rational surrogate-potential state,
an enumerating dynamic-cycle Oracle, a static greedy-spanner constructor,
compressed-flow parity, and the restored practical Push--Relabel reference.
The reports under `docs/phase-reports/P09-*.md` preserve each limitation. None
of these components is source-grade evidence for Theorem 4.6 or Theorem 5.1 of
the deterministic paper.

### P9.2 - Certified fixed-point IPM

**State:** complete. **Start SHA:** `ffb371f`. Implement the deterministic
paper's Theorem 4.6 interface by tracing its delegated CKLPPS22 Theorem 4.3
potential-reduction method. This subphase is further split so numerical
correctness cannot be inferred from an end-to-end test alone:

1. **P9.2.1 state: complete. Implementation SHA: `44d50d4`.** Bounded-word
   dyadic interval arithmetic now has explicit precision, outward rounding,
   overflow/word-size accounting, and certified enclosures for `log(x)` and
   `x^-alpha` on the checked positive domain. The full audit and mathematical
   error bounds are in
   `docs/phase-reports/P09-fixed-point-arithmetic.md`.
2. **P9.2.2 state: complete. Implementation SHA: `bba500e`.** Equation (9)
   and Definition 4.2 lengths/gradients are evaluated with certified error
   intervals; every approximation proves the factor-two length and
   scaled-gradient error hypotheses used by Theorem 4.3. Evidence is in
   `docs/phase-reports/P09-certified-ipm-quantities.md`.
3. **P9.2.3 state: complete. Start SHA: `bba500e`; implementation SHA:
   `ab6fb10`.** Lemma 4.4 updates now enforce circulation, exact ratio quality,
   source step size, strict-interior successor validation, certified potential
   decrease, iteration/coordinate counters, and lower-bound Detect accounting.
   Evidence is in `docs/phase-reports/P09-lemma-44-updates.md`.
4. **P9.2.4 state: complete. Implementation SHAs: `5719917`, `82ac962`,
   `aabe7fc`.** Exact lower-bound normalization, Appendix B.1 augmentation,
   initial/additive-half certificates, KP15 rounding, Lemma 4.11 perturbation
   constants and probability contract, and P7-verified recovery are complete.
   Evidence is in
   `docs/phase-reports/P09-initial-termination-recovery.md`.

P9.2 is complete. Its fixed-point, quantity, transition, initialization,
termination, and recovery layers retain explicit source assumptions and make
no dynamic-runtime claim.

No P9.2 item may use `f64`, the existing rational surrogate, or an unchecked
transcendental result as evidence for Equation (9). The fixed-point word-size
gate must match the papers' `O(log^O(1) z)` model and reject inputs whose chosen
precision cannot certify the required inequalities.

### P9.3 - Source-grade low-stretch and spanner structures

**State:** in_progress. **Start SHA:** `aabe7fc`. Replace the P8/P9 static
certificate baselines with the source constructions and accounting required by
Lemmas 5.4--5.5, Theorem 8.2, and Theorem 1.2. Preserve each exact
constructor/verifier as an Oracle. Require the precise decremental/vertex-split
domain, embeddings, stretch, congestion, recourse, rebuild, and bounded-weight
assumptions before recording a source runtime guarantee.

P9.3 is split into the following source-gated subphases:

1. **P9.3.1 state: complete. Implementation SHAs: `6e96916`, `934576d`.** Exact source
   parameters and checked contracts for positive lengths/weights, `k`, update
   batches, decremental forest edges, root additions, partitions, stretch
   overestimates, embeddings, encoded recourse, and quasipolynomial bounds are
   audited in `docs/phase-reports/P09-source-structure-contracts.md`.
2. **P9.3.2 state: complete. Deferred proof debt is nonblocking. Start SHA: `6e96916`; implementation SHAs:
   `9ac15b5`, `d0629c1`, `a9456e9`, `698ad7c`, `f5f91f6`, `7251038`,
   `2bca89c`, `a57e48c`, `6769ec1`, `3bb0400`, `839cb5c`, `20b0421`,
   `cdf732d`, `d6b8e6b`, `3d3afe2`, `be21982`, `720f0cb`, `27d5773`,
   `f54c29a`, `c02c7c9`, `ece2722`, `6901703`, `e4f54af`, `bc61592`,
   `14e9abb`, `8d68d59`, `5cc49f0`, `b050625`, `60fdfe4`, `0fc48a1`,
   `d17a6cd`, `0b3b704`.** The Appendix B.3
   heavy-light auxiliary tree, branch-free root closure, exact congestion
   permutation, and decremental `F_T(R,pi)` core are
   implemented. Equation (56)'s fixed global stretch vector is independently
   recomputed across every auxiliary-depth prefix and certified after root-set
   changes. Insertion/deletion batches now add endpoint ancestor closures,
   enforce decremental forest recourse, assign inserted stretch one, and rerun
   active-edge certificates. Appendix A.1 vertex splits preserve the abstract
   tree topology, add the old endpoint closure and an isolated new root, and
   retain the global stretch proof. Evidence is in
   `docs/phase-reports/P09-branch-free-forest-core.md`. Continue Lemma 5.4's
   externally delegated AN19 static LSST constructor. The full ST04
   `decompose/sub` algorithm, weighted-copy reduction, exact decomposition
   verifiers, boundary/high-stretch initialization, and all dynamic forest
   mechanics are implemented. A bounded exponential exact static LSST Oracle
   now supplies a permanent differential baseline; it is not AN19 and carries
   no production runtime claim. The recovered full AN19 source and exact
   implementation gates are mapped in
   `docs/phase-reports/P09-an19-static-lsst-source-map.md`.

   P9.3.2 continues through these source gates:

   - **P9.3.2a state: complete.** Recover Figures 4--6, Claims 1--15, the
     weighted extension, the directed region-growing construction, and the
     deterministic paper's `\widetilde O` notation. Persist exact formulas and
     reject vertex-rounded substitutes for imaginary portal points.
   - **P9.3.2b state: complete. Implementation SHA: `a57e48c`.** Exact
     single-petal membership, minimum-index radius windows, stopping
     inequalities, adaptive certified comparisons, and deterministic
     shortest-path tie breaking are implemented on the source's unit-length
     vertex domain. Fractional centers remain explicitly unresolved.
   - **P9.3.2c state: complete. Implementation SHA: `6769ec1`.** Symbolic
     interior portal points, rational Claim 15 region growing, exact highway
     interval halving, short-edge contraction/expansion, and original-edge tree
     recovery are implemented and differentially audited.
   - **P9.3.2d state: complete. Faithful implementation accepted; runtime proof deferred. Implementation SHAs: `3bb0400`, `839cb5c`,
     `20b0421`, `cdf732d`, `d6b8e6b`, `3d3afe2`, `be21982`, `720f0cb`,
     `27d5773`, `f54c29a`, `c02c7c9`, `ece2722`, `6901703`, `e4f54af`,
     `bc61592`, `14e9abb`, `8d68d59`, `5cc49f0`, `b050625`, `60fdfe4`,
     `0fc48a1`, `d17a6cd`, `0b3b704`, `7ea13da`, `28f9ff7`, `6c8cfac`,
     `98bb615`, `5e771d8`, `a25ac08`, `d4dda8f`, `02c8385`.**
     Substatus is intentionally split so implementation evidence cannot imply a
     runtime proof:

     | Substatus | State | Acceptance evidence |
     | --- | --- | --- |
     | P9.3.2d-impl: AN19-shaped exact event engine | complete | isolated `source_an19::experiment::event::Engine`; exact reduced costs; no numeric expansion, Oracle fallback, or unproved merging |
     | P9.3.2d-oracle: explicit exact event Oracle | complete | independent repeated-shortest-path thresholds and definition-level Figure 6 selection |
     | P9.3.2d-trace: canonical event and charge trace | complete | exact rational semantic/queue records, structural generations, state transitions, and six charge maps |
     | P9.3.2d-differential: exact semantic agreement | complete | 31 bounded A--H snapshots agree on normalized order, radius, membership, edge partitions, path, and stopping certificate |
     | P9.3.2d-counterexample: reduced-class adversarial families | evidence recorded; proof status open | bounded A--H campaign plus the earlier algebraic Family A lower bound; finite growth analysis is not treated as proof |
     | P9.3.2d-local-proof: fixed-snapshot event bound | complete | machine certificate proves at most `3n + 4m + 2` semantic events and `n + 2m + 2` queue items; exact-comparison time is explicitly excluded |
     | P9.3.2d-practical-pq-bound: fixed-snapshot binary heap | complete | stable exact heap certificate proves at most `3 I ceil(log2(max(I,1))) + 2m` counted comparisons; this is only `O((n+m) log(n+m))` |
     | P9.3.2d-global-proof: hierarchy-wide amortization | planned, low priority, deferred until the complete flow backend exists | no charge argument bounds all snapshots across recursion |
     | P9.3.2d-pq-proof: exact event-order data structure | planned, low priority, deferred until the complete flow backend exists | no proved source-equivalent `O(m+n log log n)` ordering structure |
     | P9.3.2d-runtime: AN19 runtime verification | planned, low priority, deferred until the two proof rows above are resolved | `source_runtime_verified()` remains false; this gates claims, not implementation |

     **Sequencing rule:** DOI `10.1137/17M1115575` does not supply the missing
     reduced-event ordering/counting proof. Preserve that fact as explicit
     proof debt, not as a hard implementation blocker. Proceed now through
     P9.3.3--P9.5 to build and differentially verify the complete source-shaped
     flow solver. After P9.5 passes its semantic and no-fallback audits, return
     to `P9.3.2d-global-proof`, `P9.3.2d-pq-proof`, and
     `P9.3.2d-runtime` before P9.6 may approve an `AlmostLinear` runtime claim.

     The exact
     arbitrary-rational Figure 6 selector derives parametric directed
     membership events and is differentially equal to the unit cone-union
     baseline. A stable-ID augmented workspace now supports exact rational edge
     splits, virtual leaves, dense Figure 6 projections, oriented original-edge
     provenance, and certified suppression back to an acyclic connected
     original tree. Figures 4--5 now compose recursively on the exact unit-length
     domain, including the source's `O(n)` imaginary first path, portal edges,
     once-only highway halving, independently checked radius witnesses, original
     tree/stretch verification, and 38 connected four-node Oracle differentials.
     Compact arbitrary-rational hierarchy support, recursive contraction,
     fast event processing, cluster-local projection, actual heap-comparison
     accounting, and scale-relative power-of-two length rounding are now
     implemented without numeric graph expansion or fallback. The source
     runtime proof remains open but is not an implementation gate. `ece2722`
     proves by potential reweighting that a
     fixed-radius Claim 15 ball can use original edge-length classes; 456 exact
     directed-distance differentials and the retained 128-node fixture pass.
     The all-radii Figure 6 event stream is faithfully implemented, but its
     claimed source runtime remains unproved: AN19's reduced cone
     metric has costs `l+d(x,u)-d(x,v)`, whereas EEST05 Definition 4.4 charges
     the original length only when leaving the forward-edge ideal. These balls
     are not source-equivalent, and KMPb Corollary 5.5 assumes distinct cone
     distances while Lemma 5.6 invokes it from distinct graph lengths without
     supplying the missing conversion for AN19's metric. `0b3b704` strengthens
     the retained 128-node, 162-class fixture to an algebraic family: for
     `N=2^q`, a unit path with power-of-two chords has exactly `q+1` original
     length classes but at least `N/2-1` distinct forward reduced costs. Hence
     an `O(log n)` reduced-class conversion is impossible for the exact AN19
     metric; this is a structural lower bound, not merely finite test evidence.
     The complete 2019 SIAM journal text (DOI `10.1137/17M1115575`, final
     Section 6, pp. 245--246) has now been inspected and repeats the same jump
     from original power-of-two lengths to improved Dijkstra on the reduced
     graph without bounding its distinct reduced costs. A read-only external
     source search completed on 2026-07-28 UTC checked the Crossref work,
     relation, and Unixref records; exact-title Crossref and DBLP results;
     OpenAlex's published-version record, both author catalogs, and all 18
     indexed citing works; and a focused DOI search for an erratum, correction,
     or supplement. It found only the 2012 STOC and 2019 SIAM versions and no
     authoritative correction, supplement, clarification, revised manuscript,
     or later proof of the exact conversion. Rejected and rate-limited requests
     are coverage limitations rather than negative evidence. The complete
     query ledger is persisted in
     `docs/phase-reports/P09-an19-static-lsst-source-map.md`. Recursive projections
     now densely remap every augmented cluster to exactly `|X|` local node
     slots while preserving original IDs in paths, portals, contraction, and
     radius certificates; local projection slot totals and maxima are counted.
     `e4f54af` additionally preserves a top-level source-edge attribution
     independently of quotient-local recovery provenance through arbitrary
     portal splits and nested contractions. Its `O(m)` aggregate audit counts
     per-original-edge and provenance-free segment occurrences, projected
     edges, and materialized projection length classes, then cross-checks every
     summary against the hierarchy metrics and requires every original input
     edge to occur. On the deterministic 500-node unit path, one original input
     length class becomes as many as 16 active projection classes; 16,948
     projected edge occurrences include 4,332 provenance-free segments, and
     one original edge reaches 111 segment occurrences. This is direct evidence
     that original power-of-two classes are not the materialized portal-segment
     classes and that fixed-cluster projection work repeats; it is not by itself
     an asymptotic counterexample or a structural amortization proof.
     `bc61592` reuses the already computed fixed `X` projection and shortest
     path when locating each nonvirtual first target. On the same fixture, seven
     reuses reduce projection calls from 172 to 165, projected edge occurrences
     from 16,948 to 15,856, provenance-free occurrences from 4,332 to 4,126,
     and the maximum source-edge occurrence count from 111 to 104. This removes
     a deterministic duplicate pass but does not yet reuse projections across
     later portal splits.
     `14e9abb` adds an `Rc`-shared single-cluster projection cache. Identical
     requests with no intervening workspace change reuse the exact snapshot in
     `O(1)`; virtual leaves, portal splits, and actual highway-length changes
     invalidate it, while a highway already halved preserves it. Portal splits
     compute all fallible provenance and node-count state before atomically
     invalidating the cache and mutating the workspace. The 500-node fixture
     records 26 cache hits: materialized projected edge occurrences fall from
     15,856 to 12,274 and workspace edge scans from 38,894 to 31,498. The
     maximum active projection length classes remains 16, so this is a
     constant-factor/materialization improvement rather than the missing
     source asymptotic proof.
     `8d68d59` extends that cache through portal splits. It queues exact stable
     split deltas and, once old readers release the snapshot, replaces the
     projected edge in place, appends the second segment and portal node, and
     updates an exact length-class multiset in `O(log m)`. A retained reader
     forces a full rebuild. Shape auditing observes cached active edge/class
     maxima without charging a full materialization. The 500-node fixture has
     83 cache hits and 39 incremental splits; materialized projected edge
     occurrences fall from 12,274 to 5,974 and workspace scans from 31,498 to
     18,290. The corrected active-class maximum remains 16, and a source edge
     still has 33 materialized occurrences versus 9 logarithmic levels.
     `5cc49f0` assigns every active segment an independently audited symbolic
     label containing its top-level source, unsplit rounded length, and highway
     halving state. Portal splits copy the label, incremental cached splits
     retain it without changing symbolic class sets, and recursive quotient
     graphs validate and inherit it independently of quotient-local recovery
     provenance. Full projection materialization recomputes exact source and
     virtual symbolic class sets; cached shape observations maintain their
     maxima, and the public audit cross-checks both against hierarchy metrics.
     On the same 500-node fixture, 16 active materialized length classes reduce
     to 2 effective source-label classes and 3 virtual-label classes. This
     removes portal segmentation from the symbolic source-class count, but
     equal labels do not prove that arbitrary Figure 6 candidate distances may
     share a monotone queue, and the 33-occurrence per-source observation still
     exceeds the 9 logarithmic levels.
     `b050625` separates logical partition scales from same-scale quotient
     contraction calls. Every radius certificate records its recursion parent,
     partition depth, contraction status, and top-level source for each stored
     edge. Verification requires every partition child radius to be at most
     `3/4` of its parent's exact radius, every quotient contraction to retain
     the same depth under a parent with a checked contraction, and the complete
     preorder to reproduce the recursion/contraction counters. It independently
     rebuilds the per-source scale-occurrence vector from the radius
     certificates and enforces both `max_source_occurrences <= max_depth + 1`
     and the Section 6 consequence `max_depth < 6 ceil(log2 n) + 4`. On the
     500-node nonvirtual path, 46 logical calls have maximum depth 8; 2,256
     source-scale participations have per-source maximum 9 and require 2,919
     attribution scans. The alternating rational path has 1,983
     participations, per-source maximum 7, and 2,016 attribution scans. These
     counters are scale invariant and mutation checked. They close the
     per-source logical-scale gate, not the separate materialization gate:
     source segments are still materialized up to 33 times on the unit path.
     `60fdfe4` separates those 33 segment occurrences into one source charge
     per full projection plus explicit descendant portal-fragment charges.
     Every interior split is attributed to its top-level source or to the
     provenance-free virtual class. The verifier enforces at most four source
     projection entries per certified scale plus one initialization entry,
     charges every extra source segment to `source splits * scale entries`, and
     separately charges virtual segment occurrences to
     `(virtual leaves + virtual splits) * scale entries`. On the 500-node
     nonvirtual path, 5,974 projected edge occurrences contain 4,533 source
     projection materializations, 61 extra source-fragment materializations,
     27 source-attributed splits, 22 virtual splits, and 1,380 provenance-free
     segment occurrences; maximum per-source materializations and fragments
     are 17 and 16. The alternating rational path has 4,181 source
     materializations, 45 extra source fragments, 22 source splits, 10 virtual
     splits, and maxima 16 and 12. Scale invariance and mutations of each
     aggregate, per-source vector, maximum, and split attribution are checked.
     This closes active projection and descendant-fragment attribution. It does
     not yet structurally bound inactive incident-edge scans, projected node
     slots, or the unresolved all-radii candidate-event work.
     `0fc48a1` classifies every full-projection adjacency visit as active
     internal, active boundary, or inactive. It requires projection calls to
     equal cache hits plus full materializations, internal incident scans to
     equal twice the materialized edge slots, and projected node slots to be at
     most edge slots plus one connected-root slot per materialization. Boundary
     and inactive references are charged to the two endpoints of active segment
     and portal-split lineages across the certified scale-entry bound. The
     500-node unit path has 82 full materializations, 6,056 node slots, 5,974
     edge slots, and 11,948 internal, 172 boundary, and 332 inactive incident
     scans. The alternating rational path has 63 materializations, 4,319 node
     slots, 4,256 edge slots, and 8,512 internal, 113 boundary, and 203 inactive
     scans. All counters are scale invariant and mutation checked. This closes
     the remaining projection-side node and incident-index work.
     `d17a6cd` classifies every remaining workspace visit as a radius,
     contraction-input, retained quotient-edge, contraction-recovery, or final
     augmented-tree recovery scan. The verifier requires all projection and
     nonprojection classes to reconstruct the workspace total exactly, rebuilds
     the first four classes from radius and contraction certificates, and
     derives final recovery from the exact source/virtual/split lineage counts.
     The unit fixture's 5,838 remaining scans are all radius scans; the rational
     fixture splits 7,215 scans into 4,032 radius, 1,496 contraction-input, and
     1,687 final-recovery scans. A recursive-contraction fixture covers nonzero
     retained and contraction-recovery classes. Scaling preserves all counts,
     and synchronized class/aggregate mutations are rejected. This closes the
     complete workspace-scan ledger, but not the all-radii event work.
     `source_an19::event::certificate::LocalBound` now closes the fixed-snapshot event-count
     component: the canonical semantic trace is at most `3n + 4m + 2`, queue
     insertions and pops are at most `n + 2m + 2`, and stale items are a subset
     of pops. `02c8385` replaces the trace-only linear minimum scan with a stable
     exact binary heap and adds an independent fixed-snapshot certificate for
     at most `3 I ceil(log2(max(I,1))) + 2m` counted comparisons. This practical
     `O((n+m) log(n+m))` bound does not satisfy the source target below, so the
     priority-queue proof status remains open as deferred proof debt.
     **Remaining exact proof obligation:** the `O(log n)` reduced-class route is refuted
     by the linear chord-family lower bound. For the parametric Figure 6 events
     generated by `source_an19::petal::WeightedPetal`, produce the stable exact order of the
     de-potentialized thresholds after source power-of-two rounding without
     enumerating one priority class per reduced cost. Potential reweighting may
     compute labels on original classes, but the ordering structure must handle
     the vertex-dependent subtraction `2 d(x,v)`, exact window denominators,
     and recursive rational portal splits in `O(m + n log log n)` work per
     cluster, or prove a source-equivalent aggregate bound that derives
     `O(m log n log log n)`. It must charge every operation and preserve the
     exact Figure 6 event order. Comparison sorting, fixed-width radix passes,
     and unstated bounded-integer assumptions do not close this obligation.
     Close this low-priority proof debt only with an authoritative corrected
     construction or a separately proved exact event-order data structure
     meeting the rational-input bound;
     that structure must preserve exact event order while consuming the
     symbolic labels. The implementation must charge the exact all-radii event
     work before selecting a structural amortization mode. `source_an19::experiment::hierarchy::AmortizationMode`
     remains `AggregateRegressionOnly`, the priority-queue mode remains
     `ReducedLengthMonotone`, and the fixed `1024` aggregate ceiling is only a
     regression guard, not an asymptotic proof.
3. **P9.3.3 state: complete. Implementation SHA: `038f762`.**
   `source_lsf::experiment::mwu::Collection` builds exactly `k` source-shaped
   LSFs using P9.3.2 weighted-copy expansion, AN19 static trees, and the
   branch-free forest initializer. Its self-verifying rational MWU certificate
   checks every source forest and supplied Lemma 5.4 envelope, then proves the
   recorded uniform per-edge average-stretch bound. The rational update
   `1 + x + x^2` retains the Appendix A.2 potential inequalities for checked
   `x <= 1/10`; the P8 weighted-Kruskal collection remains a permanent Oracle,
   never a fallback. Evidence is in
   `docs/phase-reports/P09-3-3-mwu-forest-collection.md`. This finite-instance
   certificate does not claim `O(log^7 n)` until a uniform source
   `W = O(log^4 n)` envelope and source-model word-bound audit pass.
4. **P9.3.4 state: in_progress. Start gate: P9.3.3 implementation audit passed.**
   Implement the deterministic static spanner-with-embedding primitive of
   Theorem 8.1. This phase is split before coding because Algorithm 4 depends
   on three separately delegated constructions and its composition guarantee
   cannot be inferred from an internal spanner certificate alone:

   - **P9.3.4a state: complete. Implementation SHA: `e0b7bc1`.**
     `source_spanner::{model,oracle}` now models unweighted simple `H'` and
     `J`, selected `J~ subset J`, direct and composed embeddings, and exact
     path-length, edge/vertex-congestion, maximum-degree, and size audits. A
     bounded enumerating simple-path Oracle remains isolated. Evidence is in
     `docs/phase-reports/P09-3-4a-static-embedding-contract.md`; no expander,
     sparsity, or Theorem 8.1 bound is claimed.
   - **P9.3.4b state: complete. Implementation SHAs: `77878a8`, `cc54c10`,
     `cdb2ce9`.** `source_spanner::experiment::circulant` now supplies the
     positive-level Algorithm 4 witness: it selects the first canonical
     circulant degree satisfying the exact source degree sandwich and an
     exhaustive positive expansion certificate. The older complete witness
     remains an experiment fixture, not the Algorithm 4 construction. Inputs
     outside the explicitly certified finite domain reject. Evidence is in
     `docs/phase-reports/P09-3-4b-witness-expander.md`. This is not the general
     CGLNPS20 construction and carries no source runtime claim.
   - **P9.3.4c state: complete. Implementation SHAs: `f9dd410`,
     `bce0f14`.** `source_spanner::experiment::{decomposition,domain}`
     implements a one-level, connected finite certificate with explicit
     component, edge-partition, degree-floor, and expansion evidence. It chooses
     the greatest source-valid level from the capacity lower bound through
     `ceil(log2(n))`, so positive-level finite witnesses can be sparse. Every
     source edge occurs exactly once and verification rebuilds all stored fields.
     Multi-level and general instances reject instead of claiming a generic
     CGLNPS20 construction or runtime bound. Evidence is
     `docs/phase-reports/P09-3-4c-expander-decomposition.md`.
   - **P9.3.4d state: complete. Implementation SHAs: `d5d80cc`,
     `f097cd4`, `838a321`.** `source_spanner::decremental` provides exact
     immutable deletion snapshots, a monotone isolated-vertex pruned set,
     replayable traces, stable-ID bounded BFS paths, and an independently
     enumerated simple-path certificate. This constrained model does not claim
     the general Theorem 8.6 pruning rule or decremental bounds. Evidence is
     `docs/phase-reports/P09-3-4d-decremental-expander-paths.md`. It was split
     before coding into:
     - **P9.3.4d1 state: complete. Implementation SHA: `d5d80cc`.**
       `source_spanner::decremental::state` models an immutable deletion state,
       stable edge identifiers, a recomputable monotone isolated-vertex pruned
       set, and the complete accepted/rejected deletion trace.
     - **P9.3.4d2 state: complete. Implementation SHA: `f097cd4`.**
       `source_spanner::decremental::query` runs a separate stable-edge-ID BFS
       over a verified snapshot, returns a bounded explicit path, and rejects
       pruned or unsupported endpoints without an Oracle fallback.
     - **P9.3.4d3 state: complete. Implementation SHA: `838a321`.**
       `source_spanner::decremental::certificate` independently enumerates and
       validates bounded simple paths, recomputes deletion-trace semantics, and
       rejects differential disagreement. General Theorem 8.6 construction and
       its decremental bounds stay unclaimed unless a source-backed proof and
       matching counters are added.
   - **P9.3.4e state: complete. Implementation SHAs: `93a0aa2`, `08a854c`,
     `3a637ac`, `cdb2ce9`.** arXiv:2309.16629v1 Algorithm 4 (pp. 41--42) was
     reread before coding. `source_spanner::algorithm4` now supplies a
     certified finite replay with degree-weighted witness provenance, `W -> J`
     threshold/deletion traces, an independent `J -> W` path loop, the image
     subgraph, and an exact composition audit. Evidence is
     `docs/phase-reports/P09-3-4e-algorithm4-sparsify.md`.
     - **P9.3.4e1 state: complete. Implementation SHA: `93a0aa2`.**
       `source_spanner::algorithm4::witness` builds the finite witness union
       from the certified single level and retains component weight, vertices,
       source edges, and witness-edge provenance.
     - **P9.3.4e2 state: complete. Implementation SHA: `08a854c`.**
       `source_spanner::algorithm4::first_embedding` replays the finite
       `W -> J` loop with stable bounded paths, composed congestion thresholds,
       deletion traces, and explicit unembedded witness edges.
     - **P9.3.4e3 state: complete. Implementation SHAs: `3a637ac`,
       `cdb2ce9`.** `source_spanner::algorithm4::{second_embedding,finalize}`
       independently replays `J -> W`, composes each oriented witness path
       through its `W -> J` path, loop-erases only the resulting local walk, and
       validates the image and composed embedding. This is limited to the
       certified one-level, one-component finite domain. General loops, Theorem
       8.6 pruning, and Theorem 8.1 bounds remain unimplemented and unclaimed.
5. **P9.3.5 state: complete. Start gate: P9.3.4 implementation audit
   passed. Implementation SHAs: `1d18dee`, `7282e92`, `9d7bed7`.** The finite
   implementation of Theorem 8.2's deletion/vertex-split reduction records
   exact batch encodings, stable selected-edge recourse, re-embedding sets,
   and initialization/update accounting. Its evidence is
   `docs/phase-reports/P09-3-5-dynamic-sparsify.md`. It is split into:
   - **P9.3.5a state: complete. Implementation SHA: `1d18dee`.**
     `source_spanner::dynamic::batch` models source-shaped deletion/split
     batches, smaller-side encodings, stable provenance, and replayable traces.
   - **P9.3.5b state: complete. Implementation SHA: `7282e92`.**
     `source_spanner::dynamic::rebuild` integrates the P9.3.4 finite
     `Sparsify` replay into an immutable decremental update path. It maps every
     relative Algorithm 4 edge back to its stable source ID and derives
     selected-edge addition, removal, and path-level re-embedding sets.
   - **P9.3.5c state: complete. Implementation SHA: `9d7bed7`.**
     Stable-ID certificate replay and exact cumulative update accounting are
     checked independently of construction. `dynamic::oracle` exhaustively
     rebuilds a canonical greedy bounded-path certificate, and a differential
     update test verifies both implementations on the same active source edge
     set without requiring identical selected subgraphs. General Theorem 8.2
     sparsity, recourse, and runtime bounds remain unclaimed without matching
     proofs.
   Retain the greedy-rebuild spanner as an Oracle only.
6. **P9.3.6 state: complete. Start gate: P9.3.5 implementation audit
   passed. Implementation SHAs: `8a69733`, `a9ac727`, `6985234`, `4a3ad34`.**
   The finite source-shaped implementation of Theorem 1.2/Section 9 builds a
   depth-one contracted-tree chain and immutable update replay. Its evidence is
   `docs/phase-reports/P09-3-6-dynamic-low-stretch-tree.md`. It implements
   low-stretch spanning tree for bounded integral lengths, including contracted
   forests, embedded spanners, insertions/deletions, worst-case update work,
   average stretch, and amortized tree recourse only as exact finite
   measurements, not source complexity bounds. It is split into:
   - **P9.3.6a state: complete. Implementation SHA: `8a69733`.**
     One immutable Section 9.1 level consumes a certified partial forest,
     forms `H_i = G_i/F_i`, retains original edge and endpoint provenance,
     explicitly records discarded intra-component loops, and derives each exact
     scaled length `stretch_tilde(e) * length_Gi(e)`. A separate direct
     component-enumerating Oracle differentially checks every component, edge,
     loop, and scaled length. No hierarchy recursion, spanner update, or
     complexity claim is made.
   - **P9.3.6b state: complete. Implementation SHA: `a9ac727`.**
     Exact integer arithmetic partitions each cross edge by the Section 9.1
     dyadic intervals for its stretch overestimate and scaled length. Every
     bucket is split into connected components, translated to stable source
     IDs after finite `Sparsify` initialization, and rechecked by pure replay.
     Parallel or out-of-domain components reject explicitly.
   - **P9.3.6c state: complete. Implementation SHA: `6985234`.** A finite
     depth-one immutable chain constructs `T_0 = F_0 union F_1`: it initializes
     the certified contracted/bucket path, builds an AN19-shaped terminal tree
     on the selected contracted spanner, and independently verifies original
     tree connectivity, exact weighted stretch, and per-level embedding
     congestion. The permanent enumerating LSST Oracle agrees on the bounded
     path fixture. The source asymptotic stretch induction remains unclaimed.
   - **P9.3.6d state: complete. Implementation SHA: `4a3ad34`.** Immutable
     history replay supports connected insertion, deletion, and smaller-side
     split snapshots with explicit threshold flags, exact full-snapshot work,
     and tree recourse counters. It uses a certified empty `F_0` so every
     finite source graph is still contracted honestly; each supported snapshot
     is differentially compared with the bounded exhaustive LSST Oracle in
     tests. This intentionally rebuilds every finite snapshot and makes no
     dynamic-LSF, recourse, or amortized-work claim.
7. **P9.3.7 state: complete. Start gate: P9.3.6 implementation audit
   passed. Implementation SHA: `66d7920`.** Evidence is in
   `docs/phase-reports/P09-3-7-finite-tree-audit.md`. The audit confirms the
   finite Section 9.1 tree path is source-traceable, has no production Oracle
   or legacy fallback, preserves immutable update semantics, and rejects its
   explicit finite-domain and certificate violations. It does not upgrade the
   source stretch, recourse, or runtime claims. It is split into:
   - **P9.3.7a state: complete.** Audit Section 9.1/Theorem 1.2
     traceability for `source_lsst::{level,bucket,chain,replay}`. Verify the
     production path has only the finite source-shaped contraction, bucket,
     spanner-replay, and AN19 terminal-tree calls; enumerating LSST and direct
     contraction Oracles must remain test-only and no legacy greedy or
     simple-path fallback may be reachable. The checked static audit is
     `tools/check_source_lsst_audit.py`.
   - **P9.3.7b state: complete.** Add deterministic adversarial immutable histories combining
     insertion, smaller-side split, and deletion. Rebuild every snapshot,
     preserve stable source IDs, compare bounded connected snapshots with the
     exhaustive LSST Oracle, verify recourse/scheduled-rebuild accounting, and
     prove rejected updates leave the prior immutable state unchanged. A
     four-node nonuniform-weight cycle exercises all three updates over three
     immutable batches, with a scheduled rebuild after batch two.
   - **P9.3.7c state: complete.**
     Exercise exact nonuniform positive weights, finite integral-length bounds,
     counter/certificate mutation rejection, and all explicit unsupported
     cases. Record every finite-domain limit; these checks cannot promote the
     source stretch, recourse, or runtime bounds. Nonintegral and out-of-range
     lengths, out-of-range dyadic buckets, parallel bucket edges, invalid
     updates, and corrupted tree/replay certificates reject deterministically.

### P9.4 - Source-grade dynamic minimum-ratio cycle

**State:** complete as finite-domain semantic infrastructure.** The P9.4a-d
evidence is summarized in `docs/phase-reports/P09-4-dynamic-min-ratio-summary.md`.
No Theorem 5.1 approximation or runtime claim is unlocked.
Implement the
deterministic paper's Theorem 5.1, including the complete tree chain, shifted
branches, dynamic sparsification, link-cut
updates, hidden-stability approximation, compact cycle output, and amortized
`Update`/`Query`/`Detect` accounting. The enumerating cycle query and P8 replay
remain permanent exact Oracles and are not allowed as a fallback.

P9.4 is deliberately split before implementation. Each completed subphase
provides only its finite-domain semantic evidence; none may claim Theorem 5.1's
amortized bound or unlock the `AlmostLinear` name.

1. **P9.4a - Source contract and complete tree-chain representation. State:
   complete. Implementation SHA: `4ce313b`.** The production
   `source_min_ratio::{model,chain}` namespace now models a checked, finite,
   multi-level tree chain with stable branch IDs, immutable source-tree
   snapshots, and a deterministic shifted single-branch selector. Structural
   validation, branch selection, and transition planning are pure; mutable
   tree-maintenance state remains reserved for a later explicit adapter. The
   P8 `dynamic_min_ratio` replay remains an `oracle`/`experiment` baseline
   only, and no enumerating cycle routine is reachable from this production
   representation. The audit report is
   `docs/phase-reports/P09-4a-tree-chain-contract.md`.

   The completed contract rejects malformed levels, branches, shifts, source
   IDs, and tree certificates; valid multi-level shifts select exactly one
   branch per level independently of input storage order; source-tree snapshots
   are immutable; and `tools/check_source_min_ratio_audit.py` verifies the
   no-Oracle-fallback boundary. This is finite-domain semantic evidence only;
   it does not implement a compact cycle, a hidden-stability query, link-cut
   updates, Theorem 5.1's approximation, or any amortized bound.

2. **P9.4b - Compact cycle and exact certificate decoding. State: complete. Implementation SHA: `70a80f5`.**
   Add a source-shaped compact cycle that references selected tree paths and
   off-tree arcs by stable IDs. Decode and validate it directly against the
   checked chain and circulation domain, with deterministic orientation and
   conservation certificates. The P7/P8 enumerators may compare results in
   tests only and may not participate in production decoding. `source_min_ratio::cycle`
   now binds source edges to endpoint-identical circulation arcs, expands selected
   tree paths directly, and validates the resulting signed circulation. Evidence:
   `docs/phase-reports/P09-4b-compact-cycle-decoding.md`. This is finite exact
   decoding only; query, approximation, dynamic-update, link-cut, and runtime
   claims remain for later subphases.
3. **P9.4c - Hidden-stability approximate-query contract and differential
   harness. State: complete. Implementation SHA: `0e2a423`.** Bind the checked Definition 4.2--4.5 stability
   state to the source-shaped chain through an explicit query input/output and
   certificate. Keep the witness hidden from query output, use exact arithmetic
   throughout, and differentially compare bounded fixtures with the permanent
   exact cycle Oracle. Report semantic agreement separately from any
   approximation or runtime proof. `source_min_ratio::query::decode_candidate`
   now exposes only decoded arcs and a coordinate count from an already checked
   ledger; the witness remains private. Evidence:
   `docs/phase-reports/P09-4c-hidden-stability-query.md`. No approximate-query,
   dynamic-update, link-cut, Theorem 5.1, or runtime claim is made.
4. **P9.4d - Update/query/detect execution, accounting, and audit. State:
   complete. Implementation SHA: `ef41f6c`.** Add explicit dynamic sparsification and link-cut execution
   adapters, then record update/query/detect transitions, compact-cycle
   application, bounded-domain work counters, no-fallback static checks, and
   adversarial mutation/replay campaigns. State every rejected operation and
   finite-domain limit. Do not claim an amortized, priority-queue, or AN19
   runtime bound. `source_min_ratio::execution::Executor` now forwards checked
   ledger transitions, records successful calls, and explicitly rejects the two
   unsupported source-grade operations. Evidence:
   `docs/phase-reports/P09-4d-execution-accounting.md`.

### P9.5 - Integrated exact flow backend

**State:** in_progress. **Start gate:** P9.4 finite-domain semantic and exact differential
audits passed. Introduce a clearly named experimental source-shaped backend
and integrate the IPM, dynamic query, additive-half
termination, deterministic rounding, and exact recovery without invoking
Dinic, Push--Relabel, or an enumerating Oracle. Differentially require identical
flow values and valid cuts, covers, chords, and rectangles on MRD compressed
networks. The complete solver may exist before the deferred P9.3.2d proof debt
is resolved, but it must not be named `AlmostLinear` and must report
`an19_runtime_verified: false` until all source complexity assumptions pass.

**P9.5a - Source compact-candidate selection construction. State: complete.**
Commit `91132c4` completes the first, provenance-only substep. Its immutable
`source_min_ratio::input::Input` accepts one circulation network plus exact
caller-supplied gradient, length, and positive structural tree-weight vectors;
it validates a stable `SourceEdgeId <-> CirculationArcId` correspondence and
materializes the source graph and `ArcBindings` together. Structural tree
weights remain separate from signed IPM gradients, so this step cannot silently
invent a tree metric or an exact approximation from snapshot intervals.

Commit `0bf9d37` completes the second, candidate-registry substep.
`source_min_ratio::candidate::Registry` accepts only explicit fundamental
spanner/tree compact cycles supplied by source maintenance, validates their
shape and provenance, computes exact current absolute quality, and maintains a
deterministic checked heap across insertions, replacements, and retirements.
It orients a nonzero choice for a negative IPM gradient dot product. The
registry never scans a graph for a cycle, and its finite heap counters make no
complexity claim.

Commit `cdb2ce9` closes the finite immutable core/spanner declaration substep.
`source_min_ratio::spanner::Snapshot` contracts a checked singleton forest,
builds the finite Section 9.1 chain, translates its stable selected-edge paths,
and emits one `FundamentalSpanner` per rejected core edge. Each declaration is
exactly an oriented, contiguous `SpannerPath` plus its rejected anchor edge;
the registry rejects a tree path or a discontinuous path masquerading as that
embedding. This is a one-snapshot finite construction, not dynamic recourse.

Commit `98a7d0e` closes the current-snapshot selector bridge. Its
`Step::from_maintained_candidates` requires terminal and core snapshots to
share one exact `Input` and current network, evaluates their independent
declaration registries, rejects an overlapping stable ID, and selects the
highest exact quality with stable-ID tie breaking. It then decodes the winning
cycle only through the terminal or spanner tree-chain context that produced it.
This is a finite immutable-population selector, not cross-snapshot terminal
maintenance or a complete source-flow backend.
`StableMinRatioLedger::edges()` remains an anonymous audit-coordinate slice;
its validated stability-witness input is not a cycle-selection witness; and
`source_min_ratio::execution::Executor` implements only supplied
`Update`/`Query`/`Detect` transitions. Selecting by enumerating cycles,
importing `dynamic_min_ratio`, or exposing the hidden witness would change this
contract and violate the no-fallback boundary.

The finite construction now maintains both terminal and rejected-core candidate
populations across supported same-network snapshots and returns a checked
current-snapshot choice to `Step`. The remaining construction is complete
source-flow iteration, recovery, and compressed-MRD integration rather than a
missing compact candidate. Evidence, the primary-source basis, and the next
action are recorded in
`docs/phase-reports/P09-5-candidate-selection-gap.md` and
`docs/phase-reports/P09-5-ipm-provenance.md`, and
`docs/phase-reports/P09-5-candidate-heap.md`,
`docs/phase-reports/P09-5-terminal-tree-projection.md`, and
`docs/phase-reports/P09-5-terminal-step-bridge.md`. This blocks completion of
the P9.5 backend, not P9.3.2d's deferred P9.6a proof debt. P9.5 remains
`in_progress` for its already independent semantic/differential evidence, but
`Backend::require_complete()` must continue to reject execution until the
remaining terminal handoff and complete compressed-MRD campaign are implemented
and audited.

**P9.5a - Source compact-candidate selection construction. State: complete.**
It is split into the following completed substeps:

1. **P9.5a.1 - Exact IPM/source/arc provenance. State: complete. Implementation
   SHA: `91132c4`.** Construct the checked `Input` projection and joint source
   graph/arc-binding materialization. It does not derive a tree metric from a
   signed gradient or select a cycle. Evidence:
   `docs/phase-reports/P09-5-ipm-provenance.md`.
2. **P9.5a.2 - Maintained fundamental-candidate registry and exact-quality
   heap. State: complete. Implementation SHA: `0bf9d37`.** Model only source-maintained fundamental
   spanner/tree candidates, compute their exact current quality from the
   provenance projection, and keep a deterministic checked heap over supplied
   candidate updates. The registry must reject undeclared, nondecodable, or
   duplicate candidates; it must not enumerate graph cycles or claim a
   complexity bound. Evidence:
   `docs/phase-reports/P09-5-candidate-heap.md`.
3. **P9.5a.3 - Live tree-chain/embedding and `Step` integration. State:
   complete.** Construct the source tree-chain, core/spanner embeddings, and
   candidate updates from the live IPM/source state; then convert the selected
   candidate to a certified Lemma 4.4 `Step` and run the no-fallback
   differential. This closes the finite selector gate but does not enable
   `Backend::require_complete()`. It is split into the completed substeps:
   - **P9.5a.3.1 - Terminal-tree projection and declarations. State:
     complete. Implementation SHA: `abb77ac`.** Materialize one exact AN19-shaped static tree from a live
     `Input`, preserve its source certificate, form a one-level checked chain,
     and emit one terminal fundamental declaration for every non-tree source
     edge. This must not enumerate cycles or claim a runtime bound. Evidence:
     `docs/phase-reports/P09-5-terminal-tree-projection.md`.
   - **P9.5a.3.2 - Core/spanner embeddings and live maintenance. State:
     in_progress.** Attach source core/spanner embedding provenance, declare the
     associated fundamental spanner cycles, and apply source-driven
     replacement/retirement updates to the registry across supported snapshots.
     It is split before implementation:
     - **P9.5a.3.2a - Finite core/spanner snapshot declarations. State:
       complete. Implementation SHA: `cdb2ce9`.** Positive-level Algorithm 4
       witnesses use the canonical certified circulant, and Task 3 independently
       embeds `J -> W`; therefore the K5 finite fixture yields a strict image
       subgraph and rejected core edges. An immutable `spanner::Snapshot`
       exposes each selected path as an explicit oriented `SpannerPath` plus its
       rejected anchor, then verifies every declaration and registry choice
       against the exact circulation projection. Unsupported finite-domain
       inputs reject explicitly. Evidence:
       `docs/phase-reports/P09-5a-3-2a-finite-core-spanner-snapshot.md`.
     - **P9.5a.3.2b - Cross-snapshot maintenance. State: complete.
       Implementation SHA: `9238b37`.** Immutable `spanner::Transition` rebuilds
       one supported same-network source snapshot, derives stable-ID insert,
       refresh, retire, and embedding-change sets, then synchronizes a registry
       only when it exactly matches the prior declared population. Retained
       candidates are re-scored even if their embedding did not change. This is
       finite immutable recourse, not general dynamic maintenance or a runtime
       claim. Evidence:
       `docs/phase-reports/P09-5a-3-2b-finite-core-recourse.md`.
   - **P9.5a.3.3 - Certified `Step` selection differential. State: complete.**
     Connect the maintained heap choice to the current approximation and
     `Step::from_compact_candidate`, then differentially validate the
     no-fallback source-flow transition against permanent bounded Oracles. It
     is split into the following completed substeps:
     - **P9.5a.3.3a - Terminal-candidate `Step` bridge. State: complete.
       Implementation SHA: `5afa4c7`.** Require exact equality between
       caller-supplied approximation coordinates and the checked terminal
       `Input`, select only the terminal declaration heap, and decode it through
       `Step::from_compact_candidate`. Evidence:
       `docs/phase-reports/P09-5-terminal-step-bridge.md`.
     - **P9.5a.3.3b - Complete candidate differential. State: complete for
       matching immutable snapshots. Implementation SHA: `98a7d0e`.** The
       terminal and rejected-core registries now compete by exact quality and
       stable ID while retaining their separate checked decode contexts.
       Coordinate, snapshot, and candidate-ID mismatches reject explicitly.
       A K5 no-fallback differential independently scores the two registry
       choices and requires the resulting `Step` to match direct decoding.
       Evidence: `docs/phase-reports/P09-5a-3-3b-complete-candidate-step.md`.
     - **P9.5a.3.4 - Terminal cross-snapshot candidate maintenance. State:
       complete for supported same-network snapshots. Implementation SHA:
       `b73b0fa`.** `terminal::Transition` rebuilds the exact successor tree,
       reports stable-ID insert/refresh/retire/re-embedding sets, and applies
       only to the exact preceding registry. Retained candidates are always
       re-scored. `Input::has_same_source_identity` is the shared pure identity
       check for terminal and core recourse. A successor-snapshot regression
       drives both new terminal/core snapshots through the complete candidate
       selector without fallback. Evidence:
       `docs/phase-reports/P09-5a-3-4-terminal-recourse.md`.

4. **P9.5b - One source-selected certified iteration. State: complete.
   Implementation SHA: `4043a85`.**
   Connect one matching immutable source projection to one `Session::apply`
   transition through an explicit input carrying the current
   `CertifiedIpmSnapshot`, exact `Input`, terminal and rejected-core snapshots,
   and `kappa`. The source input supplies exact
   candidate coordinates; `Session` must still certify them against its current
   fixed-point snapshot before changing state. A stale certified snapshot,
   unequal source inputs, absent source candidate, or failed Lemma 4.4 check
   must reject without mutating the session. This is a single exact semantic
   transition only: it neither derives exact coordinates from intervals nor
   enables `Backend::require_complete()`. Focused stale-input and
   stale-snapshot regressions plus the full workspace audit are recorded in
   `docs/phase-reports/P09-5b-source-selected-iteration.md`.

5. **P9.5c - Terminal source-session recovery handoff. State: complete.
   Implementation SHA: `3527d70`.**
   Compose an already additive-half-certified `source_flow::iteration::Session`
   with the compressed circulation's existing exact recovery map, yielding the
   recovered matching and Konig cover through one no-fallback operation. This
   must prove the session snapshot belongs to the same circulation before
   recovery and must not run an Oracle, infer a terminal state, or claim a
   complete iteration driver. A snapshot now retains the exact network identity
   used by update, termination, and recovery. Focused network-mismatch and
   compressed-session differentials plus the full audit are recorded in
   `docs/phase-reports/P09-5c-terminal-session-recovery.md`.

6. **P9.5d - Certified multi-step source iteration driver. State: complete.
   Implementation SHA: `0410b79`.** Introduce a narrow external `ProjectionFactory` boundary
   that prepares one exact source projection for each current certified IPM
   snapshot. Every prepared projection must carry its own snapshot identity,
   exact `Input`, terminal and rejected-core source populations, and `kappa`;
   construction must rerun the Theorem 4.3 approximation checks
   before candidate selection. A bounded driver must re-request a projection
   after each accepted update, record every accepted transition, stop only at
   the additive-half certificate, and reject a stale, uncertifiable, or
   exhausted preparation without mutating the session. It must not materialize
   exact coordinates by guessing from `DyadicInterval`s, select a fallback
   flow, or enable `Backend::require_complete()`. The next subphase will run
   this driver over the compressed MRD flow/cut/cover/chord/rectangle
   differential population. `source_flow::iteration::Projection` owns the
   snapshot-bound exact source state and certifies its Theorem 4.3 input before
   selection; `Driver` retains every accepted exact projection and direction,
   requests a fresh projection after each update, checks additive-half before
   each request, and returns an explicit limit error instead of claiming
   termination. Focused fresh/stale/terminal regressions, the source-flow
   static audit, and the full workspace audit are recorded in
   `docs/phase-reports/P09-5d-source-iteration-driver.md`.

7. **P9.5e - Compressed MRD source-driver differential. State:
   in_progress.** Use P9.5d's exact external projection boundary to execute
   terminated source sessions over compressed MRD instances, then recover the
   matching, Konig cover, selected chords, and rectangles through the existing
   no-fallback handoff. Each fixture must provide a fresh, independently
   certified exact source projection for every nonterminal snapshot; the test
   Oracle may establish expected results but must not participate in production
   selection, iteration, recovery, or coordinate preparation. Differentially
   compare matching value, cover, chord flags, and rectangle decomposition
   against the retained bounded references. Keep `Backend::require_complete()`
   unavailable until this campaign and its no-fallback audit pass. It is split
   into the following substeps:
   - **P9.5e.1 - Terminal compressed-driver differential. State: complete.
     Implementation SHA: `90f51ae`.** Compose the driver and source recovery over independently
     constructed strictly interior additive-half fixtures for an explicit
     biclique graph, a Theorem 8 chord graph, and a formal polygon rectangle
     completion. The test-only reference solver may construct expected values
     and terminal fixtures, but production code must remain source-only. This
     verifies terminal handoff composition, not nonterminal projection
     preparation. `compressed_flow::experiment::source::Circulation::run_source`
     now returns the exact driver completion plus recovered compressed solution;
     test-only strict-interior interpolation differentials cover explicit,
     chord, and formal rectangle inputs. Evidence:
     `docs/phase-reports/P09-5e-1-terminal-compressed-driver.md`.
   - **P9.5e.2 - Nonterminal compressed projection campaign. State:
     complete. Implementation SHA: `58bf417`.** A supported one-by-one
     compressed circulation now supplies independently chosen exact rational
     coordinates to a fresh `Projection`: lengths
     `11/4, 4, 8, 5, 8` and return-arc gradient `-400/3`. The constructor,
     rather than an interval endpoint, reruns the Theorem 4.3 certificate for
     all five arcs. Its `1/4` strictly interior circulation is explicitly
     nonterminal; one source-selected update is recorded before the bounded
     driver returns `IterationLimit { maximum_iterations: 1 }`. The successor
     remains nonterminal, so no recovery is attempted or claimed. The
     structural chain scales the rational input only for its integral finite
     construction; candidate scoring retains the original coordinates. The
     fixed current spanner subset accepts this fixture as single-edge bucket
     components. This establishes one applicable compressed nonterminal path,
     not the broad chord/rectangle campaign. Evidence:
     `docs/phase-reports/P09-5e-2-nonterminal-compressed-projection.md`.
   - **P9.5e.3 - Full compressed MRD closeout campaign. State:
     in_progress. Start SHA: `2796465`.** Run the combined
     driver/recovery/chord/rectangle differential population and no-fallback
     audit. The campaign must distinguish a terminal recovery from a bounded
     nonterminal witness, and must establish an explicit fresh-projection
     policy for every nonterminal snapshot it claims can terminate. Do not
     consider `Backend::require_complete()` until that policy and the complete
     output differential both pass. Commit `45849b1` establishes a bounded
     two-snapshot reconstruction witness: each accepted nonterminal update
     causes one fresh, independently certified `Projection` preparation, but
     the successor remains nonterminal under `IterationLimit { maximum_iterations:
     2 }`. Commit `c902c37` promotes that reconstruction into the production
     `FixedProjectionFactory`: it rebuilds only caller-supplied immutable exact
     source coordinates and rejects a successor whose Theorem 4.3 certificate
     fails. It is not general dynamic coordinate maintenance or a termination
     policy. Commit `5391ada` proves the failure boundary explicitly: on the
     finite source-flow fixture, 25 fixed-coordinate preparations are accepted
     and the next rejects at the exact gradient certificate. The remaining
     policy must update its exact coordinates rather than reusing them. Commit
     `20f8a18` adds `ScheduledProjectionFactory`, a caller-owned immutable
     sequence that consumes and certifies one identity-compatible exact `Input`
     per snapshot, rejects exhaustion rather than reuse, and never derives a
     coordinate from an IPM interval. The `1 x 1` compressed fixture now uses
     two distinct literal input coordinates over two nonterminal snapshots; it
     still returns an iteration limit and performs no recovery. This finite
     schedule is not a source-supported coordinate construction or a
     termination proof. Commit `8668461` adds the complementary
     `ReciprocalSlackProjectionFactory`: it reconstructs rational reciprocal
     slack lengths and the exact objective gradient term from the snapshot's
     exact flow, exact optimum, and immutable network, then independently
     reruns Theorem 4.3 certification. It does not access IPM coordinate
     intervals; `check_source_flow_audit.py` enforces that boundary. The
     general source-flow and compressed `1 x 1` fixtures each accept two such
     fresh successor projections. P9.5e.3g.1 subsequently removes the
     fixture-specific exponent-64 structural-domain constant: every factory
     derives its finite bound from the exact current `Input`. P9.5e.3b replaces
     global common-denominator normalization with
     snapshot-relative power-of-two structural coordinates: the hierarchy and
     spanner consume only the resulting scale-free topology, while raw exact
     coordinates remain authoritative for candidate scoring and Theorem 4.3
     certification. The previous fourth-update candidate-ratio overflow is
     removed by normalized arbitrary-precision `ExactRatio` components and by
     using scale-relative structural topology only for materialization and
     bindings. Raw exact coordinates remain in `Input` for candidate scoring
     and Theorem 4.3 certification. The compressed fixture now accepts 64
     consecutive reciprocal-slack updates under
     `IterationLimit { maximum_iterations: 64 }`; every adjacent record has a
     distinct input and certified snapshot, and the final snapshot remains
     nonterminal. This is representational regression evidence only, not a
     source-supported terminating structural range or a termination proof.
     Evidence:
     `docs/phase-reports/P09-5e-3-fresh-projection-policy.md`.

     P9.5e.3 is further split:
     - **P9.5e.3a - Exact-coordinate policies. State: complete for the
       supported two-snapshot fixtures.** `FixedProjectionFactory`,
       `ScheduledProjectionFactory`, and `ReciprocalSlackProjectionFactory`
       respectively establish staleness rejection, immutable external schedule
       consumption, and interval-independent exact-flow reconstruction. No
       item claims a terminating source session.
     - **P9.5e.3b - Successor-safe finite structural domain. State:
       complete.** Snapshot-relative power-of-two
       structural coordinates now remove the global common-denominator,
       hierarchy, and fourth-step raw-coordinate failure across 64
       reciprocal-slack successors. The 64-update test intentionally stops at
       its explicit iteration limit and remains nonterminal. A separate strict
       interior `1 x 1` fixture at uniform flow `547590/1000000` is nonterminal
       initially, then reaches additive-half termination after one fresh
       reciprocal-slack source projection and selected update; its ordinary
       source handoff recovers the exact matching and cover. This establishes a
       supported terminating range and genuine terminal source session without
       making a general termination or runtime claim. The complete format,
       no-fallback, lint, workspace-test, documentation, release-build, and
       release-consistency audit passed on 2026-07-31.
     - **P9.5e.3c - Complete isolated-lattice compressed-MRD output
       differential. State: complete for the declared nine-point population.**
       After P9.5e.3b reaches a genuine terminal source
       session, compare flow, matching, cover, chord flags, and rectangle
       decomposition across the supported compressed-MRD population before
       considering the complete-backend gate. Connected tree buckets use the
       exact `TreeIdentity` construction. Cyclic buckets now require an
       explicit `CanonicalTree` policy, selected before construction: stable
       source-edge-order union-find selects a spanning tree, and stable tree
       paths embed every rejected source edge. It is a finite, source-provenance
       certificate rather than an Oracle or an Algorithm 4 retry; its
       construction neither invokes Algorithm 4 nor makes a sparsity,
       congestion, stretch, termination, or runtime claim. The Figure 3 formal
       polygon now starts from a nonterminal snapshot, prepares one fresh
       Definition 4.2 projection, selects one nonzero source direction, and
       reaches additive-half termination. Its recovered matching, cover,
       selected chord flags, and rectangle decomposition exactly equal the
       retained formal references. The isolated-point formal lattice now
       exhaustively visits all 511 nonempty masks. The 101 masks with an empty
       chord family or no explicit conflict edge are outside the compressed-flow
       domain; all remaining 410 masks begin at an independently certified
       nonterminal snapshot, prepare a fresh Definition 4.2 projection for
       every accepted update, and terminate within the explicit eight-update
       limit (the observed maximum is two). Each run differentially verifies
       maximum matching, minimum cover, independent chord flags, and optimum
       rectangle count; matching vectors may differ only for nonunique optima.
       The population also regresses isolated-endpoint pruning: the internal
       circulation retains only active outer arcs, while recovery returns the
       original chord dimensions and leaves isolated endpoints uncovered. This
       closes the declared isolated-lattice output differential. It does not
       establish a general termination policy or enable the complete-backend
       gate; P9.5e.3 remains in progress for that separate obligation.
     - **P9.5e.3d - Conditional potential-reduction termination budget. State:
       complete for a snapshot-bound conditional driver.** `PotentialBudget`
       combines the independently certified CKLPPS22 Equation (9)/Lemma 4.1
       additive-half potential threshold with the Lemma 4.4 per-update
       `kappa^2 / 500` decrease. It accepts only one exact starting snapshot
       and one fixed `kappa`, uses conservative dyadic endpoints, and bounds
       accepted updates only on the condition that every requested fresh source
       projection is actually prepared and accepted. A factory failure, stale
       snapshot, changed `kappa`, or exhausted budget remains an explicit
       failure; it cannot yield recovery or a fallback. The `1 x 1`
       compressed circulation runs through this entry from a nonterminal
       snapshot to its exact matching and cover. This closes the finite
       potential-accounting component, not general source-coordinate
       maintenance, projection availability, or the complete-backend gate.
     - **P9.5e.3e - Independently recomputed Definition 4.2 coordinates.
       State: complete for the checked fixed-point domain.**
       `DefinitionProjectionFactory` reconstructs `alpha`, both
       `slack^-(1 + alpha)` length terms, and the alpha-weighted barrier
       gradient from exact flow, exact objective, network, and configuration
       data. It reads no retained snapshot coordinate interval; each fresh
       dyadic `Input` is still independently accepted by the Theorem 4.3
       certificate. The current compressed source differential, including its
       410-mask isolated-lattice population, uses this factory, and a separate
       nonterminal regression rebuilds 64 distinct inputs/snapshots. This
       closes source-coordinate construction, but not public construction of
       an inclusive-target initial source state for every compressed input, the
       complete-backend gate, or any runtime claim.
     - **P9.5e.3f - Execution-state decoupling from hidden-stability auditing.
       State: complete.** `source_min_ratio::query::decode` is a pure compact
       cycle decoder, so `Projection`, `SourceSelected`, compact candidates,
       and all source projection factories no longer require a
       `StableMinRatioLedger` or `StableWitness`. P8/P9.4 retains
       `decode_candidate` as the distinct audit adapter, including its
       checked stable-edge count. Focused source-flow, pure-decoder, and
       compressed-MRD regressions establish that the production Definition 4.2
       factory runs without constructing a stability witness. This does not
       construct an inclusive-target initial source state for every compressed
       input; it does not implement Theorem 5.1 or enable the complete-backend
       gate.
     - **P9.5e.3g - Source configuration and inclusive-target initialization.
       State: in_progress.** CKLPPS22 Section 4, Equation (9), Theorem 4.3,
       Lemma 4.12, and Algorithm 7 distinguish the remaining requirements:
       an initially strict IPM flow with an exact integral target `F*`, and a
       verified decision contract for an incorrect target. This subphase is
       split before code:
       1. **P9.5e.3g.1 - Derived finite source configuration. State:
          complete.** `source_min_ratio::spanner::Parameters::derive` is a
          pure `Input -> Parameters` transformation. It selects the canonical
          stable root `FlowNodeId(0)`, computes the smallest positive dyadic
          bound accepting the exact singleton contraction, and chooses the
          explicit `CanonicalTree` construction for cyclic buckets. All four
          source projection factories now derive that value for every fresh
          input; no constructor accepts a root or exponent. The dedicated
          source-spanner test checks root zero and an exact exponent-four
          input, while the existing 64-successor Definition 4.2 regression
          proves that the count `64` is an iteration witness rather than a
          structural configuration. `check_source_flow_audit.py` requires the
          production derivation call. Theorem 4.3 permits `kappa = 1/2`
          because it applies to every `kappa` in `(0, 1)`; that remains a
          caller-selected semantic value and creates no runtime claim.
       2. **P9.5e.3g.2 - Inclusive-target initial-point entry. State:
          complete.** `Backend::begin_with_target` constructs
          the O(m)-edge Appendix B.1 augmentation, a certified strict initial
          snapshot, and a snapshot-bound potential budget for one
          caller-provided integral target. `TargetDriver` owns that target
          through source execution and recovers terminal flow only through
          `recover_augmented_terminated_at_most`, accepting an original
          integral cost at most the target and rejecting one that exceeds it.
          `Circulation::run_with_target` is the
          compressed-flow adapter: it starts that driver and decodes only the
          recovered original circulation. The source and compressed regressions
          confirm that a valid `F*` reaches the augmented factory and a target
          equal to the initial-flow cost rejects before factory execution; a
          graph regression accepts original cost `0` under target `1` and
          returns `TargetNotMet` for target `-1`. This
          performs no Oracle call or target inference.
       3. **P9.5e.3g.3 - Target-search contract. State: blocked for automatic
          search; exact negative-certificate types are implemented and
          verifiable.** Direct source audit of arXiv:2203.00671v2 (Section 4,
          Equation (9), Theorem 4.3, Lemma 4.4, Lemma 4.12, Algorithm 7,
          Appendix B.1/C) confirms that the sole binary-search claim ("we
          assume that we know `F*`, as running our algorithm allows us to
          binary search for `F*`", Section 4, p. 24) is a remark with no
          decision invariant, no certificate construction for `F_opt > T`, and
          no analysis of an incorrect guess. The paper does not maintain dual
          variables, so no negative `epsilon`-optimality certificate is
          available from the IPM path. Consequently a failed target run must
          remain unclassified (`UnsupportedOrUndetermined`), a successful run
          proves only `F_opt <= T`, and no automatic binary-search wrapper may
          be implemented. A caller may, however, prove `F_opt > T` by supplying
          an exactly verified certificate: `DualLowerBoundCertificate` with
          `Backend::prove_infeasible_below` (graph) and `certify_cover_below`
          (compressed MRD, Konig). These verifiers never invoke a reference
          solver, a missing or failed certificate is never an infeasibility
          decision, and they do not discover `F*`. Evidence:
          `docs/phase-reports/P09-5e-3g-3-target-search-contract.md`.
       Until all three rows have source-backed acceptance evidence,
       `Backend::require_complete()` remains `Error::Incomplete`, P9.5e.3
       remains in progress, and P9.3.2d remains separate deferred proof debt.

**Current implementation marker:** commits `3397fbe`, `b6f40e1`, and
`d28a68a` establish the P9.5 source-flow boundary, document the prohibited
legacy recovery paths, and certify the non-Oracle additive-half termination
boundary. Commit `094a289` adds a separate deterministic exact cycle-cancellation
recovery implementation. It is differentially equal to the permanent recovery
implementation on a shared-cycle fixture, but production has no dependency on
that implementation; `tools/check_source_flow_audit.py` enforces the boundary.
Commit `b34be66` adds `source_flow::iteration::Session`, which applies only
externally supplied Lemma 4.4 directions with exact approximation checks and
Detect accounting. It intentionally does not select a direction: P9.4's query
boundary decodes candidate cycles but does not yet construct the required
minimum-ratio update. Commit `1a95a59` now converts an externally selected
P9.4 compact candidate to a full exact circulation direction before that
transition; it still does not select the candidate. These are incremental
integration evidence, not P9.5 closeout. Commit `8d7975b` completes the
recovery map from terminal augmented flow to the original zero-lower-bound or
lower-bounded network, retaining artificial-arc rejection and every exact
verification. Commit `6179f22` makes the no-Oracle recovery boundary explicit:
the additive-half certificate plus exact objective equality establishes
optimality, while a separate feasibility-only validator checks each recovered
representation without enumerating residual cycles. Its static audit rejects
direct optimality-validator calls from `source_flow`. Source candidate
selection, MRD compressed-network differentials, and the end-to-end no-fallback
audit remain required. P9.3.2d proof debt is deliberately nonblocking for that
work. Do not begin P9.6a until this complete source-shaped flow backend exists
and has passed those audits.

Commit `08eaae4` starts the compressed-network bridge without introducing a
flow selection fallback. `dominance::compressed_flow::experiment::source`
constructs the exact negative-return-arc circulation and, from an externally
certified terminal flow, deterministically recovers a matching and Konig
vertex cover. A bounded differential compares this bridge with the permanent
minimum-cost, Dinic, and Push--Relabel references, and a separate fixture runs
the source-flow terminal recovery into the cover mapper. This is initial
compressed-network evidence only: source candidate selection and an MRD
flow/cut/cover/chord/rectangle campaign remain open.

Commit `0359194` extends that differential to a real MRD chord fixture: it
constructs the four-dimensional dominance embedding, obtains the Theorem 8
compact biclique partition, validates that partition against its explicit
chord graph, and compares the source-circulation recovery with all permanent
matching/flow references. This gives chord-level evidence, not an end-to-end
rectangle-recovery campaign.

Commit `40bb2f1` carries a recovered source-circulation cover through the
formal-polygon completion API. On the source Figure 3 fixture, the cover
complement selects a valid independent chord family and completion yields the
same optimal rectangle count. The terminal circulation is deliberately
reference-supplied inside `#[cfg(test)]`; this is end-to-end differential
evidence, not production source-flow direction selection.

Commit `91132c4` adds the exact IPM-coordinate provenance bridge used by the
first P9.5a substep. `source_min_ratio::input::Input` makes the source-edge to
circulation-arc relationship explicit before a source tree chain is chosen and
keeps the signed gradient vector separate from positive tree-construction
weights. Its compact-cycle decode test proves the materialized bindings survive
one supplied tree-chain path; it does not choose that chain, enumerate cycles,
or implement Theorem 5.1 selection or runtime.

Commit `0bf9d37` adds the P9.5a.2 source-declared candidate registry. It
tracks only explicit fundamental spanner/tree compact cycles, evaluates their
exact absolute gradient-to-length ratio from `Input`, maintains a deterministic
checked heap through source-driven replacement/retirement, and reverses a
positive candidate for descent. It does not construct the source tree chain or
embeddings that produce those declarations, invoke an Oracle, or connect a
choice to a certified `Step`.

Commit `abb77ac` closes P9.5a.3.1's single-snapshot terminal-tree projection.
It retains the exact AN19-shaped static tree certificate, forms the checked
terminal chain branch, and supplies one fundamental tree declaration for every
non-tree source edge without cycle enumeration. Core/spanner embedding
provenance, cross-snapshot candidate maintenance, and the `Step` certificate
remain P9.5a.3.2--.3 work.

Commit `5afa4c7` closes P9.5a.3.3a's terminal-only bridge. It rejects supplied
approximation coordinates that differ from the checked terminal `Input`, takes
its choice solely from the terminal declaration registry, and decodes that
choice into an exact circulation direction through the existing compact-cycle
boundary. An empty or zero-quality terminal population returns no step. This
does not create core/spanner declarations, invoke an enumerating Oracle, or
complete the P9.5 selector.

Commit `98a7d0e` closes P9.5a.3.3b for matching immutable snapshots. It
combines the independently maintained terminal and rejected-core registries
without mixing their compact-cycle contexts, rejects mismatched source inputs,
current coordinates, and candidate-ID spaces, then decodes only the winning
source declaration. The K5 differential independently compares the two exact
registry choices with the public step result; additional regressions reject
mixed snapshots and verify stable-ID tie breaking. It does not maintain the
terminal population across updates, enable `Backend::require_complete()`, or
make an AN19 runtime claim.

Commit `b73b0fa` closes P9.5a.3.4 for supported same-network terminal
snapshots. Terminal and core transitions now share the same pure source/
circulation identity predicate, while each transition retains its own exact
candidate context and registry guard. A terminal tree change produces exact
candidate insertion, retirement, or re-embedding recourse; unchanged stable
IDs are still refreshed for their new coordinates. The successor terminal and
core snapshots pass through the complete `Step` selector in a no-fallback
regression. This is finite recourse, not a general dynamic data structure,
complete backend, or AN19 runtime claim.

Commit `4043a85` closes P9.5b's one-step source-selected IPM transition.
`SourceSelected` carries the exact current source `Input`, matching terminal
and core snapshots, checked ledger, current certified snapshot, and `kappa`.
`Session::apply_source_selected` rejects stale snapshots and unequal source
inputs before selecting a maintained candidate, then delegates to the existing
certified `Session::apply` transition. The exact candidate coordinates are
never derived from IPM intervals. This is a single checked transition, not a
complete backend driver; `Backend::require_complete()` remains unavailable and
the compressed MRD campaign remains required.

Commit `3527d70` closes P9.5c's terminal session handoff. Every certified IPM
snapshot now retains the exact circulation identity, so a changed topology,
demand, capacity, or cost rejects before update or recovery. The compressed
circulation can recover a matching and Konig cover directly from an already
terminal source session through the local exact recovery path. This does not
drive iterations or enable the complete backend; the full source iteration and
compressed MRD campaign remain P9.5 work.

Commit `0410b79` closes P9.5d's bounded multi-step driver. A projection is an
owned, snapshot-bound exact source state that checks source input identity,
network identity, both maintained populations, and Theorem 4.3 before it can
select a candidate. The driver asks its external factory for a new projection
after every accepted update, records the exact pre-update projection and
direction, checks additive-half termination before each request, and rejects a
stale projection or explicit iteration-limit exhaustion without a fallback.
This is an exact orchestration boundary, not a coordinate selector, source
runtime claim, or complete compressed-MRD solver.

Commit `90f51ae` closes P9.5e.1's terminal compressed-driver handoff. The
compressed circulation now drives an already certified `source_flow` driver to
additive-half termination and recovers its matching and Konig cover through
the existing local source recovery only. Test-only strict-interior terminal
fixtures exercise explicit biclique, Theorem 8 chord, and formal rectangle
completion differentials against retained references. This terminal composition
does not prepare a projection or execute a source-selected update, so P9.5e.2
remains active and `Backend::require_complete()` remains unavailable.

Commit `58bf417` closes P9.5e.2's exact-coordinate gap. `Input` now derives a
pure common-denominator structural graph for finite tree/spanner construction,
while all candidate contexts retain their unscaled exact source coordinates.
The explicit one-by-one compressed nonterminal fixture selects a source cycle,
certifies and records one Lemma 4.4 update, then retains its nonterminal state
under a one-step `IterationLimit` witness. The fixture uses no
`DyadicInterval` endpoint and no reference backend. P9.5e.3 remains required
for the full compressed MRD differential and `Backend::require_complete()`
remains unavailable.

### P9.6 - Phase-wide source and complexity audit

**State:** planned. **Start gate:** the complete P9.5 experimental flow backend
passes semantic, exact differential, and no-fallback audits. First return to
the low-priority P9.3.2d global-amortization and event-order proof debt, then
audit theorem-to-code traceability, checked domains, precision, operation
counters, no-fallback traces, exact differentials, and compressed-network
evidence. P9 remains `in_progress` or becomes `audit_failed` until this audit
proves the advertised deterministic almost-linear bound.

**P9.6a - P9.3.2d proof-debt closure (low priority).** Start only after P9.5
has a complete source-shaped flow backend with semantic, differential, and
no-fallback evidence. Prove an explicit bound on the number and ordering of
the exact reduced-event equivalence classes generated by
`source_an19::petal::WeightedPetal`, or replace the event-order data structure
with an independently proved construction that preserves Figure 6's exact
rational event order. The proof must account for the vertex-dependent
subtraction `2 d(x,v)`, exact window denominators, recursive portal splits,
and hierarchy-wide amortization; finite traces, workspace scans, and bounded
campaigns are not substitutes. Only a completed P9.6a may enable the
`AlmostLinear` name, `an19_runtime_verified: true`, or an AN19 runtime claim.

P9.3.2d proof debt does not block P9.3.3 through P9.5. It blocks only the
`AlmostLinear` backend name, `an19_runtime_verified: true`, P9 complexity
closeout, and any release or report that claims the AN19 runtime. P10 remains
planned behind P9.6 because the proof debt is intentionally revisited after the
complete flow-solver chain exists.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P10 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P10 automatically unless a persisted hard blocker exists.

## P10 - Layered public backend architecture

**State:** complete. The repository is usable, testable, and publishable
through an explicit three-layer solver architecture without weakening or
bypassing the unresolved automatic `F*` search blocker (P9.5e.3g.3). The
layers are: (1) a complete reference-backed exact solver; (2) a source backend
executed only under a caller-supplied inclusive target; (3) exact independent
certificate verification. No `solve_source -> optimum` automatic entry is
exposed. `Backend::require_complete()` stays `Error::Incomplete`; no AN19
runtime claim is made. Subphases:

- **P10.1 - Solver mode and provenance model. State: complete.** Define
  `SolverMode { Reference, SourceWithTarget { target, source_config } }`,
  `SolverProvenance { ReferenceExact, SourceCertifiedAtMost { target } }`, and
  a stable public result model with objective, matching, cover, chords,
  rectangle decomposition, provenance, and verification summary. No
  `AutomaticSource` mode.
- **P10.2 - Complete reference-backed public solver. State: complete.** Wrap
  the permanent reference backends in a deterministic `solve_reference` path
  that returns exact MRD output (matching, cover, chords, rectangles) and is
  independently verified, clearly marked reference-backed.
- **P10.3 - Source-with-target public solver. State: complete.** Wrap the
  source-shaped execution path in `solve_source_with_target(polygon, config)`,
  with `config.target` as the caller-supplied inclusive target, returning
  certified results only
  when recovered cost `<= target`, and returning explicit
  `UnsupportedOrUndetermined` errors otherwise, with no reference fallback and
  no `F*` inference.
- **P10.4 - Certificate-verification public entries. State: complete.** Expose
  `verify_source_feasible_at_most` and `verify_source_infeasible_below`, the
  latter accepting `DualLowerBoundCertificate` (general circulation) and
  `CoverBelowProof` (compressed bipartite MRD), verified exactly and
  independently.
- **P10.5 - CLI and deterministic serialization. State: complete.** Add
  `mrd solve --backend reference|source-with-target --target <integer>` and a
  `verify-negative-certificate` command, require a target in source mode,
  never silently fall back, and serialize provenance deterministically.
- **P10.6 - End-to-end supported pipeline. State: complete.** Wire
  geometry -> chords -> conflict -> compressed matching network -> selected
  backend -> matching/cover -> chord selection -> rectangle completion -> exact
  verification for both reference and source-with-target modes on supported
  fixtures, without leaking augmentation arcs.
- **P10.7 - Direct grid parity path. State: pending.** See the direct-grid
  parity section below (renumbered P11). Do not begin it before P10.1-P10.6
  unless it is independent of the automatic `F*` blocker.
- **P10.8 - Benchmark and evidence separation. State: complete.** Separate
  reference-complete, source-with-known-target, certificate-verification,
  direct-grid vs polygon-derived, geometry-only, compressed-representation, and
  recovery-only benchmark categories; label reference-provided-target runs and
  record target-provider/geometry/compressed-representation/source/recovery/
  verification/total-hybrid timings separately. `mrd benchmark --suite layered`
  creates only polygon-derived measurements in P10; direct-grid records are
  explicitly unavailable until P11. Source rows require `--source-target` or
  `--reference-provided-target`; neither option adds automatic `F*` search.
- **P10.9 - Documentation and release audit. State: complete.** Update
  README, ARCHITECTURE, ALGORITHMS, KNOWN_LIMITATIONS,
  NEAR_LINEAR_FLOW_IMPLEMENTATION, and TESTING with the layered architecture,
  the blocked automatic `F*` constructor, the unresolved AN19 proof, and which
  features are production-ready versus research-only. Run the full audit and
  record phase evidence.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P11 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P11 automatically unless a persisted hard blocker exists.

## P11 - Direct grid parity embedding

**State:** in_progress. Add `EmbeddingCoordinateBackend` with `RankedCoordinates`
and `DirectGridParity`. For finite integer pixel grids encode horizontal as
`(2*l, -2*r, 2*y, -2*y)` and vertical as
`(2*x+1, -2*x+1, 2*t+1, -2*b+1)`. DirectGridParity must build no coordinate
rank sets/maps/sorted vectors. Require dominance/intersection equivalence, no
cross-side equality, exact biclique/network/flow/cut/rectangle equality, and
`rank_sort_count`, `rank_map_entry_count`, and `rank_map_owned_bytes` all zero.
Keep RankedCoordinates permanently as Oracle. Suggested release:
`v2.1.0-direct-grid-parity-embedding`.

P11 is split before implementation into these source-of-truth subphases:

- **P11.1 - Backend contract and counters. State: in_progress.** Define the
  zero-cost backend selection boundary, stable embedding result, and explicit
  allocation/ordering counters without weakening the permanent ranked Oracle.
- **P11.2 - Exact direct parity encoder. State: planned.** Implement the four
  checked formulas for finite integer grid chords, with no rank sets, maps, or
  sorted coordinate vectors in the direct branch.
- **P11.3 - Grid pipeline integration. State: planned.** Route the grid
  dominance, biclique, flow, cut, and rectangle pipeline through both backends;
  keep polygon and source-shaped paths on their existing ranked/reference
  contract until separately audited.
- **P11.4 - Differential and invariant campaign. State: planned.** Prove by
  exhaustive and metamorphic finite campaigns that direct and ranked embeddings
  agree on intersections, explicit graphs, partitions, flows, covers, and
  rectangles; assert cross-side coordinate inequality and zero direct counters.
- **P11.5 - Release evidence and performance boundary. State: planned.** Add
  machine-readable direct-vs-ranked evidence, document the measured benefit,
  run the complete phase audit, and preserve RankedCoordinates as the
  correctness Oracle.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P13 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P13 automatically unless a persisted hard blocker exists.

## P13 - Constant-factor performance hardening

**State:** planned. Profile verified code and reduce constants without weakening
certificates. Candidates: scratch reuse, capacity planning, SoA dominance
points, safe narrow integers, flattened flow adjacency, iterative traversal,
biclique arenas, deterministic component parallelism, specialized pipelines,
evidence-backed backend selection, and optional certificate materialization.
Every optimization needs isolated before/after commit and benchmark; revert
regressions. Suggested release: `v2.2.0-constant-factor-hardening`.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P14 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P14 automatically unless a persisted hard blocker exists.

## P14 - Final verification, benchmark, and strict report

**State:** planned. Run exhaustive, random, polyomino, polygon, formal-hole,
metamorphic, fuzz, external CP-SAT, generic-flow, compressed-flow, memory, and
performance campaigns. Produce `results/final-correctness-report.md`,
`results/final-performance-report.md`, `results/final-complexity-report.md`,
`results/final-benchmarks.csv`, `results/final-benchmarks.json`, and
`results/final-manifest.json`. Reports include theorem/code traceability;
backend claims/assumptions; populations; disagreements/regressions; timings;
memory; flow crossover; direct-parity benefit; constant-factor changes;
environment/SHAs; reproduction; and limitations. Suggested release:
`v3.0.0-complete-artifact`.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread this plan and the complete Global Rules section.
4. Mark P14 complete only after all final outputs and release gates pass.
5. Update phase fields and stop unless an explicitly authorized release action remains.

## Append-only progress log

**Current-policy reading rule:** entries before `P9-policy-update` record the
then-current hard-blocker decision and are retained as historical evidence.
Their `blocked`, `hard blocker`, and `forbidden` wording is superseded by the
current P9.3.2d substatus table and sequencing rule above: faithful AN19
implementation may proceed through P9.5, while the missing DOI
`10.1137/17M1115575` proof is deferred low-priority P9.6a debt. It gates only
the `AlmostLinear` name, `an19_runtime_verified: true`, P9 complexity closeout,
and AN19 runtime claims.

| phase | state | start SHA | implementation SHAs | closeout SHA | remote SHA | audit report | result files | started at | completed at | blocker |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P0 | complete | 72ce32a | none | ae763ca | ae763ca | `docs/phase-reports/P00-persistent-plan-and-baseline.md` | plan and P0 report | 2026-07-27T09:17:42Z | 2026-07-27T09:40:49Z | none |
| P1 | complete | deee489 | fe1be92 | fe1be92 | fe1be92 | `docs/phase-reports/P01-v1.3-baseline-freeze.md` | `results/p1-baseline/`; P1 checker | 2026-07-27T09:46:57Z | 2026-07-27T10:07:31Z | none |
| P2 | complete | fe1be92 | 95abbcf, b8c2d15 | 521f82d | 521f82d | `docs/phase-reports/P02-formal-boundary-model.md` | formal fixture, source model, focused and Oracle-differential tests | 2026-07-27T10:07:31Z | 2026-07-27T10:39:44Z | none |
| P3 | complete | 521f82d | 093961f, fd1bbc6, 3d94851, 996ad44, 659d7fb, 6cd0845 | 3b8347a | 3b8347a | `docs/phase-reports/P03-formal-hole-geometry.md` | `results/p3-formal-fixtures.json`; `results/p3-polygon-differential.json`; zero counterexamples | 2026-07-27T10:39:44Z | 2026-07-27T12:41:01Z | none |
| P4 | complete | 3b8347a | 85c1083, bfa5a94, 4cf8250, 9238066, f5c387e | 746989f | 746989f | `docs/phase-reports/P04-presorted-biclique-construction.md` | `results/p4-adversarial.csv`; `results/p4-biclique-construction.csv`; `results/p4-biclique-construction.json`; `results/p4-dense-conflict.csv`; `results/p4-exhaustive-4x4.json`; `results/p4-formal-fixtures.json`; `results/p4-polygon-differential.json`; `results/p4-polygon-differential.counterexamples.json`; `results/p4-polyomino.csv`; `results/p4-random-8x8-seed42.json` | 2026-07-27T12:47:26Z | 2026-07-27T13:38:15Z | none |
| P5 | complete | 746989f | 1636b75, 986026d, 90acf15 | 66b6336 | 66b6336 | `docs/phase-reports/P05-exact-flow-backends.md` | `results/p5-flow-backends.csv`; `results/p5-flow-backends.json` | 2026-07-27T13:43:00Z | 2026-07-27T15:36:49Z | none |
| P6 | complete | 66b6336 | 5934890 | ece512f | ece512f | `docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md`; `docs/phase-reports/P06-near-linear-flow-specification.md` | source mapping only | 2026-07-27T15:48:00Z | 2026-07-27T16:20:00Z | none |
| P7 | in_progress | ece512f | pending | pending | pending | pending | pending | 2026-07-27T16:28:00Z | pending | none |
| P7 | complete | 9f12445 | 3184ad5, 4e8326c, 21fbbd9 | 237d55b | 237d55b | `docs/phase-reports/P07-exact-circulation-refinement.md` | exact bounded-flow differential; zero disagreements | 2026-07-27T15:48:41Z | 2026-07-27T16:23:23Z | supersedes the stale pre-closeout P7 row above |
| P8 | complete | 237d55b | 37088c9, bc09a0f, a70e783, 749536c, 200bcc8, 32d7b49 | 32d7b49 | 32d7b49 | `docs/phase-reports/P08-1-stable-min-ratio-contract.md` through `P08-6-dynamic-min-ratio-audit.md` | checked baseline contracts and exact Oracles only | 2026-07-27T16:24:35Z | 2026-07-27T17:02:26Z | source-grade constructions deferred to P9.3--P9.4 |
| P9 | blocked | 32d7b49 | 0abf2a1, d1cefdd, 48250f5, 959ed00, 4e66be4, ffb371f | pending | 09ea2bb | `docs/phase-reports/P09-integration-gate-audit.md` | exact P9.1 Oracles; no almost-linear backend | 2026-07-27T17:02:52Z | pending | P9.3.2d lacks an authoritative exact AN19 event-order reduction from graph length classes |
| P9.2.1 | complete | d1c7a3b | 44d50d4 | 44d50d4 | 44d50d4 | `docs/phase-reports/P09-fixed-point-arithmetic.md` | certified dyadic `log`, `exp`, and negative-power intervals; 191 workspace tests passed | 2026-07-27T23:36:48Z | 2026-07-28T00:05:39Z | Equation (9) integration continues in P9.2.2 |
| P9.2.2 | complete | 44d50d4 | bba500e | bba500e | bba500e | `docs/phase-reports/P09-certified-ipm-quantities.md` | certified Equation (9), lengths, gradients, and approximation hypotheses; 194 workspace tests passed | 2026-07-28T00:05:39Z | 2026-07-28T00:23:57Z | Lemma 4.4 transitions continue in P9.2.3 |
| P9.2.3 | complete | bba500e | ab6fb10 | cdc4c0d | cdc4c0d | `docs/phase-reports/P09-lemma-44-updates.md` | certified Lemma 4.4 transition, strict-interior/potential-drop checks, and Detect ledger; full workspace audit passed | 2026-07-28T00:24:00Z | 2026-07-28T01:20:00Z | P9.2.4 initial point, termination, and recovery remain |
| P9.2.4 | complete | ab6fb10 | 5719917, 82ac962, aabe7fc | pending | pending | `docs/phase-reports/P09-initial-termination-recovery.md` | lower-bound normalization, Appendix B.1 augmentation, Lemma 4.11 perturbation, additive-half/KP15/P7 recovery; full workspace audit passed | 2026-07-28T01:26:00Z | 2026-07-28T02:12:00Z | none |
| P9.3 | blocked | aabe7fc | pending | pending | 09ea2bb | pending | source-grade low-stretch and spanner structures | 2026-07-28T02:12:00Z | pending | P9.3.2d source event-order interface is unresolved |
| P9.3.1 | complete | 22e8371 | 6e96916, 934576d | pending | pending | `docs/phase-reports/P09-source-structure-contracts.md` | exact graph/update/encoding/LSF/piece/stretch/spanner/parameter contracts; full workspace audit passed | 2026-07-28T02:24:00Z | 2026-07-28T04:28:00Z | none |
| P9.3.2 | blocked | 6e96916 | 9ac15b5, d0629c1, a9456e9, 698ad7c, f5f91f6, 7251038, 2bca89c, a57e48c, 6769ec1, 3bb0400, 839cb5c, 20b0421, cdf732d, d6b8e6b, 3d3afe2, be21982, 720f0cb, 27d5773, f54c29a, c02c7c9, ece2722, 6901703, e4f54af, bc61592, 14e9abb, 8d68d59, 5cc49f0, b050625, 60fdfe4, 0fc48a1, d17a6cd, 0b3b704 | pending | 88a89b2 | `docs/phase-reports/P09-branch-free-forest-core.md`; `docs/phase-reports/P09-an19-static-lsst-source-map.md` | compact weighted AN19 hierarchy, recursive contraction/expansion, fast event processing, dense cluster-local node projection, scale-relative rounding, source-class fixed-radius cones, zero-production-binary-heap reduced-length monotone event queues, root-source/symbolic-label/recursive-scale/projection/workspace-scan audits, fixed-path reuse, unchanged-cluster caching, and incremental portal-split projection updates; 500-node logical recursion has depth 8 and per-source maximum 9; 5,974 projected occurrences, 12,452 classified incident scans, and all 18,290 workspace scans have structural source/virtual/certificate charges; 16 materialized classes reduce to 2 symbolic source and 3 virtual classes; a power-of-two chord family proves exact reduced-cost classes can grow linearly | 2026-07-28T02:48:00Z | pending | an exact de-potentialized event-order structure and candidate-event work charging remain unproved; source runtime is unverified |
| P9.3.2d | complete | 8f9ab06 | d17a6cd, 8681115, 8f9ab06, 0b3b704 | pending | 88a89b2 | `docs/phase-reports/P09-an19-static-lsst-source-map.md` | workspace scan-count audit complete; formal SIAM source identification complete; logarithmic reduced-class conversion refuted; 247 passed and 3 existing ignored | 2026-07-28T16:31:47Z | 2026-07-29T16:29:53Z | faithful implementation is complete; DOI 10.1137/17M1115575 leaves the runtime proof as deferred low-priority P9.6a debt |
| P9.3.2d-event-engine | complete | 89b5ea3 | 7ea13da, 28f9ff7, 6c8cfac, 98bb615, afea347, 4413e94 | 3fee109 | 3fee109 | `docs/phase-reports/P09-an19-static-lsst-source-map.md` | exact Oracle/reduced engine, canonical trace, six charge maps, A--H campaign, CLI, manifest and full audit; 254 passed and 3 existing ignored | 2026-07-28T17:29:46Z | 2026-07-29T16:29:53Z | P9.6a retains local/global/PQ proof and AN19-runtime debt; it does not block P9.3.3 through P9.5 implementation |
| P9.3.2d-local-proof | complete | 0dba080 | 5e771d8, a25ac08, d4dda8f, b4358a9 | cb59512 | cb59512 | `docs/AN19_LOCAL_EVENT_BOUND.md`; `docs/phase-reports/P09-an19-static-lsst-source-map.md` | machine-verified `3n + 4m + 2` semantic-event and `n + 2m + 2` queue-item bounds in all 62 A--H Oracle/reduced runs; 254 passed and 3 existing ignored | 2026-07-28T23:29:14Z | 2026-07-29T16:29:53Z | source-equivalent PQ/global-amortization/runtime proof debt remains P9.6a; P9.3.3 through P9.5 may proceed |
| P9.3.2d-practical-pq-bound | complete | ebde003 | 02c8385, fbc869e, bbf13b3 | cdd9da1 | cdd9da1 | `docs/AN19_LOCAL_EVENT_BOUND.md`; `docs/phase-reports/P09-an19-static-lsst-source-map.md` | exact stable binary heap and machine-verified `3 I ceil(log2(max(I,1))) + 2m` practical comparison bound in all 31 reduced A--H runs; Oracle independence retained; 256 passed and 3 existing ignored | 2026-07-28T23:59:57Z | 2026-07-29T16:29:53Z | source-equivalent PQ/global-amortization/runtime proof debt remains P9.6a; P9.3.3 through P9.5 may proceed |
| P9-policy-update | complete | ee97432 | pending | pending | pending | `docs/IMPLEMENTATION_MASTER_PLAN.md` | separates faithful implementation progress from deferred complexity proof debt | 2026-07-29T16:29:53Z | 2026-07-29T16:29:53Z | supersedes the earlier hard-blocker policy without changing any AN19 runtime claim |
| P9 | in_progress | ee97432 | pending | pending | pending | pending | continue the complete source-shaped flow-solver chain through P9.5 | 2026-07-29T16:29:53Z | pending | AN19 complexity proof debt gates naming, claims, and closeout only |
| P9.3 | in_progress | ee97432 | pending | pending | pending | pending | P9.3.3 is the next active implementation item | 2026-07-29T16:29:53Z | pending | none |
| P9.3.2 | complete | 6e96916 | 9ac15b5 through 02c8385 | pending | pending | existing P9.3.2 reports | faithful AN19-shaped implementation, exact Oracle agreement, traces, and local practical bounds | 2026-07-28T02:48:00Z | 2026-07-29T16:29:53Z | global/PQ/runtime proof debt deferred until after P9.5 |
| P9.3.2d | complete | 8f9ab06 | 7ea13da through 02c8385 | pending | pending | existing AN19 source-map and local-bound reports | implementation and semantic differential gates complete | 2026-07-28T16:31:47Z | 2026-07-29T16:29:53Z | DOI 10.1137/17M1115575 omits the needed proof; runtime remains unverified |
| P9.3.2d-proof-debt | planned | ee97432 | pending | pending | pending | pending | hierarchy-wide amortization, source-equivalent exact event ordering, and final runtime verification | deferred | pending | low priority until the complete P9.5 flow backend exists |
| P9.3.3 | in_progress | ee97432 | pending | pending | pending | pending | deterministic MWU collection of exactly `k` LSFs | 2026-07-29T16:29:53Z | pending | none |
| P9.3.3 | complete | c2edf16 | 038f762 | pending closeout | pending | `docs/phase-reports/P09-3-3-mwu-forest-collection.md` | exact `k`-forest weighted-copy/AN19/LSF collection, envelope certificate, rational MWU proof, mutation and full-workspace audit | 2026-07-29T16:29:53Z | 2026-07-29T17:01:06Z | `O(log^7 n)` and AN19 runtime remain unclaimed pending the uniform envelope and deferred P9.3.2d proof debt |
| P9.3.4 | in_progress | 41be08c | pending | pending | 41be08c | pending | deterministic static spanner-with-embedding primitive | 2026-07-29T17:01:06Z | pending | none |
| P9.3.4a | in_progress | 41be08c | pending | pending | 41be08c | pending | exact static embedding-composition contracts and bounded simple-path Oracle | 2026-07-29T17:01:06Z | pending | none |
| P9.3.4a | complete | 91a3e3c | e0b7bc1 | pending closeout | pending | `docs/phase-reports/P09-3-4a-static-embedding-contract.md` | exact graph/subgraph/direct-and-composed embedding audits and isolated bounded simple-path Oracle | 2026-07-29T17:01:06Z | 2026-07-29T17:01:06Z | Theorem 8.4 witness expander, Theorem 8.5 decomposition, Theorem 8.6 paths, and Algorithm 4 remain |
| P9.3.4b | in_progress | a71dcee | pending | pending | a71dcee | pending | deterministic bounded-degree witness expander and expansion certificate | 2026-07-29T17:01:06Z | pending | none |
| P9.3.4b | complete | a71dcee | 77878a8, cc54c10, cdb2ce9 | pending closeout | pending | `docs/phase-reports/P09-3-4b-witness-expander.md` | finite canonical circulant witness, exact degree sandwich, exhaustive cut-expansion certificate, explicit domain rejection, and full-workspace audit | 2026-07-29T17:01:06Z | 2026-07-30T00:00:00Z | general CGLNPS20 construction intentionally not claimed; Theorem 8.5-8.6 and Algorithm 4 remain |
| P9.3.4c | in_progress | 64ce6f4 | pending | pending | 64ce6f4 | pending | deterministic edge-disjoint expander decomposition with exact layer certificates | 2026-07-29T17:01:06Z | pending | none |
| P9.3.4c | complete | 64ce6f4 | f9dd410, bce0f14 | pending closeout | pending | `docs/phase-reports/P09-3-4c-expander-decomposition.md` | finite-domain one-level decomposition with explicit component, exact edge partition, degree-floor, and exhaustive expansion certificates; full-workspace audit | 2026-07-29T17:01:06Z | 2026-07-29T17:49:14Z | general/multi-level CGLNPS20 construction and its runtime are intentionally not claimed; Theorem 8.6 and Algorithm 4 remain |
| P9.3.4d1 | complete | d5bb65d | d5d80cc | pending closeout | pending | `docs/phase-reports/P09-3-4d-decremental-expander-paths.md` | immutable deletion state, monotone isolated-vertex prune set, and replayable accepted/rejected deletion trace | 2026-07-29T17:51:52Z | 2026-07-29T18:02:38Z | not the source expander-cut pruning rule |
| P9.3.4d2 | complete | d5bb65d | f097cd4 | pending closeout | pending | `docs/phase-reports/P09-3-4d-decremental-expander-paths.md` | stable-ID BFS path response with hop-bound and pruned-endpoint outcomes | 2026-07-29T17:51:52Z | 2026-07-29T18:02:38Z | no source decremental work/depth claim |
| P9.3.4d3 | complete | d5bb65d | 838a321 | pending closeout | pending | `docs/phase-reports/P09-3-4d-decremental-expander-paths.md` | independent enumerating simple-path differential certificate and mutation regression | 2026-07-29T17:51:52Z | 2026-07-29T18:02:38Z | Oracle is verification-only and exponential in the worst case |
| P9.3.4d | complete | d5bb65d | d5d80cc, f097cd4, 838a321 | pending closeout | pending | `docs/phase-reports/P09-3-4d-decremental-expander-paths.md` | exact decremental-path semantics, trace, production BFS, and independent bounded certificate; full-workspace audit | 2026-07-29T17:51:52Z | 2026-07-29T18:02:38Z | general Theorem 8.6 construction and bounds intentionally unclaimed; Algorithm 4 remains |
| P9.3.4e1 | complete | e396484 | 93a0aa2 | pending closeout | pending | `docs/phase-reports/P09-3-4e-algorithm4-sparsify.md` | finite Task 1 witness union with exact level weight and source/witness provenance | 2026-07-29T18:06:36Z | 2026-07-29T18:19:17Z | single-level/single-component finite input only |
| P9.3.4e2 | complete | e396484 | 08a854c | pending closeout | pending | `docs/phase-reports/P09-3-4e-algorithm4-sparsify.md` | finite `W -> J` bounded path loop with congestion threshold, deletions, and unembedded-edge trace | 2026-07-29T18:06:36Z | 2026-07-29T18:19:17Z | no general expander-pruning or iteration-bound claim |
| P9.3.4e3 | complete | e396484 | 3a637ac, cdb2ce9 | pending closeout | pending | `docs/phase-reports/P09-3-4e-algorithm4-sparsify.md` | independent finite `J -> W`, image, composed embedding, and exact audit | 2026-07-29T18:06:36Z | 2026-07-30T00:00:00Z | finite certified domain only; general Theorem 8.1 bounds remain unclaimed |
| P9.3.4e | complete | e396484 | 93a0aa2, 08a854c, 3a637ac, cdb2ce9 | pending closeout | pending | `docs/phase-reports/P09-3-4e-algorithm4-sparsify.md` | finite source-shaped Algorithm 4 replay with sparse positive-level witnesses and full-workspace audit | 2026-07-29T18:06:36Z | 2026-07-30T00:00:00Z | general Theorem 8.1 construction/bounds intentionally unclaimed; P9.3.5 remains |
| P9.3.5 | complete | e396484 | 1d18dee, 7282e92, 9d7bed7 | pending closeout | pending | `docs/phase-reports/P09-3-5-dynamic-sparsify.md` | source-shaped deletion/split batches, finite Algorithm 4 rebuild, stable-ID recourse, independent greedy Oracle, and exact update accounting | 2026-07-29T18:24:15Z | 2026-07-29T18:41:34Z | finite one-level connected domain only; no general Theorem 8.2 sparsity, recourse, or runtime claim |
| P9.3.6 | complete | 4714ee3 | 8a69733, a9ac727, 6985234, 4a3ad34 | pending closeout | pending | `docs/phase-reports/P09-3-6-dynamic-low-stretch-tree.md` | finite Section 9.1 contraction, exact buckets, static terminal tree, immutable source update replay, recourse, and bounded Oracle differential | 2026-07-29T18:49:00Z | 2026-07-29T19:03:00Z | explicit finite integral connected domain; every replay rebuilds; no source Theorem 1.2 stretch or runtime claim |
| P9.3.7 | complete | 6b3bb73 | 66d7920 | pending closeout | pending | `docs/phase-reports/P09-3-7-finite-tree-audit.md` | source trace, no-fallback static audit, adversarial immutable update history, exact weight/bound/certificate rejection evidence | 2026-07-29T19:03:00Z | 2026-07-29T19:45:43Z | finite-domain semantics only; P9.3.2d proof debt continues to prohibit AN19 complexity claims |
| P9.4a | complete | ba3779e | 4ce313b | 58bb52b | 58bb52b | `docs/phase-reports/P09-4a-tree-chain-contract.md` | immutable multi-level source-tree chain, shifted branch selection, validation, and no-fallback static audit | 2026-07-29T20:01:28Z | 2026-07-29T20:04:45Z | finite-domain structural semantics only; no compact cycle, query, link-cut, or runtime claim |
| P9.4b | complete | 58bb52b | 70a80f5 | pending closeout | pending | `docs/phase-reports/P09-4b-compact-cycle-decoding.md` | direct source-tree compact-cycle decoding and exact circulation certificate | 2026-07-29T20:04:45Z | 2026-07-29T20:12:57Z | finite semantics only; no query, update, approximation, or runtime claim |
| P9.4c | complete | 70a80f5 | 0e2a423 | 6264cb8 | 6264cb8 | `docs/phase-reports/P09-4c-hidden-stability-query.md` | hidden-stability query contract, direct compact decoding, and exact finite-domain differential | 2026-07-29T20:12:57Z | 2026-07-29T20:18:21Z | no approximate search, witness discovery, dynamic data structure, Theorem 5.1, or runtime claim |
| P9.4d | complete | 6264cb8 | ef41f6c | de4df98 | de4df98 | `docs/phase-reports/P09-4d-execution-accounting.md` | checked update/query/detect forwarding, finite counters, explicit unsupported-operation rejection, and no-fallback audit | 2026-07-29T20:18:21Z | 2026-07-29T20:23:52Z | no dynamic sparsification, link-cut maintenance, approximation, amortized, Theorem 5.1, or runtime claim |
| P9.4 | complete | ba3779e | 4ce313b, 70a80f5, 0e2a423, ef41f6c | 79f09bc | 79f09bc | `docs/phase-reports/P09-4-dynamic-min-ratio-summary.md` | finite-domain source-tree chain, compact cycle decoding, hidden-stability query boundary, and execution accounting | 2026-07-29T20:01:28Z | 2026-07-29T20:25:33Z | source-grade dynamic structures and all runtime claims remain unimplemented |
| P9.5 | in_progress | 79f09bc | 3397fbe through 3527d70 | pending | d11cb3f | `docs/phase-reports/P09-5-integration-gap.md`, `docs/phase-reports/P09-5b-source-selected-iteration.md`, `docs/phase-reports/P09-5c-terminal-session-recovery.md` | source-selected certified step, exact snapshot/network identity, and terminal session to matching/cover handoff | 2026-07-29T20:25:33Z | 2026-07-30T02:33:46Z | complete source iteration driver plus broad MRD flow/cut/cover/chord/rectangle campaign remain; P9.3.2d proof debt is nonblocking |
| P9.5a.2 | complete | 20ee78d | 0bf9d37 | pending closeout | pending | `docs/phase-reports/P09-5-candidate-heap.md` | exact source-declared fundamental candidate validation, quality, orientation, deterministic stale-record heap, and no-enumeration tests | 2026-07-29T22:04:55Z | 2026-07-29T22:20:23Z | no live tree-chain/embedding candidate construction, `Step` certificate, or runtime claim |
| P9.5a.3.1 | complete | 0bf9d37 | abb77ac | pending closeout | pending | `docs/phase-reports/P09-5-terminal-tree-projection.md` | exact AN19-shaped source tree, checked terminal branch, and one declaration per non-tree source edge | 2026-07-29T22:27:30Z | 2026-07-29T22:38:34Z | no core/spanner embeddings, cross-snapshot candidate maintenance, `Step` certificate, or runtime claim |
| P9.5a | complete | d11cb3f | 91132c4 through b73b0fa | pending | pending | `docs/phase-reports/P09-5-candidate-selection-gap.md`, `docs/phase-reports/P09-5a-3-4-terminal-recourse.md` | exact provenance, source-declared heap, terminal/core declarations, immutable recourse, and complete immutable candidate selection | 2026-07-29T22:38:34Z | 2026-07-30T01:24:35Z | complete backend integration remains; reference-cycle enumeration remains forbidden |
| P9.5a.3.2a | complete | 5afa4c7 | cdb2ce9 | d825236 | d825236 | `docs/phase-reports/P09-5a-3-2a-finite-core-spanner-snapshot.md` | finite sparse Algorithm 4 image, immutable core/spanner snapshot, explicit rejected-edge embedding cycles, exact decode, and registry selection | 2026-07-30T08:49:19Z | 2026-07-30T00:00:00Z | one finite immutable snapshot only; cross-snapshot maintenance remains P9.5a.3.2b |
| P9.5a.3.2b | complete | d825236 | 9238b37 | pending closeout | pending | `docs/phase-reports/P09-5a-3-2b-finite-core-recourse.md` | immutable same-network snapshot recourse, exact stable-ID candidate insert/refresh/retire/re-embed sets, and registry synchronization | 2026-07-30T00:00:00Z | 2026-07-30T00:00:00Z | finite same-network core only; later P9.5a.3.3b/.3.4 add current selection and terminal recourse |
| P9.5a.3.3a | complete | abb77ac | 5afa4c7 | pending closeout | pending | `docs/phase-reports/P09-5-terminal-step-bridge.md` | exact terminal-coordinate equality, terminal-registry-only choice, compact decoding, empty/zero-quality no-step path, and no-fallback audit | 2026-07-30T08:49:19Z | 2026-07-30T08:49:19Z | core/spanner population and cross-snapshot maintenance remain outside this terminal-only bridge |
| P9.5a.3.3b | complete | d63d02c | 98a7d0e | pending closeout | pending | `docs/phase-reports/P09-5a-3-3b-complete-candidate-step.md` | exact terminal/core snapshot identity, independent registry scoring, stable-ID tie handling, context-preserving compact decoding, K5 no-fallback differential, and mismatch rejection | 2026-07-30T01:05:47Z | 2026-07-30T01:05:47Z | matching immutable snapshots only; later P9.5a.3.4 adds finite terminal recourse; complete backend integration remains |
| P9.5a.3.4 | complete | 91ec25e | b73b0fa | pending closeout | pending | `docs/phase-reports/P09-5a-3-4-terminal-recourse.md` | shared pure source identity, immutable terminal insert/refresh/retire/re-embed transition, exact registry guard, tree-change and successor-selector regressions | 2026-07-30T01:16:45Z | 2026-07-30T01:24:35Z | supported same-network snapshots only; complete backend integration remains |
| P9.5b | complete | 9be04dc | 4043a85 | 3728886 | 3728886 | `docs/phase-reports/P09-5b-source-selected-iteration.md` | exact source-selected candidate step, snapshot/input identity rejection, and certified Session update | 2026-07-30T01:24:35Z | 2026-07-30T02:00:00Z | no multi-step driver or complete backend |
| P9.5c | complete | 3728886 | 3527d70 | pending closeout | pending | `docs/phase-reports/P09-5c-terminal-session-recovery.md` | snapshot-network identity plus terminated session to compressed matching/cover recovery | 2026-07-30T02:00:00Z | 2026-07-30T02:33:46Z | no iteration driver or broad MRD campaign |
| P9.5d | complete | ae7a626 | 0410b79 | pending closeout | pending | `docs/phase-reports/P09-5d-source-iteration-driver.md` | snapshot-bound exact projection factory, bounded fresh-projection driver, accepted-step trace, stale-projection rejection, and full-workspace audit | 2026-07-30T02:33:46Z | 2026-07-30T03:13:00Z | no compressed MRD driver differential or complete backend |
| P9.5e | in_progress | 0410b79 | 90f51ae, 58bf417 | pending | pending | `docs/phase-reports/P09-5e-1-terminal-compressed-driver.md`; `docs/phase-reports/P09-5e-2-nonterminal-compressed-projection.md` | terminal handoff plus one exact nonterminal compressed update/witness | 2026-07-30T03:13:00Z | pending | full chord/rectangle driver differential and complete-backend gate remain |
| P9.5e.1 | complete | b068bea | 90f51ae | pending closeout | pending | `docs/phase-reports/P09-5e-1-terminal-compressed-driver.md` | source driver to terminal matching/cover recovery across explicit, chord, and formal rectangle fixtures | 2026-07-30T03:13:00Z | 2026-07-30T03:38:00Z | terminal-only evidence; no nonterminal projection preparation |
| P9.5e.2 | complete | 90f51ae | 58bf417 | pending closeout | pending | `docs/phase-reports/P09-5e-2-nonterminal-compressed-projection.md` | rational structural normalization, Theorem 4.3-certified one-by-one compressed projection, one accepted source update, and explicit nonterminal limit witness | 2026-07-30T03:36:00Z | 2026-07-30T03:36:56Z | one supported compressed fixture only; P9.5e.3 must run the broad campaign |
| P9.5e.3 | in_progress | 2796465 | 45849b1, c902c37, 5391ada, 20f8a18, 8668461, 2802323, pending closeout | pending | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | fixed/scheduled/reciprocal and independently recomputed Definition 4.2 coordinate factories, source-certified conditional potential budget, derived finite source configuration, explicit-target Appendix B.1 initialization, 64 nonterminal Definition 4.2 updates, pure compact decoding, and exact `1 x 1`, `2 x 2`, chord, Figure 3, and 410-mask isolated-lattice nonterminal-to-terminal differentials | 2026-07-30T03:41:01Z | pending | coordinate construction, configuration derivation, execution-state decoupling, and supplied-target initialization are complete for their checked domains; target search and `Backend::require_complete()` remain unavailable |
| P9.5e.3a | complete | 2796465 | 45849b1, c902c37, 5391ada, 20f8a18, 8668461 | pending closeout | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | two-snapshot fresh exact coordinate schedules and interval-independent reciprocal reconstruction | 2026-07-30T03:41:01Z | 2026-07-30T04:39:00Z | finite fixtures only; no terminating source session |
| P9.5e.3b | complete | 8668461 | pending closeout | pending closeout | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | snapshot-relative power-of-two structural topology, arbitrary-precision candidate arithmetic, 64 accepted reciprocal-slack successors, and a one-step `1 x 1` terminating source session; full audit passed | 2026-07-30T04:39:00Z | 2026-07-31T01:10:39Z | no general termination or runtime claim |
| P9.5e.3c | complete | pending | pending closeout | pending | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | nonterminal explicit, chord, Figure 3, and exhaustive 410-member isolated-lattice flow/matching/cover/chord/rectangle differentials; explicit finite `CanonicalTree` for cyclic buckets | 2026-07-31T00:33:53Z | 2026-07-31T02:10:00Z | P9.5e.3 parent still requires a general termination policy; no complete backend gate or runtime claim |
| P9.5e.3d | complete | pending | pending closeout | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | snapshot-bound fixed-`kappa` potential budget, nonterminal `1 x 1` budgeted source run, and changed-`kappa` no-mutation regression | 2026-07-31T02:10:00Z | 2026-07-31T03:24:04Z | conditional on every fresh projection succeeding; no general source-coordinate maintenance, complete backend gate, or runtime claim |
| P9.5e.3e | complete | pending | pending closeout | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | independently recomputed dyadic Definition 4.2 coordinates, 64 nonterminal successor preparations, and the complete declared compressed-MRD population using the new factory | 2026-07-31T03:24:04Z | pending | no public all-input inclusive-target initial-state construction, complete backend gate, or runtime claim |
| P9.5e.3f | complete | pending | pending closeout | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | pure compact-cycle decoding and source-flow execution-state decoupling from hidden-stability ledger/witness construction | 2026-07-31T03:24:04Z | pending | P8/P9.4 ledger auditing remains; no Theorem 5.1, public all-input inclusive-target initial-state construction, complete backend gate, or runtime claim |
| P9.5e.3g | in_progress | 0701822 | 6be878a, 2802323, pending closeout | pending | pending | derived finite source configuration, inclusive-target initialization, and source-backed target-search contract | 2026-07-31T04:06:37Z | pending | g.1 and g.2 are complete; no lower-bound substitution for `F*`; `Backend::require_complete()` remains unavailable until the source binary-search decision invariant is recovered and verified |
| P9.5e.3g.1 | complete | 0701822 | 6be878a | pending closeout | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | pure exact `Input -> Parameters` derivation of root, minimal dyadic bound, and finite canonical-tree policy; all factories derive per projection | 2026-07-31T04:06:37Z | 2026-07-31T04:30:19Z | no initial strict point, inclusive-target entry, complete-backend gate, or runtime claim |
| P9.5e.3g.2 | complete | 059f3b6 | 2802323, pending closeout | pending closeout | pending | `docs/phase-reports/P09-5e-3-fresh-projection-policy.md` | Appendix B.1 inclusive-target driver and compressed-flow adapter with checked at-most-target recovery; graph regression accepts cost below target and rejects `TargetNotMet` | 2026-07-31T04:30:19Z | 2026-08-01T00:00:00Z | no target inference, wrong-target decision contract, complete-backend gate, or runtime claim |
| P9.5e.3g.3 | blocked | 059f3b6 | pending closeout | pending | pending | `docs/phase-reports/P09-5e-3g-3-target-search-contract.md` | direct source audit: no automatic decision invariant for an incorrect target guess; exact negative-certificate verifiers implemented (`DualLowerBoundCertificate`/`prove_infeasible_below`, `certify_cover_below`); binary search still forbidden | 2026-07-31T04:30:19Z | pending | arXiv:2203.00671v2 Section 4 p.24 binary-search remark is not a theorem; no dual variables; certificates verify but are not automatically constructed; `Backend::require_complete()` remains `Error::Incomplete` |
| P9.6a | planned | pending | pending | pending | pending | pending | deferred P9.3.2d global-amortization, exact event-order, and runtime-proof closure | deferred until P9.5 closeout | pending | low priority; gates only `AlmostLinear`, `an19_runtime_verified: true`, and AN19 runtime claims |
| P10 | complete | c5c0e68 | e265e01, aa0a618, 6905b93, ab11586, 332a5c5 | ab7c390 | ab7c390 | `docs/phase-reports/P10-layered-backend.md` | layered public backend, separated P10.8 timing/evidence categories, and P10.9 public-status audit | 2026-08-02T09:01:15Z | 2026-08-03T10:58:31Z | automatic F* search remains blocked; no `AutomaticSource` mode; `Backend::require_complete()` remains `Error::Incomplete`; no AN19 runtime claim |
| P10.1 | complete | c5c0e68 | aa0a618 | pending | pending | `docs/phase-reports/P10-layered-backend.md` | `SolverMode`, `SolverProvenance`, `SourceConfig`, `LayeredResult`, `VerificationSummary`, `LayeredError` | 2026-08-02T09:01:15Z | 2026-08-02T09:20:00Z | no automatic-source variant |
| P10.2 | complete | aa0a618 | aa0a618 | pending | pending | `docs/phase-reports/P10-layered-backend.md` | reference-backed `solve_reference` with exact output and provenance | 2026-08-02T09:20:00Z | 2026-08-02T09:25:00Z | formal-polygon reference path only |
| P10.3 | complete | aa0a618 | aa0a618, 6905b93 | pending | pending | `docs/phase-reports/P10-layered-backend.md` | source-with-target `solve_source_with_target` under inclusive target; honest `UnsupportedOrUndetermined` | 2026-08-02T09:25:00Z | 2026-08-02T09:40:00Z | Appendix B.1 path slow on Figure 3; positive unit test ignored, honest failure tested |
| P10.4 | complete | aa0a618 | aa0a618, ab11586 | pending | pending | `docs/phase-reports/P10-layered-backend.md` | `verify_source_infeasible_below`, `verify_cover_below`, `verify_source_feasible_at_most`, serializable specs | 2026-08-02T09:40:00Z | 2026-08-02T09:50:00Z | dual + compressed cover certificates verified exactly |
| P10.5 | complete | 6905b93 | 6905b93, ab11586 | pending | pending | `docs/phase-reports/P10-layered-backend.md` | CLI `--backend reference|source-with-target --target` and `verify-negative-certificate` | 2026-08-02T09:50:00Z | 2026-08-02T10:05:00Z | source mode supports formal-polygon input only |
| P10.6 | complete | ab11586 | ab11586 | pending | pending | `docs/phase-reports/P10-layered-backend.md` | static audit scans layered module; rejects `AutomaticSource`, automatic `solve_source`, binary search | 2026-08-02T10:05:00Z | 2026-08-02T10:10:00Z | layered no-fallback provenance required |
| P10.8 | complete | a773a81 | 332a5c5 | 4555070 | 4555070 | `docs/phase-reports/P10-layered-backend.md` | `mrd benchmark --suite layered`, typed categories, exact decimal source targets, explicit target provenance, and isolated source-stage timings | 2026-08-03T10:17:36Z | 2026-08-03T10:39:25Z | direct-grid measurement remains unavailable until P11; an undetermined source run is recorded without fallback or target inference |
| P10.9 | complete | c9de7a8 | ab7c390 | ab7c390 | ab7c390 | `docs/phase-reports/P10-layered-backend.md` | public architecture/status audit across README, ARCHITECTURE, ALGORITHMS, KNOWN_LIMITATIONS, NEAR_LINEAR, and TESTING; corrected CLI spelling and public source signature | 2026-08-03T10:39:25Z | 2026-08-03T10:58:31Z | direct-grid implementation remains P11; source target discovery and AN19 runtime claims remain unavailable |
| P11 | in_progress | 06a030d | pending P11.1-P11.2 closeout | pending | pending | `docs/phase-reports/P11-direct-grid-parity.md` | direct grid parity embedding with RankedCoordinates as the permanent Oracle | 2026-08-03T10:58:31Z | pending | direct formula and counters complete; P11.3 must integrate the path through the grid solver |
| P11.1 | complete | 06a030d | pending P11.1-P11.2 closeout | pending | pending | `docs/phase-reports/P11-direct-grid-parity.md` | backend contract, stable result, and zero-rank-allocation counters | 2026-08-03T11:03:42Z | 2026-08-03T11:19:20Z | RankedCoordinates remains the default permanent Oracle |
| P11.2 | complete | 06a030d | pending P11.1-P11.2 closeout | pending | pending | `docs/phase-reports/P11-direct-grid-parity.md` | exact direct parity formula encoder | 2026-08-03T11:03:42Z | 2026-08-03T11:19:20Z | P11.3 must route only finite grid calls through this backend |
| P11.3 | planned | pending | pending | pending | pending | pending | grid pipeline integration through biclique, flow, cut, and rectangle recovery | pending | pending | depends on P11.2 |
| P11.4 | planned | pending | pending | pending | pending | pending | exhaustive/metamorphic differential and invariant campaign | pending | pending | depends on P11.3 |
| P11.5 | planned | pending | pending | pending | pending | pending | direct-vs-ranked release evidence and performance boundary | pending | pending | depends on P11.4 |
