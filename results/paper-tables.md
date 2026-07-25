# Generated paper tables

```json
{
  "git_commit": "32faff61bc4577ab50010e5d253afe83f7655d83",
  "rustc_version": "rustc 1.89.0 (29483883e 2025-08-04)",
  "operating_system": "macOS-26.5-arm64-arm-64bit-Mach-O",
  "cpu": "Apple M4",
  "build_profile": "release",
  "random_seed": 42,
  "cp_sat_seed": 0,
  "cp_sat_timeout_seconds_per_component": 30.0,
  "commands": [
    "target/release/rect-cli exhaustive --width 4 --height 4 --output results/exhaustive-4x4.json",
    "target/release/rect-cli random --width 8 --height 8 --cases 10000 --seed 42 --output results/random-8x8-seed42.json",
    "target/release/rect-cli benchmark --suite adversarial --output results/adversarial.csv",
    "target/release/rect-cli benchmark --suite polyomino --max-cells 10 --oracle-cell-limit 40 --output results/polyomino.csv",
    "tools/external-oracle/verify_suite.py --rect-cli target/release/rect-cli --exhaustive-width 3 --exhaustive-height 3 --polyomino-max-cells 10 --adversarial-dir /tmp/mrd-adversarial-final-32faff6 --max-adversarial-grid-cells 20000 --max-component-cells 40 --exact-cover-cell-limit 40 --max-time-seconds 30 --work-dir /tmp/mrd-external-final-32faff6 --output results/external-oracle.json",
    "target/release/rect-cli benchmark --suite dense-conflict --sizes 4,8,16,32,64,128 --output results/dense-conflict.csv"
  ]
}
```

## Correctness

| suite | grids | components | exact-cover comparisons | CP-SAT comparisons | SG comparisons | C0 comparisons | compressed comparisons | counterexamples |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| exhaustive-binary-4x4 | 65536 | 337058 | 337058 | 0 | 337058 | 337058 | 337058 | 0 |
| random-binary-8x8 | 10000 | 162162 | 160900 | 0 | 162162 | 162162 | 162162 | 0 |
| adversarial | 17 | 19 | 9 | 0 | 19 | 19 | 19 | 0 |
| free-polyomino | 6474 | 6474 | 6474 | 0 | 6474 | 6474 | 6474 | 0 |
| external-binary-3x3 | 512 | 1794 | 1794 | 1794 | 1794 | 1794 | 1794 | 0 |
| external-free-polyomino | 6473 | 25390 | 25390 | 25390 | 25390 | 25390 | 25390 | 0 |
| external-adversarial | 13 | 44 | 44 | 44 | 44 | 44 | 44 | 0 |

## Compression

| family | q | \|E\| | biclique_count | sigma | sigma / \|E\| | C0 arcs | compressed arcs | arc reduction | C0 time | compressed time |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dense-conflict-4x4 | 16 | 32 | 17 | 40 | 1.250000 | 80 | 56 | 0.300000 | 89 | 75 |
| dense-conflict-8x8 | 32 | 96 | 24 | 84 | 0.875000 | 224 | 116 | 0.482143 | 157 | 157 |
| dense-conflict-16x16 | 64 | 320 | 51 | 189 | 0.590625 | 704 | 253 | 0.640625 | 511 | 466 |
| dense-conflict-32x32 | 128 | 1152 | 87 | 386 | 0.335069 | 2432 | 514 | 0.788651 | 1841 | 1592 |
| dense-conflict-64x64 | 256 | 4352 | 164 | 795 | 0.182675 | 8960 | 1051 | 0.882701 | 7334 | 13611 |
| dense-conflict-128x128 | 512 | 16896 | 308 | 1619 | 0.095821 | 34304 | 2131 | 0.937879 | 33822 | 26279 |

## Scope

| feature | theoretical paper | current Rust artifact | tested | notes |
| --- | --- | --- | --- | --- |
| ordinary holes | supported | supported for grid-cell regions | yes | rings and separated holes |
| degenerate holes | formal model | unsupported | scope rejection | point, segment, and arbitrary formal holes excluded |
| endpoint contacts | closed-chord conflicts | integer parity embedding | yes | pairwise geometry iff strict dominance |
| fast chord enumeration | O(n log n) | exact aligned-reflex pair tests | yes | classical sweep not implemented |
| compact biclique partition | O(q log^4 q) for d=4 | constructive Theorem 8 recursion | yes | edge multiplicity audited exactly once |
| practical Dinic backend | replaceable exact flow | implemented | yes | integral flow and residual cut |
| almost-linear theoretical flow backend | used asymptotically | not implemented | no | citation-only complexity component |
| explicit rectangle output | constructive completion | implemented | yes | cell-exact validation |
| machine-checkable certificates | not an artifact requirement | implemented | yes | matching, partition, flow, cut, and rectangles |

## CompactOnly v0.3 evidence

| total q | horizontal chords | vertical chords | bicliques | sigma | compressed vertices | compressed arcs | enumerator | explicit edges |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 512 | 256 | 256 | 308 | 1619 | 822 | 2131 | grid-interior-runs | null |
| 1024 | 512 | 512 | 578 | 3280 | 1604 | 4304 | grid-interior-runs | null |
| 2048 | 1024 | 1024 | 1126 | 6647 | 3176 | 8695 | grid-interior-runs | null |
| 4096 | 2048 | 2048 | 2187 | 13400 | 6285 | 17496 | grid-interior-runs | null |

The four rows are separate CompactOnly evidence and do not overwrite the v0.2
population above. Peak RSS is unmeasured; no null value is interpreted as zero.

Chord-family differential evidence covers 511 nonempty `3x3` masks, 65,535
nonempty `4x4` masks, and 100,000 deterministic connected larger regions. All
166,046 inputs had exact horizontal and vertical family equality, with no
missing or fabricated chord. The v0.3 CP-SAT rerun verified 6,998 inputs and
27,228 components with zero disagreement.

The final `q=4096` run measured 285,717 microseconds for boundary plus grid-run
enumeration, 201 for embedding, 2,716 for biclique construction, 539 for flow,
and 1,088,822 for geometric completion. Owned estimates were 131,072 bytes for
chords, 262,144 for embedding points, 229,672 for bicliques, 190,248 for flow
storage, and 591,248 for the certificate payload. These are not peak RSS.
