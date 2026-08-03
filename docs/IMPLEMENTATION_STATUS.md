# Implementation Status and Evidence Synthesis

## Abstract

This document is a consolidated implementation account for the Minimum
Rectangular Dissection (MRD) workspace. It separates four kinds of statement:
an implemented mechanism, an exact finite verification result, a proved local
bound, and an unresolved theorem obligation. The project contains independent
exact reference solvers, source-mapped rectilinear geometry, compressed
dominance-flow experiments, direct finite-grid embeddings, and a research
interface for an AN19-shaped flow path. It does not claim that the research
path is an automatic solver or that it realizes the cited AN19 asymptotic
runtime.

The source and result baseline summarized here is commit
`94aecfe481d6c92d13e09bb9a9420933d8afa55f`. The implementation evidence at
that commit is preserved in the final artifacts and phase reports. Later
documentation-only commits do not change the algorithms or rerun history.

## Scope and Reading Order

The supported problem is the exact minimum dissection of finite coloured
unit-cell grids and integer-coordinate rectilinear polygons. A result contains
explicit rectangles and is checked by an exact native validator. The formal
boundary model additionally represents ornaments, point holes, and segment
holes. The intended reading order is:

1. this status synthesis for scope, evidence, and limitations;
2. [`ARCHITECTURE.md`](ARCHITECTURE.md) for ownership and namespace rules;
3. [`ALGORITHMS.md`](ALGORITHMS.md) for solver-level contracts;
4. [`EXPERIMENTS.md`](EXPERIMENTS.md) and
   [`BENCHMARK_SAMPLING_REPORT.md`](BENCHMARK_SAMPLING_REPORT.md) for the
   experimental protocol and data boundaries; and
5. `docs/phase-reports/` and `results/` for immutable, detailed evidence.

The phase reports are retained as primary historical evidence. This document
does not replace their commands, hashes, counterexamples, or source maps.

## Terminology and Claim Classes

| Term | Meaning in this artifact |
| --- | --- |
| Reference Oracle | A deliberately independent, exact implementation retained for differential checking. It need not be fast. |
| Fully audited path | A production-path execution augmented with explicit graph or geometry checks. |
| Compact-only path | A path that avoids selected materializations while preserving the same exact output contract. |
| Finite census | Exhaustive evaluation of a stated finite population, not a theorem about all inputs. |
| Seeded sample | A deterministic pseudo-random population with a recorded seed, not an iid sample from every possible MRD input. |
| Local observation | A timing or allocation measurement on one recorded host and build. It is not portable performance evidence. |
| Source-shaped | An implementation organized around a cited algorithmic construction. It does not by itself prove every cited complexity bound. |

## System Decomposition

| Layer | Primary responsibility | Current status | Main residual issue |
| --- | --- | --- | --- |
| Domain model | Exact grids, rectilinear polygons, formal boundaries, certificates, validation contracts | Implemented | Disconnected outer components and unsupported input classes remain explicit scope limits. |
| Exact references | Exact cover, explicit SG matching, dense/reference geometry | Implemented and permanent | Intentionally slow; not a performance backend. |
| Geometry | Boundary extraction, effective chords, completion, sparse subdivision and validation | Implemented for stated domains | General source-wide completion and all source boundary cases are not claimed. |
| Matching and flow | 4D dominance, biclique partition, exact practical flow | Implemented and differentially audited | The production exact flow is practical Dinic, not the cited almost-linear flow algorithm. |
| Specialized grid path | Prepared components, direct-grid parity, clean path-tree representation | Implemented with finite evidence | Benefits beyond the measured grids require new evidence. |
| Source-flow research path | AN19-shaped event engine and caller-target backend | Semantic path implemented on stated tests | Automatic target discovery and AN19 runtime proof are unresolved. |
| Verification | Census campaigns, differential runs, external oracle, regressions, manifests | Implemented | Fuzzing remains unavailable; finite evidence cannot prove asymptotics. |

## Component Account

### 1. Exact Domain and Boundary Models

**Purpose.** `mrd-domain` defines the values shared by all solvers: coloured
grids, ordinary rectilinear polygons, formal rectilinear polygons, chords,
rectangles, certificates, and structured errors. Its constructors normalize
input and enforce exact integer predicates before an algorithm runs.

**Implemented effort.** The ordinary polygon model supports one nondegenerate
outer loop and ordinary two-dimensional holes. The formal model records
ornament segments and isolated points, derives elementary segments and formal
holes, and serializes canonically. Formal incidence, local nonconvexity, and
Definition 2 coverage are checked independently of area-only validation. The
source mapping is documented in [`FORMAL_BOUNDARY_MODEL.md`](FORMAL_BOUNDARY_MODEL.md).

**Evidence.** Eight named formal fixtures are verified by the final campaign.
The formal construction is also compared against explicit matching and dense
and sparse completion/validation references. The committed results record zero
disagreements for that stated fixture population.

**Current limitation.** The accepted topological model does not make every
possible contour-contact configuration a production input. Disconnected outer
components remain unsupported. These are declared model boundaries, not solver
failures silently handled by a fallback.

### 2. Independent Exact Reference Solvers

**Purpose.** The project needs exact results independent of the compressed
pipeline. `exact-cover-oracle` enumerates valid rectangles and uses bitset
branch-and-bound. The explicit SG route materializes effective-chord conflicts,
uses Hopcroft--Karp and Konig recovery, then constructs and validates
rectangles.

**Implemented effort.** These Oracles are intentionally retained rather than
replaced by compact code. They serve as the reference for chord enumeration,
matching, covers, cuts, rectangle lists, and regression minimization. The
external CP-SAT script is a further independent check that parses grids and
enumerates rectangles without calling Rust geometry.

**Evidence.** The final corpus includes a 4x4 exhaustive grid census of
65,536 inputs and 337,058 components, a seeded 8x8 set of 10,000 inputs and
162,162 components, and all 87,148 free polyominoes through 12 cells. The
reported disagreement and solver-error counts are zero within those
populations. The isolated CP-SAT comparison covers 6,998 inputs and 27,228
components, also with zero reported disagreements.

**Current limitation.** Exact cover and external CP-SAT are validation
backends, not general performance claims. Cell limits, explicit filters, and
timeouts remain part of the recorded protocol.

### 3. Effective Chords and Geometric Completion

**Purpose.** The SG formulation turns a dissection problem into the selection
of a maximum admissible effective-chord family, then adds simple chords in a
canonical horizontal-then-vertical order. Correctness therefore needs both
chord-set equality and exact post-selection construction.

**Implemented effort.** Grid execution uses indexed interior runs while
retaining pairwise enumeration as an Oracle. Ordinary polygon execution uses
an axis-generic sweep. Formal inputs implement the source's merge/delete
construction and retain a definition-level pairwise Oracle. Completion keeps
`ReferenceRescanCompletion` and `IndexedFrontierCompletion`; acceptance
requires equal selected cuts, added cuts, and canonical rectangles rather than
only equal rectangle counts.

**Evidence.** The ordinary polygon differential contains 167,082 supported
components verified with zero mismatches and 2,344 explicit model rejections.
Formal fixtures, grid chord differentials, and polygon backend campaigns are
archived separately. The event-driven sparse validator and range-scan/slab
references agree on their recorded corpora.

**Current limitation.** These implementations are exact for their accepted
domains. They do not transform finite tests into the full classical
source-wide completion bound, and source features outside the model remain
rejected rather than approximated.

### 4. Dominance, Bicliques, and Exact Practical Flow

**Purpose.** Orthogonal chord conflicts are encoded as strict dominance in four
coordinates. The Cardinal--Yuditsky construction replaces one conflict edge
per flow node with a biclique partition, reducing the exact matching/cover
problem to a compressed network.

**Implemented effort.** The fully audited mode checks every geometric conflict
and every biclique edge; compact-only omits the forbidden explicit
materializations but preserves exact certificates. The partition checker
enforces that each explicit edge occurs exactly once. Dinic returns integral
flow and residual-cut certificates, from which the cover and selected chord
family are recovered.

**Evidence.** The project records equality of matching, cover, selected
chords, and rectangles against explicit paths on its stated grid, polyomino,
polygon, and adversarial populations. Structural checks include no missing,
fabricated, or duplicated biclique edge in the audited runs. The 4D
representation bound is documented as `O(q log^4 q)` for the specialization.

**Current limitation.** The deployed exact flow engine is a practical Dinic
backend. It is not described as the cited deterministic almost-linear exact
flow algorithm, and compression measurements on finite families do not prove
a new end-to-end complexity theorem.

### 5. Clean Path-Tree and Prepared Grid Specializations

**Purpose.** Eligible hole-free grids admit a region-dual tree formulation.
Prepared component contexts avoid rebuilding occupancy, runs, boundaries, and
reflex metadata across compact pipeline stages. These are engineering
specializations that retain the 4D route as a fallback.

**Implemented effort.** The clean certificate makes eligibility explicit. The
fully audited mode keeps area-dual and transpose references; compact-only uses
boundary-laminar dual information and HLD segment records. The prepared grid
context feeds run enumeration, dense cuts, recovery, and validation without a
cross-module mutable cache.

**Evidence.** Full 4x4 differential and targeted path-tree family experiments
are retained in the v0.5-v0.8 artifacts. The heuristic bound-estimate
orientation was observed to have positive-regret cases, so both audited and
compact modes default to exact `build-both` selection.

**Current limitation.** The path-tree route applies only when its eligibility
certificate passes. The heuristic selector remains a benchmark control and is
not a correctness or production policy.

### 6. Direct Finite-Grid Parity Embedding

**Purpose.** Integer grid coordinates permit a direct even/odd four-coordinate
embedding, avoiding the rank construction required for arbitrary coordinates.

**Implemented effort.** `DirectGridParity` evaluates the exact closed-form
coordinates and reports zero ranked-coordinate sorts, rank-map entries, and
rank-map owned bytes. `RankedCoordinates` remains the general construction and
permanent comparison Oracle. Both fully audited and compact-only modes are
run on each component.

**Evidence.** The direct-grid parity census covers all 511 nonzero 3x3 masks,
897 foreground components, and 1,794 paired pipeline comparisons. It reports
zero mismatches, zero solver errors, and zero direct ranked-coordinate
counters. The separate sampling protocol and its raw process-level results
are documented in [`BENCHMARK_SAMPLING_REPORT.md`](BENCHMARK_SAMPLING_REPORT.md).

**Current limitation.** The deterministic structural benefit is the eliminated
rank work. A faster end-to-end result is not assumed from it: phase timing and
process timing are host-local observations and are reported separately.

### 7. Source-Shaped AN19 Research Backend

**Purpose.** The research path isolates a faithful all-radii event mechanism
behind replaceable interfaces, preserves exact rational ordering and symbolic
lineage, and emits traces and charges that can support either a future proof or
a counterexample.

**Implemented effort.** A definition-level event Oracle and a reduced-event
engine are compared on 31 fixed A--H snapshots. The implementation preserves
unsplit rounded lengths, source-edge and segment-lineage identities, highway
state, projection identity, exact event keys, and canonical traces. A local
certificate establishes at most `3n + 4m + 2` semantic events and
`n + 2m + 2` queue items per fixed snapshot; a stable binary-heap certificate
accounts for its own counted comparisons.

**Evidence.** `results/an19-event-adversarial.json` records agreement on event
sequence, selected radius, membership, edge partition, and stopping
certificate for all 31 snapshots. This is semantic and local-structural
evidence only. The binary heap supplies a practical per-snapshot
`O((n+m) log(n+m))` comparison bound, not the cited priority-queue bound.

**Current limitation and proof obligation.** The formal SIAM version of
Abraham--Neiman (DOI [10.1137/17M1115575](https://doi.org/10.1137/17M1115575))
was checked. It does not establish the local conversion needed to bound the
number or order of exact reduced-event classes for
`c_x(u,v) = ell(u,v) + d(x,u) - d(x,v)`. The unresolved obligation is to prove
an explicit upper bound on the reduced-event equivalence classes generated by
the cited construction and justify the ordering transformation used here,
with enough detail to derive the runtime. Therefore the source-shaped AN19
runtime is not verified and no local event campaign, workspace scan, or timing
result is treated as such a proof.

### 8. Layered Public Solver Boundary

**Purpose.** The public surface must not mislabel a reference result as a
source result, or an ordinary execution failure as a negative certificate.

**Implemented effort.** `mrd::layered` separates the complete reference
solver, a source path under a caller-supplied inclusive target, and independent
negative-certificate verification. Provenance is serialized. The source path
does not fall back to a reference solver.

**Evidence.** Layered benchmark rows distinguish reference, caller-target,
and unavailable outcomes. The interface has exact certificate checks and
regression coverage for its provenance rules.

**Current limitation.** P9.5e.3g.3 remains blocked: there is no source-backed,
automatic discovery of `F*`, because the available construction does not
provide a valid negative decision. Consequently there is no automatic source
solver entry point or binary-search wrapper.

### 9. Functional Architecture and Process Boundary

**Purpose.** The workspace should expose dependency ownership rather than
repeat ambient prefixes or hide behavior in compatibility shims.

**Implemented effort.** Packages are responsibility-bearing: immutable domain,
graph, reference Oracles, SG geometry, dominance experiments, verification,
and CLI process boundary. `oracle` and `experiment` namespaces state whether a
path is a reference or a production experiment. Shared parents contain stable
types, traits, certificates, and pure orchestration. The CLI owns filesystem
IO, clock reads, process exit, and command dispatch.

**Evidence.** The breaking namespace refactor is recorded in
`docs/phase-reports/P09-functional-architecture-refactor.md`. The architecture
uses static enums/generics and monomorphized traits rather than runtime
registries or compatibility re-exports.

**Current limitation.** Namespaces clarify ownership but do not confer
correctness or complexity by themselves. Every backend still requires the
appropriate reference comparison and stated proof obligations.

### 10. Verification, Regression, and Reproducibility

**Purpose.** Exact algorithm work needs independent disagreement detection,
counterexample minimization, and durable command/result provenance.

**Implemented effort.** Verification contains exhaustive and seeded campaigns,
polyomino enumeration, adversarial fixtures, polygon and formal differentials,
an external CP-SAT Oracle, source-flow trace audits, result manifests, and
release consistency checks. A discrepancy is intended to be minimized and
committed as a regression rather than hidden by a fallback.

**Evidence.** Final-campaign artifacts list exact inputs, components, commands,
hashes, result files, and outcome counts. At the cited source baseline the
full workspace result was 247 passing tests with 3 existing ignored tests.
This result, along with completed workspace-scan counters and invariants, is
implementation evidence rather than a proof of an asymptotic theorem.

**Current limitation.** Fuzzing is unavailable and is explicitly recorded as
unavailable. No finite campaign covers all integer-coordinate polygons or all
formal boundaries, and no performance observation is generalized beyond its
recorded host and build.

## Evidence Map

| Claim | Evidence type | Primary artifact | Boundary |
| --- | --- | --- | --- |
| Exact grid solver agreement | Exhaustive census | `results/final-campaigns/grid-exhaustive-4x4.json` | All binary 4x4 grids only. |
| Generalization beyond the census | Seeded and structural populations | `results/final-campaigns/random-8x8-seed42.json`, polyomino summary | Recorded seed and enumerated size only. |
| Independent solver agreement | External differential | `results/final-campaigns/external-oracle.json` | Selected CP-SAT-compatible components only. |
| Polygon geometry agreement | Differential campaign | `results/final-campaigns/polygon-differential.json` | Supported components; rejections are recorded separately. |
| Formal boundary behavior | Named fixture campaign | `results/final-campaigns/formal-fixtures.json` | Eight fixtures, not all formal polygons. |
| Direct parity equality | Finite census | `results/final-campaigns/direct-grid-parity.json` | All nonzero 3x3 masks only. |
| Direct parity timing variation | Repeated process sample | `results/benchmark-sampling.json` | One host, one release binary, fixed workload. |
| AN19 event semantics | Oracle/reduced trace differential | `results/an19-event-adversarial.json` | 31 fixed snapshots; no global amortization conclusion. |

## Reviewer-Facing Limitations

1. The result corpus is extensive but finite. It supports the stated exact
   populations, not universal correctness or an asymptotic runtime theorem.
2. The direct-grid result has a deterministic structural advantage; timing is
   deliberately confined to local observations with raw samples and protocol.
3. The source-shaped flow path is target-bound and research-only. Automatic
   target discovery is blocked, so it is not a complete production solver.
4. The formal AN19 text was source-checked, but the reduced-event conversion
   required by the local runtime chain remains an open proof obligation.
5. The formal-boundary and polygon implementations have explicit input-model
   limits. Rejected categories remain visible in reports instead of becoming
   unlabelled approximations.

## Maintenance Rules

When this report changes, update the associated result or phase-report link,
state the population and command, and identify whether the change is an
implementation fact, finite empirical evidence, local measurement, or proof.
Do not upgrade an AN19 runtime, automatic target search, or a cross-machine
performance conclusion without the matching implementation and evidence.
