//! Geometry-derived clean hole-free path/tree representation.
//!
//! The builder is intentionally a grid-specific reference implementation. It
//! uses local occupancy flood-fill and therefore documents an area-sensitive
//! construction; it is not the paper's planar sweep implementation.

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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChordTreePath {
    pub horizontal: HorizontalChordId,
    pub start_region: DualRegionId,
    pub end_region: DualRegionId,
    pub vertical_edges: Vec<VerticalChordId>,
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
    })
}

impl RegionDualTree {
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
    let _ = boundary;
    let tree = build_vertical_dual_tree(prepared, vertical_chords, &certificate)?;
    let paths = tree.horizontal_paths(prepared, horizontal_chords)?;
    let hld = HeavyLightDecomposition::new(&tree)?;
    let mut grouped = BTreeMap::<(usize, usize, usize), Vec<usize>>::new();
    let mut total_path_edge_incidences = 0;
    for (path_index, path) in paths.iter().enumerate() {
        total_path_edge_incidences += path.vertical_edges.len();
        for node in hld.path_intervals(&path.vertical_edges) {
            grouped.entry(node).or_default().push(path_index);
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
        hld,
        canonical_segment_node_count: bicliques.len(),
        total_path_edge_incidences,
        biclique_partition: BicliquePartition { bicliques },
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
    match orientation {
        PathTreeOrientation::VerticalTreeHorizontalPaths => {
            let partition = build_path_tree_partition(
                prepared,
                boundary,
                horizontal_chords,
                vertical_chords,
                certificate,
            )?;
            let biclique_partition = partition.biclique_partition.clone();
            let dual_region_count = partition.tree.region_count;
            let path_count = partition.paths.len();
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
            let transposed_prepared = transpose_prepared(prepared);
            let (transposed_horizontal, transposed_vertical) =
                transpose_chords(horizontal_chords, vertical_chords)?;
            let transposed = build_path_tree_partition(
                &transposed_prepared,
                boundary,
                &transposed_horizontal,
                &transposed_vertical,
                certificate,
            )?;
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
                path_count: transposed.paths.len(),
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
    let vertical = build_oriented_path_tree_partition(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate.clone(),
        PathTreeOrientation::VerticalTreeHorizontalPaths,
    )?;
    let horizontal = build_oriented_path_tree_partition(
        prepared,
        boundary,
        horizontal_chords,
        vertical_chords,
        certificate,
        PathTreeOrientation::HorizontalTreeVerticalPaths,
    )?;
    if horizontal.biclique_partition.total_vertex_occurrences()
        < vertical.biclique_partition.total_vertex_occurrences()
    {
        Ok(horizontal)
    } else {
        Ok(vertical)
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
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rect_core::{ColorGrid, validate_dissection};
    use rect_oracle_sg::{analyze_geometry, classify_clean_hole_free};

    use super::{
        DualRegionId, DualTreeEdge, HeavyLightDecomposition, PathTreeOrientation, RegionDualTree,
        VerticalChordId, build_oriented_path_tree_partition, build_path_tree_partition,
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
