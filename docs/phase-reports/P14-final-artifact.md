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
| `grid-exhaustive-4x4` | P14.2 | `cargo run --release -p mrd -- exhaustive --width 4 --height 4 --output results/final-grid-exhaustive-4x4.json` | exact grid report | Exhaustive finite `4x4` colored-grid population only; no asymptotic claim. |
| `grid-random-8x8-seed42` | P14.2 | `cargo run --release -p mrd -- random --width 8 --height 8 --cases 10000 --seed 42 --output results/final-random-8x8-seed42.json` | seeded differential report | Deterministic finite sample only. |
| `polyomino-through-12` | P14.2 | `cargo run --release -p mrd -- polyomino --max-cells 12 --all-solvers --output results/final-polyomino-max12.json` | all-solver polyomino report | Canonical free polyominoes through 12 cells only. |
| `grid-polygon-differential` | P14.2 | `cargo run --release -p mrd -- benchmark --suite polygon-differential --sizes 3,4 --output results/final-polygon-differential.json` | report plus minimized-counterexample file | Ordinary grid-derived polygon parity at the recorded finite sizes only. |
| `formal-fixtures` | P14.2 | `cargo run --release -p mrd -- benchmark --suite formal-fixtures --output results/final-formal-fixtures.json` | formal fixture report | Eight named formal-boundary fixtures only; not arbitrary formal inputs. |
| `direct-grid-parity` | P14.2 | `cargo run --release -p mrd -- benchmark --suite direct-grid-parity --output results/final-direct-grid-parity.json` | 3x3 direct-versus-ranked report | Exact finite-grid equality and zero direct rank counters; timing stays local. |
| `direct-grid-metamorphic` | P14.2 | `cargo test -p dominance direct_grid_embedding_matches` | test transcript recorded in final correctness report | The tested translated/isometric examples only. |
| `generic-and-compressed-flow` | P14.2 | `cargo test -p graph && cargo test -p dominance && cargo run --release -p mrd -- benchmark --suite construction --sizes 4,8,16,32,64,128,256 --output results/final-flow-backends.csv` | test transcript, CSV, adjacent JSON, and local manifest | Exact finite flow/cut/cover agreement and structural counters; benchmark times do not establish a crossover policy or complexity theorem. |
| `workspace-quality` | P14.2 and P14.5 | Mandatory workspace audit from the master plan | command ledger in final manifest and reports | Build, lint, test, and documentation health, not exhaustive semantic verification. |
| `external-cp-sat` | P14.3 | The isolated-venv commands in `tools/external-oracle/README.md`, followed by `tools/external-oracle/verify_suite.py` | CP-SAT/Rust comparison JSON or an unavailable record | Bounded grid components only; CP-SAT `optimal` is required. No general polygon or formal-boundary claim. |
| `resource-measurement` | P14.3 | `/usr/bin/time -l` around representative final commands, with in-process `MemoryEstimate` fields retained where emitted | resource command ledger and report | Local peak resident-memory observations and retained-memory estimates only, never a portable memory bound. |
| `fuzz-engine` | P14.2 availability decision | Registered fuzz target and runner, if one exists | unavailable record unless an actual fuzzer is run | No fuzz target is present and `cargo-fuzz` is unavailable in the P14.1 environment; random and metamorphic campaigns are not labelled fuzzing. |
| `performance-and-complexity` | P14.4 | Reuse executed P14.2/P14.3 records, P13's direct-grid ledger, and static checkers | final performance and complexity reports | Local timings and finite structural counters only. AN19 runtime evidence is excluded. |
| `an19-runtime-proof` | P14.4/P14.5 | No executable substitute | deferred record | P9.6a remains proof debt: DOI `10.1137/17M1115575` does not establish the reduced-event ordering/counting conversion. No `AlmostLinear` or AN19 runtime claim is permitted. |
| `final-release-audit` | P14.5 | Mandatory audit protocol plus schema/content/hash inspection | final reports, benchmark files, and final manifest | Artifact integrity and reproducibility at the recorded commit only. |

Commands that write a sibling `manifest.json` are intentionally directed to
`results/final-*` names, so their historical release manifests are not
rewritten. The P14.5 final manifest is an explicit aggregation over those
outputs rather than an implicit interpretation of a legacy manifest.

## Availability Probes

P14.1 performed these local probes on 2026-08-03:

| Capability | Probe | Result | Consequence |
| --- | --- | --- | --- |
| CP-SAT | `python3 -c 'import ortools; print(ortools.__version__)'` | exit 1: `ModuleNotFoundError: No module named 'ortools'` | P14.3 must either create the documented isolated environment or emit `unavailable`; it cannot claim an external rerun. |
| CP-SAT suite | `python3 tools/external-oracle/verify_suite.py --help` | exit 1 at `from ortools.sat.python import cp_model` | The local script is present but not runnable with the current interpreter. |
| Resource profilers | `command -v valgrind`, `heaptrack`, and `hyperfine` | unavailable | No profiler-derived allocation claim is available. |
| Local resource observation | `/usr/bin/time -l true` | exit 0 | P14.3 can collect local peak-resident-memory observations, with the stated local-only boundary. |
| Fuzzing | repository fuzz-target scan and `command -v cargo-fuzz` | no target found; runner unavailable | P14.2 must report fuzzing as unavailable unless it runs an actual registered fuzzer. |

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
