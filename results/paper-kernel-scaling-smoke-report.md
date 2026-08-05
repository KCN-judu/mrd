# Paper Kernel Scaling Full Report

## Scope and protocol

This campaign measures three exact implementations in one release process per family/size partition. Scope A starts from the canonical component and includes geometry, solving, completion, and verification. Scope B starts after shared geometry and chord generation and measures representation construction, matching or flow, and cover recovery only.

Source commit: `35b03a5a56c0ccb0fb20104115b710d12c7e6900`. Binary SHA-256: `c73399f8142839413b550dfa725f01a2c50afa1fd233696d02425bd6e277e4a7`. Config SHA-256: `cfb67e2f093a9523bd6f6bfc11de96f4ab63b35c2628f378e0c2aeb78edd9140`.

Host: Apple M4 on macOS-26.5-arm64-arm-64bit-Mach-O; compiler rustc 1.89.0 (29483883e 2025-08-04); power source AC.

## Correctness and coverage

The campaign contains 2048 retained measured iterations across 12 complete points. It has 0 invalid points, 0 failed production gates, 0 duplicate identities, and 0 missing planned points.

## Family-level paired results

| Family | Scope | Explicit reference | Median ratio | 95% CI | Classification | Stable crossover target |
| --- | --- | --- | ---: | ---: | --- | ---: |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | 0.05792 | [0.01431, 0.1582] | insufficient | 64 |
| dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 0.6975 | [0.08785, 2.971] | insufficient | NA |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | 0.4065 | [0.3286, 0.5064] | insufficient | 64 |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 0.9462 | [0.7722, 1.142] | insufficient | NA |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | 1.294 | [0.9326, 1.792] | insufficient | NA |
| random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | 16.2 | [13.25, 18.42] | insufficient | NA |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | 1.047 | [0.9852, 1.121] | insufficient | NA |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | 1.238 | [1.2, 1.474] | insufficient | NA |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | 0.04677 | [0.02871, 0.08907] | insufficient | 64 |
| representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | 1.232 | [0.6615, 2.268] | insufficient | NA |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | 0.9058 | [0.8996, 0.9346] | insufficient | 64 |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | 0.9971 | [0.995, 0.9987] | insufficient | NA |

Ratios are compact divided by the named explicit implementation; values below one favor compact. A crossover is emitted only after three consecutive larger measured levels favor compact and at least two corresponding confidence intervals lie wholly below one.

## Scaling and phases

Empirical exponents use one median per predeclared size level. OLS, fixed-seed bootstrap intervals, R-squared, and Theil-Sen estimates are retained in the machine-readable summary. Explicit conflict construction, biclique construction, network construction, matching or flow, recovery, completion, and verification remain separate nullable fields.

## Relationship to P15

P15 measures fresh-process wall time and remains valid for reproducibility at its measured sizes. Scope A removes process creation and CLI/config/serialization overhead while retaining the solve pipeline. Scope B additionally removes common geometry and final completion/verification. Differences between the three scopes identify whether fixed process cost masked a kernel effect; they do not invalidate or overwrite P15.

## Claim boundary

These measurements are finite, host-specific empirical evidence. They do not prove asymptotic complexity, universal speedup, AN19 runtime, or a crossover outside the measured families and host. Scope B is not end-to-end runtime.
