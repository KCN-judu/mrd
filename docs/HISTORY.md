# Project History

## Purpose

This is the compact historical record for the MRD workspace. It replaces the
former phase-by-phase reports and release-note files, which repeated command
transcripts, intermediate counters, and superseded next-action text. Current
design, implementation, experiment, and limitation documents remain at the
top level of `docs/`; machine-readable evidence remains in `results/`.

This file preserves only decisions that affect the present artifact: what was
introduced, what evidence remains, and what still does not follow from that
evidence. Full prior prose is recoverable from Git history.

## Release Line

| Release period | Lasting result | Evidence boundary |
| --- | --- | --- |
| v0.2 | Independent exact grid solvers, compressed dominance flow, exhaustive and external-oracle campaigns | Finite coloured unit-cell grids and recorded filters only |
| v0.4-v0.5 | Indexed geometric completion, prepared grid context, dense recovery | Exact equality with retained rescan/hash references |
| v0.6-v0.8.1 | Clean hole-free path-tree representation and orientation audit | Eligibility-gated grid specialization; heuristic orientation remains diagnostic only |
| v0.9-v1.1 | Boundary-native ordinary polygons, prepared polygon context, multi-backend chord differential | Accepted ordinary-loop polygon model only |
| v1.2-v1.3 | Dynamic cut index, sparse subdivision, event-driven validation, output-sensitive polygon path | Sparse/reference agreement on recorded finite corpora; no unmeasured RSS or universal crossover claim |

The complete release-era result populations remain identifiable through
`results/release-index.json`, `results/manifest.json`, and the versioned result
files. These artifacts are the evidence source; this table is only a reading
guide.

## Phase History

### P0-P1: Plan and Baseline

The implementation program established a protected long-lived branch, an
audit protocol, reference-backend retention, and reproducible baseline
artifacts. The v1.3 baseline remains the comparison point for subsequent
changes. Historical command logs were removed because the current audit
protocol is maintained in `IMPLEMENTATION_MASTER_PLAN.md` and exact outputs
remain in `results/`.

### P2-P3: Formal Boundary Geometry

The project added canonical formal rectilinear polygons, ornaments, formal
incidence, effective chords, matching, completion, and Definition 2
validation. Formal boundary behavior is documented normatively in
`IMPLEMENTATION.md`; eight final named fixtures provide the finite
end-to-end evidence. The construction does not imply support for every
topological contour-contact configuration.

### P4-P5: Compact Matching and Practical Flow

The four-dimensional dominance embedding received a presorted
Cardinal--Yuditsky biclique partition with exact partition auditing. Exact
Dinic and push-relabel implementations are available as practical flow
backends. The retained result files establish equality with explicit matching,
cover, cuts, and rectangles for their stated populations. They do not turn a
practical flow implementation into an almost-linear theorem.

### P6-P8: Exact Source-Flow Foundations

The workspace added exact circulation and min-ratio Oracles, fixed-point and
interior-state certificates, rooted-forest primitives, and finite dynamic
min-ratio contracts. These components make source-shaped experiments
traceable. Their claims remain deliberately finite and semantic; they do not
establish the full cited dynamic data structures or their running times.

### P9.3.2d Runtime-Proof Deferral and Source Flow

P9 separated two kinds of progress that must not be conflated:

- **Implemented and audited.** Exact rational state, source-structure
  contracts, static and dynamic finite-domain constructions, a source-shaped
  all-radii event engine, traces, local event certificates, and a caller-target
  source backend are implemented. The `oracle` and `experiment` namespaces
  were also made explicit in a breaking functional-architecture refactor.
- **Finite evidence.** The A--H event campaign has 31 fixed snapshots with
  Oracle/reduced trace agreement. A local certificate bounds semantic events
  by `3n + 4m + 2` and queue items by `n + 2m + 2` for one snapshot. A
  practical binary heap has its own weaker per-snapshot accounting bound.
- **Rejected shortcut.** A parameterized power-of-two chord family has only
  `O(log N)` original length classes but `Omega(N)` exact reduced-cost classes.
  Original length labels therefore cannot be used as a surrogate bound on
  reduced-event classes.
- **Blocked automatic target discovery (P9.5e.3g.3).** The source path accepts
  a caller-supplied inclusive target and verifies certificates, but cannot
  discover `F*`. The checked target-decision route presupposes an exact primal
  optimum and invokes an exact solver, so it cannot provide an independent
  negative decision.
- **Deferred runtime proof (P9.6a).** The formal SIAM paper by
  Abraham--Neiman, DOI [10.1137/17M1115575](https://doi.org/10.1137/17M1115575),
  was checked. It does not supply the reduced-event ordering/counting
  conversion required for costs
  `c_x(u,v) = ell(u,v) + d(x,u) - d(x,v)`. The AN19 runtime is therefore not
  verified. Tests, workspace scans, and local event counts are implementation
  evidence, not a substitute proof.

P9 source-flow evidence is retained in `results/an19-event-adversarial.json`,
`docs/IMPLEMENTATION.md`, and the current implementation plan. The P9 implementation is research-only
where the target or proof obligations require it; it is not an automatic
almost-linear solver.

### P10-P11: Provenance and Direct Grid Parity

The public layered boundary now distinguishes complete reference solving,
caller-target source execution, and negative-certificate verification. It
never silently falls back or infers a target. The direct finite-grid parity
embedding removes ranked-coordinate construction on its path while retaining
the ranked embedding as an Oracle. The 3x3 parity census covers 511 masks,
897 components, and 1,794 paired comparisons with zero mismatches and zero
direct rank counters.

### P13-P14: Hardening and Final Evidence

P13 hardened constant factors through direct-grid counters, prepared pipeline
reuse, and diagnostic-only local timing boundaries. P14 consolidated final
correctness, performance, and complexity reports, manifests, and campaign
artifacts. The later 31-process direct-grid sampling campaign is described in
`IMPLEMENTATION.md`; it is local descriptive evidence, not a
portable speed or complexity claim.

### P17: Prepared-Occupancy Boundary Diagnostic

P17 compared the historical directed-edge-toggle boundary constructor with a
prepared-occupancy path while retaining the same reductions, chord generation,
representations, solvers, completion, and validation. The accepted finite
campaign showed family-specific boundary improvements and zero structural or
objective mismatches. A later audit found that the first dominance paragraph
omitted coarse geometry/completion parents; the raw P17 artifacts remain
unchanged and the corrected interpretation is recorded in
`phase-reports/P17-geometry-phase-diagnostic.md`.

### P18: Canonical Ownership Audit

P18 separates the benchmark-boundary deep clone from immutable canonical input
and algorithm-local mutable workspace. The clone reference remains executable;
the borrowed path uses ordinary Rust borrowing, with no compatibility layer,
`Arc`-wide ownership, interior mutability, or unsafe aliasing. Scope A records
clone, borrow/share, release, and workspace preparation independently. P18 is
an implementation and measurement-harness optimization, not a new asymptotic
production algorithm. Structural byte values are capacity-based payload
estimates and are not allocator or RSS measurements.

The first exploratory P18 run was rejected because it used a dirty source,
missing top-level/config provenance, non-standard `NaN` JSON, stale allocation
fields, and repeated timing-accounting retries. It is retained outside the
repository as a rejected pre-acceptance record and must not be silently
overwritten by the clean campaign. The accepted clean campaign, its archive
manifest, and the one recommended post-clone representation-workspace
experiment are recorded in the P18 phase report.

## Retained Evidence Map

| Question | Current source of truth |
| --- | --- |
| What the workspace implements and what remains limited | `IMPLEMENTATION.md` |
| Which exact inputs and solvers agreed | `IMPLEMENTATION.md`, `results/final-correctness-report.md`, `results/final-campaigns/` |
| How local timing was sampled | `IMPLEMENTATION.md`, `results/benchmark-sampling.json`, `results/benchmark-sampling-runs.csv` |
| Formal input model and geometry | `IMPLEMENTATION.md` |
| Current source-flow constraints | `IMPLEMENTATION_MASTER_PLAN.md`, `IMPLEMENTATION.md` |
| Original detailed prose | Git history before the history-consolidation commit |

## Consolidation Policy

Historical process documents should record only durable decisions in this
file. Add a new item only when it changes the current supported scope,
evidence boundary, or unresolved obligation. Store measurements, manifests,
and counterexamples as structured artifacts. Keep current implementation
contracts in the consolidated implementation document instead of recreating
release notes or per-step diaries.
