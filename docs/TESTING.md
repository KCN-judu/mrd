# Testing and reproducibility

The required quality gates are:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

The workspace test suite includes:

- boundary area, outer-loop, hole, and reflex invariants;
- exact-cover examples with explicit output validation;
- independent Hopcroft--Karp/cover and Dinic/cut tests;
- exhaustive closed-segment versus 4D strict-dominance pairs over small signed
  coordinate ranges, including endpoint contacts;
- exact biclique edge-partition verification;
- deterministic endpoint-contact, dense-conflict, and topological adversarial
  families;
- mapped-back validation after translation, all square-grid dihedral
  symmetries, and positive integer scaling;
- canonical free-polyomino enumeration under all eight dihedral symmetries;
- automatic replay of every bundle under `test-data/regressions`;
- all `2^(3*3) = 512` binary grids, splitting both colors into four-connected
  components and comparing exact cover, explicit SG, C0 flow, and compact flow;
- deterministic random families through the CLI: Bernoulli masks, random walks,
  rectangle unions, combs, checkerboards, and rings/corridors.

Run larger exhaustive and random campaigns with:

```bash
cargo run --release -p rect-cli -- exhaustive --width 4 --height 4 \
  --output exhaustive-4x4.json

cargo run --release -p rect-cli -- random --width 8 --height 8 \
  --cases 10000 --seed 42 --output random-seed-42.json

cargo run --release -p rect-cli -- polyomino --max-cells 12 \
  --all-solvers --output polyomino-max12.json

cargo run --release -p rect-cli -- benchmark --suite adversarial \
  --output adversarial.csv
```

The 2026-07-24 baseline run completed the full `4x4` campaign and the 10,000-case
seed-42 campaign without a counterexample. Exact counts and timing context are in
`EXPERIMENTS.md`.

On a differential failure, the CLI minimizes the fixture by row, column, and
foreground-cell deletion, crops through those reductions, and canonicalizes
under all eight dihedral views. It writes a replayable bundle under
`test-data/regressions/` with the input, solver outputs, diagnostics embedded in
those outputs, expected behavior, and explanation. Every stored bundle is a
permanent workspace test.

`.github/workflows/ci.yml` runs the quality gates and a bounded adversarial
smoke test. `.github/workflows/full-verification.yml` runs manually or on
release tags and archives logs, JSON and CSV evidence, regression bundles, and
SVG diagnostics for minimized Rust failures.
