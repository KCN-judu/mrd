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

The campaign was generated from clean source state
`6608cacdf733561224de9328474401febb7719dd`. The release binary was
`target/release/mrd`, SHA-256
`c64868de59b74cbdded38c8da634434e2f56c7bdc6ed49a0caf634c97b88c0c9`, built
with `rustc 1.89.0 (29483883e 2025-08-04)`. The host reports macOS 26.5 on
arm64, an Apple M4, and 10 logical CPUs. Three warm-up processes were excluded
before 31 measured processes. The result fields below are copied from
[`results/benchmark-sampling.json`](../results/benchmark-sampling.json);
the flat raw samples are in
[`results/benchmark-sampling-runs.csv`](../results/benchmark-sampling-runs.csv).
Their SHA-256 values are respectively
`56e140dce41773e04a890556c6406acdf6e7f4ab9f1fa85189dadea5825d4b08` and
`3488bbe8848aa2b639951a6467072d91465f08f6be237e291cd4d56b08b60962`.
Values are microseconds except ratios.

| Metric | n | Min | Q1 | Median | Q3 | Max |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Process wall time | 31 | 64,111 | 65,546.5 | 66,311 | 67,264 | 71,209 |
| Direct embedding time | 31 | 0 | 1 | 1 | 1 | 11 |
| Ranked embedding time | 31 | 9 | 9 | 10 | 11 | 53 |
| Direct all-phase time | 31 | 5,563 | 5,815 | 6,033 | 6,185 | 6,604 |
| Ranked all-phase time | 31 | 6,432 | 6,654.5 | 6,917 | 7,038 | 7,601 |
| Direct/ranked embedding ratio | 31 | 0 | 0.0801 | 0.1000 | 0.1111 | 0.8462 |
| Direct/ranked all-phase ratio | 31 | 0.8511 | 0.8678 | 0.8713 | 0.8816 | 0.8927 |

### Correctness Gate

All 31 measured process reports and all three predeclared warm-ups satisfied
the following gate. No run had a mismatch or solver error; both verification
modes contributed exactly 897 comparisons in every measured process.

| Check | Required value |
| --- | ---: |
| Nonzero 3x3 masks | 511 |
| Foreground components | 897 |
| Paired pipeline comparisons | 1,794 |
| Mismatches | 0 |
| Solver errors | 0 |
| Direct rank sorts / map entries / owned bytes | 0 / 0 / 0 |
| Ranked rank sorts / map entries / owned bytes | 3,588 / 624 / 18,240 |

### Descriptive Reading

The process-wall-time median is 66.311 ms (IQR 65.547--67.264 ms) for the
entire finite differential workload, including process startup. The diagnostic
all-phase accumulator has a median direct/ranked ratio of 0.8713 (IQR
0.8678--0.8816). It is an internally paired local observation: direct-grid
parity was executed second in the fixed implementation order, so this result
does not identify how much, if any, observed difference is caused solely by
the coordinate encoder.

The direct embedding accumulator has a median of 1 microsecond versus 10
microseconds for ranked embedding, but seven direct observations quantized to
0 microseconds. It is therefore too close to the diagnostic clock resolution
to support a precise embedding-only comparison. The raw values are preserved
for transparency; the stable structural result is instead that all 31 runs
reported zero direct rank sorts, rank-map entries, and rank-map owned bytes.

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
