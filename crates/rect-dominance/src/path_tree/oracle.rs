use std::collections::{BTreeSet, HashSet, VecDeque};

use rect_core::{PreparedGridComponent, VerticalChord};
use rect_oracle_sg::CleanHoleFreeCertificate;

use super::{
    BoundaryInterval, DualRegionId, DualTreeEdge, GapLabelResult, PathTreeError, RegionDualTree,
};

pub(super) fn label_gaps(
    boundary_len: usize,
    intervals: &[BoundaryInterval],
    depths: &[usize],
) -> GapLabelResult {
    let mut labels = vec![DualRegionId(0); boundary_len];
    let mut membership_tests = 0;
    for (gap, label) in labels.iter_mut().enumerate() {
        let mut best = (0usize, DualRegionId(0));
        for (index, interval) in intervals.iter().enumerate() {
            membership_tests += 1;
            if interval.start <= gap && gap < interval.end && depths[index] >= best.0 {
                *label = DualRegionId(index + 1);
                best = (depths[index], *label);
            }
        }
    }
    GapLabelResult {
        labels,
        membership_tests,
        event_push_count: 0,
        event_pop_count: 0,
    }
}

/// Builds the definition-level vertical-chord region dual from occupancy cells.
///
/// # Errors
///
/// Returns an error for an ineligible component or invalid dual.
#[allow(clippy::too_many_lines)]
pub fn build_region_dual(
    prepared: &PreparedGridComponent,
    vertical_chords: &[VerticalChord],
    certificate: &CleanHoleFreeCertificate,
) -> Result<RegionDualTree, PathTreeError> {
    if !certificate.eligible {
        return Err(PathTreeError::Ineligible(certificate.clone()));
    }
    let width = prepared.width();
    let height = prepared.height();
    let mut vertical_cuts = HashSet::<(usize, usize)>::new();
    for &chord in vertical_chords {
        let x = usize::try_from(chord.x())
            .map_err(|_| PathTreeError::InvalidDualEdge { chord: chord.id() })?;
        let bottom = usize::try_from(chord.bottom())
            .map_err(|_| PathTreeError::InvalidDualEdge { chord: chord.id() })?;
        let top = usize::try_from(chord.top())
            .map_err(|_| PathTreeError::InvalidDualEdge { chord: chord.id() })?;
        for y in bottom..top {
            vertical_cuts.insert((x, y));
        }
    }

    let mut cell_region_ids = vec![usize::MAX; width * height];
    let mut region_count = 0;
    for seed in 0..cell_region_ids.len() {
        if !prepared.occupancy[seed] || cell_region_ids[seed] != usize::MAX {
            continue;
        }
        let mut queue = VecDeque::from([seed]);
        cell_region_ids[seed] = region_count;
        while let Some(index) = queue.pop_front() {
            let x = index % width;
            let y = index / width;
            let global_x = prepared.x0 + x;
            let global_y = prepared.y0 + y;
            let neighbors = [
                (x > 0 && !vertical_cuts.contains(&(global_x, global_y))).then(|| index - 1),
                (x + 1 < width && !vertical_cuts.contains(&(global_x + 1, global_y)))
                    .then(|| index + 1),
                (y > 0).then(|| index - width),
                (y + 1 < height).then(|| index + width),
            ];
            for neighbor in neighbors.into_iter().flatten() {
                if prepared.occupancy[neighbor] && cell_region_ids[neighbor] == usize::MAX {
                    cell_region_ids[neighbor] = region_count;
                    queue.push_back(neighbor);
                }
            }
        }
        region_count += 1;
    }

    let mut edges = Vec::with_capacity(vertical_chords.len());
    let mut labels = BTreeSet::new();
    for &chord in vertical_chords {
        let x = usize::try_from(chord.x())
            .map_err(|_| PathTreeError::InvalidDualEdge { chord: chord.id() })?;
        let y = usize::try_from(chord.bottom())
            .map_err(|_| PathTreeError::InvalidDualEdge { chord: chord.id() })?;
        if x <= prepared.x0 || x >= prepared.x1 || y < prepared.y0 || y >= prepared.y1 {
            return Err(PathTreeError::InvalidDualEdge { chord: chord.id() });
        }
        let left = cell_region_ids[(y - prepared.y0) * width + x - 1 - prepared.x0];
        let right = cell_region_ids[(y - prepared.y0) * width + x - prepared.x0];
        if left == usize::MAX || right == usize::MAX || left == right || !labels.insert(chord.id())
        {
            return Err(PathTreeError::InvalidDualEdge { chord: chord.id() });
        }
        edges.push(DualTreeEdge {
            chord: chord.id(),
            first: DualRegionId(left),
            second: DualRegionId(right),
        });
    }
    edges.sort_by_key(|edge| edge.chord);
    if edges.len() + 1 != region_count {
        return Err(PathTreeError::CyclicDual);
    }
    let mut adjacency = vec![Vec::new(); region_count];
    for edge in &edges {
        adjacency[edge.first.0].push((edge.second, edge.chord));
        adjacency[edge.second.0].push((edge.first, edge.chord));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
    }
    if region_count > 0 {
        let mut seen = vec![false; region_count];
        let mut queue = VecDeque::from([DualRegionId(0)]);
        seen[0] = true;
        while let Some(region) = queue.pop_front() {
            for &(neighbor, _) in &adjacency[region.0] {
                if !seen[neighbor.0] {
                    seen[neighbor.0] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        if seen.iter().any(|&present| !present) {
            return Err(PathTreeError::DisconnectedDual);
        }
    }
    Ok(RegionDualTree {
        region_count,
        edges,
        adjacency,
        cell_region_ids,
        boundary_gap_regions: Vec::new(),
        boundary_gap_membership_tests: 0,
        boundary_gap_event_push_count: 0,
        boundary_gap_event_pop_count: 0,
    })
}
