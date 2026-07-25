//! Geometry-derived clean hole-free path/tree representation.
//!
//! `ReferenceAreaFloodFill` is the deliberately redundant audited Oracle: it
//! labels the dual by cutting unit cell sides and flood-filling local
//! occupancy. `BoundaryLaminar` is the compact finite-grid backend: it derives
//! the dual from normalized boundary endpoint intervals and does not inspect
//! area cells. Neither backend claims the paper's general polygon sweep.

#![allow(clippy::missing_errors_doc)]

use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};

use rect_core::{
    Boundary, HorizontalChord, HorizontalChordId, PreparedGridComponent, VerticalChord,
    VerticalChordId, closed_chords_intersect,
};
use rect_oracle_sg::CleanHoleFreeCertificate;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::biclique::{Biclique, BicliquePartition};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct DualRegionId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DualTreeEdge {
    pub chord: VerticalChordId,
    pub first: DualRegionId,
    pub second: DualRegionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RegionDualTree {
    pub region_count: usize,
    pub edges: Vec<DualTreeEdge>,
    pub adjacency: Vec<Vec<(DualRegionId, VerticalChordId)>>,
    pub cell_region_ids: Vec<usize>,
    /// Region adjacent to each boundary gap in normalized outer-loop order.
    /// Empty for the historical area oracle and synthetic test trees.
    #[serde(default)]
    pub boundary_gap_regions: Vec<DualRegionId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChordTreePath {
    pub horizontal: HorizontalChordId,
    pub start_region: DualRegionId,
    pub end_region: DualRegionId,
    pub vertical_edges: Vec<VerticalChordId>,
}

/// Compact path certificate.  Endpoints are sufficient to identify the
/// unique path in a tree; explicit edge lists are intentionally absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompactTreePath {
    pub chord_index: usize,
    pub start_region: DualRegionId,
    pub end_region: DualRegionId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HeavyLightDecomposition {
    pub parent: Vec<Option<DualRegionId>>,
    pub parent_edge: Vec<Option<VerticalChordId>>,
    pub depth: Vec<usize>,
    pub subtree_size: Vec<usize>,
    pub heavy_child: Vec<Option<DualRegionId>>,
    pub chain_head: Vec<DualRegionId>,
    pub chain_id: Vec<usize>,
    pub edge_position: Vec<usize>,
    pub chain_edges: Vec<Vec<VerticalChordId>>,
    #[serde(skip)]
    edge_locations: BTreeMap<VerticalChordId, (usize, usize)>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreeAudit {
    pub explicit_edge_count: usize,
    pub represented_edge_count: usize,
    pub duplicate_edge_count: usize,
    pub missing_edge_count: usize,
    pub fabricated_edge_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreePartition {
    pub certificate: CleanHoleFreeCertificate,
    pub tree: RegionDualTree,
    pub paths: Vec<ChordTreePath>,
    /// Endpoint-only records used by `CompactOnly`.  The audited `paths` field
    /// remains available for backwards-compatible certificates.
    #[serde(default)]
    pub compact_paths: Vec<CompactTreePath>,
    pub hld: HeavyLightDecomposition,
    pub biclique_partition: BicliquePartition,
    pub total_path_edge_incidences: usize,
    pub canonical_segment_node_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathTreeOrientation {
    VerticalTreeHorizontalPaths,
    HorizontalTreeVerticalPaths,
}

impl PathTreeOrientation {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::VerticalTreeHorizontalPaths => "vertical-tree-horizontal-paths",
            Self::HorizontalTreeVerticalPaths => "horizontal-tree-vertical-paths",
        }
    }
}

/// Controls how the path/tree representation chooses its fixed orientation.
///
/// `BuildBothExact` is the historical audited selector. `BoundEstimate` uses
/// the paper-shaped upper bounds before constructing either full partition and
/// therefore builds only the selected orientation. The fixed policies are
/// useful for differential tests and benchmarks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathTreeOrientationPolicy {
    BuildBothExact,
    BoundEstimate,
    VerticalTree,
    HorizontalTree,
}

impl PathTreeOrientationPolicy {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::BuildBothExact => "build-both",
            Self::BoundEstimate => "bound-estimate",
            Self::VerticalTree => "vertical-tree",
            Self::HorizontalTree => "horizontal-tree",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OrientedPathTreePartition {
    pub orientation: PathTreeOrientation,
    pub path_tree: PathTreePartition,
    pub biclique_partition: BicliquePartition,
    pub total_path_edge_incidences: usize,
    pub canonical_segment_node_count: usize,
    pub dual_region_count: usize,
    pub path_count: usize,
}

#[derive(Debug, Error)]
pub enum PathTreeError {
    #[error("path-tree construction requires a clean hole-free certificate")]
    Ineligible(CleanHoleFreeCertificate),
    #[error("vertical chord {chord:?} does not separate two distinct regions")]
    InvalidDualEdge { chord: VerticalChordId },
    #[error("region dual graph is disconnected")]
    DisconnectedDual,
    #[error("region dual graph contains a cycle")]
    CyclicDual,
    #[error("tree path endpoint cell is not occupied")]
    MissingPathEndpoint,
    #[error("tree path could not be recovered")]
    MissingTreePath,
    #[error("path-tree edge partition audit failed: {audit:?}")]
    PartitionAudit { audit: PathTreeAudit },
    #[error("transposed path-tree chord construction failed")]
    InvalidTransposedChord,
    #[error("boundary-laminar dual requires a single normalized outer loop")]
    InvalidBoundaryDual,
    #[error("fixed-orientation chord intervals are not laminar")]
    NonLaminarBoundaryIntervals,
    #[error("tree path metric overflow")]
    PathMetricOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegionDualBackend {
    ReferenceAreaFloodFill,
    BoundaryLaminar,
}

impl RegionDualBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferenceAreaFloodFill => "reference-area",
            Self::BoundaryLaminar => "boundary-laminar",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct BoundaryInterval {
    start: usize,
    end: usize,
    chord: VerticalChordId,
}

/// Builds the region dual from the containment tree of noncrossing boundary
/// intervals.  The construction uses only normalized boundary order and does
/// not inspect occupancy cells or expand chords into unit cuts.
#[allow(clippy::too_many_lines)]
pub fn build_boundary_laminar_dual_tree(
    boundary: &Boundary,
    vertical_chords: &[VerticalChord],
    certificate: &CleanHoleFreeCertificate,
) -> Result<RegionDualTree, PathTreeError> {
    if !certificate.eligible
        || certificate.outer_loop_count != 1
        || certificate.hole_count != 0
        || boundary.loops.len() != 1
    {
        return Err(PathTreeError::InvalidBoundaryDual);
    }
    let vertices = &boundary.loops[0].vertices;
    let n = vertices.len();
    if n < 4 {
        return Err(PathTreeError::InvalidBoundaryDual);
    }
    let mut endpoint_indices = BTreeSet::new();
    let mut endpoint_pairs = Vec::with_capacity(vertical_chords.len());
    for &chord in vertical_chords {
        let endpoints = rect_oracle_sg::vertical_chord_endpoints(boundary, chord)
            .map_err(|_| PathTreeError::InvalidBoundaryDual)?;
        if endpoints.first.loop_id.0 != 0
            || endpoints.second.loop_id.0 != 0
            || endpoints.first == endpoints.second
        {
            return Err(PathTreeError::InvalidBoundaryDual);
        }
        endpoint_indices.insert(endpoints.first.cyclic_index);
        endpoint_indices.insert(endpoints.second.cyclic_index);
        endpoint_pairs.push((endpoints.first.cyclic_index, endpoints.second.cyclic_index));
    }
    let root_gap = (0..n)
        .find(|gap| !endpoint_indices.contains(gap))
        .ok_or(PathTreeError::InvalidBoundaryDual)?;
    let origin = (root_gap + 1) % n;
    let rotate = |index: usize| (index + n - origin) % n;
    let mut intervals = endpoint_pairs
        .into_iter()
        .zip(vertical_chords.iter().map(|chord| chord.id()))
        .map(|((first, second), chord)| {
            let first = rotate(first);
            let second = rotate(second);
            if first < second {
                BoundaryInterval {
                    start: first,
                    end: second,
                    chord,
                }
            } else {
                BoundaryInterval {
                    start: second,
                    end: first,
                    chord,
                }
            }
        })
        .collect::<Vec<_>>();
    intervals.sort_unstable_by_key(|interval| {
        (
            interval.start,
            std::cmp::Reverse(interval.end),
            interval.chord,
        )
    });

    let mut edges = Vec::with_capacity(intervals.len());
    let mut stack = Vec::<(usize, DualRegionId)>::new();
    let mut depths = Vec::with_capacity(intervals.len());
    for interval in intervals.iter().copied() {
        while stack.last().is_some_and(|(end, _)| *end <= interval.start) {
            stack.pop();
        }
        if let Some((end, _)) = stack.last()
            && interval.end > *end
        {
            return Err(PathTreeError::NonLaminarBoundaryIntervals);
        }
        let parent = stack.last().map_or(DualRegionId(0), |(_, region)| *region);
        let region = DualRegionId(edges.len() + 1);
        edges.push(DualTreeEdge {
            chord: interval.chord,
            first: parent,
            second: region,
        });
        depths.push(stack.len() + 1);
        stack.push((interval.end, region));
    }

    let region_count = edges.len() + 1;
    let mut adjacency = vec![Vec::new(); region_count];
    for edge in &edges {
        adjacency[edge.first.0].push((edge.second, edge.chord));
        adjacency[edge.second.0].push((edge.first, edge.chord));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
    }

    let mut rotated_gap_regions = vec![DualRegionId(0); n];
    for (gap, label) in rotated_gap_regions.iter_mut().enumerate() {
        let mut best = (0usize, DualRegionId(0));
        for (index, interval) in intervals.iter().enumerate() {
            if interval.start <= gap && gap < interval.end && depths[index] >= best.0 {
                *label = DualRegionId(index + 1);
                best = (depths[index], *label);
            }
        }
    }
    let mut boundary_gap_regions = vec![DualRegionId(0); n];
    for original_gap in 0..n {
        boundary_gap_regions[original_gap] = rotated_gap_regions[rotate(original_gap)];
    }
    Ok(RegionDualTree {
        region_count,
        edges,
        adjacency,
        cell_region_ids: Vec::new(),
        boundary_gap_regions,
    })
}

/// Axis-view counterpart of [`build_boundary_laminar_dual_tree`] for a
/// horizontal tree. The tree edge IDs retain the original horizontal chord
/// indices in the `VerticalChordId` carrier used by the generic HLD storage;
/// the oriented builder swaps the biclique sides back to H/V afterwards.
#[allow(clippy::too_many_lines)]
fn build_horizontal_boundary_laminar_dual_tree(
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    certificate: &CleanHoleFreeCertificate,
) -> Result<RegionDualTree, PathTreeError> {
    if !certificate.eligible
        || certificate.outer_loop_count != 1
        || certificate.hole_count != 0
        || boundary.loops.len() != 1
    {
        return Err(PathTreeError::InvalidBoundaryDual);
    }
    let vertices = &boundary.loops[0].vertices;
    let n = vertices.len();
    if n < 4 {
        return Err(PathTreeError::InvalidBoundaryDual);
    }
    let mut endpoint_indices = BTreeSet::new();
    let mut endpoint_pairs = Vec::with_capacity(horizontal_chords.len());
    for &chord in horizontal_chords {
        let endpoints = rect_oracle_sg::horizontal_chord_endpoints(boundary, chord)
            .map_err(|_| PathTreeError::InvalidBoundaryDual)?;
        if endpoints.first.loop_id.0 != 0
            || endpoints.second.loop_id.0 != 0
            || endpoints.first == endpoints.second
        {
            return Err(PathTreeError::InvalidBoundaryDual);
        }
        endpoint_indices.insert(endpoints.first.cyclic_index);
        endpoint_indices.insert(endpoints.second.cyclic_index);
        endpoint_pairs.push((
            endpoints.first.cyclic_index,
            endpoints.second.cyclic_index,
            VerticalChordId(chord.id().0),
        ));
    }
    let root_gap = (0..n)
        .find(|gap| !endpoint_indices.contains(gap))
        .ok_or(PathTreeError::InvalidBoundaryDual)?;
    let origin = (root_gap + 1) % n;
    let rotate = |index: usize| (index + n - origin) % n;
    let mut intervals = endpoint_pairs
        .into_iter()
        .map(|(first, second, chord)| {
            let first = rotate(first);
            let second = rotate(second);
            if first < second {
                BoundaryInterval {
                    start: first,
                    end: second,
                    chord,
                }
            } else {
                BoundaryInterval {
                    start: second,
                    end: first,
                    chord,
                }
            }
        })
        .collect::<Vec<_>>();
    intervals.sort_unstable_by_key(|interval| {
        (
            interval.start,
            std::cmp::Reverse(interval.end),
            interval.chord,
        )
    });
    let mut edges = Vec::with_capacity(intervals.len());
    let mut stack = Vec::<(usize, DualRegionId)>::new();
    let mut depths = Vec::with_capacity(intervals.len());
    for interval in intervals.iter().copied() {
        while stack.last().is_some_and(|(end, _)| *end <= interval.start) {
            stack.pop();
        }
        if let Some((end, _)) = stack.last()
            && interval.end > *end
        {
            return Err(PathTreeError::NonLaminarBoundaryIntervals);
        }
        let parent = stack.last().map_or(DualRegionId(0), |(_, region)| *region);
        let region = DualRegionId(edges.len() + 1);
        edges.push(DualTreeEdge {
            chord: interval.chord,
            first: parent,
            second: region,
        });
        depths.push(stack.len() + 1);
        stack.push((interval.end, region));
    }
    let region_count = edges.len() + 1;
    let mut adjacency = vec![Vec::new(); region_count];
    for edge in &edges {
        adjacency[edge.first.0].push((edge.second, edge.chord));
        adjacency[edge.second.0].push((edge.first, edge.chord));
    }
    for neighbors in &mut adjacency {
        neighbors.sort_unstable();
    }
    let mut rotated_gap_regions = vec![DualRegionId(0); n];
    for (gap, label) in rotated_gap_regions.iter_mut().enumerate() {
        let mut best = (0usize, DualRegionId(0));
        for (index, interval) in intervals.iter().enumerate() {
            if interval.start <= gap && gap < interval.end && depths[index] >= best.0 {
                *label = DualRegionId(index + 1);
                best = (depths[index], *label);
            }
        }
    }
    let mut boundary_gap_regions = vec![DualRegionId(0); n];
    for original_gap in 0..n {
        boundary_gap_regions[original_gap] = rotated_gap_regions[rotate(original_gap)];
    }
    Ok(RegionDualTree {
        region_count,
        edges,
        adjacency,
        cell_region_ids: Vec::new(),
        boundary_gap_regions,
    })
}

/// Builds the vertical-chord region dual from prepared occupancy.
///
/// # Errors
///
/// Returns [`PathTreeError`] for an ineligible component or invalid dual.
#[allow(clippy::too_many_lines)]
pub fn build_vertical_dual_tree(
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
    })
}

impl RegionDualTree {
    fn region_at_horizontal_endpoint_boundary(
        &self,
        boundary: &Boundary,
        chord: HorizontalChord,
        first: bool,
    ) -> Result<DualRegionId, PathTreeError> {
        let point =
            rect_core::Point::new(if first { chord.left() } else { chord.right() }, chord.y());
        let vertex_id = boundary
            .vertex_id(point)
            .ok_or(PathTreeError::MissingPathEndpoint)?;
        let loop_vertices = boundary
            .loops
            .get(vertex_id.loop_id.0)
            .ok_or(PathTreeError::MissingPathEndpoint)?
            .vertices
            .as_slice();
        let n = loop_vertices.len();
        let index = vertex_id.cyclic_index;
        let previous = loop_vertices[(index + n - 1) % n];
        let current = loop_vertices[index];
        let next = loop_vertices[(index + 1) % n];
        // At a proper grid reflex vertex exactly one incident boundary edge
        // is horizontal.  Its interior side is the boundary sector adjacent
        // to a horizontal chord endpoint, so its gap label is the endpoint
        // region.  This avoids any cell lookup or area flood fill.
        let gap = if previous.y == current.y {
            (index + n - 1) % n
        } else if current.y == next.y {
            index
        } else {
            return Err(PathTreeError::MissingPathEndpoint);
        };
        self.boundary_gap_regions
            .get(gap)
            .copied()
            .ok_or(PathTreeError::MissingPathEndpoint)
    }

    fn region_at_vertical_endpoint_boundary(
        &self,
        boundary: &Boundary,
        chord: VerticalChord,
        first: bool,
    ) -> Result<DualRegionId, PathTreeError> {
        let point =
            rect_core::Point::new(chord.x(), if first { chord.bottom() } else { chord.top() });
        let vertex_id = boundary
            .vertex_id(point)
            .ok_or(PathTreeError::MissingPathEndpoint)?;
        let loop_vertices = boundary
            .loops
            .get(vertex_id.loop_id.0)
            .ok_or(PathTreeError::MissingPathEndpoint)?
            .vertices
            .as_slice();
        let n = loop_vertices.len();
        let index = vertex_id.cyclic_index;
        let previous = loop_vertices[(index + n - 1) % n];
        let current = loop_vertices[index];
        let next = loop_vertices[(index + 1) % n];
        let gap = if previous.x == current.x {
            (index + n - 1) % n
        } else if current.x == next.x {
            index
        } else {
            return Err(PathTreeError::MissingPathEndpoint);
        };
        self.boundary_gap_regions
            .get(gap)
            .copied()
            .ok_or(PathTreeError::MissingPathEndpoint)
    }

    pub fn horizontal_endpoint_paths_boundary(
        &self,
        boundary: &Boundary,
        horizontal_chords: &[HorizontalChord],
    ) -> Result<Vec<CompactTreePath>, PathTreeError> {
        horizontal_chords
            .iter()
            .map(|&chord| {
                Ok(CompactTreePath {
                    chord_index: chord.id().0,
                    start_region: self
                        .region_at_horizontal_endpoint_boundary(boundary, chord, true)?,
                    end_region: self
                        .region_at_horizontal_endpoint_boundary(boundary, chord, false)?,
                })
            })
            .collect()
    }

    fn vertical_endpoint_paths_boundary(
        &self,
        boundary: &Boundary,
        vertical_chords: &[VerticalChord],
    ) -> Result<Vec<CompactTreePath>, PathTreeError> {
        vertical_chords
            .iter()
            .map(|&chord| {
                Ok(CompactTreePath {
                    chord_index: chord.id().0,
                    start_region: self
                        .region_at_vertical_endpoint_boundary(boundary, chord, true)?,
                    end_region: self
                        .region_at_vertical_endpoint_boundary(boundary, chord, false)?,
                })
            })
            .collect()
    }

    fn region_at_horizontal_endpoint(
        &self,
        prepared: &PreparedGridComponent,
        chord: HorizontalChord,
        first: bool,
    ) -> Result<DualRegionId, PathTreeError> {
        let left = usize::try_from(chord.left()).map_err(|_| PathTreeError::MissingPathEndpoint)?;
        let right =
            usize::try_from(chord.right()).map_err(|_| PathTreeError::MissingPathEndpoint)?;
        let y = usize::try_from(chord.y()).map_err(|_| PathTreeError::MissingPathEndpoint)?;
        let x = if first { left } else { right - 1 };
        let local = |x: usize, y: usize| {
            (x >= prepared.x0 && x < prepared.x1 && y >= prepared.y0 && y < prepared.y1).then_some(
                self.cell_region_ids[(y - prepared.y0) * prepared.width() + x - prepared.x0],
            )
        };
        local(x, y)
            .or_else(|| y.checked_sub(1).and_then(|below| local(x, below)))
            .map(DualRegionId)
            .ok_or(PathTreeError::MissingPathEndpoint)
    }

    /// Returns endpoint-only path records without traversing the dual tree.
    pub fn horizontal_endpoint_paths(
        &self,
        prepared: &PreparedGridComponent,
        horizontal_chords: &[HorizontalChord],
    ) -> Result<Vec<CompactTreePath>, PathTreeError> {
        horizontal_chords
            .iter()
            .map(|&chord| {
                Ok(CompactTreePath {
                    chord_index: chord.id().0,
                    start_region: self.region_at_horizontal_endpoint(prepared, chord, true)?,
                    end_region: self.region_at_horizontal_endpoint(prepared, chord, false)?,
                })
            })
            .collect()
    }

    /// Recovers all horizontal chord paths by tree geometry, never by graph neighbors.
    ///
    /// # Errors
    ///
    /// Returns [`PathTreeError`] when an endpoint region or tree path is invalid.
    pub fn horizontal_paths(
        &self,
        prepared: &PreparedGridComponent,
        horizontal_chords: &[HorizontalChord],
    ) -> Result<Vec<ChordTreePath>, PathTreeError> {
        let mut paths = Vec::with_capacity(horizontal_chords.len());
        for &chord in horizontal_chords {
            let start_region = self.region_at_horizontal_endpoint(prepared, chord, true)?;
            let end_region = self.region_at_horizontal_endpoint(prepared, chord, false)?;
            let mut parent = vec![None; self.region_count];
            let mut parent_edge = vec![None; self.region_count];
            let mut queue = VecDeque::from([start_region]);
            parent[start_region.0] = Some(start_region);
            while let Some(region) = queue.pop_front() {
                if region == end_region {
                    break;
                }
                for &(neighbor, edge) in &self.adjacency[region.0] {
                    if parent[neighbor.0].is_none() {
                        parent[neighbor.0] = Some(region);
                        parent_edge[neighbor.0] = Some(edge);
                        queue.push_back(neighbor);
                    }
                }
            }
            if parent[end_region.0].is_none() {
                return Err(PathTreeError::MissingTreePath);
            }
            let mut edges = Vec::new();
            let mut current = end_region;
            while current != start_region {
                edges.push(parent_edge[current.0].ok_or(PathTreeError::MissingTreePath)?);
                current = parent[current.0].ok_or(PathTreeError::MissingTreePath)?;
            }
            edges.reverse();
            paths.push(ChordTreePath {
                horizontal: chord.id(),
                start_region,
                end_region,
                vertical_edges: edges,
            });
        }
        Ok(paths)
    }

    fn explicit_paths_from_compact(
        &self,
        compact_paths: &[CompactTreePath],
        horizontal_chords: &[HorizontalChord],
    ) -> Result<Vec<ChordTreePath>, PathTreeError> {
        let mut paths = Vec::with_capacity(compact_paths.len());
        for compact in compact_paths {
            let mut parent = vec![None; self.region_count];
            let mut parent_edge = vec![None; self.region_count];
            let mut queue = VecDeque::from([compact.start_region]);
            parent[compact.start_region.0] = Some(compact.start_region);
            while let Some(region) = queue.pop_front() {
                if region == compact.end_region {
                    break;
                }
                for &(neighbor, edge) in &self.adjacency[region.0] {
                    if parent[neighbor.0].is_none() {
                        parent[neighbor.0] = Some(region);
                        parent_edge[neighbor.0] = Some(edge);
                        queue.push_back(neighbor);
                    }
                }
            }
            if parent[compact.end_region.0].is_none() {
                return Err(PathTreeError::MissingTreePath);
            }
            let mut edges = Vec::new();
            let mut current = compact.end_region;
            while current != compact.start_region {
                edges.push(parent_edge[current.0].ok_or(PathTreeError::MissingTreePath)?);
                current = parent[current.0].ok_or(PathTreeError::MissingTreePath)?;
            }
            edges.reverse();
            let horizontal = horizontal_chords
                .get(compact.chord_index)
                .ok_or(PathTreeError::MissingTreePath)?;
            paths.push(ChordTreePath {
                horizontal: horizontal.id(),
                start_region: compact.start_region,
                end_region: compact.end_region,
                vertical_edges: edges,
            });
        }
        Ok(paths)
    }
}

impl HeavyLightDecomposition {
    /// Builds a deterministic heavy-light decomposition rooted at region zero.
    ///
    /// # Errors
    ///
    /// Returns [`PathTreeError::DisconnectedDual`] when the tree is disconnected.
    pub fn new(tree: &RegionDualTree) -> Result<Self, PathTreeError> {
        let n = tree.region_count;
        let mut parent = vec![None; n];
        let mut parent_edge = vec![None; n];
        let mut depth = vec![0; n];
        let mut order = Vec::with_capacity(n);
        if n > 0 {
            let mut stack = vec![DualRegionId(0)];
            parent[0] = Some(DualRegionId(0));
            while let Some(region) = stack.pop() {
                order.push(region);
                for &(neighbor, edge) in &tree.adjacency[region.0] {
                    if parent[neighbor.0].is_none() {
                        parent[neighbor.0] = Some(region);
                        parent_edge[neighbor.0] = Some(edge);
                        depth[neighbor.0] = depth[region.0] + 1;
                        stack.push(neighbor);
                    }
                }
            }
        }
        if order.len() != n {
            return Err(PathTreeError::DisconnectedDual);
        }
        let mut subtree_size = vec![1; n];
        let mut heavy_child = vec![None; n];
        for &region in order.iter().rev() {
            if let Some(parent_region) = parent[region.0]
                && parent_region != region
            {
                subtree_size[parent_region.0] += subtree_size[region.0];
                let replace = heavy_child[parent_region.0].is_none_or(|heavy: DualRegionId| {
                    subtree_size[region.0] > subtree_size[heavy.0]
                        || (subtree_size[region.0] == subtree_size[heavy.0] && region < heavy)
                });
                if replace {
                    heavy_child[parent_region.0] = Some(region);
                }
            }
        }
        let mut chain_head = vec![DualRegionId(0); n];
        let mut chain_id = vec![0; n];
        let mut edge_position = vec![0; n];
        let mut chain_edges = Vec::<Vec<VerticalChordId>>::new();
        let mut stack = if n == 0 {
            Vec::new()
        } else {
            chain_edges.push(Vec::new());
            vec![(DualRegionId(0), DualRegionId(0), 0)]
        };
        while let Some((region, head, chain)) = stack.pop() {
            debug_assert!(chain < chain_edges.len());
            chain_head[region.0] = head;
            chain_id[region.0] = chain;
            if region.0 != 0 {
                edge_position[region.0] = chain_edges[chain].len();
                chain_edges[chain]
                    .push(parent_edge[region.0].ok_or(PathTreeError::MissingTreePath)?);
            }
            if let Some(heavy) = heavy_child[region.0] {
                stack.push((heavy, head, chain));
            }
            let mut light = tree.adjacency[region.0]
                .iter()
                .filter_map(|&(neighbor, _)| {
                    (parent[neighbor.0] == Some(region) && Some(neighbor) != heavy_child[region.0])
                        .then_some(neighbor)
                })
                .collect::<Vec<_>>();
            light.sort_unstable_by(|a, b| b.cmp(a));
            for child in light {
                let next_chain = chain_edges.len();
                chain_edges.push(Vec::new());
                stack.push((child, child, next_chain));
            }
        }
        let mut edge_locations = BTreeMap::new();
        for (chain, edges) in chain_edges.iter().enumerate() {
            for (position, &edge) in edges.iter().enumerate() {
                edge_locations.insert(edge, (chain, position));
            }
        }
        Ok(Self {
            parent,
            parent_edge,
            depth,
            subtree_size,
            heavy_child,
            chain_head,
            chain_id,
            edge_position,
            chain_edges,
            edge_locations,
        })
    }

    fn canonical_interval(
        node: usize,
        begin: usize,
        end: usize,
        query_begin: usize,
        query_end: usize,
        output: &mut Vec<(usize, usize, usize)>,
    ) {
        if query_begin >= end || query_end <= begin {
            return;
        }
        if query_begin <= begin && end <= query_end {
            output.push((node, begin, end));
            return;
        }
        let middle = begin.midpoint(end);
        Self::canonical_interval(node * 2, begin, middle, query_begin, query_end, output);
        Self::canonical_interval(node * 2 + 1, middle, end, query_begin, query_end, output);
    }

    fn canonical_nodes(
        &self,
        chain: usize,
        begin: usize,
        end: usize,
    ) -> Vec<(usize, usize, usize)> {
        let mut nodes = Vec::new();
        if begin < end {
            Self::canonical_interval(1, 0, self.chain_edges[chain].len(), begin, end, &mut nodes);
        }
        nodes
            .into_iter()
            .map(|(_, left, right)| (chain, left, right))
            .collect()
    }

    /// Decomposes the unique tree path using only its endpoint regions.
    /// Returned intervals are `(chain, begin, end)` half-open ranges over the
    /// chain's edge array and are edge-disjoint.
    ///
    /// # Errors
    ///
    /// Returns [`PathTreeError::MissingTreePath`] when an endpoint is outside
    /// this decomposition.
    pub fn decompose_path_endpoints(
        &self,
        start: DualRegionId,
        end: DualRegionId,
    ) -> Result<Vec<(usize, usize, usize)>, PathTreeError> {
        if start.0 >= self.parent.len() || end.0 >= self.parent.len() {
            return Err(PathTreeError::MissingTreePath);
        }
        let mut left = start;
        let mut right = end;
        let mut intervals = Vec::new();
        while self.chain_id[left.0] != self.chain_id[right.0] {
            let left_head = self.chain_head[left.0];
            let right_head = self.chain_head[right.0];
            if self.depth[left_head.0] >= self.depth[right_head.0] {
                let begin = self.edge_position[left_head.0];
                let end_position = self.edge_position[left.0] + 1;
                if begin < end_position {
                    intervals.push((self.chain_id[left.0], begin, end_position));
                }
                left = self.parent[left_head.0].ok_or(PathTreeError::MissingTreePath)?;
            } else {
                let begin = self.edge_position[right_head.0];
                let end_position = self.edge_position[right.0] + 1;
                if begin < end_position {
                    intervals.push((self.chain_id[right.0], begin, end_position));
                }
                right = self.parent[right_head.0].ok_or(PathTreeError::MissingTreePath)?;
            }
        }
        if left != right {
            if self.depth[left.0] >= self.depth[right.0] {
                let begin = if right.0 == 0 {
                    0
                } else {
                    self.edge_position[right.0] + 1
                };
                let end_position = self.edge_position[left.0] + 1;
                if begin < end_position {
                    intervals.push((self.chain_id[left.0], begin, end_position));
                }
            } else {
                let begin = if left.0 == 0 {
                    0
                } else {
                    self.edge_position[left.0] + 1
                };
                let end_position = self.edge_position[right.0] + 1;
                if begin < end_position {
                    intervals.push((self.chain_id[right.0], begin, end_position));
                }
            }
        }
        Ok(intervals)
    }

    /// Returns the number of edges on an endpoint-defined path without
    /// enumerating the path.
    pub fn path_length(
        &self,
        start: DualRegionId,
        end: DualRegionId,
    ) -> Result<usize, PathTreeError> {
        if start.0 >= self.depth.len() || end.0 >= self.depth.len() {
            return Err(PathTreeError::MissingTreePath);
        }
        let mut left = start;
        let mut right = end;
        let mut length = 0usize;
        while self.chain_id[left.0] != self.chain_id[right.0] {
            let left_head = self.chain_head[left.0];
            let right_head = self.chain_head[right.0];
            if self.depth[left_head.0] >= self.depth[right_head.0] {
                length = length
                    .checked_add(self.depth[left.0] - self.depth[left_head.0] + 1)
                    .ok_or(PathTreeError::PathMetricOverflow)?;
                left = self.parent[left_head.0].ok_or(PathTreeError::MissingTreePath)?;
            } else {
                length = length
                    .checked_add(self.depth[right.0] - self.depth[right_head.0] + 1)
                    .ok_or(PathTreeError::PathMetricOverflow)?;
                right = self.parent[right_head.0].ok_or(PathTreeError::MissingTreePath)?;
            }
        }
        length
            .checked_add(self.depth[left.0].abs_diff(self.depth[right.0]))
            .ok_or(PathTreeError::PathMetricOverflow)
    }

    #[cfg(test)]
    fn path_intervals(&self, edges: &[VerticalChordId]) -> Vec<(usize, usize, usize)> {
        let mut intervals = Vec::new();
        let mut index = 0;
        while index < edges.len() {
            let (chain, mut left) = self.edge_locations[&edges[index]];
            let mut right = left + 1;
            index += 1;
            while index < edges.len() {
                let (next_chain, next_position) = self.edge_locations[&edges[index]];
                if next_chain != chain || next_position != right {
                    break;
                }
                right += 1;
                index += 1;
            }
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            intervals.extend(self.canonical_nodes(chain, left, right));
        }
        intervals
    }
}

/// Builds HLD canonical bicliques from the geometry-derived region tree.
///
/// # Errors
///
/// Returns [`PathTreeError`] when the clean certificate or tree invariants fail.
pub fn build_path_tree_partition(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
) -> Result<PathTreePartition, PathTreeError> {
    build_path_tree_partition_with_mode(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        true,
    )
}

/// Builds a path-tree partition while choosing whether the independent
/// explicit path oracle should materialize every traversed tree edge.
pub fn build_path_tree_partition_with_mode(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    materialize_explicit_paths: bool,
) -> Result<PathTreePartition, PathTreeError> {
    build_path_tree_partition_with_backend(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        materialize_explicit_paths,
        RegionDualBackend::ReferenceAreaFloodFill,
    )
}

pub fn build_path_tree_partition_with_backend(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    materialize_explicit_paths: bool,
    dual_backend: RegionDualBackend,
) -> Result<PathTreePartition, PathTreeError> {
    let tree = match dual_backend {
        RegionDualBackend::ReferenceAreaFloodFill => {
            build_vertical_dual_tree(prepared, vertical_chords, &certificate)?
        }
        RegionDualBackend::BoundaryLaminar => {
            build_boundary_laminar_dual_tree(boundary, vertical_chords, &certificate)?
        }
    };
    let compact_paths = match dual_backend {
        RegionDualBackend::ReferenceAreaFloodFill => {
            tree.horizontal_endpoint_paths(prepared, horizontal_chords)?
        }
        RegionDualBackend::BoundaryLaminar => {
            tree.horizontal_endpoint_paths_boundary(boundary, horizontal_chords)?
        }
    };
    let paths = if materialize_explicit_paths {
        if dual_backend == RegionDualBackend::BoundaryLaminar {
            tree.explicit_paths_from_compact(&compact_paths, horizontal_chords)?
        } else {
            tree.horizontal_paths(prepared, horizontal_chords)?
        }
    } else {
        Vec::new()
    };
    build_partition_from_compact_paths(certificate, tree, compact_paths, paths)
}

fn build_partition_from_compact_paths(
    certificate: CleanHoleFreeCertificate,
    tree: RegionDualTree,
    compact_paths: Vec<CompactTreePath>,
    paths: Vec<ChordTreePath>,
) -> Result<PathTreePartition, PathTreeError> {
    let hld = HeavyLightDecomposition::new(&tree)?;
    let mut grouped = BTreeMap::<(usize, usize, usize), Vec<usize>>::new();
    let mut total_path_edge_incidences: usize = 0;
    for (path_index, path) in compact_paths.iter().enumerate() {
        total_path_edge_incidences = total_path_edge_incidences
            .checked_add(hld.path_length(path.start_region, path.end_region)?)
            .ok_or(PathTreeError::PathMetricOverflow)?;
        for interval in hld.decompose_path_endpoints(path.start_region, path.end_region)? {
            for node in hld.canonical_nodes(interval.0, interval.1, interval.2) {
                grouped.entry(node).or_default().push(path_index);
            }
        }
    }
    let mut bicliques = Vec::new();
    for ((chain, begin, end), mut left) in grouped {
        left.sort_unstable();
        left.dedup();
        let right = hld.chain_edges[chain][begin..end]
            .iter()
            .map(|edge| edge.0)
            .collect::<Vec<_>>();
        if !left.is_empty() && !right.is_empty() {
            bicliques.push(Biclique {
                id: rect_core::BicliqueId(bicliques.len()),
                left,
                right,
            });
        }
    }
    Ok(PathTreePartition {
        certificate,
        tree,
        paths,
        compact_paths,
        hld,
        canonical_segment_node_count: bicliques.len(),
        total_path_edge_incidences,
        biclique_partition: BicliquePartition { bicliques },
    })
}

fn build_horizontal_axis_view_partition(
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    let tree =
        build_horizontal_boundary_laminar_dual_tree(boundary, horizontal_chords, &certificate)?;
    let compact_paths = tree.vertical_endpoint_paths_boundary(boundary, vertical_chords)?;
    let mut partition =
        build_partition_from_compact_paths(certificate, tree, compact_paths, Vec::new())?;
    let bicliques = std::mem::take(&mut partition.biclique_partition.bicliques)
        .into_iter()
        .map(|biclique| Biclique {
            id: biclique.id,
            left: biclique.right,
            right: biclique.left,
        })
        .collect();
    let biclique_partition = BicliquePartition { bicliques };
    partition.biclique_partition = biclique_partition.clone();
    Ok(OrientedPathTreePartition {
        orientation: PathTreeOrientation::HorizontalTreeVerticalPaths,
        dual_region_count: partition.tree.region_count,
        path_count: partition.compact_paths.len(),
        total_path_edge_incidences: partition.total_path_edge_incidences,
        canonical_segment_node_count: partition.canonical_segment_node_count,
        biclique_partition,
        path_tree: partition,
    })
}

fn transpose_prepared(prepared: &PreparedGridComponent) -> PreparedGridComponent {
    let width = prepared.width();
    let height = prepared.height();
    let mut occupancy = vec![false; width * height];
    for y in 0..height {
        for x in 0..width {
            occupancy[x * height + y] = prepared.occupancy[y * width + x];
        }
    }
    PreparedGridComponent {
        x0: prepared.y0,
        y0: prepared.x0,
        x1: prepared.y1,
        y1: prepared.x1,
        occupancy,
        occupancy_prefix_sums: vec![0; (height + 1) * (width + 1)],
        horizontal_interior_runs: vec![Vec::new(); width + 1],
        vertical_interior_runs: vec![Vec::new(); height + 1],
    }
}

fn transpose_boundary(boundary: &Boundary) -> Boundary {
    let loops = boundary
        .loops
        .iter()
        .map(|boundary_loop| {
            let mut vertices = boundary_loop
                .vertices
                .iter()
                .map(|point| rect_core::Point::new(point.y, point.x))
                .collect::<Vec<_>>();
            // Coordinate transposition reverses orientation; restore the
            // normalized outer-loop convention used by endpoint identities.
            vertices.reverse();
            rect_core::BoundaryLoop {
                vertices,
                twice_signed_area: -boundary_loop.twice_signed_area,
                is_hole: boundary_loop.is_hole,
            }
        })
        .collect();
    let reflex_vertices = boundary
        .reflex_vertices
        .iter()
        .map(|vertex| rect_core::ReflexVertex {
            point: rect_core::Point::new(vertex.point.y, vertex.point.x),
        })
        .collect();
    let unit_edges = boundary
        .unit_edges
        .iter()
        .map(|(first, second)| {
            (
                rect_core::Point::new(first.y, first.x),
                rect_core::Point::new(second.y, second.x),
            )
        })
        .collect();
    Boundary {
        loops,
        reflex_vertices,
        unit_edges,
    }
}

fn transpose_chords(
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
) -> Result<(Vec<HorizontalChord>, Vec<VerticalChord>), PathTreeError> {
    let horizontal = vertical_chords
        .iter()
        .map(|&chord| {
            HorizontalChord::new(
                HorizontalChordId(chord.id().0),
                chord.bottom(),
                chord.top(),
                chord.x(),
            )
            .map_err(|_| PathTreeError::InvalidTransposedChord)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let vertical = horizontal_chords
        .iter()
        .map(|&chord| {
            VerticalChord::new(
                VerticalChordId(chord.id().0),
                chord.y(),
                chord.left(),
                chord.right(),
            )
            .map_err(|_| PathTreeError::InvalidTransposedChord)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((horizontal, vertical))
}

/// Builds a path-tree partition in either orientation.
///
/// The horizontal-tree orientation is constructed by transposing the prepared
/// occupancy and chord coordinates, reusing the same reference dual and HLD
/// code, and swapping the resulting biclique sides back to the original H/V
/// convention.  The two orientations therefore have identical edge semantics
/// while exposing different compact-size tradeoffs.
///
/// # Errors
///
/// Returns [`PathTreeError`] when the clean certificate, transposed chord
/// coordinates, or either reference dual construction is invalid.
pub fn build_oriented_path_tree_partition(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    orientation: PathTreeOrientation,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    build_oriented_path_tree_partition_with_mode(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        orientation,
        true,
    )
}

pub fn build_oriented_path_tree_partition_with_mode(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    orientation: PathTreeOrientation,
    materialize_explicit_paths: bool,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    build_oriented_path_tree_partition_with_backend(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        orientation,
        materialize_explicit_paths,
        RegionDualBackend::ReferenceAreaFloodFill,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_oriented_path_tree_partition_with_backend(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    orientation: PathTreeOrientation,
    materialize_explicit_paths: bool,
    dual_backend: RegionDualBackend,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    match orientation {
        PathTreeOrientation::VerticalTreeHorizontalPaths => {
            let partition = build_path_tree_partition_with_backend(
                prepared,
                boundary,
                horizontal_chords,
                vertical_chords,
                certificate,
                materialize_explicit_paths,
                dual_backend,
            )?;
            let biclique_partition = partition.biclique_partition.clone();
            let dual_region_count = partition.tree.region_count;
            let path_count = partition.compact_paths.len();
            let total_path_edge_incidences = partition.total_path_edge_incidences;
            let canonical_segment_node_count = partition.canonical_segment_node_count;
            Ok(OrientedPathTreePartition {
                orientation,
                path_tree: partition,
                dual_region_count,
                path_count,
                total_path_edge_incidences,
                canonical_segment_node_count,
                biclique_partition,
            })
        }
        PathTreeOrientation::HorizontalTreeVerticalPaths => {
            if dual_backend == RegionDualBackend::BoundaryLaminar && !materialize_explicit_paths {
                return build_horizontal_axis_view_partition(
                    boundary,
                    horizontal_chords,
                    vertical_chords,
                    certificate,
                );
            }
            let (transposed_horizontal, transposed_vertical) =
                transpose_chords(horizontal_chords, vertical_chords)?;
            let transposed_boundary = transpose_boundary(boundary);
            let transposed = if dual_backend == RegionDualBackend::BoundaryLaminar {
                // BoundaryLaminar never needs occupancy, prefix sums, or run
                // indexes. Reuse the prepared reference by view while
                // swapping only the combinatorial boundary/chord axes.
                build_path_tree_partition_with_backend(
                    prepared,
                    &transposed_boundary,
                    &transposed_horizontal,
                    &transposed_vertical,
                    certificate,
                    materialize_explicit_paths,
                    dual_backend,
                )?
            } else {
                let transposed_prepared = transpose_prepared(prepared);
                build_path_tree_partition_with_backend(
                    &transposed_prepared,
                    &boundary.clone(),
                    &transposed_horizontal,
                    &transposed_vertical,
                    certificate,
                    materialize_explicit_paths,
                    dual_backend,
                )?
            };
            let mut transposed = transposed;
            let bicliques = std::mem::take(&mut transposed.biclique_partition.bicliques)
                .into_iter()
                .map(|biclique| Biclique {
                    id: biclique.id,
                    left: biclique.right,
                    right: biclique.left,
                })
                .collect();
            let biclique_partition = BicliquePartition { bicliques };
            transposed.biclique_partition = biclique_partition.clone();
            Ok(OrientedPathTreePartition {
                orientation,
                dual_region_count: transposed.tree.region_count,
                path_count: transposed.compact_paths.len(),
                total_path_edge_incidences: transposed.total_path_edge_incidences,
                canonical_segment_node_count: transposed.canonical_segment_node_count,
                biclique_partition,
                path_tree: transposed,
            })
        }
    }
}

/// Builds both orientations and returns the one with the smaller compact
/// vertex-occurrence size. Ties are resolved in favor of the historical
/// vertical-tree orientation for stable certificates.
///
/// # Errors
///
/// Returns [`PathTreeError`] when either orientation cannot be constructed.
pub fn build_best_path_tree_partition(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    build_best_path_tree_partition_with_mode(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        true,
    )
}

pub fn build_best_path_tree_partition_with_mode(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    materialize_explicit_paths: bool,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    build_best_path_tree_partition_with_backend(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        materialize_explicit_paths,
        RegionDualBackend::ReferenceAreaFloodFill,
    )
}

pub fn build_best_path_tree_partition_with_backend(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    materialize_explicit_paths: bool,
    dual_backend: RegionDualBackend,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    build_path_tree_partition_with_orientation_policy(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        materialize_explicit_paths,
        dual_backend,
        PathTreeOrientationPolicy::BuildBothExact,
    )
}

/// Builds a path-tree partition using an explicit orientation policy.
///
/// `BuildBothExact` constructs both independent orientations and selects the
/// smaller actual sigma, preserving the historical behavior. `BoundEstimate`
/// computes the paper-shaped orientation bounds first and constructs only the
/// selected orientation. Ties use the vertical-tree orientation.
#[allow(clippy::too_many_arguments)]
pub fn build_path_tree_partition_with_orientation_policy(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    materialize_explicit_paths: bool,
    dual_backend: RegionDualBackend,
    policy: PathTreeOrientationPolicy,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    let orientation = match policy {
        PathTreeOrientationPolicy::VerticalTree => {
            return build_oriented_path_tree_partition_with_backend(
                prepared,
                boundary,
                horizontal_chords,
                vertical_chords,
                certificate,
                PathTreeOrientation::VerticalTreeHorizontalPaths,
                materialize_explicit_paths,
                dual_backend,
            );
        }
        PathTreeOrientationPolicy::HorizontalTree => {
            return build_oriented_path_tree_partition_with_backend(
                prepared,
                boundary,
                horizontal_chords,
                vertical_chords,
                certificate,
                PathTreeOrientation::HorizontalTreeVerticalPaths,
                materialize_explicit_paths,
                dual_backend,
            );
        }
        PathTreeOrientationPolicy::BuildBothExact => {
            return build_best_both(
                prepared,
                boundary,
                horizontal_chords,
                vertical_chords,
                certificate,
                materialize_explicit_paths,
                dual_backend,
            );
        }
        PathTreeOrientationPolicy::BoundEstimate => {
            estimate_orientation(horizontal_chords.len(), vertical_chords.len())
        }
    };
    build_oriented_path_tree_partition_with_backend(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        orientation,
        materialize_explicit_paths,
        dual_backend,
    )
}

fn build_best_both(
    prepared: &PreparedGridComponent,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    certificate: CleanHoleFreeCertificate,
    materialize_explicit_paths: bool,
    dual_backend: RegionDualBackend,
) -> Result<OrientedPathTreePartition, PathTreeError> {
    let vertical = build_oriented_path_tree_partition_with_backend(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate.clone(),
        PathTreeOrientation::VerticalTreeHorizontalPaths,
        materialize_explicit_paths,
        dual_backend,
    )?;
    let horizontal = build_oriented_path_tree_partition_with_backend(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        PathTreeOrientation::HorizontalTreeVerticalPaths,
        materialize_explicit_paths,
        dual_backend,
    )?;
    if horizontal.biclique_partition.total_vertex_occurrences()
        < vertical.biclique_partition.total_vertex_occurrences()
    {
        Ok(horizontal)
    } else {
        Ok(vertical)
    }
}

fn estimate_orientation(horizontal_count: usize, vertical_count: usize) -> PathTreeOrientation {
    let q = horizontal_count.saturating_add(vertical_count);
    let l = ceil_log2(q.saturating_add(1));
    let l_squared = l.saturating_mul(l);
    let vertical_estimate = horizontal_count
        .saturating_mul(l_squared)
        .saturating_add(vertical_count.saturating_mul(l));
    let horizontal_estimate = vertical_count
        .saturating_mul(l_squared)
        .saturating_add(horizontal_count.saturating_mul(l));
    if horizontal_estimate < vertical_estimate {
        PathTreeOrientation::HorizontalTreeVerticalPaths
    } else {
        PathTreeOrientation::VerticalTreeHorizontalPaths
    }
}

const fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

impl PathTreePartition {
    /// Audits the tree-derived edge partition against chord geometry.
    ///
    /// # Errors
    ///
    /// Returns [`PathTreeError::PartitionAudit`] when an edge is missing,
    /// fabricated, or represented with multiplicity other than one.
    pub fn audit_edge_partition(
        &self,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
    ) -> Result<PathTreeAudit, PathTreeError> {
        let explicit = horizontal_chords
            .iter()
            .enumerate()
            .flat_map(|(left, &horizontal)| {
                vertical_chords
                    .iter()
                    .enumerate()
                    .filter_map(move |(right, &vertical)| {
                        closed_chords_intersect(horizontal, vertical).then_some((left, right))
                    })
            })
            .collect::<BTreeSet<_>>();
        let mut multiplicities = BTreeMap::<(usize, usize), usize>::new();
        for biclique in &self.biclique_partition.bicliques {
            for &left in &biclique.left {
                for &right in &biclique.right {
                    *multiplicities.entry((left, right)).or_default() += 1;
                }
            }
        }
        let represented = multiplicities.keys().copied().collect::<BTreeSet<_>>();
        let audit = PathTreeAudit {
            explicit_edge_count: explicit.len(),
            represented_edge_count: multiplicities.values().sum(),
            duplicate_edge_count: multiplicities
                .values()
                .map(|count| count.saturating_sub(1))
                .sum(),
            missing_edge_count: explicit.difference(&represented).count(),
            fabricated_edge_count: represented.difference(&explicit).count(),
        };
        if audit.duplicate_edge_count != 0
            || audit.missing_edge_count != 0
            || audit.fabricated_edge_count != 0
        {
            return Err(PathTreeError::PartitionAudit { audit });
        }
        Ok(audit)
    }

    /// Verifies that every recovered tree path equals the geometric crossing set.
    ///
    /// # Errors
    ///
    /// Returns [`PathTreeError::MissingTreePath`] on a path mismatch.
    pub fn verify_paths(
        &self,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
    ) -> Result<(), PathTreeError> {
        if self.paths.is_empty() {
            for path in &self.compact_paths {
                let horizontal = horizontal_chords
                    .get(path.chord_index)
                    .ok_or(PathTreeError::MissingTreePath)?;
                let actual = vertical_chords
                    .iter()
                    .filter(|&&vertical| closed_chords_intersect(*horizontal, vertical))
                    .map(|vertical| vertical.id())
                    .collect::<BTreeSet<_>>();
                let mut represented = BTreeSet::new();
                for (chain, begin, end) in self
                    .hld
                    .decompose_path_endpoints(path.start_region, path.end_region)?
                {
                    represented.extend(self.hld.chain_edges[chain][begin..end].iter().copied());
                }
                if actual != represented {
                    return Err(PathTreeError::MissingTreePath);
                }
            }
            return Ok(());
        }
        for path in &self.paths {
            let horizontal = horizontal_chords[path.horizontal.0];
            let actual = vertical_chords
                .iter()
                .filter(|&&vertical| closed_chords_intersect(horizontal, vertical))
                .map(|vertical| vertical.id())
                .collect::<BTreeSet<_>>();
            let recovered = path.vertical_edges.iter().copied().collect::<BTreeSet<_>>();
            if actual != recovered || recovered.len() != path.vertical_edges.len() {
                return Err(PathTreeError::MissingTreePath);
            }
        }
        Ok(())
    }

    /// Independently compares endpoint-HLD paths with the explicit BFS oracle.
    /// This method is intentionally an audited-only operation.
    pub fn verify_endpoint_paths(
        &self,
        prepared: &PreparedGridComponent,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
    ) -> Result<(), PathTreeError> {
        let explicit = if self.tree.boundary_gap_regions.is_empty() {
            self.tree.horizontal_paths(prepared, horizontal_chords)?
        } else {
            self.tree
                .explicit_paths_from_compact(&self.compact_paths, horizontal_chords)?
        };
        if explicit.len() != self.compact_paths.len() {
            return Err(PathTreeError::MissingTreePath);
        }
        for (compact, audited) in self.compact_paths.iter().zip(explicit.iter()) {
            if compact.chord_index != audited.horizontal.0
                || compact.start_region != audited.start_region
                || compact.end_region != audited.end_region
            {
                return Err(PathTreeError::MissingTreePath);
            }
            let mut represented = BTreeSet::new();
            for (chain, begin, end) in self
                .hld
                .decompose_path_endpoints(compact.start_region, compact.end_region)?
            {
                represented.extend(self.hld.chain_edges[chain][begin..end].iter().copied());
            }
            let expected = audited
                .vertical_edges
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if represented != expected
                || expected.len() != audited.vertical_edges.len()
                || expected
                    .iter()
                    .any(|edge| !vertical_chords.iter().any(|chord| chord.id() == *edge))
            {
                return Err(PathTreeError::MissingTreePath);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use rect_core::{ColorGrid, validate_dissection};
    use rect_oracle_sg::{analyze_geometry, classify_clean_hole_free};

    use super::{
        DualRegionId, DualTreeEdge, HeavyLightDecomposition, PathTreeOrientation,
        PathTreeOrientationPolicy, RegionDualBackend, RegionDualTree, VerticalChordId,
        build_oriented_path_tree_partition, build_path_tree_partition,
        build_path_tree_partition_with_backend,
    };

    fn synthetic_tree(node_count: usize, edges: &[(usize, usize)]) -> RegionDualTree {
        let mut adjacency = vec![Vec::new(); node_count];
        let mut dual_edges = Vec::with_capacity(edges.len());
        for (index, &(first, second)) in edges.iter().enumerate() {
            let chord = VerticalChordId(index);
            dual_edges.push(DualTreeEdge {
                chord,
                first: DualRegionId(first),
                second: DualRegionId(second),
            });
            adjacency[first].push((DualRegionId(second), chord));
            adjacency[second].push((DualRegionId(first), chord));
        }
        for neighbors in &mut adjacency {
            neighbors.sort_unstable();
        }
        RegionDualTree {
            region_count: node_count,
            edges: dual_edges,
            adjacency,
            cell_region_ids: Vec::new(),
            boundary_gap_regions: Vec::new(),
        }
    }

    fn assert_decomposes_exactly(tree: &RegionDualTree, path: &[VerticalChordId]) {
        let hld = HeavyLightDecomposition::new(tree).unwrap();
        for &edge in path {
            assert!(
                hld.edge_locations.contains_key(&edge),
                "missing edge {edge:?}"
            );
        }
        let intervals = hld.path_intervals(path);
        let mut represented = BTreeMap::<VerticalChordId, usize>::new();
        for (chain, begin, end) in intervals {
            assert!(begin < end);
            for &edge in &hld.chain_edges[chain][begin..end] {
                *represented.entry(edge).or_default() += 1;
            }
        }
        let mut expected = BTreeMap::<VerticalChordId, usize>::new();
        for &edge in path {
            *expected.entry(edge).or_default() += 1;
        }
        assert_eq!(represented, expected);
    }

    #[test]
    fn hld_decomposes_path_tree_shapes_without_edge_loss() {
        let cases = [
            (
                synthetic_tree(6, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]),
                vec![VerticalChordId(4), VerticalChordId(3), VerticalChordId(2)],
            ),
            (
                synthetic_tree(5, &[(0, 1), (0, 2), (0, 3), (0, 4)]),
                vec![VerticalChordId(0), VerticalChordId(3)],
            ),
            (
                synthetic_tree(7, &[(0, 1), (0, 2), (1, 3), (1, 4), (2, 5), (2, 6)]),
                vec![
                    VerticalChordId(4),
                    VerticalChordId(1),
                    VerticalChordId(0),
                    VerticalChordId(3),
                ],
            ),
            (
                synthetic_tree(8, &[(0, 1), (1, 2), (1, 3), (3, 4), (0, 5), (5, 6), (6, 7)]),
                vec![VerticalChordId(1), VerticalChordId(2), VerticalChordId(3)],
            ),
        ];
        for (tree, path) in cases {
            assert_decomposes_exactly(&tree, &path);
        }
    }

    #[test]
    fn orientation_bound_policy_is_stable_and_tie_breaks_vertical() {
        assert_eq!(
            super::estimate_orientation(2, 8),
            PathTreeOrientation::VerticalTreeHorizontalPaths
        );
        assert_eq!(
            super::estimate_orientation(8, 2),
            PathTreeOrientation::HorizontalTreeVerticalPaths
        );
        assert_eq!(
            super::estimate_orientation(4, 4),
            PathTreeOrientation::VerticalTreeHorizontalPaths
        );
        assert_eq!(
            PathTreeOrientationPolicy::BoundEstimate.name(),
            "bound-estimate"
        );
    }

    #[test]
    fn clean_dual_tree_paths_match_geometry_on_small_grids() {
        let mut audited = 0;
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(3, 3, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let Ok(geometry) = analyze_geometry(&component) else {
                    continue;
                };
                let certificate = classify_clean_hole_free(
                    &component,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                );
                if !certificate.eligible || geometry.vertical_chords.is_empty() {
                    continue;
                }
                let partition = build_path_tree_partition(
                    &geometry.prepared,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                    certificate,
                )
                .unwrap();
                partition
                    .verify_paths(&geometry.horizontal_chords, &geometry.vertical_chords)
                    .unwrap();
                partition
                    .audit_edge_partition(&geometry.horizontal_chords, &geometry.vertical_chords)
                    .unwrap();
                assert_eq!(partition.tree.edges.len() + 1, partition.tree.region_count);
                audited += 1;
            }
        }
        assert!(audited > 0);
    }

    #[test]
    fn boundary_axis_view_matches_transposed_reference_on_small_clean_population() {
        let mut compared = 0;
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(3, 3, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let Ok(geometry) = analyze_geometry(&component) else {
                    continue;
                };
                let certificate = classify_clean_hole_free(
                    &component,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                );
                if !certificate.eligible {
                    continue;
                }
                let axis = super::build_oriented_path_tree_partition_with_backend(
                    &geometry.prepared,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                    certificate.clone(),
                    PathTreeOrientation::HorizontalTreeVerticalPaths,
                    false,
                    RegionDualBackend::BoundaryLaminar,
                )
                .unwrap();
                let reference = super::build_oriented_path_tree_partition_with_backend(
                    &geometry.prepared,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                    certificate,
                    PathTreeOrientation::HorizontalTreeVerticalPaths,
                    false,
                    RegionDualBackend::ReferenceAreaFloodFill,
                )
                .unwrap();
                let normalize = |partition: &super::BicliquePartition| {
                    let mut rows = partition
                        .bicliques
                        .iter()
                        .map(|biclique| (biclique.left.clone(), biclique.right.clone()))
                        .collect::<Vec<_>>();
                    rows.sort_unstable();
                    rows
                };
                assert_eq!(
                    normalize(&axis.biclique_partition),
                    normalize(&reference.biclique_partition)
                );
                assert_eq!(
                    axis.total_path_edge_incidences,
                    reference.total_path_edge_incidences
                );
                assert_eq!(
                    axis.biclique_partition.total_vertex_occurrences(),
                    reference.biclique_partition.total_vertex_occurrences()
                );
                compared += 1;
            }
        }
        assert!(compared > 0);
    }

    #[test]
    fn both_path_tree_orientations_partition_clean_conflicts() {
        let mut compared = 0;
        for mask in 1_u32..(1_u32 << 16) {
            let cells = (0..16)
                .map(|index| mask & (1_u32 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(4, 4, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let Ok(geometry) = analyze_geometry(&component) else {
                    continue;
                };
                let certificate = classify_clean_hole_free(
                    &component,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                );
                if !certificate.eligible
                    || geometry.horizontal_chords.is_empty()
                    || geometry.vertical_chords.is_empty()
                {
                    continue;
                }
                let graph = rect_oracle_sg::build_conflict_graph(
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                )
                .unwrap();
                for orientation in [
                    PathTreeOrientation::VerticalTreeHorizontalPaths,
                    PathTreeOrientation::HorizontalTreeVerticalPaths,
                ] {
                    let partition = build_oriented_path_tree_partition(
                        &geometry.prepared,
                        &geometry.boundary,
                        &geometry.horizontal_chords,
                        &geometry.vertical_chords,
                        certificate.clone(),
                        orientation,
                    )
                    .unwrap();
                    partition
                        .biclique_partition
                        .verify_exact_partition(&graph)
                        .unwrap();
                }
                compared += 1;
            }
        }
        assert!(compared > 0);
    }

    #[test]
    fn path_tree_solver_matches_general_four_dimensional_solver() {
        let mut compared = 0;
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(3, 3, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let Ok(path_tree) = crate::solve_with_representation(
                    &component,
                    crate::VerificationMode::FullyAudited,
                    crate::ConflictRepresentationBackend::CleanHoleFreePathTree,
                    crate::ChordEnumerator::GridInteriorRuns,
                    rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
                ) else {
                    continue;
                };
                let general = crate::solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                    &component,
                    crate::VerificationMode::FullyAudited,
                    crate::ChordEnumerator::GridInteriorRuns,
                    rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
                )
                .unwrap();
                assert_eq!(
                    path_tree.optimum_rectangle_count,
                    general.optimum_rectangle_count
                );
                assert_eq!(path_tree.rectangles, general.rectangles);
                validate_dissection(&component, &path_tree).unwrap();
                compared += 1;
            }
        }
        assert!(compared > 0);
    }

    #[test]
    fn compact_boundary_dual_matches_general_on_small_clean_population() {
        let mut compared = 0;
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(3, 3, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let Ok(path_tree) = crate::solve_with_representation(
                    &component,
                    crate::VerificationMode::CompactOnly,
                    crate::ConflictRepresentationBackend::CleanHoleFreePathTree,
                    crate::ChordEnumerator::GridInteriorRuns,
                    rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
                ) else {
                    continue;
                };
                let general = crate::solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                    &component,
                    crate::VerificationMode::CompactOnly,
                    crate::ChordEnumerator::GridInteriorRuns,
                    rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
                )
                .unwrap();
                assert_eq!(
                    path_tree.optimum_rectangle_count,
                    general.optimum_rectangle_count
                );
                assert_eq!(path_tree.rectangles, general.rectangles);
                assert_eq!(
                    path_tree.diagnostics.region_dual_backend.as_deref(),
                    Some("boundary-laminar")
                );
                assert!(
                    !path_tree
                        .diagnostics
                        .execution_trace
                        .full_tree_path_edge_lists_materialized
                );
                compared += 1;
            }
        }
        assert!(compared > 0);
    }

    #[test]
    fn boundary_laminar_dual_matches_area_dual_paths_on_small_population() {
        let mut compared = 0;
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(3, 3, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let Ok(geometry) = analyze_geometry(&component) else {
                    continue;
                };
                let certificate = classify_clean_hole_free(
                    &component,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                );
                if !certificate.eligible || geometry.vertical_chords.is_empty() {
                    continue;
                }
                let area = build_path_tree_partition_with_backend(
                    &geometry.prepared,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                    certificate.clone(),
                    false,
                    RegionDualBackend::ReferenceAreaFloodFill,
                )
                .unwrap();
                let laminar = build_path_tree_partition_with_backend(
                    &geometry.prepared,
                    &geometry.boundary,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                    certificate,
                    false,
                    RegionDualBackend::BoundaryLaminar,
                )
                .unwrap();
                for (left, right) in area.compact_paths.iter().zip(laminar.compact_paths.iter()) {
                    let area_edges = area
                        .hld
                        .decompose_path_endpoints(left.start_region, left.end_region)
                        .unwrap()
                        .into_iter()
                        .flat_map(|(chain, begin, end)| {
                            area.hld.chain_edges[chain][begin..end].iter().copied()
                        })
                        .collect::<BTreeSet<_>>();
                    let laminar_edges = laminar
                        .hld
                        .decompose_path_endpoints(right.start_region, right.end_region)
                        .unwrap()
                        .into_iter()
                        .flat_map(|(chain, begin, end)| {
                            laminar.hld.chain_edges[chain][begin..end].iter().copied()
                        })
                        .collect::<BTreeSet<_>>();
                    assert_eq!(area_edges, laminar_edges);
                }
                compared += 1;
            }
        }
        assert!(compared > 0);
    }

    #[test]
    #[ignore = "full 4x4 compact boundary differential campaign"]
    fn compact_boundary_dual_matches_general_on_all_clean_four_by_four() {
        let mut compared = 0usize;
        for mask in 1_u32..(1_u32 << 16) {
            let cells = (0..16)
                .map(|index| mask & (1_u32 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(4, 4, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let Ok(path_tree) = crate::solve_with_representation(
                    &component,
                    crate::VerificationMode::CompactOnly,
                    crate::ConflictRepresentationBackend::CleanHoleFreePathTree,
                    crate::ChordEnumerator::GridInteriorRuns,
                    rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
                ) else {
                    continue;
                };
                let general = crate::solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                    &component,
                    crate::VerificationMode::CompactOnly,
                    crate::ChordEnumerator::GridInteriorRuns,
                    rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
                )
                .unwrap();
                assert_eq!(path_tree.rectangles, general.rectangles);
                assert_eq!(
                    path_tree.optimum_rectangle_count,
                    general.optimum_rectangle_count
                );
                compared += 1;
            }
        }
        assert_eq!(compared, 155_389);
    }

    #[test]
    fn auto_representation_falls_back_for_holes() {
        let cells = (0..9).map(|index| index != 4).collect::<Vec<_>>();
        let component = ColorGrid::new(3, 3, cells)
            .unwrap()
            .four_connected_components()
            .into_iter()
            .find(|component| component.color)
            .unwrap();
        let result = crate::solve_with_representation(
            &component,
            crate::VerificationMode::CompactOnly,
            crate::ConflictRepresentationBackend::Auto,
            crate::ChordEnumerator::GridInteriorRuns,
            rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
        )
        .unwrap();
        assert_eq!(
            result.diagnostics.conflict_representation.as_deref(),
            Some("dominance-4d")
        );
        assert_eq!(result.diagnostics.clean_hole_free_eligible, Some(false));
        assert_eq!(result.diagnostics.explicit_conflict_edge_count, None);
        assert!(matches!(
            crate::solve_with_representation(
                &component,
                crate::VerificationMode::CompactOnly,
                crate::ConflictRepresentationBackend::CleanHoleFreePathTree,
                crate::ChordEnumerator::GridInteriorRuns,
                rect_oracle_sg::CompletionBackendKind::ReferenceRescan,
            ),
            Err(crate::DominanceError::PathTreeIneligible(_))
        ));
    }
}
