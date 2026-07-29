//! Output-sensitive path-tree implementations.

use super::{BoundaryInterval, DualRegionId, GapLabelResult, PathTreeError};

pub(super) fn label_gaps(
    boundary_len: usize,
    intervals: &[BoundaryInterval],
) -> Result<GapLabelResult, PathTreeError> {
    let mut starts = vec![Vec::<usize>::new(); boundary_len + 1];
    let mut ends = vec![Vec::<usize>::new(); boundary_len + 1];
    for (index, interval) in intervals.iter().enumerate() {
        if interval.start >= interval.end
            || interval.start > boundary_len
            || interval.end > boundary_len
        {
            return Err(PathTreeError::BoundaryGapEventImbalance);
        }
        starts[interval.start].push(index);
        ends[interval.end].push(index);
    }
    for bucket in &mut starts {
        bucket.sort_unstable_by_key(|&index| (std::cmp::Reverse(intervals[index].end), index));
    }
    for bucket in &mut ends {
        bucket.sort_unstable_by_key(|&index| (std::cmp::Reverse(intervals[index].start), index));
    }
    let mut active = Vec::<usize>::new();
    let mut labels = vec![DualRegionId(0); boundary_len];
    for gap in 0..boundary_len {
        for &index in &ends[gap] {
            let Some(popped) = active.pop() else {
                return Err(PathTreeError::BoundaryGapEventImbalance);
            };
            if popped != index {
                return Err(PathTreeError::BoundaryGapEventImbalance);
            }
        }
        for &index in &starts[gap] {
            if active
                .last()
                .is_some_and(|&parent| intervals[index].end > intervals[parent].end)
            {
                return Err(PathTreeError::BoundaryGapEventImbalance);
            }
            active.push(index);
        }
        if let Some(&index) = active.last() {
            labels[gap] = DualRegionId(index + 1);
        }
    }
    for &index in &ends[boundary_len] {
        let Some(popped) = active.pop() else {
            return Err(PathTreeError::BoundaryGapEventImbalance);
        };
        if popped != index || intervals[index].end != boundary_len {
            return Err(PathTreeError::BoundaryGapEventImbalance);
        }
    }
    if !starts[boundary_len].is_empty()
        || !active.is_empty()
        || ends
            .iter()
            .flatten()
            .any(|&index| intervals[index].end > boundary_len)
    {
        return Err(PathTreeError::BoundaryGapEventImbalance);
    }
    Ok(GapLabelResult {
        labels,
        membership_tests: 0,
        event_push_count: intervals.len(),
        event_pop_count: intervals.len(),
    })
}
