# Paper-Kernel-Scaling Schema and Reproduction

This document specifies the schema-v2 in-process geometry diagnostic and the
schema-v1 P18 ownership wrapper. It is separate from the accepted schema-v1
P15 artifacts. A P18 analyzer invocation must receive the wrapper explicitly;
an unwrapped or incompatible document fails with a version/campaign error
instead of being interpreted as ownership evidence.

## Protocol Identity

Each campaign records the normalized configuration SHA-256, git commit, release
binary SHA-256, generator version, host/compiler metadata, and schema version.
The checkpoint additionally records its own schema version and a campaign
identity derived from those values. Resume accepts only an exact match. Every
point is keyed by family, target size, seed, and the configuration identity;
every measured row has the stable identity

```text
paper-kernel-scaling:v2:{family}:{target}:{seed}:{canonical-instance}:measured:{scope}:{algorithm}:{iteration}:{boundary-backend}
```

P18 runs two independently identified ownership campaigns. Within the combined
P18 artifact, the unique sample key is
`(campaign_identity, sample_identity)`: the nested campaign identity includes
the canonical backend through its normalized configuration SHA-256. The bare
sample identity is intentionally equal across ownership backends so paired
iterations can be matched without parsing a backend-specific suffix.

Atomic checkpoint replacement, duplicate detection, exact point census, and
runner-error/timeout retry are part of the protocol. A point stopped by a
declared resource limit retains any measured rows already produced. The
analyzer treats those rows as censored and excludes them from complete-point
fits; it does not discard or reinterpret them as timings.

## Structural Fields

The `sizes` object reports the same generated component for both backends:

| Symbol | Field | Meaning |
| --- | --- | --- |
| `N` | `foreground_cells_n` | foreground cell count |
| `A` | `bounding_box_area_a` | area of the foreground component bounding box; `N <= A <= width*height` |
| `B` | `boundary_size_b` | normalized boundary vertex count |
| `U` | `boundary_unit_edge_count_u` | exposed unit-edge count |
| `r` | `reflex_count` | reflex boundary vertices |
| `H`, `V` | `horizontal_chord_count_h`, `vertical_chord_count_v` | effective chord families |
| `q` | `q` | `H + V` |
| `K` | `explicit_conflict_edge_count_k` | explicit conflict edges |
| `M` | `compressed_representation_size_m` | compressed network nodes plus arcs |
| `sigma` | `biclique_total_vertex_occurrences_sigma` | total biclique vertex occurrences |

`structure` mirrors the canonical graph and biclique counts and records
candidate probes, exposed edges, trace visits, completion counters, and
estimated retained structural bytes for explicit, compact, and C0 paths. The
byte quantities are estimates, not allocator or RSS measurements. The
`max_estimated_structural_bytes` stop condition is enforced; the optional
`max_rss_delta_bytes` field remains null when the platform probe is unavailable.

The structural estimates are computed from current Rust `Vec` capacities. For
an owned `Vec<T>`, the payload estimate is `capacity * size_of::<T>()`;
allocator metadata, spare allocator rounding, and inline `Vec` headers are
excluded. The audited formulas are:

- `GridComponent` clone payload: `N * size_of::<Cell>()`;
- explicit `BipartiteGraph`: `adjacency.capacity() * size_of::<Vec<usize>>() +
  sum(adjacency[i].capacity()) * size_of::<usize>()`;
- explicit `FlowNetwork`: `arcs.capacity() * size_of::<ArcSpec>()`;
- biclique `Partition`: `blocks.capacity() * size_of::<Block>() +
  sum(left.capacity() + right.capacity()) * size_of::<usize>()`;
- compressed-flow `Network`: its graph estimate plus its partition estimate;
- Scope-A selection workspace: `(horizontal_capacity + vertical_capacity) *
  size_of::<bool>()` bytes; Rust's `Vec<bool>` is not bit-packed.

These are retained structural payload estimates and must not be described as
allocator counts, process RSS, or a memory-safety guarantee. P18 allocation
records additionally report cloned cells and an ownership-layer `Vec`
allocation estimate; the latter is a deterministic audit count, not an
allocator trace.

## Timing Fields

All durations are monotonic `Instant` measurements serialized as integer
nanoseconds. Setup contains instance generation, input normalization, and
connected-component extraction. Scope A includes canonical acquisition, shared
geometry, chords, representation/solver, completion/recovery, and validation.
For `clone-canonical-reference`, canonical acquisition contains the deep
`GridComponent.cells` copy and is recorded in
`canonical_component_clone_ns`. For `borrowed-canonical`, that field is zero
because the copy is absent; the ordinary-reference acquisition is measured in
`canonical_context_borrow_or_share_ns`. Mutable selection-buffer allocation is
separate in `solver_workspace_prepare_ns`. Scope B begins after common geometry
and does not create the Scope A selection workspace. The following nested
parent/child ledgers are emitted for every applicable row:

- boundary extraction and its discovery, adjacency, trace, normalization,
  reflex, and unit-edge-sort children;
- geometry preprocessing and prepared-component, boundary-total, index, and
  reflex-grouping children;
- chord generation and horizontal, vertical, filtering, and endpoint-index
  children;
- completion/recovery and cut materialization, horizontal/vertical completion,
  rectangle reconstruction, and finalization children;
- output validation and internal/final validation children;
- Scope A and Scope B non-overlapping parent-phase sums.

Each ledger reports `*_leaf_sum_ns`, `*_unattributed_ns`, and
`*_accounting_ok`. The strict runner and analyzer require exact sum identities
and reject malformed v2 rows. Shared preprocessing is recorded once per point
in `shared_scope_b_preprocessing` and exported to CSV with the
`shared_preprocessing_` prefix; it is analyzed as the `shared-preprocessing`
phase scope rather than silently mixed into per-iteration Scope B timings.

Every P18 measured row also carries an `allocations` object. It records cloned
cell count, estimated clone payload bytes, estimated retained solver-workspace
bytes, estimated retained representation bytes, and the number of `Vec`
allocations in the audited ownership layer. These are deterministic structural
estimates. They do not include allocator metadata and are not RSS measurements.
The representation estimate uses the actual capacities of graph, network, and
partition buffers; `q`, `K`, `M`, and `sigma` remain the canonical structural
variables used to interpret those estimates.

## Backends and Pairing

`reference-edge-toggle` is the historical directed-edge cancellation path.
`prepared-exposed-edges` probes prepared occupancy and inserts only exposed
edges. Both paths share all downstream boundary reductions and are required to
produce identical structural statistics, optimum, validity, and canonical
rectangle witness on paired points.

The comparison configuration is
`results/geometry-before-after-config.json`. The analyzer consumes it when
`--comparison-config` is supplied with `--compare-input`; it requires equal
protocol fields, the same source commit and release binary, equal generator
identity, matching canonical instances, and paired scope/algorithm/iteration
keys. It emits per-phase speedups, censoring/state changes, structural mismatch
counts, and fits for the two campaigns. Speedups and slopes are finite,
host-specific empirical measurements only.

P18 adds the canonical ownership identities `clone-canonical-reference` and
`borrowed-canonical`. Prepared-context reuse is a future experiment and has no
serializable backend identity yet, so an unimplemented path cannot be recorded
as measured prepared-geometry reuse. The P18 wrapper hashes
the top-level comparison configuration and each backend-specific normalized
configuration, checks source and binary identity, requires clean Git provenance
by default, and requires checkpoint storage outside the repository. It defers
all repository output until both backend provenance snapshots have been
captured. The analyzer independently recomputes both configuration hashes and
campaign identities, verifies the exact terminal and sample census, rejects
duplicates, checks retry counts, and requires zero canonical, structural,
objective, and witness mismatches.

### P18 wrapper and nested schema

The outer document has `schema_version = 1`, `campaign =
"p18-canonical-sharing"`, a top-level configuration SHA-256, two backend
payloads, and a combined completion census. Each nested payload is an ordinary
`paper-kernel-scaling` document with `schema_version = 2` and a normalized
backend-specific `canonical_backend`. The nested campaign identity binds its
configuration SHA, source commit, release binary SHA, checkpoint schema, and
sample schema. The wrapper binds both backend payloads to one predeclared
comparison configuration.

The clean-evidence gate requires `git_dirty = false` at the wrapper and both
nested environments, equal source and release-binary hashes, stable CPU/power
metadata, exact planned and terminal point census, exact adaptive repetitions,
zero duplicate retries, and identical cross-backend measured sample identity
and order for every paired point. A stopped point is retained as censored data;
it contributes no timing median or fit. The P18 analyzer repeats these checks
even when the runner has already validated them.

Older unwrapped schema-v2 kernel files remain valid inputs to the generic
kernel runner/analyzer according to their own schema contract, but they are
not silently promoted to P18 ownership evidence. The P18 analyzer rejects them
with an explicit wrapper schema/campaign error.

## Reproduction

Build one release binary, then run the reference and optimized configurations
with separate outputs and checkpoints:

```text
cargo build --workspace --release
python3 tools/run_paper_kernel_scaling.py \
  --config results/geometry-phase-diagnostic-config.json \
  --binary target/release/mrd \
  --output results/geometry-phase-diagnostic.json \
  --csv results/geometry-phase-diagnostic-runs.csv \
  --checkpoint results/geometry-phase-diagnostic-checkpoint.json
python3 tools/run_paper_kernel_scaling.py \
  --config results/geometry-phase-optimized-config.json \
  --binary target/release/mrd \
  --output results/geometry-phase-optimized.json \
  --csv results/geometry-phase-optimized-runs.csv \
  --checkpoint results/geometry-phase-optimized-checkpoint.json
```

Interrupted runs resume with the same command plus `--resume`. After both
campaigns are complete, analyze the optimized input and pair it with the
reference input:

```text
python3 tools/analyze_paper_kernel_scaling.py \
  --input results/geometry-phase-optimized.json \
  --compare-input results/geometry-phase-diagnostic.json \
  --comparison-config results/geometry-before-after-config.json \
  --summary-json results/geometry-phase-summary.json \
  --summary-csv results/geometry-phase-summary.csv \
  --report results/geometry-phase-report.md \
  --tables results/geometry-phase-tables.tex \
  --figure-dir results/geometry-phase-figures
```

The analyzer requires at least six complete size levels for any reported fit.
It emits seven phase tables plus one paired before/after table, machine-readable
JSON/CSV summaries, booktabs LaTeX, and seven parseable SVG figures. No output
from this diagnostic may be used to claim an asymptotic improvement, an AN19
runtime proof, or automatic source-flow target discovery.

The P17 raw and summary JSON/CSV files are compressed after verification to
avoid committing large duplicate text files. Their SHA-256 values, compressed
sizes, and restore commands are recorded in
`results/geometry-phase-archive-manifest.json`. The archive is evidence
equivalent to the uncompressed files; restore them before invoking the analyzer
again.

### P18 canonical-sharing reproduction

Build the release binary from a clean source commit. Store checkpoints outside
the repository so an interrupted run cannot make the measured source dirty:

```text
cargo build --workspace --release --locked
python3 tools/run_p18_canonical_sharing.py \
  --config results/p18-canonical-sharing-config.json \
  --binary target/release/mrd \
  --output results/p18-canonical-sharing.json \
  --checkpoint-dir /tmp/mrd-p18-canonical-sharing-checkpoints
python3 tools/analyze_p18_canonical_sharing.py \
  --input results/p18-canonical-sharing.json \
  --summary-json results/p18-canonical-sharing-summary.json \
  --summary-csv results/p18-canonical-sharing-summary.csv \
  --report results/p18-canonical-sharing-report.md
```

Resume uses the same command with `--resume` and the same external checkpoint
directory. The campaign must finish with 60 terminal points per backend; the
predeclared adaptive protocol currently expects 57 complete and 3 stopped
points per backend when the dense-conflict limit is reached. Before committing
evidence, compress the raw wrapper, both backend CSV files, summary JSON/CSV,
and report with `zstd -19`, then write
`results/p18-canonical-sharing-archive-manifest.json` containing each archive's
relative path, SHA-256, byte length, source commit, binary SHA, config SHA, and
the exact restore command. Rejected exploratory runs are archived outside the
repository under a distinct name and are described in the final report; they
are never overwritten by a clean run.
