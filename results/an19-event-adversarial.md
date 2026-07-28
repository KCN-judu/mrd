# AN19 exact event adversarial campaign

- Commit: `98bb6156fba5c983d8d7f8c6b37da4f26a37fb32`
- Cases: 31
- Naive reduced-class conversion survived: false
- AN19 runtime verified: false

| family | size | call | nodes | edges | original classes | reduced costs | event radii | events | comparisons | stale | Oracle |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| many_reduced_costs_few_source_lengths | 16 | 0 | 16 | 29 | 5 | 20 | 5 | 29 | 232 | 11 | true |
| many_reduced_costs_few_source_lengths | 32 | 0 | 32 | 61 | 6 | 40 | 9 | 57 | 960 | 23 | true |
| repeated_portal_splitting | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| repeated_portal_splitting | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| full_depth_persistence | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| full_depth_persistence | 16 | 1 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| full_depth_persistence | 16 | 2 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| full_depth_persistence | 16 | 3 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| full_depth_persistence | 16 | 4 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| full_depth_persistence | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| full_depth_persistence | 32 | 1 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| full_depth_persistence | 32 | 2 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| full_depth_persistence | 32 | 3 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| full_depth_persistence | 32 | 4 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| full_depth_persistence | 32 | 5 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| all_equal_reduced_keys | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| all_equal_reduced_keys | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| all_distinct_reduced_keys | 16 | 0 | 16 | 29 | 5 | 20 | 5 | 29 | 232 | 11 | true |
| all_distinct_reduced_keys | 32 | 0 | 32 | 61 | 6 | 40 | 9 | 57 | 960 | 23 | true |
| alternating_partition_contraction | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 15 | 20 | 0 | true |
| alternating_partition_contraction | 16 | 1 | 16 | 15 | 1 | 2 | 6 | 16 | 20 | 0 | true |
| alternating_partition_contraction | 16 | 2 | 16 | 15 | 1 | 2 | 6 | 16 | 20 | 0 | true |
| alternating_partition_contraction | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 29 | 65 | 0 | true |
| alternating_partition_contraction | 32 | 1 | 32 | 31 | 1 | 2 | 11 | 30 | 65 | 0 | true |
| alternating_partition_contraction | 32 | 2 | 32 | 31 | 1 | 2 | 11 | 30 | 65 | 0 | true |
| highway_halving_reorder | 16 | 0 | 16 | 15 | 2 | 3 | 5 | 14 | 20 | 0 | true |
| highway_halving_reorder | 16 | 1 | 16 | 15 | 2 | 3 | 6 | 15 | 20 | 0 | true |
| highway_halving_reorder | 32 | 0 | 32 | 31 | 2 | 3 | 11 | 29 | 77 | 0 | true |
| highway_halving_reorder | 32 | 1 | 32 | 31 | 2 | 3 | 11 | 29 | 65 | 0 | true |
| virtual_real_mixed_segments | 16 | 0 | 16 | 15 | 1 | 2 | 6 | 17 | 20 | 0 | true |
| virtual_real_mixed_segments | 32 | 0 | 32 | 31 | 1 | 2 | 11 | 32 | 65 | 0 | true |

The campaign establishes exact differential semantics on these finite fixtures. It does not prove a fixed-snapshot event bound, hierarchy-wide amortization, priority-queue bound, or the AN19 runtime.
