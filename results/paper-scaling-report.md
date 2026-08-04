# Paper Scaling Benchmark Report

This report is generated from `results/paper-scaling.json` by the committed analyzer. It reports finite local measurements, not an asymptotic runtime theorem.

## Protocol

- Fit time variable: `process_wall_time_ns`.
- Predeclared fit exclusion: `target_size < fit.minimum_target_size; missing or invalid values are excluded`.
- Timeout policy: censored and retained; excluded from exact-time fits.
- Bootstrap: seed `20260804`, `10,000` resamples.
- A slope is emitted only after `6` valid size levels satisfy the predeclared rule.
- `M` is the compressed network node count plus compressed network arc count; `K` is the explicit conflict-edge count.

## Coverage

| Family | Size range | Instances | Paired | Mismatches | Timeouts | Unsupported |
| --- | --- | --- | --- | --- | --- | --- |
| random-connected | [4, 10] | 12 | 12 | 0 | 0 | 0 |
| dense-conflict | [4, 10] | 12 | 12 | 0 | 0 | 12 |
| representation-crossover | [4, 10] | 12 | 12 | 0 | 0 | 12 |

## Paired timing ratios

| Family | Paired | Median compact/explicit | Bootstrap 95% CI | Stable crossover target |
| --- | --- | --- | --- | --- |
| random-connected | 12 | 0.996 | [0.98361024246111, 1.0189211441239592] | none |
| dense-conflict | 12 | 0.996 | [0.9912696945550197, 1.0201441112719194] | none |
| representation-crossover | 12 | 1.01 | [0.990075555737219, 1.0166704766345571] | none |

## Empirical scaling fits

No empirical exponent is reported: this run does not meet the predeclared six-size-level minimum.

## Phase decomposition

The phase rows expose geometry, representation, flow/matching, recovery, and verification costs. Missing phases are not zero-cost claims; they are not applicable to that solver path.

## Interpretation boundary

A fitted slope is an empirical exponent over the declared fit interval and independent variable. It is not the exponent of the algorithm and cannot establish the unproved AN19/source-flow runtime claim. Exact-cover rows are a separate correctness Oracle category.
