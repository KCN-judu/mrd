# MRD Full Implementation Master Plan

- Plan schema: 1
- Current branch: codex/full-implementation
- Baseline local SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Baseline origin/main SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Current phase: P9
- Current phase state: in_progress
- Last completed phase: P8
- Last pushed SHA: b2b307367f504205a0fc8643d51125d95a9de4bd
- Plan last updated: 2026-07-28T01:24:00Z
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
production path. `BicliqueConstructionMetrics` proves four initial production
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

**State:** in_progress. The source-assumption gate is recorded in
`docs/phase-reports/P09-integration-gate-audit.md`. The first corrective
subphase is recorded in `docs/phase-reports/P09-fractional-rounding.md`:
exact rational feasible-flow validation, deterministic costed cycle rounding,
and a bounded-domain rational surrogate-potential verifier now exist. These
are explicitly superlinear/auditing components, not the fixed-point IPM from
the source. Source-grade IPM accounting, a source-grade Theorem 5.1 query,
source-grade P8 constructions, and MRD integration evidence remain missing. No `AlmostLinear`
backend or fallback has been introduced. Continue in the report's exact
implementation order.
Suggested release:
`v2.0.0-deterministic-almost-linear-flow`.

P9 is split into the following source-gated subphases. Completion of an Oracle
subphase proves only its stated audit contract; it does not satisfy a theorem
gate that is explicitly retained by a later subphase.

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

**State:** in_progress. **Start SHA:** `ffb371f`. Implement the deterministic
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
4. **P9.2.4 state: in_progress. Start SHA: `ab6fb10`.** Implement and certify
   the source initial-point and additive-half termination boundary, then use
   deterministic KP15 rounding and the P7 exact Oracle for recovery
   differentials.

No P9.2 item may use `f64`, the existing rational surrogate, or an unchecked
transcendental result as evidence for Equation (9). The fixed-point word-size
gate must match the papers' `O(log^O(1) z)` model and reject inputs whose chosen
precision cannot certify the required inequalities.

### P9.3 - Source-grade low-stretch and spanner structures

**State:** planned. Replace the P8/P9 static certificate baselines with the
source constructions and accounting required by Lemmas 5.4--5.5, Theorem 8.2,
and Theorem 1.2. Preserve each exact constructor/verifier as an Oracle. Require
the precise decremental/vertex-split domain, embeddings, stretch, congestion,
recourse, rebuild, and bounded-weight assumptions before recording a source
runtime guarantee.

### P9.4 - Source-grade dynamic minimum-ratio cycle

**State:** planned. Implement the deterministic paper's Theorem 5.1, including
the complete tree chain, shifted branches, dynamic sparsification, link-cut
updates, hidden-stability approximation, compact cycle output, and amortized
`Update`/`Query`/`Detect` accounting. The enumerating cycle query and P8 replay
remain permanent exact Oracles and are not allowed as a fallback.

### P9.5 - Integrated exact flow backend

**State:** planned. Introduce an `AlmostLinear` backend only after P9.2--P9.4
prove every source assumption. Integrate the IPM, dynamic query, additive-half
termination, deterministic rounding, and exact recovery without invoking
Dinic, Push--Relabel, or an enumerating Oracle. Differentially require identical
flow values and valid cuts, covers, chords, and rectangles on MRD compressed
networks.

### P9.6 - Phase-wide source and complexity audit

**State:** planned. Audit theorem-to-code traceability, checked domains,
precision, operation counters, no-fallback traces, exact differentials, and
compressed-network evidence. P9 remains `in_progress` or `audit_failed` until
this audit proves the advertised deterministic almost-linear bound.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P10 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P10 automatically unless a persisted hard blocker exists.

## P10 - Direct grid parity embedding

**State:** planned. Add `EmbeddingCoordinateBackend` with `RankedCoordinates`
and `DirectGridParity`. For finite integer pixel grids encode horizontal as
`(2*l, -2*r, 2*y, -2*y)` and vertical as
`(2*x+1, -2*x+1, 2*t+1, -2*b+1)`. DirectGridParity must build no coordinate
rank sets/maps/sorted vectors. Require dominance/intersection equivalence, no
cross-side equality, exact biclique/network/flow/cut/rectangle equality, and
`rank_sort_count`, `rank_map_entry_count`, and `rank_map_owned_bytes` all zero.
Keep RankedCoordinates permanently as Oracle. Suggested release:
`v2.1.0-direct-grid-parity-embedding`.

### Mandatory transition after this phase

After the phase has passed its full audit, been committed, and been pushed:

1. Run `git fetch origin`.
2. Verify `origin/codex/full-implementation` equals local `HEAD`.
3. Reopen and reread `docs/IMPLEMENTATION_MASTER_PLAN.md`.
4. Reread the complete Global Rules section.
5. Reread the complete P11 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P11 automatically unless a persisted hard blocker exists.

## P11 - Constant-factor performance hardening

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
5. Reread the complete P12 phase section. Do not rely on memory.
6. Update `Current phase`, `Current phase state`, `Last completed phase`, and `Last pushed SHA`.
7. Begin P12 automatically unless a persisted hard blocker exists.

## P12 - Final verification, benchmark, and strict report

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
4. Mark P12 complete only after all final outputs and release gates pass.
5. Update phase fields and stop unless an explicitly authorized release action remains.

## Append-only progress log

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
| P9 | in_progress | 32d7b49 | 0abf2a1, d1cefdd, 48250f5, 959ed00, 4e66be4, ffb371f | pending | ffb371f | `docs/phase-reports/P09-integration-gate-audit.md` | exact P9.1 Oracles; no almost-linear backend | 2026-07-27T17:02:52Z | pending | P9.2--P9.6 source gates remain |
| P9.2.1 | complete | d1c7a3b | 44d50d4 | 44d50d4 | 44d50d4 | `docs/phase-reports/P09-fixed-point-arithmetic.md` | certified dyadic `log`, `exp`, and negative-power intervals; 191 workspace tests passed | 2026-07-27T23:36:48Z | 2026-07-28T00:05:39Z | Equation (9) integration continues in P9.2.2 |
| P9.2.2 | complete | 44d50d4 | bba500e | bba500e | bba500e | `docs/phase-reports/P09-certified-ipm-quantities.md` | certified Equation (9), lengths, gradients, and approximation hypotheses; 194 workspace tests passed | 2026-07-28T00:05:39Z | 2026-07-28T00:23:57Z | Lemma 4.4 transitions continue in P9.2.3 |
| P9.2.3 | complete | bba500e | ab6fb10 | b2b3073 | b2b3073 | `docs/phase-reports/P09-lemma-44-updates.md` | certified Lemma 4.4 transition, strict-interior/potential-drop checks, and Detect ledger; full workspace audit passed | 2026-07-28T00:24:00Z | 2026-07-28T01:20:00Z | P9.2.4 initial point, termination, and recovery remain |
