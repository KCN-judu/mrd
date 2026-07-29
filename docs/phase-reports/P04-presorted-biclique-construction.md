# P04 - Presorted Biclique Construction

## Scope and semantic basis

P4 replaces repeated recursive coordinate sorting in the Cardinal--Yuditsky
Theorem 8 construction with four initial coordinate orders, stable child
filtering, and reusable recursion scratch buffers. The historical recursive
sort implementation remains the permanent reference backend. The acceptance
contract is exact canonical `biclique::Partition` equality, not merely an equal
matching value: downstream selected chords, cuts, covers, and rectangles use
the audited partition path.

## Implementation evidence

- `85c1083`: reference and presorted construction backends plus structural
  counters and strict-order differential coverage.
- `bfa5a94`: production integration, audited equality checks, and permanent
  mismatch/metric error reporting.
- `4cf8250`: construction benchmark CSV/JSON evidence harness.
- `9238066`, `f5c387e`: portable `rect-cli` provenance normalization.

The largest construction-evidence case has 1,024 horizontal and 1,024 vertical
chords, 1,126 blocks, and 6,647 emitted vertex occurrences. Its reference
backend recorded 4,989 recursive sorts; the presorted backend recorded four
initial sorts and zero recursive sorts, with identical occurrences and
partition status `verified`.

## Campaign evidence

All artifacts carry commit `f5c387e89ea339a7e4b98a2a91741ac96da348de` and a
portable command beginning `rect-cli`.

| Campaign | Result |
| --- | --- |
| biclique construction, sizes 4..512 | 8 verified; 0 solver errors; 0 counterexamples |
| exhaustive 4x4 | 65,536 grids; 337,058 components; all four comparison counts 337,058; 0 counterexamples |
| random 8x8 seed 42 | 10,000 cases; 162,162 components; 160,900 exact-cover comparisons; 0 counterexamples |
| polyomino through 10 cells | 6,474 records; 0 non-verified records |
| adversarial | 19 components; all recorded statuses verified |
| dense-conflict 4..128 | 6 verified records |
| formal fixtures | 8 verified; 0 disagreements; 0 solver errors |
| polygon differential sizes 3,4 | 66,046 inputs; 169,426 components; 167,082 supported/verified; 0 disagreements; 0 solver errors; 0 timeouts |

The polygon campaign reports 2,344 expected model rejections for inputs
outside its one-component ordinary-polygon support boundary; these are not
solver disagreements. Its counterexample artifact is the required empty list.

## Reproduction

```text
./target/release/rect-cli benchmark --suite biclique-construction --sizes 4,8,16,32,64,128,256,512 --output results/p4-biclique-construction.csv
./target/release/rect-cli exhaustive --width 4 --height 4 --output results/p4-exhaustive-4x4.json
./target/release/rect-cli random --width 8 --height 8 --cases 10000 --seed 42 --output results/p4-random-8x8-seed42.json
./target/release/rect-cli polyomino --max-cells 10 --all-solvers --oracle-cell-limit 40 --output results/p4-polyomino.csv
./target/release/rect-cli benchmark --suite adversarial --output results/p4-adversarial.csv
./target/release/rect-cli benchmark --suite dense-conflict --sizes 4,8,16,32,64,128 --output results/p4-dense-conflict.csv
./target/release/rect-cli benchmark --suite formal-fixtures --output results/p4-formal-fixtures.json
./target/release/rect-cli benchmark --suite polygon-differential --sizes 3,4 --output results/p4-polygon-differential.json
```

## Full audit

All commands exited 0 on 2026-07-27 after evidence generation. The quick
format/bound/clippy group completed in 1.7 seconds wall time; the workspace
test suite completed in 23.9 seconds; the documentation/release/consistency
group completed in 3.6 seconds (the cargo commands serialized through Cargo's
package-cache lock).

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

The release checker reports baseline release consistency for `1.3.0` and 29
reachable manifest commits. A post-stage diff and provenance scan found no
local worktree paths, credentials, ignored-test changes, or fallback behavior
added by P4.

## Limitations

P4 does not assert a timing improvement on every input. Its verified claim is
structural: production construction has no recursive coordinate sorting while
the reference backend remains available for differential verification.
