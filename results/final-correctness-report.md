# Final Correctness Report

Evidence baseline: `0752fce60d5a801173e963b4a1fb55c8a331949e`.

## Populations

| Campaign | Population | Result |
| --- | ---: | --- |
| Exhaustive 4x4 grid | 65,536 grids / 337,058 components | 0 counterexamples across exact-cover, SG, C0, and compressed comparisons |
| Random 8x8, seed 42 | 10,000 inputs / 162,162 components | 0 mismatches, 0 solver errors |
| Free polyominoes through 12 cells | 87,148 inputs/components | 87,148 verified; 0 counterexamples |
| Ordinary polygon differential | 66,046 inputs / 169,426 components | 167,082 supported verified; 2,344 explicit model rejections; 0 disagreement/error/timeout |
| Formal fixtures | 8 fixture/parity records | 8 verified; 0 disagreement/error |
| Direct-grid parity | 511 nonzero masks / 897 components / 1,794 comparisons | 0 mismatch/error; direct rank counters all zero |
| External CP-SAT | 6,998 inputs / 27,228 components | 6,998 verified; 0 disagreement, timeout, unsupported case, or solver error |

The generic flow package passed 225 tests and the compressed pipeline passed 62
tests with 2 existing ignored tests. The direct metamorphic test passed 2
filtered tests. All generated campaign data is under `results/final-campaigns/`.

## Boundaries

These are finite exact-correctness populations. They establish implementation
agreement, not an asymptotic runtime theorem. The fuzz-engine activity is
unavailable because this repository has no registered fuzz target and the host
has no `cargo-fuzz`; random and metamorphic tests are not relabelled as fuzzing.

The permanent reference backends remain the comparison authority. No fallback
was introduced by P14. The formal SIAM Abraham--Neiman source, DOI
`10.1137/17M1115575`, does not supply the missing reduced-event conversion.

## Reproduction

The exact commands, toolchain, result commit SHAs, and unsupported-input lists
are stored in the campaign JSON files and the P14 phase report.
