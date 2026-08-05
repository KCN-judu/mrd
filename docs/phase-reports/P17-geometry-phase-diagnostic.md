# P17 Geometry-Phase Diagnostic

## Status and scope

P17 is complete for its predeclared finite campaign. The phase isolates the
shared geometry work exposed by the in-process kernel campaign and evaluates a
prepared-occupancy boundary-discovery path against the historical
directed-edge-toggle path. The result is a diagnostic of measured costs, not an
asymptotic theorem, a universal backend policy, or an AN19 runtime proof.

The source implementation is recorded at commit
`211308a4981c09ccd549bd0ed322db847f427ce3`. The release binary has SHA-256
`59ae735a1c99726cb9d298aba1aafc71d5aae4da6e26683d248f6dddb6821ba3`, and the
schema-v2 campaign configuration has SHA-256
`206d1a3cab0c0d7f4d9821acf7f86ba73a4843105ed2b04bc1b928fe951fba51`. The
compressed machine-readable paired analysis is
[`geometry-phase-summary.json.zst`](../../results/geometry-phase-summary.json.zst),
and the generated report is
[`geometry-phase-report.md`](../../results/geometry-phase-report.md).

## Research question

The historical boundary constructor enumerates four directed unit edges for
every foreground cell, cancels opposite edges in a hash set, and then applies
the reductions that produce normalized loops, reflex vertices, and sorted unit
edges. This path is exact, but the cancellation stage performs work for every
internal edge even though only exposed edges are needed downstream. P17 asks
whether a prepared occupancy index can remove that cancellation work while
preserving every geometric and solver invariant.

The comparison is deliberately narrow. Both paths share adjacency construction,
loop tracing, loop normalization, area conservation, reflex detection, index
construction, chord generation, representation construction, solver execution,
completion, and output validation. Only the edge-discovery backend changes.
This design makes a boundary-phase difference measurable without conflating it
with a different solver or a different generated instance.

## Component responsibilities and known issues

### Boundary discovery

`Boundary::from_component_with_metrics` implements the reference
`reference-edge-toggle` backend. It probes four candidate edges per occupied
cell and toggles the reverse edge in a hash set. The implementation is simple
and independently auditable, but its transient set contains edges that later
cancel. `Boundary::from_prepared_component` implements the
`prepared-exposed-edges` backend. It performs the same four occupancy probes,
inserts only exposed edges, and sends the resulting edge set through the same
downstream builder.

Both paths are output-sensitive after discovery. The reference path has
expected linear work in the number of cells under a constant-time hash model
and linear worst-case auxiliary storage. The prepared path has four occupancy
probes per cell plus one insertion per exposed unit edge, hence measured work
of the form `O(N + U)` and retained boundary-edge storage `O(U)`; because
`U <= 4N`, this is not a new asymptotic bound. Its intended benefit is a lower
constant and less cancellation traffic, not a complexity-class change.

### Boundary reductions and geometric indexes

The shared builder constructs outgoing adjacency, traces every boundary loop,
simplifies collinear vertices, checks signed area against the occupied-cell
count, identifies reflex vertices, and sorts unit edges deterministically. The
known risk is that a faster discovery path could silently change loop
orientation, hole handling, or endpoint identity. P17 therefore measures these
stages separately and treats their equality as a correctness gate.

### Chord generation

The chord stage groups reflex vertices, emits horizontal and vertical chords,
filters invalid candidates, and builds endpoint indexes. It is common to both
backends and is recorded once in `shared_scope_b_preprocessing`. The prior
reporting defect was that this shared row retained only fine-grained leaves;
the analyzer now retains and fits the recorded geometry and chord parent
totals as explicit shared phases.

### Representation and solver kernels

Scope B constructs either the compact representation or one of the two
explicit reference networks, then runs matching/flow and minimum-vertex-cover
recovery. These paths are not changed by P17. They remain in the campaign so
that the report can distinguish a boundary optimization from a downstream
solver bottleneck.

### Completion and validation

Scope A includes cut selection, rectangle completion, reconstruction, internal
validation, and final output validation. These stages enforce the output
rectangle count and witness checks. They are included in end-to-end local
comparisons but are not attributed to the boundary speedup.

### Evidence protocol

The runner records setup, nested phase ledgers, structural counts, correctness
census, stable sample identities, and source/binary/config provenance. The
analyzer validates exact identities and parent/child timing sums, retains
stopped levels as censored observations, and uses fixed-seed bootstrap intervals
over predeclared size levels. This prevents a favorable boundary timing from
being reported without its corresponding structural and correctness evidence.

## Implementation effort

The implementation introduced `BoundaryBuild` and
`BoundaryBuildMetrics`, factored the common boundary reductions into one
builder, and retained the old discovery path under the oracle/reference
namespace. `PreparedComponentContext` carries the prepared occupancy object,
boundary metrics, index timing, reflex-grouping timing, and an explicit backend
identity. The verification kernel exports these values without changing the
schema-v1 P15 artifacts; schema v2 is used only for the kernel campaign.

The analyzer was extended with setup and shared-preprocessing scopes,
fine-phase accounting, structural invariants, before/after pairing, and
shared-parent fits. The runner accepts partial stopped rows, propagates a
predeclared stop to larger levels, retries only nonterminal runner failures,
and rejects a resume when any provenance identity differs. The protocol tests
cover canonical sample census, partial stopped rows, shared parent accounting,
checkpoint identity, and CSV flattening.

## Correctness invariants

The following invariants are checked before timing comparisons are accepted:

1. The reference and prepared paths produce byte-for-byte equal normalized
   boundaries, including loops, orientations, reflex vertices, and sorted unit
   edges.
2. Exposed-edge and trace-visit counts agree between paths; the reference
   candidate probe count is exactly `4N`.
3. Signed boundary area equals twice the foreground-cell count, and all
   structural equalities (`q = H + V`, `M = compact nodes + compact arcs`, and
   output count equals optimum) hold.
4. Every production algorithm returns the same optimum rectangle count and
   passes the witness checksum gate on every complete point.
5. Pairing uses family, target size, seed, canonical instance identity, scope,
   algorithm, and iteration. A structural mismatch is a hard comparison
   failure, not an omitted data point.

The Rust differential suite exhausts all 511 nonempty 3x3 masks, samples 2,048
deterministic 4x4 masks, exercises topology fixtures, and checks translated,
reflected, and rotated instances. The final campaign adds 174 correctness
checks per backend across its 57 complete points; no check failed.

## Scientific sampling protocol

The population contains six deterministic families:
`random-connected`, `dense-conflict`, `sparse-conflict`, `comb-staircase`,
`supported-holes`, and `representation-crossover`. Each family is evaluated at
target sizes 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, and 8192. The two
backends use identical seeds, generator versions, protocol fields, and release
binary. Three production algorithms are timed in a counterbalanced in-process
design: compact MRD, explicit Hopcroft--Karp, and explicit C0 flow.

Warm-up and repetition counts follow the predeclared adaptive rule in the
configuration. Measured rows retain their stable canonical identity; no rows
are discarded because they are slow or inconvenient. Phase fits require at
least six complete, distinct size levels. Confidence intervals use the fixed
bootstrap seed and 10,000 resamples specified by the protocol.

The final census contains 60 planned and observed points per backend. Each
backend has 57 complete points and three stopped points, with 9,826 retained
iterations for the reference backend and 9,954 for the prepared backend. The
paired comparison contains 57 complete point pairs, zero structural mismatches,
zero objective mismatches, identical canonical instances, and no change in
stop/censoring state.

## Results

### Boundary discovery and Scope A

The table reports the compact-MRD representative. The ratio is reference time
divided by prepared time; values above one favor prepared occupancy. It is the
median of per-iteration ratios aggregated across complete target-size levels.

| Family | Valid levels | Boundary total speedup | 95% CI |
| --- | ---: | ---: | ---: |
| comb-staircase | 10 | 1.244 | [1.209, 2.114] |
| dense-conflict | 7 | 7.773 | [3.319, 17.062] |
| random-connected | 10 | 2.865 | [1.870, 4.586] |
| representation-crossover | 10 | 3.449 | [3.151, 3.773] |
| sparse-conflict | 10 | 1.515 | [1.467, 1.545] |
| supported-holes | 10 | 1.779 | [1.695, 1.849] |

The boundary-edge-discovery leaf shows the same ordering and a larger signal:
the family-level medians range from 2.17x on comb-staircase to 17.77x on
dense-conflict. This confirms that the measured gain is located in the
intended discovery stage rather than being an artifact of a downstream solver.

The Scope A total-time speedup is smaller because it includes cloning,
representation, solver, completion, and validation:

| Family | Valid levels | Scope A speedup | 95% CI |
| --- | ---: | ---: | ---: |
| comb-staircase | 10 | 1.150 | [1.141, 1.292] |
| dense-conflict | 7 | 1.705 | [1.391, 1.838] |
| random-connected | 10 | 1.364 | [1.201, 1.561] |
| representation-crossover | 10 | 1.808 | [1.763, 1.852] |
| sparse-conflict | 10 | 1.297 | [1.277, 1.310] |
| supported-holes | 10 | 1.392 | [1.359, 1.410] |

These are finite host-specific measurements. They do not imply that the
prepared path is faster for every component or every machine.

### Shared preprocessing diagnosis

At the largest complete level of each family, the dominant shared phase is
`geometry_preprocessing_ns`, accounting for 92.1%--99.8% of the two recorded
shared parent phases. The best structural variable and its empirical log-log
slope are:

| Family | Target | Share | Variable | OLS slope | 95% CI | R2 | Levels |
| --- | ---: | ---: | --- | ---: | ---: | ---: | ---: |
| comb-staircase | 8192 | 0.965 | H | 1.724 | [1.571, 1.858] | 0.994 | 8 |
| dense-conflict | 1024 | 0.982 | N | 0.702 | [0.645, 0.755] | 0.996 | 7 |
| random-connected | 8192 | 0.921 | N | 0.555 | [0.486, 0.621] | 0.985 | 10 |
| representation-crossover | 8192 | 0.998 | N | 1.047 | [1.018, 1.073] | 0.999 | 10 |
| sparse-conflict | 8192 | 0.974 | N | 1.053 | [1.035, 1.074] | 0.999 | 10 |
| supported-holes | 8192 | 0.942 | N | 1.046 | [1.023, 1.071] | 0.999 | 10 |

The explanatory variable is selected by R-squared among correlated structural
measures. It is descriptive and does not identify a causal law. The shared
parent fits are empirical descriptions over the measured levels, not proofs
of `O(N)`, `O(B)`, or any other asymptotic bound.

### Post-optimization bottlenecks

After boundary discovery is accelerated, the largest Scope A phase depends on
the family. For compact MRD, the dominant phase at the largest complete level
is canonical-component cloning on comb-staircase and representation-crossover,
and representation construction on the other four families.

| Family | Dominant Scope A phase | Share |
| --- | --- | ---: |
| comb-staircase | canonical_component_clone_ns | 0.800 |
| dense-conflict | representation_construction_ns | 0.607 |
| random-connected | representation_construction_ns | 0.758 |
| representation-crossover | canonical_component_clone_ns | 0.626 |
| sparse-conflict | representation_construction_ns | 0.643 |
| supported-holes | representation_construction_ns | 0.657 |

This result rejects a single universal post-optimization bottleneck claim.
The next optimization hypotheses are therefore family-specific: reduce
canonical cloning overhead where geometry is simple, and inspect representation
construction where conflict structure dominates. No change is justified solely
by the current empirical slopes; each hypothesis requires a separate paired
experiment and the same invariant gates.

### Structural context

At the largest complete point, dense-conflict has `K/M = 44.27` and
representation-crossover has `K/M = 30.25`; random-connected has `K/M = 1.385`.
Sparse-conflict and supported-holes have zero explicit conflicts, so a positive
`K/M` compression ratio is undefined. These values explain why solver and
representation phases differ across families, but they do not establish a
causal performance model.

## Censoring and limitations

The dense-conflict point at target 2048 stopped when its preflight iteration
exceeded the 5,000,000,000 ns limit. Targets 4096 and 8192 were retained as
propagated stopped states. They remain visible in `level_accounting`, but no
stopped point contributes a timing median, speedup, or fit. This is deliberate
censoring, not an estimate of an unobserved runtime.

The campaign does not measure allocator-level maximum RSS; structural byte
counts are declared estimates used by the executable budget. Results are
specific to the recorded Apple M4 host, compiler, generator families, and
target range. Correlated structural variables prevent causal interpretation of
the highest-R2 variable. Scope B is a kernel view, not an end-to-end runtime.

The implementation path for P9.3.2d is complete and exercised by the existing
source-flow chain, but its reduced-event conversion and ordering proof remain
deferred at low priority. P17 neither blocks that implementation path nor
supplies the missing proof.

## Claim-evidence map

| Claim | Evidence | Status |
| --- | --- | --- |
| Prepared occupancy reduces measured boundary construction time on the declared families. | Paired `boundary_total_build_ns` speedups, 10,000-bootstrap CIs, identical binary and canonical instances. | Supported for the finite campaign. |
| The optimization preserves geometry and solver semantics. | Exhaustive 3x3 mask differential tests, deterministic 4x4 samples, topology transforms, 174 correctness checks per backend, zero structural/objective mismatches. | Supported for the tested domain. |
| Geometry remains a major shared cost after the optimization. | Shared parent phase decomposition; geometry accounts for 92.1%--99.8% at largest complete levels. | Supported in the measured range. |
| The prepared path changes the asymptotic complexity class. | Both paths retain four occupancy probes per cell and shared output-sensitive reductions. | Rejected; no such claim is made. |
| The optimized implementation has one universal downstream bottleneck. | Family-level dominant-phase table shows cloning versus representation construction. | Rejected by the observed evidence. |
| P17 proves the AN19 runtime theorem or automatic source target decision. | No such theorem or target contract is measured here. | Out of scope; P9.6a remains deferred and P9.5e.3g.3 remains the blocker. |

## Reviewer-facing self-review

- Contribution: the phase isolates a concrete constant-factor geometry change
  and records the exact conditions under which it is beneficial.
- Reproducibility: source, binary, configuration, canonical pairing, sampling
  rule, censored states, and generated artifacts are all bound by hashes.
- Experimental strength: the comparison uses an independent reference path,
  six families, ten target levels, three solver implementations, and fixed-seed
  bootstrap intervals rather than a single favorable example.
- Evaluation completeness: exhaustive small masks and production correctness
  gates cover geometry semantics; remaining bottleneck hypotheses are marked
  as future experiments rather than silently optimized.
- Method soundness: the implementation preserves the common downstream
  builder and does not claim an unsupported asymptotic improvement, RSS result,
  or AN19 proof.

The raw and derived machine-readable artifacts can be restored using the
[`geometry-phase-archive-manifest.json`](../../results/geometry-phase-archive-manifest.json).
The complete reproduction commands and
schema contract are in
[`PAPER_KERNEL_SCALING_SCHEMA.md`](../PAPER_KERNEL_SCALING_SCHEMA.md).
