# Testing and reproducibility

The required quality gates are:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

The workspace test suite includes:

- boundary area, outer-loop, hole, and reflex invariants;
- exact-cover examples with explicit output validation;
- independent Hopcroft--Karp/cover and Dinic/cut tests;
- exhaustive closed-segment versus 4D strict-dominance pairs over small signed
  coordinate ranges, including endpoint contacts;
- exact biclique edge-partition verification;
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
```

The 2026-07-24 baseline run completed the full `4x4` campaign and the 10,000-case
seed-42 campaign without a counterexample. Exact counts and timing context are in
`EXPERIMENTS.md`.

On the first differential failure, the CLI writes
`test-data/counterexamples/first.json`. Convert confirmed failures into permanent
tests before changing any assertion or expected value.

