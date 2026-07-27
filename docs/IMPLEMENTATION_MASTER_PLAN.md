# MRD Full Implementation Master Plan

- Plan schema: 1
- Current branch: codex/full-implementation
- Baseline local SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Baseline origin/main SHA: 72ce32a6fbde3c2d285ca7b8c9a21dc17e0dea64
- Current phase: P3
- Current phase state: committed
- Last completed phase: P2
- Last pushed SHA: 659d7fbbe0ae513058a5a1f41f2cd3d3a39e7118
- Plan last updated: 2026-07-27T12:41:01Z
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

Known gaps, established by code and `docs/KNOWN_LIMITATIONS.md`, are formal
boundary ornaments/points/segment holes/point holes and merge-delete cases;
recursive re-sorting in biclique construction; only Dinic for flow; no
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

**State:** committed. **Start SHA:** `521f82d`. Implement the original Soltan--Gorpinevich formal-boundary
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
the accepted one-component formal input model. Closeout SHA is pending this
documentation commit and remote verification.

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

**State:** planned. Remove recursive re-sorting from Cardinal--Yuditsky using
presorted coordinate orders, stable partitioning, reusable scratch arenas, and
structural complexity counters. Require exact edge partition, matching, cut,
cover, and rectangle equality with the current implementation. Suggested
release: `v1.6.0-presorted-biclique-construction`.

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

**State:** planned. Generalize flow selection and add robust practical exact
backends, including highest-label push-relabel with global relabel and gap
heuristics; preserve Dinic. Require exact value and canonical source-cut
equality where applicable, valid certificates otherwise, generic-network
differential fuzzing, and compressed-network benchmarks. Suggested release:
`v1.7.0-exact-flow-backends`.

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

**State:** planned. Read the FOCS 2023 / arXiv `2309.16629` source and required
predecessors. Create `docs/NEAR_LINEAR_FLOW_IMPLEMENTATION.md`, mapping every
theorem, data structure, assumption, precision rule, and recovery step to an
intended Rust module. Do not claim implementation from an interface skeleton.
Mark blocked, rather than inventing a component, if authoritative source is
unavailable.

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

**State:** planned. Implement generic circulation with exact capacities, costs,
demands, residual operations, a baseline minimum-ratio-cycle Oracle,
iterative refinement, and exact recovery. Superlinear work is permitted here
to establish correctness before dynamic structures.

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

**State:** planned. Before coding, split P8 into source-backed numbered
subphases here. Implement required dynamic low-stretch trees, vertex and edge
sparsification, dynamic spanner, embeddings, and amortized update accounting.
Each subphase gets its own audit, commits, push, and reread transition.

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

**State:** planned. Integrate dynamic minimum-ratio-cycle with iterative
refinement and exact max/min-cost recovery. Name it `AlmostLinear` only with
complete source mapping, all assumptions checked, no Dinic/push-relabel
fallback, exact differential gates, proof-matching counters, checked
polynomial capacity/cost bounds, and identical compressed MRD cuts, covers,
chords, and rectangles. Suggested release:
`v2.0.0-deterministic-almost-linear-flow`.

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
| P3 | committed | 521f82d | 093961f, fd1bbc6, 3d94851, 996ad44, 659d7fb, 6cd0845 | pending | pending | `docs/phase-reports/P03-formal-hole-geometry.md` | `results/p3-formal-fixtures.json`; `results/p3-polygon-differential.json`; zero counterexamples | 2026-07-27T10:39:44Z | 2026-07-27T12:41:01Z | none |
