# Paper Kernel Scaling Full Report

## Scope and protocol

This campaign measures three exact implementations in one release process per family/size partition. Scope A starts from the canonical component and includes geometry, solving, completion, and verification. Scope B starts after shared geometry and chord generation and measures representation construction, matching or flow, and cover recovery only.

Source commit: `103700eaa2b55de14daab010a82556efdf16fb84`. Binary SHA-256: `c73399f8142839413b550dfa725f01a2c50afa1fd233696d02425bd6e277e4a7`. Config SHA-256: `ada9f4c3ad749b67f2a6d6da429a28aa640330f3a130e33e28dce9687681512c`.

Host: Apple M4 on macOS-26.5-arm64-arm-64bit-Mach-O; compiler rustc 1.89.0 (29483883e 2025-08-04); power source AC.

## Correctness and coverage

The campaign contains 1070372 retained measured iterations across 45 complete points. It has 0 invalid points, 0 failed production gates, 0 duplicate identities, and 0 missing planned points.

## Family-level paired results

| Family | Scope | Explicit reference | Median ratio | 95% CI | Classification | Stable crossover target |
| --- | --- | --- | ---: | ---: | --- | ---: |
| comb-staircase | representation-and-solver-kernel | explicit-c0-flow | 1.59 | [1.221, 1.696] | compact-clearly-slower | NA |
| comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | 4.844 | [3.328, 6.279] | compact-clearly-slower | NA |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | 1.002 | [1.001, 1.006] | compact-clearly-slower | NA |
| comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | 1.003 | [1.001, 1.013] | compact-clearly-slower | NA |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | 0.03447 | [0.005347, 0.1563] | insufficient | 64 |
| dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 0.2711 | [0.0242, 2.557] | insufficient | 128 |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | 0.3722 | [0.2683, 0.4964] | insufficient | 64 |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 0.8787 | [0.6231, 1.142] | insufficient | 256 |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | 0.8664 | [0.5362, 1.454] | unresolved | 512 |
| random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | 14.25 | [10.53, 16.51] | compact-clearly-slower | NA |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | 0.9738 | [0.9033, 1.098] | unresolved | 512 |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | 1.2 | [1.172, 1.264] | compact-clearly-slower | NA |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | 0.02101 | [0.008594, 0.05415] | compact-clearly-faster | 64 |
| representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | 0.4548 | [0.09901, 1.341] | unresolved | 256 |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | 0.8937 | [0.8738, 0.9136] | compact-clearly-faster | 64 |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | 0.9942 | [0.9848, 1] | unresolved | 512 |
| sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | 2.393 | [2.115, 2.808] | compact-clearly-slower | NA |
| sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 12.08 | [8.798, 14.35] | compact-clearly-slower | NA |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | 1.013 | [1.01, 1.014] | compact-clearly-slower | NA |
| sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 1.02 | [1.016, 1.023] | compact-clearly-slower | NA |
| supported-holes | representation-and-solver-kernel | explicit-c0-flow | 2.403 | [2.157, 2.807] | compact-clearly-slower | NA |
| supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | 12.36 | [9.737, 14.35] | compact-clearly-slower | NA |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | 1.022 | [1.02, 1.025] | compact-clearly-slower | NA |
| supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | 1.034 | [1.029, 1.036] | compact-clearly-slower | NA |

Ratios are compact divided by the named explicit implementation; values below one favor compact. A crossover is emitted only after three consecutive larger measured levels favor compact and at least two corresponding confidence intervals lie wholly below one.

## Scaling and phases

Empirical exponents use one median per predeclared size level. OLS, fixed-seed bootstrap intervals, R-squared, and Theil-Sen estimates are retained in the machine-readable summary. Explicit conflict construction, biclique construction, network construction, matching or flow, recovery, completion, and verification remain separate nullable fields.

## Structural compression

| Family | Largest complete target | q | K | M | K/M |
| --- | ---: | ---: | ---: | ---: | ---: |
| comb-staircase | 8192 | 0 | 0 | 2 | NA |
| dense-conflict | 1024 | 4096 | 1052672 | 23781 | 44.27 |
| random-connected | 8192 | 165 | 2158 | 1558 | 1.385 |
| representation-crossover | 8192 | 364 | 33124 | 1095 | 30.25 |
| sparse-conflict | 8192 | 8190 | 0 | 16382 | NA |
| supported-holes | 8192 | 16382 | 0 | 32766 | NA |

K/M is descriptive structural evidence. Zero-conflict families have K=0 and therefore no positive compression ratio; dense and crossover families show whether explicit conflict materialization grows faster than the measured compressed topology.

## Dominant phases

| Family | Scope | Algorithm | Target | Dominant phase | Share |
| --- | --- | --- | ---: | --- | ---: |
| comb-staircase | representation-and-solver-kernel | compact-mrd | 8192 | biclique_construction_ns | 0.5981 |
| comb-staircase | representation-and-solver-kernel | explicit-c0-flow | 8192 | max_flow_ns | 0.672 |
| comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | explicit_conflict_construction_ns | NA |
| comb-staircase | solve-from-canonical-instance | compact-mrd | 8192 | rectangle_completion_recovery_ns | 0.5374 |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | 8192 | rectangle_completion_recovery_ns | 0.5397 |
| comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | rectangle_completion_recovery_ns | 0.5379 |
| dense-conflict | representation-and-solver-kernel | compact-mrd | 1024 | biclique_construction_ns | 0.8733 |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | 1024 | explicit_conflict_construction_ns | 0.6699 |
| dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 1024 | explicit_conflict_construction_ns | 0.99 |
| dense-conflict | solve-from-canonical-instance | compact-mrd | 1024 | geometry_preprocessing_ns | 0.5788 |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | 1024 | explicit_conflict_construction_ns | 0.3518 |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 1024 | explicit_conflict_construction_ns | 0.4195 |
| random-connected | representation-and-solver-kernel | compact-mrd | 8192 | biclique_construction_ns | 0.771 |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | 8192 | max_flow_ns | 0.6563 |
| random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | explicit_conflict_construction_ns | 0.7989 |
| random-connected | solve-from-canonical-instance | compact-mrd | 8192 | geometry_preprocessing_ns | 0.456 |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | 8192 | geometry_preprocessing_ns | 0.4956 |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | geometry_preprocessing_ns | 0.5554 |
| representation-crossover | representation-and-solver-kernel | compact-mrd | 8192 | biclique_construction_ns | 0.7073 |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | 8192 | explicit_conflict_construction_ns | 0.4035 |
| representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | explicit_conflict_construction_ns | 0.98 |
| representation-crossover | solve-from-canonical-instance | compact-mrd | 8192 | geometry_preprocessing_ns | 0.6977 |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | 8192 | geometry_preprocessing_ns | 0.6735 |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | geometry_preprocessing_ns | 0.6905 |
| sparse-conflict | representation-and-solver-kernel | compact-mrd | 8192 | biclique_construction_ns | 0.6701 |
| sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | 8192 | max_flow_ns | 0.8605 |
| sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | vertex_cover_recovery_ns | 0.5931 |
| sparse-conflict | solve-from-canonical-instance | compact-mrd | 8192 | geometry_preprocessing_ns | 0.7876 |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | 8192 | geometry_preprocessing_ns | 0.8014 |
| sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | geometry_preprocessing_ns | 0.8053 |
| supported-holes | representation-and-solver-kernel | compact-mrd | 8192 | biclique_construction_ns | 0.6735 |
| supported-holes | representation-and-solver-kernel | explicit-c0-flow | 8192 | max_flow_ns | 0.8577 |
| supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | vertex_cover_recovery_ns | 0.5967 |
| supported-holes | solve-from-canonical-instance | compact-mrd | 8192 | geometry_preprocessing_ns | 0.7757 |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | 8192 | geometry_preprocessing_ns | 0.7937 |
| supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | geometry_preprocessing_ns | 0.8006 |

## Relationship to P15

P15 measures fresh-process wall time and remains valid for reproducibility at its measured sizes. Scope A removes process creation and CLI/config/serialization overhead while retaining the solve pipeline. Scope B additionally removes common geometry and final completion/verification.

| Family | P15 fresh-process ratio | Scope A ratio | Scope B ratio | Fixed process cost masked kernel difference |
| --- | ---: | ---: | ---: | --- |
| comb-staircase | 0.9934 | 1.003 | 4.844 | true |
| dense-conflict | 1.008 | 0.8787 | 0.2711 | true |
| random-connected | 1.003 | 1.2 | 14.25 | true |
| representation-crossover | 0.9957 | 0.9942 | 0.4548 | true |
| sparse-conflict | 1.004 | 1.02 | 12.08 | true |
| supported-holes | 1 | 1.034 | 12.36 | true |

The masking indicator is a predeclared descriptive comparison: P15 lies within 5% of parity while Scope B differs from parity by more than 10%. It does not assert hardware-independent causality and does not invalidate P15.

## Claim boundary

These measurements are finite, host-specific empirical evidence. They do not prove asymptotic complexity, universal speedup, AN19 runtime, or a crossover outside the measured families and host. Scope B is not end-to-end runtime.
