# P07 - Exact Circulation Refinement

## Contract and source boundary

P7 adds `rect-graph::min_cost::CirculationNetwork`, a generic exact integral
min-cost circulation Oracle with signed `i128` demands, capacities, costs,
residual arcs, and recovery verification. The implementation first constructs
a feasible circulation, then exhaustively enumerates simple signed residual
cycles and augments a lowest exact cost-to-unit-length-ratio cycle while its
cost is negative. The trace records every augmentation and objective transition.

The source boundary is deliberate. Equation (1) and Section 3.1 of van den
Brand et al., arXiv:2309.16629v1, establish the min-ratio-cycle update form;
this implementation supplies only a finite, superlinear exact Oracle for it.
It does not claim the source's IPM, approximate gradients, hidden stable
witness, fractional rounding, dynamic trees, dynamic spanners, or
almost-linear running time.

## Evidence

- The min-ratio API handles positive forward and negative reverse residual
  directions using exact `i128` cross multiplication and rejects nonpositive
  lengths.
- Negative-cost self-loops are treated as valid one-arc directed cycles.
- `verify_solution` recomputes balances and cost, validates capacities, and
  rejects every recovered circulation with a negative residual cycle.
- The focused `rect-graph` test suite has 14 tests. Its bounded independent
  enumerator compares the solver against all 125 assignments of three costs in
  `[-2, 2]` on a capacity-two, three-arc demand network. There are zero
  objective disagreements.
- Focused regressions cover feasibility routing past a reachable negative
  cycle, reverse residual cycles, malformed recovery, strict trace decreases,
  infeasible demand, and negative self-loops.

No benchmark artifact is generated: exhaustive cycle enumeration is an Oracle
rather than a performance backend, and this phase makes no performance claim.

## Full audit

Phase baseline: `9f124459154d5c801c34a97ddb968338afe472b`.

The following combined audit completed with exit status 0 in 26.4 seconds;
the commands produce no generated result files:

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

The release checker reports baseline release consistency. A final diff review
found no ignored-test change, fallback selection, stale generated evidence,
credential, private-key, token, or local-absolute-path addition. Reference
flow backends remain unchanged and permanently available.

## Remaining limitations

The P7 Oracle is intentionally exhaustive and may be exponential in the
number of nodes. P8 must first split its source-backed dynamic data structures
into numbered subphases before implementation. P9 remains prohibited from an
almost-linear claim until every source assumption and recovery gate is present.
