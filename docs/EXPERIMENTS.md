# Reproducible experiment baseline

Date: 2026-07-24

Toolchain: `rustc 1.89.0`, `cargo 1.89.0`

Profile: Cargo `--release`, warm build
Machine timing: local macOS workspace; elapsed times are informative, not claims
about asymptotic performance.

## Exhaustive binary grids

```bash
/usr/bin/time -p target/release/rect-cli exhaustive \
  --width 4 --height 4 --output exhaustive-4x4.json
```

Result:

```text
grid_count      65,536
component_count 337,058
counterexamples 0
real            5.19 s
```

Every component was solved by independent exact cover, explicit SG,
dominance C0, and compact dominance flow. Every returned rectangle list was
validated cell by cell.

## Deterministic mixed-family random campaign

```bash
/usr/bin/time -p target/release/rect-cli random \
  --width 8 --height 8 --cases 10000 --seed 42 \
  --output random-8x8-seed42.json
```

Result:

```text
case_count      10,000
component_count 162,162
counterexamples 0
real            8.80 s
```

The six repeating families are Bernoulli masks, connected random walks, unions
of rectangles, combs, checkerboards, and rings/corridors. Exact cover is used for
components up to 40 cells; larger components compare the two independent graph
pipelines and validate feasibility/certificates.

These experiments establish agreement for the tested domain. They are not a
proof of support for ornament or degenerate formal-hole inputs, which remain
explicitly out of scope.
