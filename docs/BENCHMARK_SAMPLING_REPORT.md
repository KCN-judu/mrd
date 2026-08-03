# Benchmark Sampling Report

## Abstract

This report defines a reproducible local benchmark campaign for the direct
finite-grid parity embedding. It is deliberately separate from correctness
campaigns: the workload is an exhaustive finite census used to gate semantic
equality, while repeated fresh processes produce a sample of host-local timing
observations. The campaign reports robust descriptive statistics and raw
samples. It does not claim a cross-machine speedup, a throughput guarantee, a
causal performance effect, or an asymptotic result.

## Research Questions

1. Does every repeated run preserve the direct-versus-ranked equality and the
   expected structural counters on the fixed finite workload?
2. What local run-to-run variation is observed for process wall time,
   embedding time, and accumulated phase time on one release binary?
3. What is the paired direct-to-ranked ratio inside the same process, subject
   to the fixed backend ordering of the implementation?

## Population Design

The campaign does not mix distinct populations into one sample. Each kind has
a different inferential meaning.

| Population | Selection method | What it can support | What it cannot support |
| --- | --- | --- | --- |
| 4x4 binary grids | Exhaustive census | Exact agreement on every member of that finite population | Behavior on larger or non-grid inputs |
| 8x8 binary grids | 10,000 deterministic seed-42 draws | Reproducible stress evidence for that generator and seed | An iid estimate for all grids |
| Free polyominoes through 12 cells | Complete canonical enumeration | Exact agreement for the enumerated sizes | Claims about larger polyominoes |
| Ordinary polygon corpus | Grid-derived and named differential corpus | Exact agreement on supported recorded components | All rectilinear polygons or rejected input classes |
| External CP-SAT corpus | Filtered independent-oracle population | Cross-implementation agreement on selected components | Runtime comparison to CP-SAT |
| Direct parity timing workload | All nonzero 3x3 masks, repeated as paired processes | Host-local variation for one fixed complete workload | General speedup or an asymptotic law |

The final-campaign correctness totals remain in
[`results/final-benchmarks.json`](../results/final-benchmarks.json). This
report samples only the final row's direct-grid parity workload; it does not
resample correct and incorrect inputs until a desired result is obtained.

## Workload and Pairing

One measured observation executes:

```bash
target/release/mrd benchmark --suite direct-grid-parity --output <temporary-relative-path>
```

The workload visits all 511 nonzero 3x3 binary masks, extracts 897 foreground
components, and compares the ranked-coordinate and direct-grid-parity paths in
both `fully-audited` and `compact-only` modes. It therefore contains 1,794
paired pipeline comparisons per process.

Within each component and mode, the implementation executes
`ranked-coordinates` before `direct-grid-parity`. That order is intentionally
not randomized because the CLI's existing deterministic correctness campaign
is the object being measured. Pairing controls for a large part of the
per-process host state, but it does not remove fixed-order, cache, thermal, or
background-load bias. The report calls the result an observation rather than a
causal speedup estimate.

## Protocol

The committed driver is
[`tools/run_sampling_benchmark.py`](../tools/run_sampling_benchmark.py). The
standard campaign uses three unrecorded warm-up processes and 31 measured fresh
CLI processes:

```bash
cargo build --release -p mrd
python3 tools/run_sampling_benchmark.py \
  --binary target/release/mrd \
  --warmups 3 \
  --repetitions 31 \
  --output results/benchmark-sampling.json \
  --csv results/benchmark-sampling-runs.csv
```

The script aborts a campaign if any measured or warm-up process returns a
nonzero status, records a mismatch or solver error, changes a finite-population
count, changes the direct zero counters, changes the ranked structural
counters, or omits one of the two verification modes. It never drops a failed
or inconvenient measured sample. The only exclusions are the predeclared
warm-ups.

The JSON artifact records the complete report emitted by every measured CLI
process, the executable hash, source revision, public environment metadata,
and the exact protocol. The CSV exposes one flat row per process for quick
inspection. Temporary CLI output paths are relative and are removed after a
successful run; no absolute local paths are committed.

## Statistical Method

For each of 31 measured observations, the driver records:

- full-process wall time from `perf_counter_ns`;
- accumulated direct and ranked `dominance_embedding` microseconds;
- accumulated direct and ranked diagnostic phase microseconds; and
- direct-to-ranked ratios for the two paired timing quantities.

It reports minimum, first quartile, median, third quartile, and maximum. The
quartiles use inclusive linear interpolation, equivalent to R's type-7
definition. There is no hypothesis test, confidence interval, p-value, or
normality assumption. With one host and a fixed ordering, descriptive robust
summaries are more defensible than a general population inference.

## Recorded Results

The campaign output is generated from the clean source state before the
evidence-publication commit. The result fields below are copied from
`results/benchmark-sampling.json`; raw per-process data is in that JSON and
the parallel CSV. Values are microseconds except ratios.

| Metric | n | Min | Q1 | Median | Q3 | Max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Process wall time | pending | pending | pending | pending | pending | pending |
| Direct embedding time | pending | pending | pending | pending | pending | pending |
| Ranked embedding time | pending | pending | pending | pending | pending | pending |
| Direct all-phase time | pending | pending | pending | pending | pending | pending |
| Ranked all-phase time | pending | pending | pending | pending | pending | pending |
| Direct/ranked embedding ratio | pending | pending | pending | pending | pending | pending |
| Direct/ranked all-phase ratio | pending | pending | pending | pending | pending | pending |

### Correctness Gate

The actual campaign will be reported as a success only if every one of the 31
measured process reports has all of the following fixed outcomes:

| Check | Required value |
| --- | ---: |
| Nonzero 3x3 masks | 511 |
| Foreground components | 897 |
| Paired pipeline comparisons | 1,794 |
| Mismatches | 0 |
| Solver errors | 0 |
| Direct rank sorts / map entries / owned bytes | 0 / 0 / 0 |
| Ranked rank sorts / map entries / owned bytes | 3,588 / 624 / 18,240 |

## Interpretation Boundary

The direct encoder's zero rank-counter result is a deterministic structural
property on this finite-grid path. The process and phase samples quantify only
how that property appeared on the recorded release binary and host. The total
phase figures are diagnostic accumulators, not a substitute for an end-to-end
profiling methodology. A difference in medians or paired ratios should be read
as a local observation under the stated order, not as a portable ranking of
the two implementations.

The data has no bearing on the separate AN19 runtime-proof obligation,
automatic source target discovery, generic polygon execution, or the
Cardinal--Yuditsky/AN19 asymptotic bounds.

## Threats to Validity

| Threat | Mitigation | Residual risk |
| --- | --- | --- |
| Accidental semantic regression | Every sampled process is a complete direct-versus-ranked differential with exact structural gates. | The census is only 3x3. |
| Startup and cache transients | Three prespecified warm-up processes are executed and excluded. | Later process samples still include normal OS and thermal variation. |
| Host load | Fresh process wall time is retained for all samples; no outlier deletion occurs. | There is no isolation, CPU pinning, or controlled thermal environment. |
| Backend order | Both backends run in every process and the order is recorded. | The fixed ranked-then-direct order can bias timing. |
| Measurement scope | Phase and process time are reported separately with raw observations. | No allocator, energy, cache-counter, or peak-RSS claim is made. |
| Reproducibility | Commit, binary hash, tool versions, environment metadata, protocol, and raw reports are retained. | Another machine can legitimately produce different timings. |

## Reproduction and Extension Rules

Regenerating this report requires a release build, an unchanged script, and a
new result pair. Do not overwrite historical artifacts when changing the
workload, repetitions, warm-ups, machine, compiler, binary, backend order, or
statistic. Instead, store a separately named campaign and compare protocols
before comparing numbers. A future cross-machine study should use multiple
independent hosts, explicit CPU governor/thermal controls where available,
randomized or counterbalanced backend ordering, and a predefined aggregation
plan. Even that would remain empirical evidence rather than a complexity
proof.
