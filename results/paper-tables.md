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

The metadata above belongs to the historical v0.2 paper-table population. Later release evidence retains its own producing commits.

## Release summaries

```json
[
  {
    "version": "0.2.0",
    "tag": "v0.2.0-adversarial-validation",
    "peeled_commit": "a7766ef0245799d19a5282ec9e8a00015269cec6",
    "evidence": "historical v0.2 result population",
    "result_commits": [
      "32faff61bc4577ab50010e5d253afe83f7655d83"
    ]
  },
  {
    "version": "0.3.0",
    "tag": "v0.3.0-compact-execution",
    "peeled_commit": "7e30fa8b15d870f3fa1ed92272e8236a72f1815a",
    "evidence": "compact execution, exact grid-run chord differential, dense CompactOnly, random, adversarial, and CP-SAT reruns",
    "result_commits": [
      "20de1aafb8bcef2537495171720c21f969f84445",
      "e90f18f1313a25a91c9151f734f17ab663049a69",
      "04c721c890eefee71dbe333053051bba8cd36374",
      "b90cde9c711184fe5857dd611e75c37d62035297",
      "8100fd8aa7a4745615f450fd70ac2629c6d0561b",
      "4bee682ae9e4851d57b438d31cb612d08dd6a883"
    ]
  },
  {
    "version": "0.4.0",
    "tag": "v0.4.0-indexed-completion",
    "peeled_commit": "00b6160a12f95e4b1722ec0f831f61fbf72228e0",
    "evidence": "indexed frontier completion with exact differential cuts and dense backend comparison",
    "result_commits": [
      "47f0412aff29eafb80de29201cf8f4b5f825bf92",
      "24af74330ef0f3b4cd287463af694e5d72afa952"
    ]
  },
  {
    "version": "0.5.0",
    "tag": "v0.5.0-prepared-grid-pipeline",
    "peeled_commit": "602df9ad4737c067311f438c94ddf880fd6c7ca1",
    "evidence": "single-build prepared geometry, prepared-run enumeration, dense cut storage, dense recovery, and exact differential outputs",
    "result_commits": [
      "602df9ad4737c067311f438c94ddf880fd6c7ca1"
    ]
  },
  {
    "version": "0.5.0-clean-hole-free-path-tree",
    "tag": "v0.5.0-clean-hole-free-path-tree",
    "peeled_commit": "c8646f60c2056c4c87811e4e93ca6e75edd06d6b",
    "evidence": "clean census, symmetric path-tree geometry, and complete-bipartite family",
    "result_commits": [
      "02f1532fe2caa25a05d04a6990743c4ddac28ec1",
      "f2742d9e5a578ca3c2f6236aacf8d64a190b06ca"
    ]
  },
  {
    "version": "0.6.0",
    "tag": "v0.6.0-true-compact-path-tree",
    "peeled_commit": "ce773a21e44cb5ecba6b6442e6c2bb9172f37141",
    "evidence": "boundary-laminar dual, endpoint-only HLD, compact path certificates, and scaled clean-family campaign",
    "result_commits": [
      "b03ae75039b4f55a7c8940d41bbcf364d72a2d77",
      "349487ddef9d34b49cf48ac1e0dfc432676aa8b5",
      "ce773a21e44cb5ecba6b6442e6c2bb9172f37141"
    ]
  }
]
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
| fast chord enumeration | O(n log n) | GridInteriorRunEnumerator, O(N + r log r + q) | exact differential | CompactOnly default; pairwise reference retained |
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

These rows are separate v0.3 CompactOnly evidence and do not overwrite the historical v0.2 population.
Exact chord-family differential comparisons: 253,219 inputs, 0 disagreements.
The bounded v0.3 CP-SAT rerun compared 27,228 components with 0 disagreements.
Peak RSS is unmeasured; no null value is interpreted as zero.

## v0.5 clean path-tree evidence

The binary `4x4` clean census covers 168,529 foreground components: 155,389
pass the clean certificate, with 19,908 effective chords in the eligible
population. Rejection counts are 593 hole components and 20,736 shared-endpoint
rejections; the complete q histogram and chord-mass denominators are generated
in `results/v0.5-clean-census.json` and `.csv`.

The `3x3` path-tree comparison covers 871 eligible components with zero output
or optimum disagreements. The reference dual is area-sensitive; the final
backend evaluates both orientations and records the selected one.

## v0.5 clean geometry completion

The final finite-grid clean geometry campaign is bound to commit `f2742d9`.
The complete-bipartite fixtures for `t=1..4` have exact chord families
`(2t,2t)` and conflict counts `4t^2`; all four are verified. The full binary
`4x4` path-tree comparison covers 155,389 eligible components with zero
counterexamples and zero solver errors. The compressed evidence is in
`results/v0.5-path-tree-comparison-4x4-summary.json` and the gzipped row table.

## v0.6 true compact path-tree evidence

The generated compact family table is
`results/v0.6-clean-complete-bipartite-compact.csv`. It covers
`t=1,2,4,8,16,32,64,128` (`q=4t` through 512), with zero solver errors or
counterexamples. CompactOnly uses `BoundaryLaminar`; the area dual, per-path
BFS, explicit path-edge lists, unit chord cuts, and prepared occupancy
transpose are all disabled and recorded as false in `ExecutionTrace`.

| t | q | |H| | |V| | dual regions | path records | path-edge incidences | canonical nodes | sigma | explicit E |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 4 | 2 | 2 | 3 | 2 | 4 | 1 | 4 | null |
| 2 | 8 | 4 | 4 | 5 | 4 | 16 | 1 | 8 | null |
| 4 | 16 | 8 | 8 | 9 | 8 | 64 | 1 | 16 | null |
| 8 | 32 | 16 | 16 | 17 | 16 | 256 | 1 | 32 | null |
| 16 | 64 | 32 | 32 | 33 | 32 | 1024 | 1 | 64 | null |
| 32 | 128 | 64 | 64 | 65 | 64 | 4096 | 1 | 128 | null |
| 64 | 256 | 128 | 128 | 129 | 128 | 16384 | 1 | 256 | null |
| 128 | 512 | 256 | 256 | 257 | 256 | 65536 | 1 | 512 | null |

The path-edge incidence column is a metric, not materialized storage: the
CompactOnly certificate contains zero explicit path-edge records. Owned
allocation estimates are serialized in the CSV; process peak RSS remains
unmeasured and is never interpreted as zero.

The table and summary above are regenerated by
`tools/generate_path_tree_tables.py`; the canonical generated artifacts are
`results/v0.6-clean-complete-bipartite-compact-summary.json` and
`results/v0.6-clean-complete-bipartite-compact.md`.
