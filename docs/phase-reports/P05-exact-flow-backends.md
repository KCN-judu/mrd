# P05 - Exact Flow Backends

## Contract and implementation

P5 preserves `DinicBackend` as a permanent exact reference and adds a
deterministic integral highest-label push-relabel backend. The implementation
uses global relabeling, the gap heuristic, checked final flow conversion, and
residual source-side reachability. `FlowBackendKind` dispatches between the
two; existing public APIs keep their Dinic defaults, while
`solve_with_flow_backend` exposes backend selection for fully audited grids.

The semantic basis is Goldberg and Tarjan, "A New Approach to the
Maximum-Flow Problem," JACM 35(4), 1988. This is a practical exact backend,
not an implementation or complexity claim for the FOCS 2023 almost-linear
algorithm deferred to later phases.

## Evidence

- 1,024 deterministic directed integral networks compare Dinic and
  push-relabel maximum-flow values. Every push-relabel result also proves its
  reported source-side cut has capacity equal to its flow value.
- A compressed biclique certificate test compares both backends' flow values,
  recovered cover sizes, and absence of internal cut arcs.
- An end-to-end fully audited grid test selects both backends and requires the
  same optimum rectangle count.
- `results/p5-flow-backends.json` records seven verified dense compressed
  networks, sizes 4 through 256. It has zero solver errors and zero
  counterexamples. Its largest 512-horizontal/512-vertical-chord case reports
  value 512 for both backends.

The dense benchmark measured 413 microseconds for Dinic and 54,215
microseconds for push-relabel on that largest case. No crossover or automatic
selection policy is claimed from this finite population.

## Reproduction

```text
cargo test -p rect-graph -p rect-dominance -p rect-verify
./target/release/rect-cli benchmark --suite biclique-construction --sizes 4,8,16,32,64,128,256 --output results/p5-flow-backends.csv
```

## Full audit

All commands exited 0 after the closeout state was prepared:

```text
git diff --check
cargo fmt --all -- --check
python3 tools/check_biclique_bound.py
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps
cargo build --workspace --release
python3 tools/check_release_consistency.py
```

The release checker reports baseline release consistency and 30 reachable
manifest commits. The final staged-diff review checks for ignored-test changes,
fallbacks, stale generated evidence, credentials, and local absolute paths.
No fallback is used by the selected backend; backend choice is explicit and
Dinic remains permanently available.
