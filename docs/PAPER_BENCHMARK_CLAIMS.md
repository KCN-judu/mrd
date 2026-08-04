# Paper Benchmark Claim Boundary

## Scope

This document governs claims based on the independently named
`paper-scaling` campaign. It supplements the consolidated experimental and
limitation material in [`IMPLEMENTATION.md`](IMPLEMENTATION.md); the former
standalone experiment, sampling, and limitation documents intentionally remain
consolidated rather than being recreated.

The campaign compares four exact paths on paired deterministic finite-grid
instances: `compact-mrd`, `explicit-hopcroft-karp`, `explicit-c0-flow`, and a
bounded `exact-cover-oracle`. CP-SAT remains a correctness Oracle and is not a
direct timing baseline. All values below are local observations attached to the
recorded binary, operating system, compiler, and process protocol.

## Evidence Map

| Paper-facing statement | Evidence artifact | Status |
| --- | --- | --- |
| The compact, explicit Hopcroft--Karp, and explicit C0 paths agree on every successful full-campaign pair. | `results/paper-scaling-full.json`, `paired_validation_errors` | Supported for 1,288 paired instance groups; zero mismatches. |
| The full campaign exercised seven predeclared families, eight target sizes, fresh processes, and counterbalanced order. | `results/paper-scaling-config.json`, raw `execution_order` fields | Supported for all 5,824 planned rows. |
| The compact grid path used direct-grid parity with zero rank sort/map counters. | Raw `structure` fields for `compact-mrd` | Supported on the complete recorded population. |
| Exact-cover rows beyond the limit were retained as unsupported. | Raw `state`, `outcome`, and `message` fields | Supported: 1,302 rows remain unsupported and are excluded from timing fits. |
| Compact/explicit paired process-wall ratios can be described for the measured full pairs. | `results/paper-scaling-full-summary.json` | Supported as local descriptive data with fixed-seed bootstrap intervals. |
| `K` and compressed `M` can be compared without timing a hidden explicit graph in compact samples. | Raw `paired_structural` provenance sidecar and `paper-scaling-k-vs-m.svg` | Supported where the representation exposes both quantities. |
| Empirical exponents can be reported for declared six-level fits. | `results/paper-scaling-full-summary.json`, `paper-scaling-full-tables.tex` | Supported for all seven families and three production paths against target size; structural variables are emitted only when defined. |
| A stable compact/explicit crossover can be reported. | Full paired ratios and fixed crossover rule | A crossover is reported at target 60 only for `representation-crossover`; no universal crossover is established. |

## Supported Wording

The following wording is permitted when it names the population and artifact:

- “On the 1,288 paired instance groups in the complete full campaign, the
  compact, explicit Hopcroft--Karp, and explicit C0 paths had zero optimum
  mismatches.”
- “The compact grid path reported zero direct rank-sort, rank-map-entry, and
  rank-map-byte counters on the measured finite-grid rows.”
- “The raw full campaign retained 4,522 successful, 1,302 unsupported, zero
  timeout, and zero error rows; unsupported Oracle rows were not counted as
  compact wins.”
- “The full protocol reports empirical log--log slopes only over the
  predeclared independent variable and fit range after at least six valid size
  levels; the complete campaign provides six valid target-size levels for each
  production path in every family.”
- “A `K` versus `M` comparison is structural evidence about this
  implementation's explicit and compressed representations.”

## Unsupported Wording

The following claims are prohibited unless new proof or separately documented
evidence closes the stated gap:

- “The benchmark proves the algorithm's asymptotic runtime.”
- “AN19 runtime verified,” “almost-linear implementation,” or any statement
  that treats the P9.6a proof debt as closed.
- “The compact solver has a universal speedup,” “beats all `n^1.5`
  implementations,” or any hardware-independent ranking.
- “The full campaign proves a stable or hardware-independent crossover”; only
  the declared `representation-crossover` target-60 observation is reported.
- “CP-SAT is slower than the compact solver” without a separately committed
  same-population, same-timeout, startup-inclusive protocol.
- Causal attribution to one backend where process startup, cache state,
  allocator behavior, or coupled geometry phases remain uncontrolled.
- Counting an unsupported Oracle row, an error, or a timeout as a compact win.

## Reporting Rules

Every empirical table or paragraph must state the timed quantity, independent
variable, population, and censoring rule. Timeout rows are censored
observations, not timeout-duration measurements in an uncensored regression.
Fits use one median per size level, OLS plus Theil--Sen sensitivity, and a
fixed-seed 10,000-resample confidence interval. A confidence interval whose
upper endpoint is below 1.5 may support only the narrow statement “an empirical
exponent below 1.5 over this declared measured range”; it is never an
asymptotic proof.

The intended paper narrative is therefore ordered as follows: establish exact
agreement on the paired population, show representation counts independently
of timing, report end-to-end paired wall times with uncertainty, then state
the measured-range fit and its limitations. This order avoids presenting a
small local timing effect as a theorem.
