# P18 Canonical Sharing

## Status and scope

P18 is complete for its predeclared finite ownership campaign. The phase
isolates the deep canonical-component copy at the in-process Scope A benchmark
boundary, preserves an executable clone reference, and evaluates an ordinary
Rust borrowing path. It is an implementation and measurement-harness result;
it is not a new production algorithm, an RSS experiment, or an asymptotic
claim.

The production solve functions already accept an immutable `&GridComponent`.
Consequently, the removed copy is a benchmark-boundary ownership operation,
not a claim that ordinary callers previously paid a hidden clone. P18 does not
implement prepared-context reuse. There is no executable
`shared-prepared-context` backend identity; such reuse would require a separate
experiment with its own lifetime and reset protocol.

## Provenance

| Item | Accepted value |
| --- | --- |
| Source commit | `ccd121d12cce0290dccd24447e0722230b31f74c` |
| Analysis and validation commit | `8e0b67d` |
| Release binary SHA-256 | `2f1fe9eb884ac53399e9e65fc05a6751488fa372313a9d11942af38db8672ff8` |
| P18 comparison-config SHA-256 | `7da028ed4680720410464fc85a5a6c2c6614a79e89a3187d02abf87074588dda` |
| Host | Apple M4, 10 logical CPUs, macOS 26.5 arm64 |
| Compiler | `rustc 1.89.0 (29483883e 2025-08-04)` |
| Power state | AC; turbo/power mode unknown |
| Provenance | clean Git tree, monotonic `std::time::Instant` |

The compressed raw and derived artifacts, their hashes, sizes, and restore
commands are recorded in
[`results/p18-canonical-sharing-archive-manifest.json`](../../results/p18-canonical-sharing-archive-manifest.json).
The raw wrapper is the numerical source of truth; this report is an
interpretation of that record.

## Research question

P17 exposed setup and representation costs in Scope A. P18 asks whether the
deep copy of canonical geometry can be removed without sharing mutable solver
state, and whether the resulting cost moves to a separately measurable phase.
The paired design compares the same canonical instance, seed, algorithm,
target, iteration identity, and output certificate under two ownership paths.

## Ownership audit and design

The pre-refactor ownership boundary was classified into five layers:

1. **Canonical input.** `GridComponent` owns the immutable cell buffer. The
   clone reference copies the `GridComponent.cells` `Vec<Cell>` payload. Its
   structural payload is proportional to `N`, the foreground-cell count.
2. **Prepared geometry.** Occupancy, boundary loops, chords, endpoint indexes,
   reflex groups, and structural metadata are immutable after preparation. P18
   constructs these independently in both paths so prepared-context reuse is
   not conflated with clone removal.
3. **Solver workspace.** Selection buffers and algorithm-local graph/flow
   state are mutable and remain independent for compact MRD, explicit
   Hopcroft--Karp, and explicit C0 flow. A workspace is prepared per solve and
   deterministically released/reset at the ownership boundary.
4. **Result and witness.** Rectangles, certificates, and validation outputs are
   owned by each solve and checked independently.
5. **Temporary validation state.** Differential and mutation checks use local
   scratch state and cannot mutate canonical input or prepared geometry.

`clone-canonical-reference` retains the deep copy as an internal executable
reference. `borrowed-canonical` borrows the immutable canonical component and
records any shallow borrow/share and release work separately. The design uses
ordinary lifetimes only: no `Arc`, interior mutability, unsafe aliasing,
compatibility re-exports, or runtime backend registry was introduced. The
clone payload is not replaced by a mislabeled zero: the borrowed path records
zero only for the absent deep copy and records its remaining borrow, release,
and workspace phases independently.

## Protocol and acceptance gates

The campaign contains six deterministic families:
`comb-staircase`, `representation-crossover`, `dense-conflict`,
`random-connected`, `sparse-conflict`, and `supported-holes`. Target levels are
16, 32, 64, 128, 256, 512, 1024, 2048, 4096, and 8192. Three algorithms are
timed in counterbalanced order: `compact-mrd`, `explicit-hopcroft-karp`, and
`explicit-c0-flow`. Scope A starts from canonical input; Scope B starts after
common preprocessing and excludes Scope A ownership workspace preparation.

The protocol uses fixed seeds, five-to-ten warmups, adaptive measured
repetitions with a maximum of 31, a five-second preflight stop threshold, a
minimum of six complete levels for fits, and 10,000 bootstrap resamples with
seed `604019`. Checkpoints are external to the repository. The clean-evidence
gate requires source, binary, configuration, CPU/power, and Git provenance;
exact planned/terminal census; duplicate-free retries and sample identities;
matching cross-backend sample identities and algorithm order for every shared
iteration prefix; and zero canonical, structural, objective, and witness
mismatches. Different adaptive repetition counts are retained rather than
padded or discarded.

## Census and correctness

| Backend | Planned | Terminal | Complete | Stopped | Measured rows | Correctness records | Retries |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `clone-canonical-reference` | 60 | 60 | 57 | 3 | 9,906 | 174 | 0 |
| `borrowed-canonical` | 60 | 60 | 57 | 3 | 9,922 | 174 | 0 |

The three stopped points are `dense-conflict` at targets 2048, 4096, and
8192. Target 2048 exceeded the declared 5,000,000,000 ns preflight limit;
4096 and 8192 are propagated stopped states. They contribute no timing median,
speedup, or fit. There are 60 paired terminal points, zero canonical-instance
identity mismatches, zero structural mismatches, zero objective mismatches,
zero witness mismatches, zero sample-identity/order mismatches, and three
reported adaptive-count differences.

## Scope A results

Speedup is reference time divided by borrowed time; values below one favor the
clone reference. The table reports the compact-MRD representative, with the
predeclared family-level bootstrap interval.

| Family | Scope A speedup | 95% CI | Material improvement | Clone bytes (reference -> borrowed) | Workspace preparation (ns) | Largest phase before -> after |
| --- | ---: | --- | --- | ---: | ---: | --- |
| comb-staircase | 0.8394 | [0.8266, 0.8677] | No | 4,520 -> 0 | 41 -> 42 | completion -> completion |
| dense-conflict | 0.9878 | [0.9424, 1.0239] | No | 278,672 -> 0 | 84 -> 83 | completion -> completion |
| random-connected | 0.9135 | [0.8537, 0.9549] | No | 6,144 -> 0 | 42 -> 42 | completion -> completion |
| representation-crossover | 0.8625 | [0.8266, 0.9273] | No | 345,016 -> 0 | 83 -> 83 | completion -> completion |
| sparse-conflict | 0.9042 | [0.8710, 1.0504] | No | 49,232 -> 0 | 41.5 -> 42 | geometry -> geometry |
| supported-holes | 1.0834 | [1.0596, 1.1095] | Yes | 80,432 -> 0 | 42 -> 42 | geometry -> geometry |

The primary `representation-crossover` result is therefore negative: the
borrowed path is slower, and its confidence interval is wholly below one.
The same direction is present for compact MRD, explicit Hopcroft--Karp, and
explicit C0 flow at that family. `supported-holes` is the only family with a
consistent compact Scope A interval wholly above one. This is finite,
host-specific evidence, not a universal ownership policy.

For the primary compact-MRD pair, the deep clone phase changes from a median
5,104 ns to zero. Borrow/share changes from zero to 20.5 ns, release changes
from 62.5 ns to 20.5 ns, and workspace preparation remains 83 ns. The
capacity-based solver-workspace estimate remains 78 bytes on both paths; the
ownership-layer `Vec` estimate changes from 3 to 2. The retained representation
estimate remains 5,288 bytes. The Amdahl ownership fraction removed is
0.002104, giving an idealized upper bound of only 1.0021x before any replacement
costs. This explains why eliminating the deep copy does not imply a material
Scope A improvement.

Scope B confirms that the clone experiment did not alter the solver kernel.
For the primary compact pair, Scope B speedup is 0.8387 with 95% CI
[0.8246, 0.8947]; the representation phase ratio is 1.193 (borrowed/reference)
and representation construction remains the largest Scope B phase, accounting
for 71.3% before and 70.1% after at the largest complete level. The compact
Scope A empirical slope changes from 0.9046 to 0.8725; Scope B changes from
0.4594 to 0.4285. These are descriptive fits over ten valid levels and do not
establish a complexity class.

## Interpretation of the bottleneck

At the largest complete `representation-crossover` Scope A level,
`rectangle_completion_recovery_ns` remains dominant (50.9% before and 52.1%
after). For `comb-staircase`, `dense-conflict`, and `random-connected`, the
same completion parent remains dominant. Geometry preprocessing remains
dominant for `sparse-conflict` and `supported-holes`. Representation
construction is dominant in the Scope B kernel for conflict-heavy compact
cases, but it is not a universal Scope A bottleneck. The P17 phase-dominance
erratum is preserved: the earlier raw P17 artifacts are not rewritten, and
their corrected parent-phase interpretation is the basis for this statement.

The clone removal changes no algorithmic dependence on `B`, `U`, `q`, `K`, or
`M`; the removed payload is a single `N`-proportional canonical cell buffer.
The measured slopes and structural-byte estimates are therefore implementation
diagnostics, not evidence for a new asymptotic law.

## Rejected exploratory run

The pre-acceptance exploratory run is retained outside the repository at
`/tmp/mrd-p18-rejected-4ae1719` with its `REJECTED.md` and hashes. It is not
part of the accepted evidence. Its source commit was
`4ae1719a31833758effd73e6e0df895ab4f97e3e`; it recorded 60 terminal points per
backend, 57 complete and 3 stopped, 9,954 measured rows, 114 clone retries and
43 borrowed retries. All 157 retries were timing-accounting mismatches. The
run also had dirty Git provenance, no valid top-level configuration hash,
non-standard `NaN` summary values, and stale allocation-field names. It is
disclosed to prevent silent replacement by the clean run and does not affect
the accepted census.

## Post-clone representation audit

After the clone experiment, the remaining representation phase was audited by
algorithm and family. The audit identifies repeated allocation and conversion
within compact representation construction, but the corrected P17 dominance
table does not justify a universal bottleneck claim. Exactly one follow-up
experiment is recommended:

> Give each algorithm and campaign lane an exclusive compact-representation
> workspace whose graph, partition, and network buffers are preallocated from
> observed capacities and deterministically reset before every solve.

The workspace must not be shared across algorithms or iterations without a
reset proof. No selector, hybrid policy, zero-conflict shortcut, or direct
representation rewrite is part of P18. That experiment must use a new source
commit, new config, paired ownership/representation protocol, and its own
acceptance report.

## Claim boundaries

- Deep canonical cloning was eliminated on the borrowed benchmark path; this
  is an implementation optimization, not a new algorithm.
- Capacity-based structural bytes are payload estimates. Allocator metadata,
  process RSS, cache effects, energy, and cross-host timing were not measured.
- The accepted intervals apply only to the recorded Apple M4 host, binary,
  families, seeds, target range, and protocol. A negative result is valid
  evidence and was not hidden by weakening the gate.
- P18 does not prove an asymptotic improvement, AN19 runtime, automatic target
  decision, or universal backend crossover.
- P9.5e.3g.3 remains the hard automatic-target blocker. P9.6a remains a
  low-priority proof debt. P9.3.2d implementation is complete and nonblocking,
  while its reduced-event ordering and runtime proof remain deferred.

## Reproduction

From a clean checkout of the source commit, build the release binary and run
the two ownership backends with checkpoints outside the repository:

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

The accepted large files are stored as Zstandard archives. Restore them with
the commands in the archive manifest before invoking the analyzer. The
analyzer and runner self-tests, the 14 protocol tests, and the full Rust audit
are required before a future campaign is accepted.

## Claim-evidence map

| Claim | Evidence | Status |
| --- | --- | --- |
| The borrowed path removes the deep canonical copy. | Zero borrowed clone payload and zero clone-phase time, with nonzero clone reference values and allocation semantics checked independently. | Supported for the measured Scope A path. |
| The refactor preserves solver semantics. | 174 correctness records per backend and zero canonical, structural, objective, witness, sample-identity, or sample-order mismatches. | Supported for the accepted finite campaign. |
| Clone removal materially improves the primary family. | `representation-crossover` compact Scope A CI `[0.8266, 0.9273]`. | Rejected; the measured result is negative. |
| Representation construction is the universal next bottleneck. | Family- and scope-dependent phase tables. | Rejected; one representation-workspace experiment is recommended, not implemented. |
| A new asymptotic or AN19 runtime result follows. | No source-proof or automatic-target evidence is included in P18. | Out of scope and unclaimed. |
