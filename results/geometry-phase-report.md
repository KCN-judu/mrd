# Paper Kernel Scaling Phase Diagnosis

## Scope and protocol

This campaign measures three exact implementations in one release process per family/size partition. Scope A starts from the canonical component and includes geometry, solving, completion, and verification. Scope B starts after shared geometry and chord generation and measures representation construction, matching or flow, and cover recovery only.

Input schema: 2 (`native-v2-fine-phases`). Fine-phase status: `available`.

Source commit: `211308a4981c09ccd549bd0ed322db847f427ce3`. Binary SHA-256: `59ae735a1c99726cb9d298aba1aafc71d5aae4da6e26683d248f6dddb6821ba3`. Config SHA-256: `206d1a3cab0c0d7f4d9821acf7f86ba73a4843105ed2b04bc1b928fe951fba51`.

Host: Apple M4 on macOS-26.5-arm64-arm-64bit-Mach-O; compiler rustc 1.89.0 (29483883e 2025-08-04); power source AC.

## Correctness and coverage

The campaign contains 9954 retained measured iterations across 57 analysis-eligible complete points. It has 3 censored stopped points, 0 invalid points, 0 failed production gates, 0 duplicate identities, and 0 missing planned points.

Stopped levels remain in `level_accounting` and every fit's `excluded_levels`; no stopped point contributes a timing median or exponent.

## Family-level paired results

| Backend | Family | Scope | Explicit reference | Median ratio | 95% CI | Classification | Stable crossover target |
| --- | --- | --- | --- | ---: | ---: | --- | ---: |
| prepared-exposed-edges | comb-staircase | representation-and-solver-kernel | explicit-c0-flow | 1.483 | [1.287, 1.55] | compact-clearly-slower | NA |
| prepared-exposed-edges | comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | 4.168 | [3.445, 4.959] | compact-clearly-slower | NA |
| prepared-exposed-edges | comb-staircase | solve-from-canonical-instance | explicit-c0-flow | 1.004 | [1.001, 1.009] | compact-clearly-slower | NA |
| prepared-exposed-edges | comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | 1.009 | [1.003, 1.021] | compact-clearly-slower | NA |
| prepared-exposed-edges | dense-conflict | representation-and-solver-kernel | explicit-c0-flow | 0.0831 | [0.01379, 0.261] | compact-clearly-faster | 16 |
| prepared-exposed-edges | dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 1.111 | [0.08672, 5.523] | unresolved | 256 |
| prepared-exposed-edges | dense-conflict | solve-from-canonical-instance | explicit-c0-flow | 0.315 | [0.2092, 0.5045] | compact-clearly-faster | 16 |
| prepared-exposed-edges | dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 1.03 | [0.6439, 1.421] | unresolved | 256 |
| prepared-exposed-edges | random-connected | representation-and-solver-kernel | explicit-c0-flow | 1.03 | [0.6645, 1.679] | unresolved | 512 |
| prepared-exposed-edges | random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | 13.04 | [9.45, 16.05] | compact-clearly-slower | NA |
| prepared-exposed-edges | random-connected | solve-from-canonical-instance | explicit-c0-flow | 1.001 | [0.8711, 1.102] | unresolved | 512 |
| prepared-exposed-edges | random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | 1.303 | [1.232, 1.357] | compact-clearly-slower | NA |
| prepared-exposed-edges | representation-crossover | representation-and-solver-kernel | explicit-c0-flow | 0.03877 | [0.01323, 0.09077] | compact-clearly-faster | 16 |
| prepared-exposed-edges | representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | 0.9717 | [0.2245, 2.303] | unresolved | 512 |
| prepared-exposed-edges | representation-crossover | solve-from-canonical-instance | explicit-c0-flow | 0.8309 | [0.8096, 0.8758] | compact-clearly-faster | 16 |
| prepared-exposed-edges | representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | 1.004 | [0.9902, 1.01] | unresolved | 2048 |
| prepared-exposed-edges | sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | 2.181 | [1.885, 2.708] | compact-clearly-slower | NA |
| prepared-exposed-edges | sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 9.812 | [7.611, 13.5] | compact-clearly-slower | NA |
| prepared-exposed-edges | sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | 1.016 | [1.013, 1.019] | compact-clearly-slower | NA |
| prepared-exposed-edges | sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 1.027 | [1.025, 1.034] | compact-clearly-slower | NA |
| prepared-exposed-edges | supported-holes | representation-and-solver-kernel | explicit-c0-flow | 2.349 | [2.029, 2.788] | compact-clearly-slower | NA |
| prepared-exposed-edges | supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | 11.49 | [8.5, 13.84] | compact-clearly-slower | NA |
| prepared-exposed-edges | supported-holes | solve-from-canonical-instance | explicit-c0-flow | 1.028 | [1.026, 1.031] | compact-clearly-slower | NA |
| prepared-exposed-edges | supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | 1.05 | [1.048, 1.052] | compact-clearly-slower | NA |

Ratios are compact divided by the named explicit implementation; values below one favor compact. A crossover is emitted only after three consecutive larger measured levels favor compact and at least two corresponding confidence intervals lie wholly below one.

## Scaling and phases

Empirical exponents use one median per predeclared size level. The JSON and CSV retain total-time fits and per-leaf-phase fits against N, B, r, H, V, q, K, and M, including OLS, fixed-seed bootstrap intervals, R-squared, Theil-Sen, and explicit exclusions. A fit is not estimated with fewer than six valid target-size levels.

## Structural compression

| Backend | Family | Largest complete target | q | K | M | K/M |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| prepared-exposed-edges | comb-staircase | 8192 | 0 | 0 | 2 | NA |
| prepared-exposed-edges | dense-conflict | 1024 | 4096 | 1052672 | 23781 | 44.27 |
| prepared-exposed-edges | random-connected | 8192 | 165 | 2158 | 1558 | 1.385 |
| prepared-exposed-edges | representation-crossover | 8192 | 364 | 33124 | 1095 | 30.25 |
| prepared-exposed-edges | sparse-conflict | 8192 | 8190 | 0 | 16382 | NA |
| prepared-exposed-edges | supported-holes | 8192 | 16382 | 0 | 32766 | NA |

K/M is descriptive structural evidence. Zero-conflict families have K=0 and therefore no positive compression ratio; dense and crossover families show whether explicit conflict materialization grows faster than the measured compressed topology.

## Phase diagnosis

Dominance uses mutually disjoint leaf medians for per-run scopes. Shared preprocessing uses its recorded geometry and chord parent totals; enclosing scope totals and unattributed measurement overhead are never candidates.

| Backend | Family | Scope | Algorithm | Target | Dominant phase | Operation | Share | Best variable | OLS slope (95% CI) | Theil-Sen | R2 | Levels | Cost assessment | Unattributed larger? |
| --- | --- | --- | --- | ---: | --- | --- | ---: | --- | --- | ---: | ---: | ---: | --- | --- |
| prepared-exposed-edges | comb-staircase | campaign-setup | shared | 8192 | connected_component_extraction_ns | campaign-input-setup | 0.9571 | N | 0.6706 [0.5404, 0.7731] | 0.6382 | 0.9672 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | comb-staircase | representation-and-solver-kernel | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.4296 | M | 0.7648 [0.7165, 0.7835] | 0.7735 | 0.997 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | comb-staircase | representation-and-solver-kernel | explicit-c0-flow | 8192 | matching_or_flow_ns | representation-or-solver-kernel | 0.3765 | M | 0.7544 [0.7384, 0.7724] | 0.7562 | 0.9988 | 10 | size-associated-in-measured-range | True |
| prepared-exposed-edges | comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | conflict_discovery_ns | indexed-or-pairwise-conflict-discovery | NA | H | 0.01243 [0, 0.02265] | 0.01097 | 0.8026 | 6 | weak-size-dependence-consistent-with-fixed-cost-over-measured-range | True |
| prepared-exposed-edges | comb-staircase | shared-preprocessing | shared | 8192 | geometry_preprocessing_ns | geometry-copying-or-index-preparation | 0.9652 | H | 1.724 [1.571, 1.858] | 1.754 | 0.9944 | 8 | size-associated-in-measured-range | NA |
| prepared-exposed-edges | comb-staircase | solve-from-canonical-instance | compact-mrd | 8192 | canonical_component_clone_ns | geometry-copying-or-index-preparation | 0.8003 | H | 1.472 [1.086, 1.82] | 1.486 | 0.9592 | 7 | size-associated-in-measured-range | True |
| prepared-exposed-edges | comb-staircase | solve-from-canonical-instance | explicit-c0-flow | 8192 | canonical_component_clone_ns | geometry-copying-or-index-preparation | 0.7583 | N | 0.76 [0.6037, 0.9418] | 0.7812 | 0.9404 | 10 | size-associated-in-measured-range | True |
| prepared-exposed-edges | comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | canonical_component_clone_ns | geometry-copying-or-index-preparation | 0.9047 | N | 0.778 [0.5984, 1.026] | 0.8036 | 0.9163 | 10 | size-associated-in-measured-range | True |
| prepared-exposed-edges | dense-conflict | campaign-setup | shared | 1024 | connected_component_extraction_ns | campaign-input-setup | 0.9975 | N | 0.9578 [0.9144, 0.998] | 0.9544 | 0.999 | 7 | size-associated-in-measured-range | False |
| prepared-exposed-edges | dense-conflict | representation-and-solver-kernel | compact-mrd | 1024 | representation_construction_ns | representation-or-solver-kernel | 0.8756 | H | 1.045 [0.9559, 1.125] | 1.044 | 0.9963 | 7 | size-associated-in-measured-range | False |
| prepared-exposed-edges | dense-conflict | representation-and-solver-kernel | explicit-c0-flow | 1024 | representation_construction_ns | representation-or-solver-kernel | 0.6666 | N | 1.114 [1.096, 1.13] | 1.107 | 0.9998 | 7 | size-associated-in-measured-range | False |
| prepared-exposed-edges | dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 1024 | conflict_discovery_ns | indexed-or-pairwise-conflict-discovery | 0.9898 | N | 1.35 [1.271, 1.42] | 1.359 | 0.9983 | 7 | size-associated-in-measured-range | False |
| prepared-exposed-edges | dense-conflict | shared-preprocessing | shared | 1024 | geometry_preprocessing_ns | geometry-copying-or-index-preparation | 0.9819 | N | 0.7023 [0.6451, 0.7554] | 0.6969 | 0.9964 | 7 | size-associated-in-measured-range | NA |
| prepared-exposed-edges | dense-conflict | solve-from-canonical-instance | compact-mrd | 1024 | representation_construction_ns | representation-or-solver-kernel | 0.6072 | H | 1.058 [0.9688, 1.136] | 1.054 | 0.9963 | 7 | size-associated-in-measured-range | False |
| prepared-exposed-edges | dense-conflict | solve-from-canonical-instance | explicit-c0-flow | 1024 | representation_construction_ns | representation-or-solver-kernel | 0.6633 | N | 1.113 [1.095, 1.129] | 1.106 | 0.9998 | 7 | size-associated-in-measured-range | False |
| prepared-exposed-edges | dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 1024 | conflict_discovery_ns | indexed-or-pairwise-conflict-discovery | 0.9876 | N | 1.35 [1.271, 1.42] | 1.359 | 0.9983 | 7 | size-associated-in-measured-range | False |
| prepared-exposed-edges | random-connected | campaign-setup | shared | 8192 | instance_generation_ns | campaign-input-setup | 0.648 | N | 0.7895 [0.3587, 1.196] | 0.9257 | 0.7671 | 10 | fixed-versus-size-dependent-unresolved | False |
| prepared-exposed-edges | random-connected | representation-and-solver-kernel | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.7495 | M | 1.089 [1.034, 1.173] | 1.051 | 0.9945 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | random-connected | representation-and-solver-kernel | explicit-c0-flow | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.6604 | K | 0.9911 [0.9615, 1.026] | 1.004 | 0.999 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | conflict_discovery_ns | indexed-or-pairwise-conflict-discovery | 0.7404 | K | 0.9236 [0.822, 1.051] | 0.9983 | 0.9857 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | random-connected | shared-preprocessing | shared | 8192 | geometry_preprocessing_ns | geometry-copying-or-index-preparation | 0.9214 | N | 0.5549 [0.4864, 0.6215] | 0.5601 | 0.9852 | 10 | size-associated-in-measured-range | NA |
| prepared-exposed-edges | random-connected | solve-from-canonical-instance | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.7578 | K | 0.7292 [0.6789, 0.765] | 0.7305 | 0.995 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | random-connected | solve-from-canonical-instance | explicit-c0-flow | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.6599 | K | 0.9917 [0.9619, 1.027] | 1.007 | 0.9989 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | conflict_discovery_ns | indexed-or-pairwise-conflict-discovery | 0.7153 | K | 0.8819 [0.7838, 1.041] | 0.9827 | 0.9813 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | representation-crossover | campaign-setup | shared | 8192 | connected_component_extraction_ns | campaign-input-setup | 0.9968 | N | 0.9625 [0.8545, 1.048] | 0.9686 | 0.9807 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | representation-crossover | representation-and-solver-kernel | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.7092 | q | 0.9448 [0.9106, 1.011] | 0.9439 | 0.9947 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | representation-crossover | representation-and-solver-kernel | explicit-c0-flow | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.7525 | M | 2.127 [2.118, 2.134] | 2.127 | 1 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | conflict_discovery_ns | indexed-or-pairwise-conflict-discovery | 0.9801 | N | 1.346 [1.294, 1.386] | 1.342 | 0.9988 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | representation-crossover | shared-preprocessing | shared | 8192 | geometry_preprocessing_ns | geometry-copying-or-index-preparation | 0.9976 | N | 1.047 [1.018, 1.073] | 1.036 | 0.9992 | 10 | size-associated-in-measured-range | NA |
| prepared-exposed-edges | representation-crossover | solve-from-canonical-instance | compact-mrd | 8192 | canonical_component_clone_ns | geometry-copying-or-index-preparation | 0.6261 | N | 1.044 [0.9788, 1.113] | 1.039 | 0.9947 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | representation-crossover | solve-from-canonical-instance | explicit-c0-flow | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.744 | M | 2.127 [2.115, 2.136] | 2.127 | 1 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | conflict_discovery_ns | indexed-or-pairwise-conflict-discovery | 0.8762 | N | 1.342 [1.285, 1.387] | 1.336 | 0.9986 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | sparse-conflict | campaign-setup | shared | 8192 | connected_component_extraction_ns | campaign-input-setup | 0.9932 | N | 0.8672 [0.8058, 0.9262] | 0.881 | 0.9954 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | sparse-conflict | representation-and-solver-kernel | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.6703 | N | 1.024 [0.9774, 1.074] | 1.013 | 0.9975 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | 8192 | matching_or_flow_ns | representation-or-solver-kernel | 0.8296 | N | 0.9153 [0.8878, 0.9504] | 0.9298 | 0.9988 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | minimum_vertex_cover_recovery_ns | representation-or-solver-kernel | 0.5675 | N | 0.8389 [0.7766, 0.9122] | 0.8563 | 0.9942 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | sparse-conflict | shared-preprocessing | shared | 8192 | geometry_preprocessing_ns | geometry-copying-or-index-preparation | 0.9739 | N | 1.053 [1.035, 1.074] | 1.042 | 0.9995 | 10 | size-associated-in-measured-range | NA |
| prepared-exposed-edges | sparse-conflict | solve-from-canonical-instance | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.6433 | N | 1.022 [0.9735, 1.072] | 1.007 | 0.9975 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | 8192 | matching_or_flow_ns | representation-or-solver-kernel | 0.7547 | N | 0.9125 [0.878, 0.953] | 0.9268 | 0.9984 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | minimum_vertex_cover_recovery_ns | representation-or-solver-kernel | 0.3717 | N | 0.8467 [0.78, 0.9239] | 0.8646 | 0.9937 | 10 | size-associated-in-measured-range | True |
| prepared-exposed-edges | supported-holes | campaign-setup | shared | 8192 | connected_component_extraction_ns | campaign-input-setup | 0.9922 | N | 0.8866 [0.8394, 0.9655] | 0.893 | 0.9927 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | supported-holes | representation-and-solver-kernel | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.6764 | N | 1.065 [1.052, 1.093] | 1.056 | 0.9989 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | supported-holes | representation-and-solver-kernel | explicit-c0-flow | 8192 | matching_or_flow_ns | representation-or-solver-kernel | 0.8308 | N | 0.9404 [0.9124, 0.9733] | 0.9571 | 0.9988 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | 8192 | minimum_vertex_cover_recovery_ns | representation-or-solver-kernel | 0.5779 | N | 0.8936 [0.8496, 0.9413] | 0.9017 | 0.9975 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | supported-holes | shared-preprocessing | shared | 8192 | geometry_preprocessing_ns | geometry-copying-or-index-preparation | 0.9425 | N | 1.046 [1.023, 1.071] | 1.036 | 0.9994 | 10 | size-associated-in-measured-range | NA |
| prepared-exposed-edges | supported-holes | solve-from-canonical-instance | compact-mrd | 8192 | representation_construction_ns | representation-or-solver-kernel | 0.657 | N | 1.067 [1.041, 1.099] | 1.059 | 0.9986 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | supported-holes | solve-from-canonical-instance | explicit-c0-flow | 8192 | matching_or_flow_ns | representation-or-solver-kernel | 0.7653 | N | 0.943 [0.9104, 0.9786] | 0.955 | 0.9986 | 10 | size-associated-in-measured-range | False |
| prepared-exposed-edges | supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | 8192 | minimum_vertex_cover_recovery_ns | representation-or-solver-kernel | 0.3973 | N | 0.8992 [0.8561, 0.9475] | 0.9177 | 0.9974 | 10 | size-associated-in-measured-range | True |

The highest-R2 variable is a descriptive ranking among correlated structural measures, not a causal identification. The fixed-overhead assessment in JSON and CSV is likewise limited to the measured range.

The dominant_phase_variation records show that at least one backend/scope/algorithm group changes dominant phase across families.

## Paired before/after comparison

57 complete point pairs passed canonical-instance, structural, and optimum equality gates. Structural mismatches: 0; objective mismatches: 0; stop/censoring changes: 0.

| Family | Scope | Algorithm | Phase | Levels | Median speedup | 95% CI | Status |
| --- | --- | --- | --- | ---: | ---: | ---: | --- |
| comb-staircase | representation-and-solver-kernel | compact-mrd | matching_or_flow_ns | 10 | 0.9998 | [0.9508, 1] | estimated |
| comb-staircase | representation-and-solver-kernel | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1] | estimated |
| comb-staircase | representation-and-solver-kernel | compact-mrd | representation_construction_ns | 10 | 1 | [1, 1.037] | estimated |
| comb-staircase | representation-and-solver-kernel | compact-mrd | total_elapsed_ns | 10 | 0.9996 | [0.9805, 1.028] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [1, 1.006] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-c0-flow | matching_or_flow_ns | 10 | 1 | [0.9508, 1] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-c0-flow | representation_construction_ns | 10 | 1 | [1, 1.374] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-c0-flow | total_elapsed_ns | 10 | 0.9995 | [0.9664, 1.021] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [1, 1] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 1 | [1, 1.002] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9966, 1.001] | estimated |
| comb-staircase | representation-and-solver-kernel | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1 | [0.9613, 1.05] | estimated |
| comb-staircase | solve-from-canonical-instance | compact-mrd | canonical_component_clone_ns | 10 | 1 | [1, 1.098] | estimated |
| comb-staircase | solve-from-canonical-instance | compact-mrd | matching_or_flow_ns | 10 | 1 | [0.9508, 1.026] | estimated |
| comb-staircase | solve-from-canonical-instance | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1] | estimated |
| comb-staircase | solve-from-canonical-instance | compact-mrd | representation_construction_ns | 10 | 1 | [1, 1.027] | estimated |
| comb-staircase | solve-from-canonical-instance | compact-mrd | total_elapsed_ns | 10 | 1.15 | [1.141, 1.292] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | canonical_component_clone_ns | 10 | 1 | [1, 1.062] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [1, 1.006] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | matching_or_flow_ns | 10 | 1 | [0.9496, 1.025] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.994, 1] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | representation_construction_ns | 10 | 1 | [1, 1.253] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-c0-flow | total_elapsed_ns | 10 | 1.149 | [1.144, 1.279] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | canonical_component_clone_ns | 10 | 1 | [1, 1.074] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [1, 1] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 1 | [0.9525, 1.142] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1.006] | estimated |
| comb-staircase | solve-from-canonical-instance | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1.147 | [1.143, 1.286] | estimated |
| dense-conflict | representation-and-solver-kernel | compact-mrd | matching_or_flow_ns | 7 | 0.9865 | [0.9727, 1.001] | estimated |
| dense-conflict | representation-and-solver-kernel | compact-mrd | minimum_vertex_cover_recovery_ns | 7 | 0.9986 | [0.9547, 1.006] | estimated |
| dense-conflict | representation-and-solver-kernel | compact-mrd | representation_construction_ns | 7 | 0.9997 | [0.9857, 1.001] | estimated |
| dense-conflict | representation-and-solver-kernel | compact-mrd | total_elapsed_ns | 7 | 0.9878 | [0.9829, 1.001] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | conflict_discovery_ns | 7 | 0.9958 | [0.9728, 1.003] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | matching_or_flow_ns | 7 | 0.9898 | [0.9785, 1.017] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 7 | 0.9753 | [0.9258, 1.008] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | representation_construction_ns | 7 | 0.9877 | [0.9818, 0.9986] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-c0-flow | total_elapsed_ns | 7 | 0.9967 | [0.976, 1.001] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | conflict_discovery_ns | 7 | 0.9987 | [0.9637, 1.003] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | matching_or_flow_ns | 7 | 1 | [0.9888, 1.013] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 7 | 1 | [0.9225, 1.135] | estimated |
| dense-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | total_elapsed_ns | 7 | 1 | [0.9662, 1.001] | estimated |
| dense-conflict | solve-from-canonical-instance | compact-mrd | canonical_component_clone_ns | 7 | 1.021 | [1, 1.058] | estimated |
| dense-conflict | solve-from-canonical-instance | compact-mrd | matching_or_flow_ns | 7 | 0.996 | [0.9856, 0.9994] | estimated |
| dense-conflict | solve-from-canonical-instance | compact-mrd | minimum_vertex_cover_recovery_ns | 7 | 1 | [0.9895, 1] | estimated |
| dense-conflict | solve-from-canonical-instance | compact-mrd | representation_construction_ns | 7 | 0.9942 | [0.991, 0.9992] | estimated |
| dense-conflict | solve-from-canonical-instance | compact-mrd | total_elapsed_ns | 7 | 1.705 | [1.391, 1.838] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | canonical_component_clone_ns | 7 | 1 | [0.9865, 1.002] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | conflict_discovery_ns | 7 | 0.9991 | [0.9833, 1.003] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | matching_or_flow_ns | 7 | 1.011 | [0.9876, 1.03] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 7 | 0.972 | [0.9672, 1] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | representation_construction_ns | 7 | 0.985 | [0.9828, 1.008] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-c0-flow | total_elapsed_ns | 7 | 1.196 | [1.173, 1.199] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | canonical_component_clone_ns | 7 | 1.026 | [0.989, 1.088] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | conflict_discovery_ns | 7 | 0.9948 | [0.9636, 1.002] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | matching_or_flow_ns | 7 | 1.013 | [1, 1.021] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 7 | 1 | [1, 1.164] | estimated |
| dense-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | total_elapsed_ns | 7 | 1.555 | [1.47, 1.672] | estimated |
| random-connected | representation-and-solver-kernel | compact-mrd | matching_or_flow_ns | 10 | 1.012 | [0.956, 1.052] | estimated |
| random-connected | representation-and-solver-kernel | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9976, 1.019] | estimated |
| random-connected | representation-and-solver-kernel | compact-mrd | representation_construction_ns | 10 | 1.016 | [0.9931, 1.055] | estimated |
| random-connected | representation-and-solver-kernel | compact-mrd | total_elapsed_ns | 10 | 1.018 | [0.9777, 1.049] | estimated |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [1, 1.028] | estimated |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | matching_or_flow_ns | 10 | 1 | [0.9561, 1.04] | estimated |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1] | estimated |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | representation_construction_ns | 10 | 1.009 | [0.9865, 1.036] | estimated |
| random-connected | representation-and-solver-kernel | explicit-c0-flow | total_elapsed_ns | 10 | 1.017 | [0.9754, 1.044] | estimated |
| random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [0.9948, 1.024] | estimated |
| random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 1 | [0.9748, 1.042] | estimated |
| random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.997, 1.14] | estimated |
| random-connected | representation-and-solver-kernel | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1.002 | [0.9893, 1.041] | estimated |
| random-connected | solve-from-canonical-instance | compact-mrd | canonical_component_clone_ns | 10 | 1 | [0.9996, 1.01] | estimated |
| random-connected | solve-from-canonical-instance | compact-mrd | matching_or_flow_ns | 10 | 1.025 | [0.9556, 1.093] | estimated |
| random-connected | solve-from-canonical-instance | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9552, 1.05] | estimated |
| random-connected | solve-from-canonical-instance | compact-mrd | representation_construction_ns | 10 | 1.015 | [0.995, 1.072] | estimated |
| random-connected | solve-from-canonical-instance | compact-mrd | total_elapsed_ns | 10 | 1.364 | [1.201, 1.561] | estimated |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | canonical_component_clone_ns | 10 | 1 | [0.994, 1.021] | estimated |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [0.9976, 1.039] | estimated |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | matching_or_flow_ns | 10 | 1.002 | [0.9586, 1.033] | estimated |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.994, 1.002] | estimated |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | representation_construction_ns | 10 | 1.011 | [0.9617, 1.037] | estimated |
| random-connected | solve-from-canonical-instance | explicit-c0-flow | total_elapsed_ns | 10 | 1.357 | [1.221, 1.467] | estimated |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | canonical_component_clone_ns | 10 | 1 | [0.994, 1.017] | estimated |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [0.9917, 1.023] | estimated |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 1 | [0.9754, 1.023] | estimated |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1.216] | estimated |
| random-connected | solve-from-canonical-instance | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1.48 | [1.331, 1.731] | estimated |
| representation-crossover | representation-and-solver-kernel | compact-mrd | matching_or_flow_ns | 10 | 0.9622 | [0.9484, 0.9833] | estimated |
| representation-crossover | representation-and-solver-kernel | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9453, 1] | estimated |
| representation-crossover | representation-and-solver-kernel | compact-mrd | representation_construction_ns | 10 | 0.9551 | [0.8956, 0.9751] | estimated |
| representation-crossover | representation-and-solver-kernel | compact-mrd | total_elapsed_ns | 10 | 0.9447 | [0.9141, 0.9702] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | conflict_discovery_ns | 10 | 0.9801 | [0.974, 0.9957] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | matching_or_flow_ns | 10 | 0.9451 | [0.9211, 0.9855] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 0.9361 | [0.8966, 1] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | representation_construction_ns | 10 | 0.9775 | [0.9616, 0.9886] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-c0-flow | total_elapsed_ns | 10 | 0.9712 | [0.9642, 0.9807] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 0.9729 | [0.964, 0.9923] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 0.9723 | [0.9393, 1] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 0.8256 | [0.6135, 1] | estimated |
| representation-crossover | representation-and-solver-kernel | explicit-hopcroft-karp | total_elapsed_ns | 10 | 0.9798 | [0.9701, 0.9843] | estimated |
| representation-crossover | solve-from-canonical-instance | compact-mrd | canonical_component_clone_ns | 10 | 0.8956 | [0.8198, 0.9328] | estimated |
| representation-crossover | solve-from-canonical-instance | compact-mrd | matching_or_flow_ns | 10 | 0.9648 | [0.95, 0.9872] | estimated |
| representation-crossover | solve-from-canonical-instance | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 0.947 | [0.7802, 1] | estimated |
| representation-crossover | solve-from-canonical-instance | compact-mrd | representation_construction_ns | 10 | 0.9747 | [0.9509, 0.9861] | estimated |
| representation-crossover | solve-from-canonical-instance | compact-mrd | total_elapsed_ns | 10 | 1.808 | [1.763, 1.852] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | canonical_component_clone_ns | 10 | 0.8781 | [0.8357, 0.9421] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | conflict_discovery_ns | 10 | 0.9827 | [0.9751, 0.9922] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | matching_or_flow_ns | 10 | 0.9655 | [0.9261, 0.9886] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 0.929 | [0.8805, 1] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | representation_construction_ns | 10 | 0.9816 | [0.9641, 0.9841] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-c0-flow | total_elapsed_ns | 10 | 1.667 | [1.613, 1.738] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | canonical_component_clone_ns | 10 | 0.869 | [0.8161, 0.9454] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 0.9805 | [0.975, 0.9965] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 0.9652 | [0.9234, 1] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 0.8261 | [0.664, 1] | estimated |
| representation-crossover | solve-from-canonical-instance | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1.812 | [1.757, 1.868] | estimated |
| sparse-conflict | representation-and-solver-kernel | compact-mrd | matching_or_flow_ns | 10 | 1.036 | [1.009, 1.052] | estimated |
| sparse-conflict | representation-and-solver-kernel | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1.023] | estimated |
| sparse-conflict | representation-and-solver-kernel | compact-mrd | representation_construction_ns | 10 | 1.002 | [0.9814, 1.018] | estimated |
| sparse-conflict | representation-and-solver-kernel | compact-mrd | total_elapsed_ns | 10 | 1.011 | [0.983, 1.021] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [1, 1.001] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | matching_or_flow_ns | 10 | 1.037 | [1.014, 1.05] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1 | [1, 1.001] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | representation_construction_ns | 10 | 1.005 | [1, 1.062] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-c0-flow | total_elapsed_ns | 10 | 1.031 | [1.011, 1.05] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [1, 1.042] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 1.013 | [1, 1.048] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 1.011 | [1, 1.067] | estimated |
| sparse-conflict | representation-and-solver-kernel | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1.023 | [1.006, 1.046] | estimated |
| sparse-conflict | solve-from-canonical-instance | compact-mrd | canonical_component_clone_ns | 10 | 1.056 | [1, 1.112] | estimated |
| sparse-conflict | solve-from-canonical-instance | compact-mrd | matching_or_flow_ns | 10 | 1.025 | [1.009, 1.054] | estimated |
| sparse-conflict | solve-from-canonical-instance | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1.027 | [1, 1.165] | estimated |
| sparse-conflict | solve-from-canonical-instance | compact-mrd | representation_construction_ns | 10 | 1.006 | [1, 1.018] | estimated |
| sparse-conflict | solve-from-canonical-instance | compact-mrd | total_elapsed_ns | 10 | 1.297 | [1.277, 1.31] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | canonical_component_clone_ns | 10 | 1.056 | [1, 1.141] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [1, 1.168] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | matching_or_flow_ns | 10 | 1.034 | [1.015, 1.052] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1.008 | [1, 1.048] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | representation_construction_ns | 10 | 1.019 | [1, 1.095] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-c0-flow | total_elapsed_ns | 10 | 1.301 | [1.277, 1.317] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | canonical_component_clone_ns | 10 | 1.086 | [1, 1.153] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [1, 1.128] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 1.071 | [1, 1.107] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 1.018 | [1, 1.067] | estimated |
| sparse-conflict | solve-from-canonical-instance | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1.306 | [1.283, 1.323] | estimated |
| supported-holes | representation-and-solver-kernel | compact-mrd | matching_or_flow_ns | 10 | 0.9843 | [0.974, 0.9998] | estimated |
| supported-holes | representation-and-solver-kernel | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9906, 1] | estimated |
| supported-holes | representation-and-solver-kernel | compact-mrd | representation_construction_ns | 10 | 0.9858 | [0.9797, 0.9938] | estimated |
| supported-holes | representation-and-solver-kernel | compact-mrd | total_elapsed_ns | 10 | 0.9874 | [0.9722, 0.995] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [0.9833, 1] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-c0-flow | matching_or_flow_ns | 10 | 0.9865 | [0.9741, 0.9998] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9877, 1] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-c0-flow | representation_construction_ns | 10 | 0.993 | [0.9817, 1] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-c0-flow | total_elapsed_ns | 10 | 0.9856 | [0.9785, 0.9965] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [0.9918, 1] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 0.9828 | [0.9639, 1] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 0.9971 | [0.9887, 1.001] | estimated |
| supported-holes | representation-and-solver-kernel | explicit-hopcroft-karp | total_elapsed_ns | 10 | 0.9822 | [0.9759, 0.9989] | estimated |
| supported-holes | solve-from-canonical-instance | compact-mrd | canonical_component_clone_ns | 10 | 1 | [0.979, 1.033] | estimated |
| supported-holes | solve-from-canonical-instance | compact-mrd | matching_or_flow_ns | 10 | 0.9821 | [0.9749, 1.005] | estimated |
| supported-holes | solve-from-canonical-instance | compact-mrd | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9803, 1.008] | estimated |
| supported-holes | solve-from-canonical-instance | compact-mrd | representation_construction_ns | 10 | 0.983 | [0.9774, 0.9895] | estimated |
| supported-holes | solve-from-canonical-instance | compact-mrd | total_elapsed_ns | 10 | 1.392 | [1.359, 1.41] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | canonical_component_clone_ns | 10 | 1 | [0.9857, 1.03] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | conflict_discovery_ns | 10 | 1 | [0.9733, 1.033] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | matching_or_flow_ns | 10 | 0.9822 | [0.9698, 1.002] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | minimum_vertex_cover_recovery_ns | 10 | 1 | [0.9797, 1.003] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | representation_construction_ns | 10 | 0.999 | [0.9818, 1.027] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-c0-flow | total_elapsed_ns | 10 | 1.397 | [1.381, 1.418] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | canonical_component_clone_ns | 10 | 1 | [0.9742, 1.047] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | conflict_discovery_ns | 10 | 1 | [1, 1.135] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | matching_or_flow_ns | 10 | 1.003 | [0.9457, 1.041] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | minimum_vertex_cover_recovery_ns | 10 | 1.001 | [0.979, 1.021] | estimated |
| supported-holes | solve-from-canonical-instance | explicit-hopcroft-karp | total_elapsed_ns | 10 | 1.413 | [1.382, 1.43] | estimated |

Speedups are paired on family, target, seed, canonical instance, scope, algorithm, and common iteration. They are empirical and host-specific.

## Relationship to P15

P15 measures fresh-process wall time and remains valid for reproducibility at its measured sizes. Scope A removes process creation and CLI/config/serialization overhead while retaining the solve pipeline. Scope B additionally removes common geometry and final completion/verification.

| Backend | Family | P15 fresh-process ratio | Scope A ratio | Scope B ratio | Fixed process cost masked kernel difference |
| --- | --- | ---: | ---: | ---: | --- |
| prepared-exposed-edges | comb-staircase | 0.9934 | 1.009 | 4.168 | true |
| prepared-exposed-edges | dense-conflict | 1.008 | 1.03 | 1.111 | true |
| prepared-exposed-edges | random-connected | 1.003 | 1.303 | 13.04 | true |
| prepared-exposed-edges | representation-crossover | 0.9957 | 1.004 | 0.9717 | false |
| prepared-exposed-edges | sparse-conflict | 1.004 | 1.027 | 9.812 | true |
| prepared-exposed-edges | supported-holes | 1 | 1.05 | 11.49 | true |

The masking indicator is a predeclared descriptive comparison: P15 lies within 5% of parity while Scope B differs from parity by more than 10%. It does not assert hardware-independent causality and does not invalidate P15.

## Claim boundary

- No exponent is estimated from fewer than six valid, distinct target-size levels.
- Log-log fits are empirical descriptions of the recorded families, host, compiler, and measured range; they are not complexity proofs.
- The variable with greatest R-squared is descriptive, not causal, because N, B, r, H, V, q, K, and M may be correlated.
- Stopped levels are censored and excluded from fits; they are never converted into timing observations.
- Coarse aliases and enclosing totals are excluded from per-run dominant-phase sums; shared preprocessing instead compares its two disjoint recorded parent phases.

These measurements do not prove asymptotic complexity, universal speedup, AN19 runtime, or a crossover outside the measured families and host. Scope B is not end-to-end runtime.
