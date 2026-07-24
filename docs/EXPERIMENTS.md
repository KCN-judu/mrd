# Reproducible v0.2 experiments

Evidence date: 2026-07-25 (Asia/Tokyo)

Code commit under test:
`10f1b311b6907643c4609a22cbe8929b0989b6c6`

Environment:

- macOS 26.5 (build 25F71), arm64;
- Apple M4, 16 GiB physical memory;
- `rustc 1.89.0 (29483883e 2025-08-04)`;
- Cargo release profile, warm build;
- Python 3.13 isolated environment with OR-Tools 9.15 for the optional oracle.

Elapsed times are local wall-clock observations, not asymptotic claims. Rust
solvers have no wall-clock timeout. Verification uses the exact-cover oracle
through 40 component cells unless a command states otherwise. The external
CP-SAT suite uses a 30-second limit per component.

## Quality gates

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

Result: all gates passed. The workspace test run reported 29 passed tests
across 13 test binaries and doc-test groups. These tests include endpoint
contacts, dense and topological adversarial families, mapped-back metamorphic
validation, biclique edge-multiset auditing, and stored-regression replay.

## Exhaustive and random grids

```bash
target/release/rect-cli exhaustive \
  --width 4 --height 4 \
  --output /tmp/exhaustive-4x4-10f1b31.json

target/release/rect-cli random \
  --width 8 --height 8 --cases 10000 --seed 42 \
  --output /tmp/random-8x8-seed42-10f1b31.json
```

| Population | Inputs | Components | Seed | Discrepancies | Elapsed |
| --- | ---: | ---: | ---: | ---: | ---: |
| all binary `4x4` grids | 65,536 | 337,058 | deterministic enumeration | 0 | 5.64 s |
| mixed-family random `8x8` grids | 10,000 | 162,162 | 42 | 0 | 9.11 s |

Every `4x4` component was checked by exact cover, explicit SG,
dominance C0, and compressed dominance flow. In the random campaign, exact
cover was used through 40 cells; larger components still compared all three
effective-chord pipelines and validated every returned dissection.

## Adversarial benchmark

```bash
target/release/rect-cli benchmark \
  --suite adversarial \
  --output results/adversarial.csv
```

The deterministic population contains 17 grids and 19 foreground components:
endpoint-contact variants, dense conflict grids, rings and multiple holes,
nested-looking legal geometry, one-cell corridors, combs, double combs,
staircases, spirals, reflex-heavy cases, long runs, disconnected same-color
regions, and diagonal-only contact. All 19 components are `verified`; there are
0 unsupported cases, solver errors, discrepancies, or counterexamples.

Aggregate compact representation statistics are:

| Metric | Value |
| --- | ---: |
| total effective chords `q` | 151 |
| explicit conflict edges `E` | 246 |
| bicliques | 111 |
| total biclique size `sigma` | 295 |
| compact network vertices | 300 |
| compact network arcs | 446 |
| aggregate maximum matching | 64 |
| output rectangles | 136 |

The one-biclique-per-edge C0 networks would total 435 vertices and 643 arcs on
the same component population. The compact partition uses 31.03% fewer
vertices and 30.64% fewer arcs. Exact per-component ratios, phase timings, and
all requested structural fields are in `results/adversarial.csv`. The
`peak_memory_bytes` field is blank because this run did not have a portable
process-level peak-memory sampler; blank means unmeasured, not zero.

## Free polyominoes

The repository-sized structural CSV uses the practical bound 10:

```bash
target/release/rect-cli benchmark \
  --suite polyomino --max-cells 10 --oracle-cell-limit 40 \
  --output results/polyomino.csv
```

It records 6,474 verified instances: 6,473 canonical free polyominoes plus one
explicit ordinary-hole fixture. There are 0 unsupported cases, solver errors,
discrepancies, or counterexamples. Its aggregates are `q = 15,992`,
`E = 8,092`, 6,294 bicliques, `sigma = 14,191`, 35,234 compact network
vertices, and 30,183 compact network arcs. Relative to C0 totals, compact
networks use 4.86% fewer vertices and 6.19% fewer arcs.

The full requested validation was run separately to avoid committing a 35 MB
record-level JSON file:

```bash
target/release/rect-cli polyomino \
  --max-cells 12 --all-solvers --oracle-cell-limit 40 \
  --output /tmp/polyomino-max12-timed-10f1b31.json
```

The known canonical free counts by size 1 through 12 were
`1, 1, 2, 5, 12, 35, 108, 369, 1,285, 4,655, 17,073, 63,600`.
All 87,146 free polyominoes and two separately generated ordinary-hole
fixtures were verified: 87,148 `verified`, 0 in every other status, 6.13 s.

## External CP-SAT oracle

```bash
target/release/rect-cli export-adversarial \
  --output-dir /tmp/rect-adversarial-10f1b31

/tmp/rect-oracle-venv/bin/python tools/external-oracle/verify_suite.py \
  --rect-cli target/release/rect-cli \
  --exhaustive-width 2 --exhaustive-height 3 \
  --adversarial-dir /tmp/rect-adversarial-10f1b31 \
  --max-adversarial-grid-cells 64 \
  --max-time-seconds 30 \
  --work-dir /tmp/rect-external-suite-10f1b31 \
  --output results/external-oracle.json
```

The independently parsed and modeled population contains all 64 binary `2x3`
grids plus 4 adversarial grids, totaling 68 inputs and 187 four-connected
components. There were 0 CP-SAT/Rust discrepancies and no CP-SAT timeout.
Thirteen exported adversarial grids exceeded the explicit 64-grid-cell filter;
the JSON records them in `skipped_adversarial_grid_count` rather than silently
omitting them. None of the 187 selected components exceeded the Rust
exact-cover cutoff. Larger selected components would record that solver skip
while the other three pipelines continue.

## Outcomes and scope

Across these populations there were 0 solver discrepancies and therefore 0
new minimized counterexamples. No supported benchmark or polyomino instance was
marked unsupported. The repository results explicitly exclude ornaments,
isolated formal-boundary points, line-segment holes, point holes, arbitrary
degenerate formal holes, and general polygon input.

The evidence does not establish an end-to-end `n^(1+o(1))` implementation. The
effective-chord enumerator is exact for supported grids but is not the paper's
`O(n log n)` enumeration algorithm, and Dinic remains the practical exact flow
backend rather than the cited almost-linear theoretical backend.
