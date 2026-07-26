//! Prepared exact indexes for boundary-native ordinary polygons.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Boundary, BoundaryIndex, BoundaryIndexError, BoundaryLoopId, Coord, DoubledPoint, Point,
    PolygonError, PolygonLoopId, PolygonValidationBackend, PolygonValidator, RectilinearPolygon,
    ReferenceQuadraticValidator,
};

/// Selects the exact geometry-query implementation used by a polygon pipeline.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonGeometryBackend {
    /// The v0.9 linear edge scans.
    ReferenceScan,
    /// Static exact orthogonal indexes.
    #[default]
    Indexed,
}

impl PolygonGeometryBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferenceScan => "reference-scan",
            Self::Indexed => "indexed",
        }
    }
}

/// Axis direction used by exact boundary ray shooting.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrthogonalDirection {
    East,
    North,
    West,
    South,
}

/// Stable record for one normalized polygon boundary edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IndexedBoundaryEdge {
    pub loop_id: BoundaryLoopId,
    pub edge_index: usize,
    pub first: Point,
    pub second: Point,
}

impl IndexedBoundaryEdge {
    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        self.first.y == self.second.y
    }

    #[must_use]
    pub const fn left(self) -> Coord {
        if self.first.x < self.second.x {
            self.first.x
        } else {
            self.second.x
        }
    }

    #[must_use]
    pub const fn right(self) -> Coord {
        if self.first.x > self.second.x {
            self.first.x
        } else {
            self.second.x
        }
    }

    #[must_use]
    pub const fn bottom(self) -> Coord {
        if self.first.y < self.second.y {
            self.first.y
        } else {
            self.second.y
        }
    }

    #[must_use]
    pub const fn top(self) -> Coord {
        if self.first.y > self.second.y {
            self.first.y
        } else {
            self.second.y
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AxisStabbingIndex {
    coordinates: Vec<Coord>,
    nodes: Vec<Vec<(Coord, usize)>>,
}

impl AxisStabbingIndex {
    fn new(edges: &[IndexedBoundaryEdge], vertical: bool, closed: bool) -> Self {
        let mut coordinates = edges
            .iter()
            .filter(|edge| edge.is_horizontal() != vertical)
            .flat_map(|edge| {
                if vertical {
                    [edge.bottom(), edge.top()]
                } else {
                    [edge.left(), edge.right()]
                }
            })
            .collect::<Vec<_>>();
        coordinates.sort_unstable();
        coordinates.dedup();
        let slot_count = coordinates.len().saturating_mul(2).saturating_sub(1);
        let mut index = Self {
            coordinates,
            nodes: vec![Vec::new(); slot_count.saturating_mul(4).max(1)],
        };
        if slot_count == 0 {
            return index;
        }
        for (edge_id, edge) in edges.iter().copied().enumerate() {
            if edge.is_horizontal() == vertical {
                continue;
            }
            let (low, high, key) = if vertical {
                (edge.bottom(), edge.top(), edge.first.x)
            } else {
                (edge.left(), edge.right(), edge.first.y)
            };
            let low_index = index
                .coordinates
                .binary_search(&low)
                .expect("edge endpoint indexed");
            let high_index = index
                .coordinates
                .binary_search(&high)
                .expect("edge endpoint indexed");
            let query_left = low_index * 2;
            let query_right = if closed {
                high_index * 2
            } else {
                high_index * 2 - 1
            };
            index.insert(
                1,
                0,
                slot_count - 1,
                query_left,
                query_right,
                (key, edge_id),
            );
        }
        for node in &mut index.nodes {
            node.sort_unstable();
        }
        index
    }

    #[allow(clippy::too_many_arguments)]
    fn insert(
        &mut self,
        node: usize,
        left: usize,
        right: usize,
        query_left: usize,
        query_right: usize,
        value: (Coord, usize),
    ) {
        if query_left <= left && right <= query_right {
            self.nodes[node].push(value);
            return;
        }
        let middle = left + (right - left) / 2;
        if query_left <= middle {
            self.insert(node * 2, left, middle, query_left, query_right, value);
        }
        if middle < query_right {
            self.insert(
                node * 2 + 1,
                middle + 1,
                right,
                query_left,
                query_right,
                value,
            );
        }
    }

    fn slot(&self, doubled_coordinate: i128) -> Option<usize> {
        if self.coordinates.is_empty() {
            return None;
        }
        let insertion = self
            .coordinates
            .partition_point(|&coordinate| 2 * i128::from(coordinate) < doubled_coordinate);
        if insertion < self.coordinates.len()
            && 2 * i128::from(self.coordinates[insertion]) == doubled_coordinate
        {
            return Some(insertion * 2);
        }
        if insertion == 0 || insertion == self.coordinates.len() {
            return None;
        }
        Some((insertion - 1) * 2 + 1)
    }

    fn visit_nodes(&self, doubled_coordinate: i128, mut visit: impl FnMut(&[(Coord, usize)])) {
        let Some(slot) = self.slot(doubled_coordinate) else {
            return;
        };
        let slot_count = self.coordinates.len() * 2 - 1;
        let (mut node, mut left, mut right) = (1, 0, slot_count - 1);
        loop {
            visit(&self.nodes[node]);
            if left == right {
                break;
            }
            let middle = left + (right - left) / 2;
            if slot <= middle {
                node *= 2;
                right = middle;
            } else {
                node = node * 2 + 1;
                left = middle + 1;
            }
        }
    }

    fn report_open_key_range(
        &self,
        doubled_coordinate: i128,
        low: Coord,
        high: Coord,
    ) -> Vec<usize> {
        let mut result = Vec::new();
        self.visit_nodes(doubled_coordinate, |values| {
            let start = values.partition_point(|&(key, _)| key <= low);
            let end = values.partition_point(|&(key, _)| key < high);
            result.extend(values[start..end].iter().map(|&(_, edge_id)| edge_id));
        });
        result.sort_unstable();
        result.dedup();
        result
    }

    fn report_closed_key_range(
        &self,
        doubled_coordinate: i128,
        low: Coord,
        high: Coord,
    ) -> Vec<usize> {
        let mut result = Vec::new();
        self.visit_nodes(doubled_coordinate, |values| {
            let start = values.partition_point(|&(key, _)| key < low);
            let end = values.partition_point(|&(key, _)| key <= high);
            result.extend(values[start..end].iter().map(|&(_, edge_id)| edge_id));
        });
        result.sort_unstable();
        result.dedup();
        result
    }

    fn report_keys_right_of(&self, doubled_coordinate: i128, doubled_key: i128) -> Vec<usize> {
        let mut result = Vec::new();
        self.visit_nodes(doubled_coordinate, |values| {
            let start = values.partition_point(|&(key, _)| 2 * i128::from(key) <= doubled_key);
            result.extend(values[start..].iter().map(|&(_, edge_id)| edge_id));
        });
        result.sort_unstable();
        result.dedup();
        result
    }

    fn count_keys_right_of(&self, doubled_coordinate: i128, doubled_key: i128) -> usize {
        let mut count = 0;
        self.visit_nodes(doubled_coordinate, |values| {
            let start = values.partition_point(|&(key, _)| 2 * i128::from(key) <= doubled_key);
            count += values.len() - start;
        });
        count
    }

    fn nearest_greater(&self, doubled_coordinate: i128, key: Coord) -> Option<Coord> {
        let mut nearest = None;
        self.visit_nodes(doubled_coordinate, |values| {
            let index = values.partition_point(|&(value, _)| value <= key);
            if let Some(&(value, _)) = values.get(index) {
                nearest = Some(nearest.map_or(value, |current: Coord| current.min(value)));
            }
        });
        nearest
    }

    fn nearest_less(&self, doubled_coordinate: i128, key: Coord) -> Option<Coord> {
        let mut nearest = None;
        self.visit_nodes(doubled_coordinate, |values| {
            let index = values.partition_point(|&(value, _)| value < key);
            if let Some(&(value, _)) = index.checked_sub(1).and_then(|index| values.get(index)) {
                nearest = Some(nearest.map_or(value, |current: Coord| current.max(value)));
            }
        });
        nearest
    }

    fn owned_bytes_estimate(&self) -> usize {
        self.coordinates.len() * std::mem::size_of::<Coord>()
            + self
                .nodes
                .iter()
                .map(|node| node.len() * std::mem::size_of::<(Coord, usize)>())
                .sum::<usize>()
    }
}

/// Exact static index over normalized horizontal and vertical boundary edges.
///
/// Stabbing queries use a segment tree over doubled-coordinate point slots.
/// Point-location crossings use half-open `[bottom, top)` vertical intervals;
/// boundary reporting and ray shooting use closed intervals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrthogonalEdgeIndex {
    edges: Vec<IndexedBoundaryEdge>,
    horizontal_by_y: BTreeMap<Coord, Vec<usize>>,
    vertical_by_x: BTreeMap<Coord, Vec<usize>>,
    vertical_half_open: AxisStabbingIndex,
    vertical_closed: AxisStabbingIndex,
    horizontal_closed: AxisStabbingIndex,
}

impl OrthogonalEdgeIndex {
    #[must_use]
    pub fn new(boundary: &Boundary) -> Self {
        let mut edges = Vec::with_capacity(boundary.boundary_complexity());
        let mut horizontal_by_y = BTreeMap::<Coord, Vec<usize>>::new();
        let mut vertical_by_x = BTreeMap::<Coord, Vec<usize>>::new();
        for (loop_index, boundary_loop) in boundary.loops.iter().enumerate() {
            for edge_index in 0..boundary_loop.vertices.len() {
                let first = boundary_loop.vertices[edge_index];
                let second =
                    boundary_loop.vertices[(edge_index + 1) % boundary_loop.vertices.len()];
                let edge_id = edges.len();
                let edge = IndexedBoundaryEdge {
                    loop_id: BoundaryLoopId(loop_index),
                    edge_index,
                    first,
                    second,
                };
                edges.push(edge);
                if edge.is_horizontal() {
                    horizontal_by_y.entry(first.y).or_default().push(edge_id);
                } else {
                    vertical_by_x.entry(first.x).or_default().push(edge_id);
                }
            }
        }
        for edge_ids in horizontal_by_y.values_mut() {
            edge_ids.sort_unstable_by_key(|&edge_id| {
                let edge = edges[edge_id];
                (edge.left(), edge.right(), edge.loop_id, edge.edge_index)
            });
        }
        for edge_ids in vertical_by_x.values_mut() {
            edge_ids.sort_unstable_by_key(|&edge_id| {
                let edge = edges[edge_id];
                (edge.bottom(), edge.top(), edge.loop_id, edge.edge_index)
            });
        }
        Self {
            vertical_half_open: AxisStabbingIndex::new(&edges, true, false),
            vertical_closed: AxisStabbingIndex::new(&edges, true, true),
            horizontal_closed: AxisStabbingIndex::new(&edges, false, true),
            edges,
            horizontal_by_y,
            vertical_by_x,
        }
    }

    #[must_use]
    pub fn edge(&self, edge_id: usize) -> Option<IndexedBoundaryEdge> {
        self.edges.get(edge_id).copied()
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub fn horizontal_edge_ids_on_line(&self, y: Coord) -> &[usize] {
        self.horizontal_by_y.get(&y).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn vertical_edge_ids_on_line(&self, x: Coord) -> &[usize] {
        self.vertical_by_x.get(&x).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn report_vertical_crossings(
        &self,
        doubled_y: i128,
        left: Coord,
        right: Coord,
    ) -> Vec<usize> {
        self.vertical_closed
            .report_open_key_range(doubled_y, left, right)
    }

    #[must_use]
    pub fn report_horizontal_crossings(
        &self,
        doubled_x: i128,
        bottom: Coord,
        top: Coord,
    ) -> Vec<usize> {
        self.horizontal_closed
            .report_open_key_range(doubled_x, bottom, top)
    }

    #[must_use]
    pub fn report_vertical_crossings_closed(
        &self,
        doubled_y: i128,
        left: Coord,
        right: Coord,
    ) -> Vec<usize> {
        self.vertical_closed
            .report_closed_key_range(doubled_y, left, right)
    }

    #[must_use]
    pub fn report_horizontal_crossings_closed(
        &self,
        doubled_x: i128,
        bottom: Coord,
        top: Coord,
    ) -> Vec<usize> {
        self.horizontal_closed
            .report_closed_key_range(doubled_x, bottom, top)
    }

    /// Reports all vertical edges active at a doubled scanline under the
    /// point-location half-open `[bottom, top)` convention.
    #[must_use]
    pub fn active_vertical_edge_ids(&self, doubled_y: i128) -> Vec<usize> {
        self.vertical_half_open
            .report_keys_right_of(doubled_y, i128::MIN)
    }

    #[must_use]
    pub fn point_on_boundary(&self, point: DoubledPoint) -> bool {
        if point.y % 2 == 0 {
            let y = point.y / 2;
            if let Ok(y) = Coord::try_from(y)
                && self.horizontal_edge_ids_on_line(y).iter().any(|&edge_id| {
                    let edge = self.edges[edge_id];
                    2 * i128::from(edge.left()) <= point.x
                        && point.x <= 2 * i128::from(edge.right())
                })
            {
                return true;
            }
        }
        if point.x % 2 == 0 {
            let x = point.x / 2;
            if let Ok(x) = Coord::try_from(x)
                && self.vertical_edge_ids_on_line(x).iter().any(|&edge_id| {
                    let edge = self.edges[edge_id];
                    2 * i128::from(edge.bottom()) <= point.y
                        && point.y <= 2 * i128::from(edge.top())
                })
            {
                return true;
            }
        }
        false
    }

    /// Exact strict point location using the half-open vertical crossing rule.
    #[must_use]
    pub fn contains_doubled_point_strict(&self, point: DoubledPoint) -> bool {
        !self.point_on_boundary(point)
            && self
                .vertical_half_open
                .count_keys_right_of(point.y, point.x)
                % 2
                == 1
    }

    /// Returns loop identities whose bounded interiors contain the point.
    #[must_use]
    pub fn containing_loop_ids(&self, point: DoubledPoint) -> BTreeSet<BoundaryLoopId> {
        let mut parity = BTreeMap::<BoundaryLoopId, bool>::new();
        for edge_id in self
            .vertical_half_open
            .report_keys_right_of(point.y, point.x)
        {
            let entry = parity.entry(self.edges[edge_id].loop_id).or_default();
            *entry = !*entry;
        }
        parity
            .into_iter()
            .filter_map(|(loop_id, inside)| inside.then_some(loop_id))
            .collect()
    }

    #[must_use]
    pub fn horizontal_collinear_overlap(&self, y: Coord, left: Coord, right: Coord) -> bool {
        self.horizontal_edge_ids_on_line(y).iter().any(|&edge_id| {
            let edge = self.edges[edge_id];
            left.max(edge.left()) < right.min(edge.right())
        })
    }

    #[must_use]
    pub fn vertical_collinear_overlap(&self, x: Coord, bottom: Coord, top: Coord) -> bool {
        self.vertical_edge_ids_on_line(x).iter().any(|&edge_id| {
            let edge = self.edges[edge_id];
            bottom.max(edge.bottom()) < top.min(edge.top())
        })
    }

    #[must_use]
    pub fn nearest_boundary_blocker(
        &self,
        point: Point,
        direction: OrthogonalDirection,
    ) -> Option<Point> {
        match direction {
            OrthogonalDirection::East => {
                let perpendicular = self
                    .vertical_closed
                    .nearest_greater(2 * i128::from(point.y), point.x);
                let collinear = self
                    .horizontal_edge_ids_on_line(point.y)
                    .iter()
                    .filter_map(|&edge_id| {
                        let edge = self.edges[edge_id];
                        (edge.left() > point.x).then_some(edge.left())
                    })
                    .min();
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .min()
                    .map(|x| Point::new(x, point.y))
            }
            OrthogonalDirection::West => {
                let perpendicular = self
                    .vertical_closed
                    .nearest_less(2 * i128::from(point.y), point.x);
                let collinear = self
                    .horizontal_edge_ids_on_line(point.y)
                    .iter()
                    .filter_map(|&edge_id| {
                        let edge = self.edges[edge_id];
                        (edge.right() < point.x).then_some(edge.right())
                    })
                    .max();
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .max()
                    .map(|x| Point::new(x, point.y))
            }
            OrthogonalDirection::North => {
                let perpendicular = self
                    .horizontal_closed
                    .nearest_greater(2 * i128::from(point.x), point.y);
                let collinear = self
                    .vertical_edge_ids_on_line(point.x)
                    .iter()
                    .filter_map(|&edge_id| {
                        let edge = self.edges[edge_id];
                        (edge.bottom() > point.y).then_some(edge.bottom())
                    })
                    .min();
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .min()
                    .map(|y| Point::new(point.x, y))
            }
            OrthogonalDirection::South => {
                let perpendicular = self
                    .horizontal_closed
                    .nearest_less(2 * i128::from(point.x), point.y);
                let collinear = self
                    .vertical_edge_ids_on_line(point.x)
                    .iter()
                    .filter_map(|&edge_id| {
                        let edge = self.edges[edge_id];
                        (edge.top() < point.y).then_some(edge.top())
                    })
                    .max();
                perpendicular
                    .into_iter()
                    .chain(collinear)
                    .max()
                    .map(|y| Point::new(point.x, y))
            }
        }
    }

    #[must_use]
    pub fn owned_bytes_estimate(&self) -> usize {
        self.edges.len() * std::mem::size_of::<IndexedBoundaryEdge>()
            + self
                .horizontal_by_y
                .values()
                .chain(self.vertical_by_x.values())
                .map(|edge_ids| edge_ids.len() * std::mem::size_of::<usize>())
                .sum::<usize>()
            + self.vertical_half_open.owned_bytes_estimate()
            + self.vertical_closed.owned_bytes_estimate()
            + self.horizontal_closed.owned_bytes_estimate()
    }
}

/// Stable broad category used by validator differential reports.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonErrorCategory {
    NonAxisAligned,
    ZeroLength,
    TooFewVertices,
    SelfIntersection,
    BoundaryTouch,
    DuplicateBoundaryElement,
    HoleOutsideOuter,
    HoleIntersection,
    NestedHole,
    WrongOrientation,
    AreaOverflow,
    DisconnectedInterior,
    UnsupportedDegeneracy,
}

impl PolygonErrorCategory {
    #[must_use]
    pub const fn from_error(error: &PolygonError) -> Self {
        match error {
            PolygonError::NonAxisAlignedEdge { .. } => Self::NonAxisAligned,
            PolygonError::ZeroLengthEdge { .. } => Self::ZeroLength,
            PolygonError::TooFewVertices { .. } => Self::TooFewVertices,
            PolygonError::SelfIntersection { .. } => Self::SelfIntersection,
            PolygonError::NonAdjacentBoundaryTouch { .. } => Self::BoundaryTouch,
            PolygonError::DuplicateVertex { .. } | PolygonError::DuplicateEdge { .. } => {
                Self::DuplicateBoundaryElement
            }
            PolygonError::HoleOutsideOuter { .. } => Self::HoleOutsideOuter,
            PolygonError::HoleIntersectsOuter { .. } | PolygonError::HoleIntersectsHole { .. } => {
                Self::HoleIntersection
            }
            PolygonError::NestedHole { .. } => Self::NestedHole,
            PolygonError::WrongOrientation { .. } => Self::WrongOrientation,
            PolygonError::AreaOverflow => Self::AreaOverflow,
            PolygonError::DisconnectedInterior => Self::DisconnectedInterior,
            PolygonError::UnsupportedDegenerateBoundary { .. } => Self::UnsupportedDegeneracy,
        }
    }
}

/// Deterministic exact orthogonal range-sweep validator.
#[derive(Clone, Copy, Debug, Default)]
pub struct OrthogonalSweepValidator;

impl PolygonValidator for OrthogonalSweepValidator {
    fn validate(&self, polygon: &RectilinearPolygon) -> Result<(), PolygonError> {
        validate_polygon_sweep(polygon)
    }

    fn name(&self) -> &'static str {
        PolygonValidationBackend::OrthogonalSweep.name()
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SweepFailureKey {
    x: Coord,
    y: Coord,
    relation: u8,
    first_loop: usize,
    first_edge: usize,
    second_loop: usize,
    second_edge: usize,
}

fn validate_polygon_sweep(polygon: &RectilinearPolygon) -> Result<(), PolygonError> {
    validate_loop_linear(&polygon.outer, false, PolygonLoopId(0))?;
    for (index, boundary_loop) in polygon.holes.iter().enumerate() {
        validate_loop_linear(boundary_loop, true, PolygonLoopId(index + 1))?;
    }

    let boundary = Boundary::from_polygon(polygon);
    let edge_index = OrthogonalEdgeIndex::new(&boundary);
    validate_polygon_sweep_with_indexes(polygon, &boundary, &edge_index)
}

fn validate_polygon_sweep_with_indexes(
    polygon: &RectilinearPolygon,
    boundary: &Boundary,
    edge_index: &OrthogonalEdgeIndex,
) -> Result<(), PolygonError> {
    if let Some(error) = first_boundary_intersection(boundary, edge_index) {
        return Err(error);
    }

    for (hole_index, hole) in polygon.holes.iter().enumerate() {
        let loop_id = BoundaryLoopId(hole_index + 1);
        let probe = loop_interior_probe(hole)?;
        let containing = edge_index.containing_loop_ids(probe);
        if !containing.contains(&BoundaryLoopId(0)) {
            return Err(PolygonError::HoleOutsideOuter {
                hole: PolygonLoopId(loop_id.0),
            });
        }
        if let Some(other) = containing
            .iter()
            .copied()
            .find(|other| other.0 != 0 && *other != loop_id)
        {
            return Err(PolygonError::NestedHole {
                first: PolygonLoopId(other.0.min(loop_id.0)),
                second: PolygonLoopId(other.0.max(loop_id.0)),
            });
        }
    }

    if polygon.twice_signed_area()? <= 0 {
        return Err(PolygonError::DisconnectedInterior);
    }
    Ok(())
}

fn validate_loop_linear(
    boundary_loop: &crate::OrthogonalLoop,
    is_hole: bool,
    loop_id: PolygonLoopId,
) -> Result<(), PolygonError> {
    let vertices = &boundary_loop.vertices;
    if vertices.len() < 4 {
        return Err(PolygonError::TooFewVertices {
            loop_id,
            count: vertices.len(),
        });
    }
    let mut vertex_ids = HashMap::with_capacity(vertices.len());
    let mut edge_ids = HashMap::with_capacity(vertices.len());
    for index in 0..vertices.len() {
        let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        if current == next {
            return Err(PolygonError::ZeroLengthEdge { loop_id, index });
        }
        if current.x != next.x && current.y != next.y {
            return Err(PolygonError::NonAxisAlignedEdge { loop_id, index });
        }
        let incoming = (current.x - previous.x, current.y - previous.y);
        let outgoing = (next.x - current.x, next.y - current.y);
        if i128::from(incoming.0) * i128::from(outgoing.1)
            - i128::from(incoming.1) * i128::from(outgoing.0)
            == 0
        {
            return Err(PolygonError::UnsupportedDegenerateBoundary {
                loop_id,
                vertex: current,
            });
        }
        if let Some(first) = vertex_ids.insert(current, index) {
            return Err(PolygonError::DuplicateVertex {
                loop_id,
                first,
                second: index,
                point: current,
            });
        }
        let edge = if current < next {
            (current, next)
        } else {
            (next, current)
        };
        if let Some(first) = edge_ids.insert(edge, index) {
            return Err(PolygonError::DuplicateEdge {
                loop_id,
                first,
                second: index,
            });
        }
    }
    let area = boundary_loop.twice_signed_area()?;
    if (is_hole && area >= 0) || (!is_hole && area <= 0) {
        return Err(PolygonError::WrongOrientation { loop_id, area });
    }
    Ok(())
}

fn first_boundary_intersection(
    boundary: &Boundary,
    edge_index: &OrthogonalEdgeIndex,
) -> Option<PolygonError> {
    let mut first_failure = None::<(SweepFailureKey, PolygonError)>;
    for edge in edge_index
        .edges
        .iter()
        .copied()
        .filter(|edge| edge.is_horizontal())
    {
        for vertical_id in edge_index.report_vertical_crossings_closed(
            2 * i128::from(edge.first.y),
            edge.left(),
            edge.right(),
        ) {
            let vertical = edge_index.edges[vertical_id];
            let point = Point::new(vertical.first.x, edge.first.y);
            if adjacent_shared_endpoint(boundary, edge, vertical, point) {
                continue;
            }
            let proper = point != edge.first
                && point != edge.second
                && point != vertical.first
                && point != vertical.second;
            record_sweep_failure(&mut first_failure, edge, vertical, point, u8::from(!proper));
        }
    }
    for edge_ids in edge_index
        .horizontal_by_y
        .values()
        .chain(edge_index.vertical_by_x.values())
    {
        for first_index in 0..edge_ids.len() {
            let first = edge_index.edges[edge_ids[first_index]];
            let first_end = if first.is_horizontal() {
                first.right()
            } else {
                first.top()
            };
            for &second_id in &edge_ids[first_index + 1..] {
                let second = edge_index.edges[second_id];
                let second_start = if second.is_horizontal() {
                    second.left()
                } else {
                    second.bottom()
                };
                if second_start > first_end {
                    break;
                }
                let overlap = second_start < first_end;
                let point = if first.is_horizontal() {
                    Point::new(second_start, first.first.y)
                } else {
                    Point::new(first.first.x, second_start)
                };
                record_sweep_failure(
                    &mut first_failure,
                    first,
                    second,
                    point,
                    if overlap { 2 } else { 1 },
                );
            }
        }
    }
    first_failure.map(|(_, error)| error)
}

fn adjacent_shared_endpoint(
    boundary: &Boundary,
    first: IndexedBoundaryEdge,
    second: IndexedBoundaryEdge,
    point: Point,
) -> bool {
    if first.loop_id != second.loop_id {
        return false;
    }
    let len = boundary.loops[first.loop_id.0].vertices.len();
    let adjacent = first.edge_index + 1 == second.edge_index
        || second.edge_index + 1 == first.edge_index
        || (first.edge_index == 0 && second.edge_index + 1 == len)
        || (second.edge_index == 0 && first.edge_index + 1 == len);
    adjacent
        && (point == first.first || point == first.second)
        && (point == second.first || point == second.second)
}

fn record_sweep_failure(
    target: &mut Option<(SweepFailureKey, PolygonError)>,
    first: IndexedBoundaryEdge,
    second: IndexedBoundaryEdge,
    point: Point,
    relation: u8,
) {
    let (first, second) =
        if (first.loop_id, first.edge_index) <= (second.loop_id, second.edge_index) {
            (first, second)
        } else {
            (second, first)
        };
    let key = SweepFailureKey {
        x: point.x,
        y: point.y,
        relation,
        first_loop: first.loop_id.0,
        first_edge: first.edge_index,
        second_loop: second.loop_id.0,
        second_edge: second.edge_index,
    };
    let error = if first.loop_id == second.loop_id {
        if relation == 0 {
            PolygonError::SelfIntersection {
                loop_id: PolygonLoopId(first.loop_id.0),
                first_edge: first.edge_index,
                second_edge: second.edge_index,
            }
        } else {
            PolygonError::NonAdjacentBoundaryTouch {
                loop_id: PolygonLoopId(first.loop_id.0),
                first_edge: first.edge_index,
                second_edge: second.edge_index,
            }
        }
    } else if first.loop_id.0 == 0 || second.loop_id.0 == 0 {
        PolygonError::HoleIntersectsOuter {
            hole: PolygonLoopId(first.loop_id.0.max(second.loop_id.0)),
        }
    } else {
        PolygonError::HoleIntersectsHole {
            first: PolygonLoopId(first.loop_id.0),
            second: PolygonLoopId(second.loop_id.0),
        }
    };
    if target.as_ref().is_none_or(|(current, _)| key < *current) {
        *target = Some((key, error));
    }
}

fn loop_interior_probe(
    boundary_loop: &crate::OrthogonalLoop,
) -> Result<DoubledPoint, PolygonError> {
    let first = boundary_loop.vertices[0];
    let second = boundary_loop.vertices[1];
    let area = boundary_loop.twice_signed_area()?;
    let direction = (second.x - first.x, second.y - first.y);
    let side = if area > 0 { 1_i128 } else { -1_i128 };
    let normal = (
        -i128::from(direction.1).signum(),
        i128::from(direction.0).signum(),
    );
    Ok(DoubledPoint::new(
        i128::from(first.x) + i128::from(second.x) + side * normal.0,
        i128::from(first.y) + i128::from(second.y) + side * normal.1,
    ))
}

/// One-time polygon preparation counters used by solver diagnostics.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonPreparationMetrics {
    pub polygon_prepare_build_count: usize,
    pub polygon_normalization_count: usize,
    pub polygon_validation_count: usize,
    pub polygon_boundary_build_count: usize,
    pub polygon_boundary_index_build_count: usize,
    pub polygon_edge_index_build_count: usize,
    pub polygon_prepare_microseconds: u128,
    pub polygon_prepare_owned_bytes: usize,
    pub polygon_aligned_reflex_candidate_pairs: usize,
}

/// Shared normalized polygon metadata for one complete solve.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedPolygonContext {
    polygon: RectilinearPolygon,
    boundary: Boundary,
    boundary_index: BoundaryIndex,
    edge_index: OrthogonalEdgeIndex,
    reflex_by_x: BTreeMap<Coord, Vec<Point>>,
    reflex_by_y: BTreeMap<Coord, Vec<Point>>,
    abscissas: Vec<Coord>,
    ordinates: Vec<Coord>,
    validation_backend: PolygonValidationBackend,
    metrics: PolygonPreparationMetrics,
}

impl PreparedPolygonContext {
    /// Builds the v0.9-compatible reference-validated prepared context.
    ///
    /// # Errors
    ///
    /// Returns a structured polygon or boundary-index error.
    pub fn new(polygon: &RectilinearPolygon) -> Result<Self, PreparedPolygonError> {
        Self::new_with_validator(polygon, PolygonValidationBackend::ReferenceQuadratic)
    }

    /// Builds all shared metadata with the selected exact structural validator.
    ///
    /// # Errors
    ///
    /// Returns a structured polygon or boundary-index error.
    pub fn new_with_validator(
        polygon: &RectilinearPolygon,
        validation_backend: PolygonValidationBackend,
    ) -> Result<Self, PreparedPolygonError> {
        let started = Instant::now();
        let polygon = RectilinearPolygon::normalize_unvalidated(
            polygon.outer.clone(),
            polygon.holes.clone(),
        )?;
        if validation_backend == PolygonValidationBackend::ReferenceQuadratic {
            ReferenceQuadraticValidator.validate(&polygon)?;
        } else {
            validate_loop_linear(&polygon.outer, false, PolygonLoopId(0))?;
            for (index, boundary_loop) in polygon.holes.iter().enumerate() {
                validate_loop_linear(boundary_loop, true, PolygonLoopId(index + 1))?;
            }
        }
        let boundary = Boundary::from_polygon(&polygon);
        let edge_index = OrthogonalEdgeIndex::new(&boundary);
        if validation_backend == PolygonValidationBackend::OrthogonalSweep {
            validate_polygon_sweep_with_indexes(&polygon, &boundary, &edge_index)?;
        }
        let boundary_index = BoundaryIndex::new(&boundary)?;
        let mut reflex_by_x = BTreeMap::<Coord, Vec<Point>>::new();
        let mut reflex_by_y = BTreeMap::<Coord, Vec<Point>>::new();
        for reflex in &boundary.reflex_vertices {
            reflex_by_x
                .entry(reflex.point.x)
                .or_default()
                .push(reflex.point);
            reflex_by_y
                .entry(reflex.point.y)
                .or_default()
                .push(reflex.point);
        }
        for points in reflex_by_x.values_mut().chain(reflex_by_y.values_mut()) {
            points.sort_unstable();
        }
        let aligned_pairs = reflex_by_x
            .values()
            .chain(reflex_by_y.values())
            .map(|points| points.len().saturating_sub(1) * points.len() / 2)
            .sum();
        let mut abscissas = polygon
            .loops()
            .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
            .collect::<Vec<_>>();
        let mut ordinates = polygon
            .loops()
            .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
            .collect::<Vec<_>>();
        abscissas.sort_unstable();
        abscissas.dedup();
        ordinates.sort_unstable();
        ordinates.dedup();
        let owned_bytes = polygon.boundary_complexity() * std::mem::size_of::<Point>()
            + boundary.boundary_complexity() * std::mem::size_of::<Point>()
            + boundary_index.owned_bytes_estimate()
            + edge_index.owned_bytes_estimate()
            + reflex_by_x
                .values()
                .chain(reflex_by_y.values())
                .map(|points| points.len() * std::mem::size_of::<Point>())
                .sum::<usize>()
            + (abscissas.len() + ordinates.len()) * std::mem::size_of::<Coord>();
        Ok(Self {
            polygon,
            boundary,
            boundary_index,
            edge_index,
            reflex_by_x,
            reflex_by_y,
            abscissas,
            ordinates,
            validation_backend,
            metrics: PolygonPreparationMetrics {
                polygon_prepare_build_count: 1,
                polygon_normalization_count: 1,
                polygon_validation_count: 1,
                polygon_boundary_build_count: 1,
                polygon_boundary_index_build_count: 1,
                polygon_edge_index_build_count: 1,
                polygon_prepare_microseconds: started.elapsed().as_micros(),
                polygon_prepare_owned_bytes: owned_bytes,
                polygon_aligned_reflex_candidate_pairs: aligned_pairs,
            },
        })
    }

    #[must_use]
    pub const fn polygon(&self) -> &RectilinearPolygon {
        &self.polygon
    }

    #[must_use]
    pub const fn boundary(&self) -> &Boundary {
        &self.boundary
    }

    #[must_use]
    pub const fn boundary_index(&self) -> &BoundaryIndex {
        &self.boundary_index
    }

    #[must_use]
    pub const fn edge_index(&self) -> &OrthogonalEdgeIndex {
        &self.edge_index
    }

    #[must_use]
    pub const fn reflex_by_x(&self) -> &BTreeMap<Coord, Vec<Point>> {
        &self.reflex_by_x
    }

    #[must_use]
    pub const fn reflex_by_y(&self) -> &BTreeMap<Coord, Vec<Point>> {
        &self.reflex_by_y
    }

    #[must_use]
    pub fn base_x_coordinates(&self) -> &[Coord] {
        &self.abscissas
    }

    #[must_use]
    pub fn base_y_coordinates(&self) -> &[Coord] {
        &self.ordinates
    }

    #[must_use]
    pub const fn metrics(&self) -> &PolygonPreparationMetrics {
        &self.metrics
    }

    #[must_use]
    pub const fn validation_backend(&self) -> PolygonValidationBackend {
        self.validation_backend
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PreparedPolygonError {
    #[error(transparent)]
    Polygon(#[from] PolygonError),
    #[error(transparent)]
    BoundaryIndex(#[from] BoundaryIndexError),
}

#[cfg(test)]
mod tests {
    use crate::{
        OrthogonalLoop, Point, PolygonErrorCategory, PolygonValidationBackend, PolygonValidator,
        RectilinearPolygon, ReferenceQuadraticValidator,
    };

    use super::{OrthogonalDirection, OrthogonalSweepValidator, PreparedPolygonContext};

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    fn clockwise_rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        let mut boundary_loop = rectangle(x0, y0, x1, y1);
        boundary_loop.vertices.reverse();
        boundary_loop
    }

    fn assert_validator_categories_match(polygon: &RectilinearPolygon) {
        let reference = ReferenceQuadraticValidator.validate(polygon);
        let indexed = OrthogonalSweepValidator.validate(polygon);
        assert_eq!(
            reference
                .as_ref()
                .err()
                .map(PolygonErrorCategory::from_error),
            indexed.as_ref().err().map(PolygonErrorCategory::from_error),
            "reference={reference:?}, indexed={indexed:?}, polygon={polygon:?}"
        );
        assert_eq!(indexed, OrthogonalSweepValidator.validate(polygon));
    }

    #[test]
    fn prepared_polygon_builds_every_static_index_once() {
        let polygon =
            RectilinearPolygon::new(rectangle(0, 0, 20, 20), vec![rectangle(4, 4, 8, 8)]).unwrap();
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        assert_eq!(prepared.metrics().polygon_prepare_build_count, 1);
        assert_eq!(prepared.metrics().polygon_normalization_count, 1);
        assert_eq!(prepared.metrics().polygon_validation_count, 1);
        assert_eq!(prepared.metrics().polygon_boundary_build_count, 1);
        assert_eq!(prepared.metrics().polygon_boundary_index_build_count, 1);
        assert_eq!(prepared.metrics().polygon_edge_index_build_count, 1);
        assert!(prepared.metrics().polygon_prepare_owned_bytes > 0);

        let indexed = PreparedPolygonContext::new_with_validator(
            &polygon,
            PolygonValidationBackend::OrthogonalSweep,
        )
        .unwrap();
        assert_eq!(
            indexed.validation_backend(),
            PolygonValidationBackend::OrthogonalSweep
        );
        assert_eq!(indexed.metrics().polygon_prepare_build_count, 1);
        assert_eq!(indexed.metrics().polygon_normalization_count, 1);
        assert_eq!(indexed.metrics().polygon_validation_count, 1);
        assert_eq!(indexed.metrics().polygon_boundary_build_count, 1);
        assert_eq!(indexed.metrics().polygon_boundary_index_build_count, 1);
        assert_eq!(indexed.metrics().polygon_edge_index_build_count, 1);
    }

    #[test]
    fn indexed_point_location_and_ray_shooting_preserve_holes() {
        let polygon =
            RectilinearPolygon::new(rectangle(0, 0, 20, 20), vec![rectangle(4, 4, 8, 8)]).unwrap();
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let index = prepared.edge_index();
        for point in [
            crate::DoubledPoint::new(1, 1),
            crate::DoubledPoint::new(10, 10),
            crate::DoubledPoint::new(30, 30),
            crate::DoubledPoint::new(0, 1),
        ] {
            assert_eq!(
                index.contains_doubled_point_strict(point),
                prepared.polygon().contains_doubled_point_strict(point)
            );
        }
        assert_eq!(
            index.nearest_boundary_blocker(Point::new(2, 5), OrthogonalDirection::East),
            Some(Point::new(4, 5))
        );
        assert_eq!(
            index.nearest_boundary_blocker(Point::new(10, 5), OrthogonalDirection::West),
            Some(Point::new(8, 5))
        );
    }

    #[test]
    fn validator_backends_accept_the_same_normalized_polygons() {
        for polygon in [
            RectilinearPolygon::new(rectangle(0, 0, 20, 20), vec![]).unwrap(),
            RectilinearPolygon::new(
                OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(12, 0),
                    Point::new(12, 4),
                    Point::new(8, 4),
                    Point::new(8, 8),
                    Point::new(12, 8),
                    Point::new(12, 12),
                    Point::new(0, 12),
                ]),
                vec![rectangle(2, 2, 4, 4)],
            )
            .unwrap(),
        ] {
            assert_validator_categories_match(&polygon);
        }
    }

    #[test]
    fn validator_backends_match_broad_negative_categories() {
        let cases = [
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 1),
                    Point::new(4, 4),
                    Point::new(0, 4),
                ]),
                holes: vec![],
            },
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(4, 4),
                    Point::new(0, 4),
                    Point::new(0, 4),
                ]),
                holes: vec![],
            },
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(4, 4),
                ]),
                holes: vec![],
            },
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(4, 4),
                    Point::new(1, 4),
                    Point::new(1, -1),
                    Point::new(0, -1),
                ]),
                holes: vec![],
            },
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(2, 0),
                    Point::new(0, 0),
                    Point::new(0, -4),
                    Point::new(6, -4),
                    Point::new(6, 0),
                    Point::new(2, 0),
                    Point::new(2, 2),
                    Point::new(6, 2),
                    Point::new(6, 6),
                    Point::new(0, 6),
                    Point::new(0, 2),
                    Point::new(2, 2),
                ]),
                holes: vec![],
            },
            RectilinearPolygon {
                outer: rectangle(0, 0, 10, 10),
                holes: vec![clockwise_rectangle(12, 2, 14, 4)],
            },
            RectilinearPolygon {
                outer: rectangle(0, 0, 10, 10),
                holes: vec![clockwise_rectangle(0, 2, 4, 4)],
            },
            RectilinearPolygon {
                outer: rectangle(0, 0, 20, 20),
                holes: vec![
                    clockwise_rectangle(2, 2, 8, 8),
                    clockwise_rectangle(6, 6, 12, 12),
                ],
            },
            RectilinearPolygon {
                outer: rectangle(0, 0, 20, 20),
                holes: vec![
                    clockwise_rectangle(2, 2, 12, 12),
                    clockwise_rectangle(4, 4, 6, 6),
                ],
            },
            RectilinearPolygon {
                outer: clockwise_rectangle(0, 0, 10, 10),
                holes: vec![],
            },
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(6, 0),
                    Point::new(6, 4),
                    Point::new(0, 4),
                ]),
                holes: vec![],
            },
        ];
        for polygon in &cases {
            assert_validator_categories_match(polygon);
        }
    }
}
