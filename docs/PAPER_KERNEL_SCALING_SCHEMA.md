# Paper-Kernel-Scaling Schema and Reproduction

This document specifies the schema-v2 in-process geometry diagnostic. It is
separate from the accepted schema-v1 P15 artifacts; v1 files remain readable by
the analyzer with fine phases marked unavailable.

## Protocol Identity

Each campaign records the normalized configuration SHA-256, git commit, release
binary SHA-256, generator version, host/compiler metadata, and schema version.
The checkpoint additionally records its own schema version and a campaign
identity derived from those values. Resume accepts only an exact match. Every
point is keyed by family, target size, seed, and the configuration identity;
every measured row has the stable identity

```text
paper-kernel-scaling:v2:{family}:{target}:{seed}:{canonical-instance}:measured:{scope}:{algorithm}:{iteration}:{backend}
```

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

## Timing Fields

All durations are monotonic `Instant` measurements serialized as integer
nanoseconds. Setup contains instance generation, input normalization, and
connected-component extraction. Scope A includes the canonical component clone,
shared geometry, chords, representation/solver, completion/recovery, and
validation. Scope B includes only representation/solver work after common
geometry. The following nested parent/child ledgers are emitted for every
applicable row:

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
