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
    "evidence": "clean eligibility census, region-dual tree, symmetric HLD path partition, complete-bipartite family, representation selection, and differential path-tree audits",
    "result_commits": [
      "02f1532fe2caa25a05d04a6990743c4ddac28ec1",
      "d90acb5",
      "f2742d9e5a578ca3c2f6236aacf8d64a190b06ca"
    ]
  },
  {
    "version": "0.6.0",
    "tag": "v0.6.0-true-compact-path-tree",
    "peeled_commit": "c6bafda3a0493d60f6468f4298765468376753d8",
    "evidence": "boundary-laminar region dual, endpoint-only HLD, compact path certificates, and scaled clean-family campaign",
    "result_commits": [
      "b03ae75039b4f55a7c8940d41bbcf364d72a2d77",
      "349487ddef9d34b49cf48ac1e0dfc432676aa8b5",
      "ce773a21e44cb5ecba6b6442e6c2bb9172f37141",
      "c6bafda3a0493d60f6468f4298765468376753d8"
    ]
  },
  {
    "version": "0.7.0",
    "tag": "v0.7.0-path-tree-evidence-and-adaptive-dispatch",
    "peeled_commit": "3994cb101606733bae815560c25815f2355d4b34",
    "evidence": "path-tree structural families, branching witness, orientation-regret audit, axis-view differential, bounded dual differential, Auto fallback, and path-tree-vs-4d comparison",
    "result_commits": [
      "224737f",
      "2028d12",
      "bd945f5",
      "3994cb1"
    ]
  },
  {
    "version": "0.8.0",
    "tag": "v0.8.0-boundary-indexed-adaptive-path-tree",
    "peeled_commit": "0e5a68706b0fd527c906c4dc8a60ac4ab2b12e9f",
    "evidence": "boundary index, indexed endpoint classifier, laminar event sweep, mixed-branching witnesses, and adaptive orientation evidence",
    "result_commits": [
      "d6a683ef253cdf88dab2423d688541e6e74034f6",
      "72836293637c6fbb35f7c6982705632bbe8c2f3f",
      "1fd11cc88bd7b0859bc57e6e17efb19e6ed40518",
      "0e5a68706b0fd527c906c4dc8a60ac4ab2b12e9f"
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
| clean hole-free eligibility | Definition 9.1 | integer grid classifier with loop identities | yes | component and chord-mass census; ornaments remain out of model |
| path-tree biclique partition | Theorems 9.5-9.6 | BoundaryLaminar axis view plus endpoint HLD in CompactOnly; area dual Oracle in FullyAudited | yes | full 4x4 differential and axis-view equality |
| clean complete-bipartite family | Theorem 9.2 | integer-grid realization | yes | compact campaign through t=128 |

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

## v0.6 BoundaryLaminar differential evidence

The full 4x4 campaign covers 65,535 masks and 155,389 clean eligible components. It records 155,389 verified rows, 0 counterexamples, and 0 execution-trace violations.
Orientation counts: `{"vertical-tree-horizontal-paths": 155389}`; q range `0..3`, sigma range `0..2`.

## v0.7 Path-tree geometry families

| family | instance_name | status | path_tree_orientation | path_tree_orientation_policy | dual_region_count | dual_tree_max_depth | dual_tree_max_branching_degree | heavy_chain_interval_count | canonical_segment_node_count | path_tree_sigma |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| laminar-chain | laminar-chain-7 | verified | vertical-tree-horizontal-paths | bound-estimate | 15 | 14 | 2 | 14 | 1 | 28 |
| laminar-star | laminar-star-7 | verified | horizontal-tree-vertical-paths | bound-estimate | 7 | 1 | 6 | 0 | 0 | 0 |
| balanced-laminar | balanced-laminar-7 | verified | horizontal-tree-vertical-paths | bound-estimate | 8 | 1 | 7 | 0 | 0 | 0 |
| asymmetric-orientation | asymmetric-orientation-7 | verified | vertical-tree-horizontal-paths | bound-estimate | 2 | 1 | 1 | 1 | 1 | 2 |

## v0.7 Path-tree versus 4D

| family | instance_name | q | q_bucket | sigma_path_tree | sigma_4d | network_arcs_path_tree | network_arcs_4d | path_tree_total_microseconds | four_d_total_microseconds | optimum_equal | rectangles_equal | status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| laminar-chain | laminar-chain-8 | 32 | 9-32 | 32 | 32 | 64 | 64 | 436 | 2284 | true | true | verified |
| laminar-star | laminar-star-8 | 7 | 0-8 | 0 | 0 | 7 | 7 | 22 | 122 | true | true | verified |
| balanced-laminar | balanced-laminar-8 | 8 | 0-8 | 0 | 0 | 8 | 8 | 24 | 155 | true | true | verified |
| asymmetric-orientation | asymmetric-orientation-8 | 2 | 0-8 | 2 | 2 | 4 | 4 | 8 | 17 | true | true | verified |
| clean-complete-bipartite | clean-complete-bipartite-t1 | 4 | 0-8 | 4 | 4 | 8 | 8 | 41 | 205 | true | true | verified |
| clean-complete-bipartite | clean-complete-bipartite-t2 | 8 | 0-8 | 8 | 8 | 16 | 16 | 68 | 383 | true | true | verified |
| clean-complete-bipartite | clean-complete-bipartite-t4 | 16 | 9-32 | 16 | 16 | 32 | 32 | 150 | 879 | true | true | verified |
| clean-complete-bipartite | clean-complete-bipartite-t8 | 32 | 9-32 | 32 | 32 | 64 | 64 | 509 | 2413 | true | true | verified |

## v0.7 Orientation regret audit

The row-level CSV contains 160,443 clean instances. Exact sigma matches: 160,443; positive-regret mismatches: 0; equal-sigma direction ties: 409; maximum absolute regret: 0.

## v0.7 BoundaryLaminar versus area dual

The row-level CSV contains 156,267 clean instances. Verified: 156,267; counterexamples: 0; solver errors: 0.

## v0.8 Boundary-indexed adaptive path-tree

### Indexed frontend and boundary-gap differential

The complete differential campaign covers 950,557 inputs, 1,053,939 components, and 385,947 clean components.
It performs 16,530,974 boundary-index comparisons, 3,368,464 endpoint-metadata comparisons, 1,053,939 clean-classifier comparisons, and 771,894 orientation comparisons.
Verified clean components: 385,947; mismatches: 0; solver errors: 0. ReferenceNested performs 52,388,636 interval-membership tests; EventSweep records 409,593 pushes and 409,593 pops.

### Minimized mixed-branching witnesses

| name | cells | horizontal_chords | vertical_chords | dual_max_branching_degree | path_count | heavy_chain_interval_count | paths_using_multiple_heavy_chains | canonical_segment_node_count |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| permuted-notch-grid-74 | 54 | 3 | 3 | 3 | 3 | 4 | 1 | 2 |
| mutated-notch-064-119 | 60 | 3 | 3 | 3 | 3 | 5 | 2 | 2 |
| mutated-notch-057-054 | 63 | 3 | 3 | 3 | 3 | 4 | 1 | 2 |
| mutated-notch-074-034 | 64 | 5 | 4 | 4 | 5 | 4 | 1 | 3 |
| permuted-notch-grid-706 | 64 | 4 | 3 | 4 | 3 | 4 | 2 | 3 |
| mutated-notch-074-008 | 66 | 3 | 3 | 3 | 3 | 4 | 1 | 3 |
| mutated-notch-003-024 | 66 | 3 | 5 | 3 | 5 | 4 | 1 | 3 |
| mutated-notch-046-026 | 67 | 3 | 3 | 3 | 3 | 4 | 1 | 3 |
| mutated-notch-027-048 | 70 | 3 | 3 | 3 | 3 | 4 | 1 | 3 |
| mutated-notch-074-011 | 71 | 3 | 4 | 3 | 4 | 4 | 1 | 3 |
| mutated-notch-074-024 | 80 | 3 | 5 | 5 | 3 | 4 | 2 | 3 |
| mutated-notch-064-012 | 83 | 5 | 3 | 3 | 3 | 4 | 1 | 3 |
| mutated-notch-074-003 | 84 | 3 | 4 | 3 | 4 | 4 | 1 | 2 |
| mutated-notch-064-025 | 97 | 4 | 5 | 4 | 4 | 4 | 1 | 4 |
| mutated-notch-074-037 | 115 | 3 | 5 | 3 | 3 | 4 | 2 | 4 |
| mutated-notch-074-038 | 116 | 3 | 4 | 3 | 4 | 4 | 1 | 2 |

The deterministic witness search examined 74,547 production geometry candidates and retained 16 translation/dihedral-canonical witnesses after delta-debugging minimization.
Minimized cell counts range from 54 to 116.

### v0.8 scaled geometry families

The generated family campaign contains 30 rows (30 nontrivial chord-bearing rows), all with status `verified`.
Chain q grows from 12 to 512; star and balanced rows reach dual branching degrees 127 and 128.
The mixed-branching connected-sum family contains 6 verified members and reaches q=61, 33 dual regions, 29 paths, 39 heavy-chain intervals, and 24 canonical nodes.
The connected-sum members are rebuilt through production geometry; no coordinate-only scaling law or synthetic dual graph is used.

### v0.8 representation comparison

The generated comparison contains 31 rows, 31 verified, across q buckets `0-8`, `9-32`, `33-128`, `129-512`, `513-2048`, `2049+`.
It records sigma, network size, phase timings, final equality, and owned-allocation estimates for both path-tree and 4D representations.

### v0.8 representation advantage search

The generated advantage search retains 29 eligible mixed-orientation rows; strict path-tree advantages: 13; strict 4D advantages: 0.
Retained rows have owned-allocation maxima of 1,084,024 bytes for path-tree and 432,926,462 bytes for 4D; final optimum and rectangle equality are recorded per row.

### v0.8 orientation regret audit

The expanded audit contains 160,462 rows: 160,457 exact sigma matches and 5 positive-regret rows.
Maximum absolute regret is 2 and the maximum recorded regret ratio is 2/6. These counterexamples keep exact `build-both` as the CompactOnly default; `bound-estimate` remains an explicit benchmark policy.
