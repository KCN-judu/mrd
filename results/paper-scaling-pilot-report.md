# Paper Scaling Benchmark Report

This report is generated from `results/paper-scaling-pilot.json` by the committed analyzer. It reports finite local measurements, not an asymptotic runtime theorem.

## Protocol

- Fit time variable: `process_wall_time_ns`.
- Predeclared fit exclusion: `target_size < fit.minimum_target_size; missing or invalid values are excluded`.
- Timeout policy: censored and retained; excluded from exact-time fits.
- Bootstrap: seed `20260804`, `10,000` resamples.
- A slope is emitted only after `6` valid size levels satisfy the predeclared rule.
- `M` is the compressed network node count plus compressed network arc count; `K` is the explicit conflict-edge count.

## Coverage

| Family | Size range | Instances | Planned | Observed | Success | Paired | Mismatches | Timeouts | Unsupported | Errors |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| random-connected | [8, 60] | 30 | 120 | 120 | 115 | 30 | 0 | 0 | 5 | 0 |
| dense-conflict | [8, 60] | 30 | 120 | 120 | 90 | 30 | 0 | 0 | 30 | 0 |
| sparse-conflict | [8, 60] | 30 | 120 | 120 | 90 | 30 | 0 | 0 | 30 | 0 |
| comb-staircase | [8, 60] | 30 | 120 | 120 | 90 | 30 | 0 | 0 | 30 | 0 |
| supported-holes | [8, 60] | 30 | 120 | 120 | 90 | 30 | 0 | 0 | 30 | 0 |
| polyomino | [8, 60] | 30 | 120 | 120 | 90 | 30 | 0 | 0 | 30 | 0 |
| representation-crossover | [8, 60] | 30 | 120 | 120 | 90 | 30 | 0 | 0 | 30 | 0 |

## Paired timing ratios

| Family | Paired | Median compact/explicit | Bootstrap 95% CI | Stable crossover target |
| --- | --- | --- | --- | --- |
| random-connected | 30 | 0.998 | [0.9920813518539668, 1.006377995833803] | none |
| dense-conflict | 30 | 1 | [0.9987404199119323, 1.0108174104415153] | none |
| sparse-conflict | 30 | 1.01 | [0.9963372026147423, 1.0133516460995207] | none |
| comb-staircase | 30 | 1.01 | [0.9949839172778202, 1.0078600747309754] | 40 |
| supported-holes | 30 | 1 | [0.9962765511690923, 1.0108950731902144] | none |
| polyomino | 30 | 0.999 | [0.9938871471427999, 1.0064647327209073] | none |
| representation-crossover | 30 | 1.01 | [0.9932764371773841, 1.0122006119049916] | none |

## Empirical scaling fits

No empirical exponent is reported: this run does not meet the predeclared six-size-level minimum.

## Phase decomposition

The phase rows expose geometry, representation, flow/matching, recovery, and verification costs. Missing phases are not zero-cost claims; they are not applicable to that solver path.

## Interpretation boundary

A fitted slope is an empirical exponent over the declared fit interval and independent variable. It is not the exponent of the algorithm and cannot establish the unproved AN19/source-flow runtime claim. Exact-cover rows are a separate correctness Oracle category.
