# Final Performance Report

Evidence baseline: `0752fce60d5a801173e963b4a1fb55c8a331949e`.

Paper-scaling full evidence: source commit
`252d01f08c6ba64b17b8fe22ce7317d7c2d58c76`, binary SHA-256
`e58e1b898a97dfba9334439b9a8b5b86aa10a7b554e312fdfe2ace98929c9129`,
and configuration SHA-256
`6245b382bccc7cfddb32806ad5dff20a1d4019a6991fdfde57254226db632fa3`.

## Observations

| Evidence | Observation | Interpretation |
| --- | --- | --- |
| Direct-grid parity | 0 direct rank sorts, 0 rank-map entries, 0 rank-map bytes; ranked Oracle totals 3,588 / 624 / 18,240 | Structural finite-grid allocation distinction; timings remain local. |
| Direct-grid benchmark | 1,794 exact comparisons with zero failures | Correctness population, not a speed claim. |
| Flow construction | 7 dense sizes, both flow backends agree | Finite backend observations; no automatic crossover policy. |
| P13 optimization ledger | retained storage/layout/execution changes with exact differentials | Constant-factor changes are isolated and do not change asymptotic claims. |
| Resource sample | 23,674,880-byte maximum RSS for both sampled local processes | Local macOS observation only; no portable memory bound. |
| Full paper scaling | 5,824 planned and observed fresh-process rows; 4,522 success, 1,302 bounded-Oracle unsupported, 0 timeout/error/paired mismatch | Complete finite local comparison, not a complexity proof. |

## Full Paper-Scaling Campaign

The pilot's 1,008 child-process walls sum to 45.967 seconds, yielding a linear
full-plan child-wall projection of 265.587 seconds. The predeclared
seven-family, eight-size campaign then completed with 312.813 seconds of
summed child-process wall time and 1,255.110 runner-wall seconds on the
recorded Apple M4 host. The 942.297-second residual belongs to the runner
protocol, not to a solver-specific timing: it includes launch, validation, and
atomic whole-checkpoint persistence after each terminal record. Its internal
allocation was not measured, so no fraction is attributed to any one activity.
Fits and paired ratios use the child process-wall observations only.

Every one of the 1,288 measured production-path instance groups agreed
exactly. The bounded exact-cover Oracle is intentionally unsupported over most
of this population; those 1,302 rows are retained, but are neither timing
observations nor compact wins.

The full paired compact/Hopcroft--Karp process-wall medians are near one:
random-connected 1.003, dense-conflict 1.008, sparse-conflict 1.004,
comb-staircase 0.993, supported-holes 1.000, polyomino 1.007, and
representation-crossover 0.996. Their fixed-seed bootstrap intervals are in
`results/paper-scaling-full-summary.json`. The crossover rule emits target 60
only for `representation-crossover`; it is not a general backend-selection
policy.

The representation-crossover family demonstrates the intended structural
separation without a universal wall-time victory: at target 135 its recorded
explicit conflict count is 72,900 and compressed representation size is 1,620,
while median end-to-end process walls remain approximately 0.50 seconds for
both paths. Its phase decomposition assigns most in-process time to geometry
preprocessing and geometric completion, whereas explicit graph construction is
only one explicit-path phase. The raw phase medians, tables, and SVGs retain
the complete breakdown.

All production paths satisfy the predeclared six-level target-size fit rule.
The resulting slopes are empirical descriptions of this fresh-process,
startup-inclusive measured interval. Exact-cover has no six-level fit because
of its declared support boundary; `K`-based fits are omitted where the family
does not define an explicit conflict count. No measured slope, ratio, or
crossover proves an asymptotic running time, the unresolved AN19 runtime, or
an automatic source-flow solver contract.

The external CP-SAT run took 44.14 seconds for its bounded population. This is
an experiment duration, not a solver complexity result. No claim is made about
hardware-independent speed, throughput, scaling, or peak allocation.

## Exclusions

`valgrind`, `heaptrack`, and `hyperfine` were unavailable. The report therefore
contains no profiler-derived allocation count, cross-machine benchmark, or
portable performance ranking. All phase timings are retained in their source
reports and machine-readable evidence.
