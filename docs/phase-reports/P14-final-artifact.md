# P14 Final Verification and Artifact

## P14.1 Status

**State: complete. Baseline: `37fa5069e0eea8552d99843b6044f466588d2107`.**
P14.1 inventories the campaigns before P14 creates final result artifacts. It
does not reinterpret historical evidence as a final P14 execution, and it does
not manufacture a passing record for a campaign that has not run.

The final artifact must be `results/final-manifest.json` and validate against
`docs/final-manifest.schema.json`. The schema permits exactly three campaign
dispositions:

- `executed` requires commands with exit statuses and durations, content-hashed
  artifacts, the observed population, and a zero-disagreement/zero-solver-error
  outcome.
- `unavailable` requires a failed availability probe, a concrete reason, the
  claims omitted because of it, and a next action. It is not success.
- `deferred` requires a named debt, a reason, omitted claims, and a next action.
  It is not success and is not a failed execution.

There is deliberately no `pending` disposition in the final manifest. Pending
work belongs in this P14.1 inventory, not in a supposedly complete artifact.
All artifact paths must be repository-relative and each executed artifact must
have a SHA-256 digest. The manifest also records toolchain/environment data,
so a finite experiment remains tied to its actual execution context.

## Campaign Inventory

| ID | P14 action | Reproduction command | Expected final artifact | Claim boundary |
| --- | --- | --- | --- | --- |
| `grid-exhaustive-4x4` | P14.2 | `cargo run --release -p mrd -- exhaustive --width 4 --height 4 --output results/final-campaigns/grid-exhaustive-4x4.json` | exact grid report | Exhaustive finite `4x4` colored-grid population only; no asymptotic claim. |
| `grid-random-8x8-seed42` | P14.2 | `cargo run --release -p mrd -- random --width 8 --height 8 --cases 10000 --seed 42 --output results/final-campaigns/random-8x8-seed42.json` | seeded differential report | Deterministic finite sample only. |
| `polyomino-through-12` | P14.2 | `cargo run --release -p mrd -- polyomino --max-cells 12 --all-solvers --output results/final-campaigns/polyomino-max12.json` | compact all-solver summary | Canonical free polyominoes through 12 cells only. The transient per-instance output is reduced to a committed count/status summary, following the P1 baseline convention. |
| `grid-polygon-differential` | P14.2 | `cargo run --release -p mrd -- benchmark --suite polygon-differential --sizes 3,4 --output results/final-campaigns/polygon-differential.json` | report plus minimized-counterexample file | Ordinary grid-derived polygon parity at the recorded finite sizes only. |
| `formal-fixtures` | P14.2 | `cargo run --release -p mrd -- benchmark --suite formal-fixtures --output results/final-campaigns/formal-fixtures.json` | formal fixture report | Eight named formal-boundary fixtures only; not arbitrary formal inputs. |
| `direct-grid-parity` | P14.2 | `cargo run --release -p mrd -- benchmark --suite direct-grid-parity --output results/final-campaigns/direct-grid-parity.json` | 3x3 direct-versus-ranked report | Exact finite-grid equality and zero direct rank counters; timing stays local. |
| `direct-grid-metamorphic` | P14.2 | `cargo test -p dominance direct_grid_embedding_matches` | test transcript recorded in final correctness report | The tested translated/isometric examples only. |
| `generic-and-compressed-flow` | P14.2 | `cargo test -p graph && cargo test -p dominance && cargo run --release -p mrd -- benchmark --suite construction --sizes 4,8,16,32,64,128,256 --output results/final-campaigns/flow-backends.csv` | test transcript, CSV, adjacent JSON, and local manifest | Exact finite flow/cut/cover agreement and structural counters; benchmark times do not establish a crossover policy or complexity theorem. |
| `workspace-quality` | P14.2 and P14.5 | Mandatory workspace audit from the master plan | command ledger in final manifest and reports | Build, lint, test, and documentation health, not exhaustive semantic verification. |
| `external-cp-sat` | P14.3 | The isolated-venv commands in `tools/external-oracle/README.md`, followed by `tools/external-oracle/verify_suite.py` | CP-SAT/Rust comparison JSON or an unavailable record | Bounded grid components only; CP-SAT `optimal` is required. No general polygon or formal-boundary claim. |
| `resource-measurement` | P14.3 | `/usr/bin/time -l` around representative final commands, with in-process `MemoryEstimate` fields retained where emitted | resource command ledger and report | Local peak resident-memory observations and retained-memory estimates only, never a portable memory bound. |
| `fuzz-engine` | P14.2 availability decision | Registered fuzz target and runner, if one exists | unavailable record unless an actual fuzzer is run | No fuzz target is present and `cargo-fuzz` is unavailable in the P14.1 environment; random and metamorphic campaigns are not labelled fuzzing. |
| `performance-and-complexity` | P14.4 | Reuse executed P14.2/P14.3 records, P13's direct-grid ledger, and static checkers | final performance and complexity reports | Local timings and finite structural counters only. AN19 runtime evidence is excluded. |
| `an19-runtime-proof` | P14.4/P14.5 | No executable substitute | deferred record | P9.6a remains proof debt: DOI `10.1137/17M1115575` does not establish the reduced-event ordering/counting conversion. No `AlmostLinear` or AN19 runtime claim is permitted. |
| `final-release-audit` | P14.5 | Mandatory audit protocol plus schema/content/hash inspection | final reports, benchmark files, and final manifest | Artifact integrity and reproducibility at the recorded commit only. |

P14.2 inspection verified that several benchmark commands write a sibling
`manifest.json`. Their outputs are therefore directed to
`results/final-campaigns/`, whose sibling manifest is isolated from the
historical `results/manifest.json`. The P14.5 final manifest remains the
explicit `results/final-manifest.json` aggregation over those outputs, rather
than an implicit interpretation of a legacy manifest.

## P14.2 Exact Correctness Campaigns

**State: complete. Baseline: `2351cc6bf0791bc3a79bb4a4a82759d016f76d9b`.**
Every runnable campaign below exited zero. Generated data is under
`results/final-campaigns/`; the directory's local `manifest.json` contains the
three benchmark runs that use the CLI manifest writer.

| Campaign | Observed population and result | Duration |
| --- | --- | ---: |
| 4x4 exhaustive grid | 65,536 grids and 337,058 components; exact-cover, SG, C0, and compressed comparisons each total 337,058; zero counterexamples | 10.84 s |
| 8x8 seeded random | 10,000 inputs and 162,162 components at seed 42; zero mismatches and solver errors | 11.70 s |
| Free polyominoes through 12 cells | 87,148 inputs/components; all records `verified` | 9.34 s |
| Ordinary grid-derived polygon differential | 66,046 inputs and 169,426 components; 167,082 supported components all verified, zero disagreements, solver errors, and timeouts; 2,344 model rejections are recorded explicitly | 9.50 s |
| Formal fixtures | 8 fixture/parity records; zero disagreements and solver errors | 0.67 s |
| Direct-grid parity | 511 nonzero 3x3 masks, 897 components, and 1,794 exact pipeline comparisons; zero mismatches/errors; direct rank sort/map-entry/map-byte counters are all zero | 0.34 s |
| Direct-grid metamorphic test | `cargo test -p dominance direct_grid_embedding_matches`: 2 passed, 62 filtered | 0.40 s |
| Generic exact flow | `cargo test -p graph`: 225 passed across 2 suites | 7.15 s |
| Compressed pipeline | `cargo test -p dominance`: 62 passed, 2 existing ignored across 2 suites | 148.14 s |
| Flow construction differential | 7 dense compressed networks with zero counterexamples and solver errors; both flow backends agree on recorded values | 0.35 s |

The direct-grid report also records ranked Oracle totals of 3,588 rank sorts,
624 map entries, and 18,240 estimated map-owned bytes. That structural
comparison is deterministic evidence for the finite direct-grid path; neither
it nor the recorded timings is a portable speedup or complexity claim.

No registered fuzz target exists in the repository and `cargo-fuzz` is absent.
P14.2 therefore records fuzzing as unavailable evidence, not as a successful
random or metamorphic campaign. P14.5 must emit the corresponding
`unavailable` manifest entry with the P14.1 probes, omitted fuzzing claims, and
the next action.

## P14.3 External and Resource Evidence

**State: complete. Baseline: `69cf77d0c537b665d5c04ae666767fd58bd3cf83`.**
The documented isolated environment installed OR-Tools `9.15.6755`. After the
focused `--mrd` runner repair described below, the bounded CP-SAT suite ran in
44.14 seconds with all 6,998 inputs and all 27,228 components `verified`:

- 512 exhaustive 3x3 inputs and 1,794 components;
- 6,473 free polyominoes through ten cells and 25,390 components; and
- 13 admitted adversarial grids and 44 components.

Every component had a CP-SAT optimal comparison, an exact-cover comparison,
and a Rust comparison; there were zero disagreement components, timeouts,
unsupported cases, or solver errors. The executable report is
`results/final-campaigns/external-oracle.json`.

The local resource tool available on this host is `/usr/bin/time -l`. Its
observations are intentionally not committed as a cross-machine benchmark:

| Command | Wall time | Maximum resident set size | Boundary |
| --- | ---: | ---: | --- |
| `target/release/mrd exhaustive --width 4 --height 4` | 10.48 s | 23,674,880 bytes | One local process observation over the finite 4x4 campaign. |
| `target/release/mrd benchmark --suite direct-grid-parity` | 0.07 s | 23,674,880 bytes | One local process observation over the finite direct-grid campaign. |

`valgrind`, `heaptrack`, and `hyperfine` remain unavailable. Consequently P14
does not claim allocation counts, portable peak memory, or a profiler-derived
performance result. In-process `MemoryEstimate` values and P13 structural
counters remain separate diagnostic evidence.

## P14.4 Performance and Complexity Synthesis

**State: complete.** The final performance report retains only local timing
observations and deterministic structural counters. The final complexity report
separates finite exactness evidence from the deferred P9.6a reduced-event proof
obligation. Neither report claims a portable speedup, a profiler-derived memory
bound, an automatic backend crossover, or an AN19 runtime.

The required final reports, benchmark CSV/JSON, and strict manifest are now in
`results/`. The manifest has executed entries only for recorded successful
activities, an unavailable fuzz entry with its probes, and a deferred AN19
proof-debt entry. It does not convert either absence into success.

## Availability Probes

P14.1 performed these local probes on 2026-08-03:

| Capability | Probe | Result | Consequence |
| --- | --- | --- | --- |
| CP-SAT | `python3 -c 'import ortools; print(ortools.__version__)'` | exit 1: `ModuleNotFoundError: No module named 'ortools'` | P14.3 must either create the documented isolated environment or emit `unavailable`; it cannot claim an external rerun. |
| CP-SAT suite | `python3 tools/external-oracle/verify_suite.py --help` | exit 1 at `from ortools.sat.python import cp_model` | The local script is present but not runnable with the current interpreter. |
| Resource profilers | `command -v valgrind`, `heaptrack`, and `hyperfine` | unavailable | No profiler-derived allocation claim is available. |
| Local resource observation | `/usr/bin/time -l true` | exit 0 | P14.3 can collect local peak-resident-memory observations, with the stated local-only boundary. |
| Fuzzing | repository fuzz-target scan and `command -v cargo-fuzz` | no target found; runner unavailable | P14.2 must report fuzzing as unavailable unless it runs an actual registered fuzzer. |

P14.3 installed the documented isolated OR-Tools environment and exposed a
separate script defect before any CP-SAT case ran: `parse_args` defines
`--mrd`, while `main` read the nonexistent `args.rect_cli`. The focused repair
renames the `run_case` parameter to `mrd_binary` and passes `args.mrd`; it does
not alter the CP-SAT model, input population, or Rust comparison contract.

## Deferred Proof Boundary

The formal SIAM version of Abraham--Neiman, DOI `10.1137/17M1115575`, was
checked but does not establish the conversion from original power-of-two
edge-length classes to a bound or ordering on exact reduced-event classes for
`c_x(u,v) = ell(u,v) + d(x,u) - d(x,v)`. P9.6a is therefore a low-priority
deferred proof debt. Exact Oracles, finite differentials, trace counters, and
local event measurements remain implementation evidence only. They do not
establish the reduced-event bound, hierarchy-wide amortization, priority-queue
bound, or the AN19 asymptotic runtime.

## P14.1 Audit

All commands below were run from the phase baseline. Durations are wall-clock
seconds reported by `/usr/bin/time -p` where shown.

| Command | Exit status | Result |
| --- | ---: | --- |
| `git status --short --branch` | 0 | branch `codex/full-implementation`; only the two P14.1 files were untracked before staging |
| `git diff --check` | 0 | no whitespace errors |
| `jq empty docs/final-manifest.schema.json` and Python `json.load` | 0 | schema parses as JSON |
| `cargo fmt --all -- --check` | 0 | passed (0.53 s) |
| `python3 tools/check_biclique_bound.py` | 0 | passed (0.12 s) |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | no issues (0.19 s) |
| `cargo test --workspace` | 0 | 429 passed, 4 existing ignored, 15 suites (546.54 s) |
| `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` | 0 | generated all workspace docs (0.10 s) |
| `cargo build --workspace --release` | 0 | release profile already up to date (0.07 s) |
| `python3 tools/check_release_consistency.py` | 0 | 10 manifest runs, 499,220 grid comparisons, 174,767 supported polygon components/rows, 27,228 CP-SAT components in the preserved baseline (2.84 s) |
| sensitive-path scan over staged P14.1 files | 0 | no credentials or local absolute paths; schema rejects absolute and parent-traversal artifact paths |

The full test result is implementation evidence for the current finite domain.
It does not validate an asymptotic runtime. The CP-SAT and profiler probes in
this report remain availability evidence until P14.3 either executes them or
records the final unavailable/deferred disposition in `final-manifest.json`.
