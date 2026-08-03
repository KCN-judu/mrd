# Final Performance Report

Evidence baseline: `0752fce60d5a801173e963b4a1fb55c8a331949e`.

## Observations

| Evidence | Observation | Interpretation |
| --- | --- | --- |
| Direct-grid parity | 0 direct rank sorts, 0 rank-map entries, 0 rank-map bytes; ranked Oracle totals 3,588 / 624 / 18,240 | Structural finite-grid allocation distinction; timings remain local. |
| Direct-grid benchmark | 1,794 exact comparisons with zero failures | Correctness population, not a speed claim. |
| Flow construction | 7 dense sizes, both flow backends agree | Finite backend observations; no automatic crossover policy. |
| P13 optimization ledger | retained storage/layout/execution changes with exact differentials | Constant-factor changes are isolated and do not change asymptotic claims. |
| Resource sample | 23,674,880-byte maximum RSS for both sampled local processes | Local macOS observation only; no portable memory bound. |

The external CP-SAT run took 44.14 seconds for its bounded population. This is
an experiment duration, not a solver complexity result. No claim is made about
hardware-independent speed, throughput, scaling, or peak allocation.

## Exclusions

`valgrind`, `heaptrack`, and `hyperfine` were unavailable. The report therefore
contains no profiler-derived allocation count, cross-machine benchmark, or
portable performance ranking. All phase timings are retained in their source
reports and machine-readable evidence.
