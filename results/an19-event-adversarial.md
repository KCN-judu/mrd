# AN19 exact event adversarial campaign

- Commit: `fbc869eef5b43aaa3d7c66331a5bccc1e54b71dd`
- Cases: 31
- Naive reduced-class conversion survived: false
- Fixed-snapshot event-cardinality bound proved: true
- Practical stable-binary-heap comparison bound certified: true
- Priority-queue comparison bound proved: false
- AN19 runtime verified: false

| family | size | call | nodes | edges | original classes | reduced costs | event radii | events | comparisons | practical bound | stale | Oracle |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| many_reduced_costs_few_source_lengths | 16 | 0 | 16 | 29 | 5 | 20 | 5 | 29 | 169 | 463 | 11 | true |
| many_reduced_costs_few_source_lengths | 32 | 0 | 32 | 61 | 6 | 40 | 9 | 57 | 473 | 1112 | 23 | true |
| repeated_portal_splitting | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| repeated_portal_splitting | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| full_depth_persistence | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| full_depth_persistence | 16 | 1 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| full_depth_persistence | 16 | 2 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| full_depth_persistence | 16 | 3 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| full_depth_persistence | 16 | 4 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| full_depth_persistence | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| full_depth_persistence | 32 | 1 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| full_depth_persistence | 32 | 2 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| full_depth_persistence | 32 | 3 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| full_depth_persistence | 32 | 4 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| full_depth_persistence | 32 | 5 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| all_equal_reduced_keys | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| all_equal_reduced_keys | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| all_distinct_reduced_keys | 16 | 0 | 16 | 29 | 5 | 20 | 5 | 29 | 169 | 463 | 11 | true |
| all_distinct_reduced_keys | 32 | 0 | 32 | 61 | 6 | 40 | 9 | 57 | 473 | 1112 | 23 | true |
| alternating_partition_contraction | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 19 | 222 | 0 | true |
| alternating_partition_contraction | 16 | 1 | 16 | 15 | 1 | 2 | 6 | 16 | 19 | 222 | 0 | true |
| alternating_partition_contraction | 16 | 2 | 16 | 15 | 1 | 2 | 6 | 16 | 19 | 222 | 0 | true |
| alternating_partition_contraction | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 49 | 542 | 0 | true |
| alternating_partition_contraction | 32 | 1 | 32 | 31 | 1 | 2 | 11 | 30 | 49 | 542 | 0 | true |
| alternating_partition_contraction | 32 | 2 | 32 | 31 | 1 | 2 | 11 | 30 | 49 | 542 | 0 | true |
| highway_halving_reorder | 16 | 0 | 16 | 15 | 2 | 3 | 5 | 14 | 19 | 222 | 0 | true |
| highway_halving_reorder | 16 | 1 | 16 | 15 | 2 | 3 | 6 | 15 | 19 | 222 | 0 | true |
| highway_halving_reorder | 32 | 0 | 32 | 31 | 2 | 3 | 11 | 29 | 58 | 542 | 0 | true |
| highway_halving_reorder | 32 | 1 | 32 | 31 | 2 | 3 | 11 | 29 | 49 | 542 | 0 | true |
| virtual_real_mixed_segments | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 17 | 19 | 222 | 0 | true |
| virtual_real_mixed_segments | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 32 | 49 | 542 | 0 | true |

Each run carries a verified fixed-snapshot certificate: semantic events are at most 3n + 4m + 2 and queue insertions/pops are at most n + 2m + 2. Each reduced-engine run separately certifies the practical stable binary heap bound 3 I ceil(log2(max(I, 1))) + 2m on its counted heap and relaxation-label comparisons; Oracle runs do not carry that implementation certificate. This is an O((n+m) log(n+m)) practical bound, not the source-equivalent O(m+n log log n) priority-queue proof, hierarchy-wide amortization, or AN19 runtime.
