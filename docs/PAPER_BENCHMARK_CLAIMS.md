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
| The compact, explicit Hopcroft--Karp, and explicit C0 paths agree on every successful smoke pair. | `results/paper-scaling.json`, `paired_validation_errors` | Supported for the smoke population. |
| The smoke campaign exercised three predeclared families, four target sizes, fresh processes, and counterbalanced order. | `results/paper-scaling-smoke-config.json`, raw `execution_order` fields | Supported. |
| The compact grid path used direct-grid parity with zero rank sort/map counters. | Raw `structure` fields for `compact-mrd` | Supported on recorded grid rows. |
| Exact-cover rows beyond the limit were retained as unsupported. | Raw `state`, `outcome`, and `message` fields | Supported. |
| Compact/explicit paired process-wall ratios can be described for the measured smoke pairs. | `results/paper-scaling-summary.json` | Supported as local descriptive data. |
| `K` and compressed `M` can be compared without timing a hidden explicit graph in compact samples. | Raw `paired_structural` provenance sidecar | Supported by the protocol design. |
| An empirical exponent can be reported. | No eligible six-size-level smoke fit | Not yet supported. |
| A stable compact/explicit crossover can be reported. | Smoke has fewer than six sizes; analyzer reports none | Not yet supported. |

## Supported Wording

The following wording is permitted when it names the population and artifact:

- “On the recorded paired smoke population, the compact, explicit
  Hopcroft--Karp, and explicit C0 paths had zero optimum mismatches.”
- “The compact grid path reported zero direct rank-sort, rank-map-entry, and
  rank-map-byte counters on the measured finite-grid rows.”
- “The raw campaign retained successful, unsupported, error, and timeout
  states; the current smoke run recorded 160 successful and 32 unsupported
  rows with no timeout or paired mismatch.”
- “The full protocol will estimate empirical log--log slopes only over the
  predeclared independent variable and fit range after at least six valid size
  levels.”
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
- “The smoke crossover is stable” or any exponent claim: the smoke does not
  meet its own six-size-level fit condition.
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
