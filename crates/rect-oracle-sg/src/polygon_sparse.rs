//! Sparse planar subdivision and slab validation for polygon completion.
//!
//! This module deliberately never materializes the Cartesian product of the
//! x and y coordinate sets.  The coordinate arrangement in
//! `polygon_arrangement` remains the independent dense oracle.

use std::collections::{BTreeMap, BTreeSet};

use rect_core::{CoordinateRect, DoubledPoint, Point, PreparedPolygonContext, RectilinearPolygon};
use serde::{Deserialize, Serialize};

use crate::polygon::{
    HorizontalCutSegment, PolygonSgError, PolygonValidationError, VerticalCutSegment,
};

/// Final-geometry recovery implementation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonRecoveryBackend {
    /// Preserved coordinate-compressed flood-fill oracle.
    DenseCoordinateArrangement,
    /// Sparse half-edge subdivision and face walk.
    #[default]
    SparseSubdivision,
}

impl PolygonRecoveryBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DenseCoordinateArrangement => "dense-arrangement",
            Self::SparseSubdivision => "sparse-subdivision",
        }
    }
}

/// Exact polygon-dissection validation implementation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonDissectionValidatorBackend {
    /// Preserved coordinate-compressed difference-array oracle.
    DenseArrangement,
    /// Sparse vertical slab sweep.
    #[default]
    SparseSlab,
}

impl PolygonDissectionValidatorBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DenseArrangement => "dense-arrangement",
            Self::SparseSlab => "sparse-slab",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SubdivisionVertexId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SubdivisionHalfEdgeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SubdivisionFaceId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubdivisionVertex {
    pub id: SubdivisionVertexId,
    pub point: Point,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubdivisionHalfEdge {
    pub id: SubdivisionHalfEdgeId,
    pub origin: SubdivisionVertexId,
    pub destination: SubdivisionVertexId,
    pub twin: SubdivisionHalfEdgeId,
    pub next: SubdivisionHalfEdgeId,
    pub previous: SubdivisionHalfEdgeId,
    pub face: SubdivisionFaceId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SubdivisionFace {
    pub id: SubdivisionFaceId,
    pub boundary: Vec<SubdivisionHalfEdgeId>,
    pub signed_area_twice: i128,
    pub polygon_interior_on_left: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SparseSubdivisionMetrics {
    pub vertex_count: usize,
    pub half_edge_count: usize,
    pub face_count: usize,
    pub junction_count: usize,
    pub owned_bytes: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SparseSlabMetrics {
    pub slab_count: usize,
    pub polygon_interval_events: usize,
    pub rectangle_interval_events: usize,
    pub owned_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Segment {
    first: Point,
    second: Point,
}

impl Segment {
    fn new(first: Point, second: Point) -> Result<Self, PolygonSgError> {
        if first == second {
            return Err(PolygonSgError::SparseSubdivision {
                message: "zero-length segment".to_owned(),
            });
        }
        if first.x != second.x && first.y != second.y {
            return Err(PolygonSgError::SparseSubdivision {
                message: "non-orthogonal segment".to_owned(),
            });
        }
        Ok(if first <= second {
            Self { first, second }
        } else {
            Self {
                first: second,
                second: first,
            }
        })
    }

    const fn horizontal(self) -> bool {
        self.first.y == self.second.y
    }

    const fn low(self) -> i64 {
        if self.horizontal() {
            self.first.x
        } else {
            self.first.y
        }
    }

    const fn high(self) -> i64 {
        if self.horizontal() {
            self.second.x
        } else {
            self.second.y
        }
    }

    const fn line(self) -> i64 {
        if self.horizontal() {
            self.first.y
        } else {
            self.first.x
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Direction {
    East,
    North,
    West,
    South,
}

impl Direction {
    fn between(first: Point, second: Point) -> Self {
        if second.x > first.x {
            Self::East
        } else if second.y > first.y {
            Self::North
        } else if second.x < first.x {
            Self::West
        } else {
            Self::South
        }
    }

    const fn clockwise(self) -> Self {
        match self {
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
            Self::North => Self::East,
        }
    }
}

/// Sparse embedded orthogonal graph built from boundary and final cuts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SparseOrthogonalSubdivision {
    pub vertices: Vec<SubdivisionVertex>,
    pub half_edges: Vec<SubdivisionHalfEdge>,
    pub faces: Vec<SubdivisionFace>,
    pub metrics: SparseSubdivisionMetrics,
}

impl SparseOrthogonalSubdivision {
    /// Splits boundary and cut segments at all exact orthogonal crossings and
    /// T-junctions, then constructs a left-face half-edge traversal.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for malformed segments, failed sparse graph
    /// construction, or exact arithmetic overflow.
    #[allow(clippy::too_many_lines)]
    pub fn new(
        prepared: &PreparedPolygonContext,
        horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
        vertical_cuts: &BTreeSet<VerticalCutSegment>,
    ) -> Result<Self, PolygonSgError> {
        let mut segments = BTreeSet::new();
        for boundary_loop in prepared.polygon().loops() {
            for (first, second) in boundary_loop.edges() {
                segments.insert(Segment::new(first, second)?);
            }
        }
        for cut in horizontal_cuts {
            segments.insert(Segment::new(
                Point::new(cut.left, cut.y),
                Point::new(cut.right, cut.y),
            )?);
        }
        for cut in vertical_cuts {
            segments.insert(Segment::new(
                Point::new(cut.x, cut.bottom),
                Point::new(cut.x, cut.top),
            )?);
        }
        let segments = segments.into_iter().collect::<Vec<_>>();

        let mut horizontal_by_y = BTreeMap::<i64, Vec<usize>>::new();
        let mut vertical_by_x = BTreeMap::<i64, Vec<usize>>::new();
        let mut points = BTreeSet::new();
        for (id, segment) in segments.iter().copied().enumerate() {
            points.extend([segment.first, segment.second]);
            if segment.horizontal() {
                horizontal_by_y.entry(segment.line()).or_default().push(id);
            } else {
                vertical_by_x.entry(segment.line()).or_default().push(id);
            }
        }
        for horizontal_ids in horizontal_by_y.values() {
            for &horizontal_id in horizontal_ids {
                let horizontal = segments[horizontal_id];
                for (&x, vertical_ids) in vertical_by_x.range(horizontal.low()..=horizontal.high())
                {
                    for &vertical_id in vertical_ids {
                        let vertical = segments[vertical_id];
                        if vertical.low() <= horizontal.line()
                            && horizontal.line() <= vertical.high()
                        {
                            points.insert(Point::new(x, horizontal.line()));
                        }
                    }
                }
            }
        }

        let mut horizontal_points = BTreeMap::<i64, BTreeSet<i64>>::new();
        let mut vertical_points = BTreeMap::<i64, BTreeSet<i64>>::new();
        for point in &points {
            horizontal_points
                .entry(point.y)
                .or_default()
                .insert(point.x);
            vertical_points.entry(point.x).or_default().insert(point.y);
        }

        let mut atomic_edges = BTreeSet::new();
        for segment in segments {
            let coordinates = if segment.horizontal() {
                horizontal_points
                    .get(&segment.line())
                    .into_iter()
                    .flat_map(|coordinates| coordinates.range(segment.low()..=segment.high()))
                    .copied()
                    .collect::<Vec<_>>()
            } else {
                vertical_points
                    .get(&segment.line())
                    .into_iter()
                    .flat_map(|coordinates| coordinates.range(segment.low()..=segment.high()))
                    .copied()
                    .collect::<Vec<_>>()
            };
            for pair in coordinates.windows(2) {
                let first = if segment.horizontal() {
                    Point::new(pair[0], segment.line())
                } else {
                    Point::new(segment.line(), pair[0])
                };
                let second = if segment.horizontal() {
                    Point::new(pair[1], segment.line())
                } else {
                    Point::new(segment.line(), pair[1])
                };
                atomic_edges.insert(Segment::new(first, second)?);
            }
        }

        let vertex_points = atomic_edges
            .iter()
            .flat_map(|segment| [segment.first, segment.second])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let vertex_ids = vertex_points
            .iter()
            .copied()
            .enumerate()
            .map(|(id, point)| (point, SubdivisionVertexId(id)))
            .collect::<BTreeMap<_, _>>();
        let vertices = vertex_points
            .iter()
            .copied()
            .enumerate()
            .map(|(id, point)| SubdivisionVertex {
                id: SubdivisionVertexId(id),
                point,
            })
            .collect::<Vec<_>>();
        let mut half_edges = Vec::with_capacity(atomic_edges.len() * 2);
        let mut outgoing =
            BTreeMap::<SubdivisionVertexId, BTreeMap<Direction, SubdivisionHalfEdgeId>>::new();
        for edge in atomic_edges {
            let first = vertex_ids[&edge.first];
            let second = vertex_ids[&edge.second];
            let forward = SubdivisionHalfEdgeId(half_edges.len());
            let backward = SubdivisionHalfEdgeId(half_edges.len() + 1);
            half_edges.push(SubdivisionHalfEdge {
                id: forward,
                origin: first,
                destination: second,
                twin: backward,
                next: forward,
                previous: forward,
                face: SubdivisionFaceId(usize::MAX),
            });
            half_edges.push(SubdivisionHalfEdge {
                id: backward,
                origin: second,
                destination: first,
                twin: forward,
                next: backward,
                previous: backward,
                face: SubdivisionFaceId(usize::MAX),
            });
            outgoing
                .entry(first)
                .or_default()
                .insert(Direction::between(edge.first, edge.second), forward);
            outgoing
                .entry(second)
                .or_default()
                .insert(Direction::between(edge.second, edge.first), backward);
        }
        for half_edge in &mut half_edges {
            let destination = half_edge.destination;
            let origin = half_edge.origin;
            let reverse_direction =
                Direction::between(vertices[destination.0].point, vertices[origin.0].point);
            let options =
                outgoing
                    .get(&destination)
                    .ok_or_else(|| PolygonSgError::SparseSubdivision {
                        message: "missing outgoing half-edge".to_owned(),
                    })?;
            let mut direction = reverse_direction.clockwise();
            let next = loop {
                if let Some(&next) = options.get(&direction) {
                    break next;
                }
                direction = direction.clockwise();
            };
            half_edge.next = next;
        }
        for index in 0..half_edges.len() {
            let next = half_edges[index].next.0;
            half_edges[next].previous = SubdivisionHalfEdgeId(index);
        }

        let mut faces = Vec::new();
        for seed in 0..half_edges.len() {
            if half_edges[seed].face.0 != usize::MAX {
                continue;
            }
            let id = SubdivisionFaceId(faces.len());
            let mut boundary = Vec::new();
            let mut current = SubdivisionHalfEdgeId(seed);
            loop {
                if half_edges[current.0].face.0 != usize::MAX {
                    return Err(PolygonSgError::SparseSubdivision {
                        message: "half-edge traversal joined an assigned face".to_owned(),
                    });
                }
                half_edges[current.0].face = id;
                boundary.push(current);
                current = half_edges[current.0].next;
                if current.0 == seed {
                    break;
                }
            }
            let points_on_face = boundary
                .iter()
                .map(|edge| vertices[half_edges[edge.0].origin.0].point)
                .collect::<Vec<_>>();
            let signed_area_twice = signed_area_twice(&points_on_face)?;
            let probe_edge = &half_edges[seed];
            let first = vertices[probe_edge.origin.0].point;
            let second = vertices[probe_edge.destination.0].point;
            let probe = left_probe(first, second);
            faces.push(SubdivisionFace {
                id,
                boundary,
                signed_area_twice,
                polygon_interior_on_left: prepared
                    .edge_index()
                    .contains_doubled_point_strict(probe),
            });
        }
        let junction_count = vertices
            .iter()
            .filter(|vertex| {
                outgoing
                    .get(&vertex.id)
                    .is_some_and(|edges| edges.len() >= 3)
            })
            .count();
        let metrics = SparseSubdivisionMetrics {
            vertex_count: vertices.len(),
            half_edge_count: half_edges.len(),
            face_count: faces.len(),
            junction_count,
            owned_bytes: vertices.len() * std::mem::size_of::<SubdivisionVertex>()
                + half_edges.len() * std::mem::size_of::<SubdivisionHalfEdge>()
                + faces
                    .iter()
                    .map(|face| face.boundary.len() * std::mem::size_of::<SubdivisionHalfEdgeId>())
                    .sum::<usize>(),
        };
        Ok(Self {
            vertices,
            half_edges,
            faces,
            metrics,
        })
    }

    /// Recovers only rectangular polygon-interior face cycles.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError::NonRectangularCompletionRegion`] when an
    /// interior sparse face is not exactly one coordinate rectangle.
    pub fn recover_rectangles(
        &self,
        polygon: &RectilinearPolygon,
    ) -> Result<Vec<CoordinateRect>, PolygonSgError> {
        let mut rectangles = Vec::new();
        for face in self
            .faces
            .iter()
            .filter(|face| face.polygon_interior_on_left)
        {
            let mut points = face
                .boundary
                .iter()
                .map(|edge| self.vertices[self.half_edges[edge.0].origin.0].point)
                .collect::<Vec<_>>();
            simplify_cycle(&mut points);
            if points.len() != 4 || face.signed_area_twice <= 0 {
                return Err(PolygonSgError::NonRectangularCompletionRegion {
                    point: points.first().copied().unwrap_or(Point::new(0, 0)),
                });
            }
            let left = points.iter().map(|point| point.x).min().unwrap_or(0);
            let right = points.iter().map(|point| point.x).max().unwrap_or(0);
            let bottom = points.iter().map(|point| point.y).min().unwrap_or(0);
            let top = points.iter().map(|point| point.y).max().unwrap_or(0);
            let rectangle = CoordinateRect::new(left, bottom, right, top)?;
            if signed_area_twice(&points)? != rectangle.area() * 2
                || !cycle_is_rectangle(&points, rectangle)
            {
                return Err(PolygonSgError::NonRectangularCompletionRegion { point: points[0] });
            }
            rectangles.push(rectangle);
        }
        rectangles.sort_unstable();
        let rectangle_area_twice = rectangles.iter().try_fold(0_i128, |area, rectangle| {
            area.checked_add(rectangle.area() * 2)
                .ok_or(PolygonSgError::CoordinateOverflow)
        })?;
        if rectangle_area_twice != polygon.twice_signed_area()? {
            return Err(PolygonSgError::NonRectangularCompletionRegion {
                point: self
                    .vertices
                    .first()
                    .map_or(Point::new(0, 0), |vertex| vertex.point),
            });
        }
        Ok(rectangles)
    }
}

fn left_probe(first: Point, second: Point) -> DoubledPoint {
    let x = i128::from(first.x) + i128::from(second.x);
    let y = i128::from(first.y) + i128::from(second.y);
    match Direction::between(first, second) {
        Direction::East => DoubledPoint::new(x, y + 1),
        Direction::North => DoubledPoint::new(x - 1, y),
        Direction::West => DoubledPoint::new(x, y - 1),
        Direction::South => DoubledPoint::new(x + 1, y),
    }
}

fn signed_area_twice(points: &[Point]) -> Result<i128, PolygonSgError> {
    if points.len() < 3 {
        return Ok(0);
    }
    let mut area = 0_i128;
    for index in 0..points.len() {
        let first = points[index];
        let second = points[(index + 1) % points.len()];
        area = area
            .checked_add(
                i128::from(first.x) * i128::from(second.y)
                    - i128::from(first.y) * i128::from(second.x),
            )
            .ok_or(PolygonSgError::CoordinateOverflow)?;
    }
    Ok(area)
}

fn simplify_cycle(points: &mut Vec<Point>) {
    loop {
        let before = points.len();
        if before < 3 {
            return;
        }
        let mut simplified = Vec::with_capacity(before);
        for index in 0..before {
            let previous = points[(index + before - 1) % before];
            let current = points[index];
            let next = points[(index + 1) % before];
            let collinear = (previous.x == current.x && current.x == next.x)
                || (previous.y == current.y && current.y == next.y);
            if !collinear {
                simplified.push(current);
            }
        }
        *points = simplified;
        if points.len() == before {
            return;
        }
    }
}

fn cycle_is_rectangle(points: &[Point], rectangle: CoordinateRect) -> bool {
    let corners = BTreeSet::from([
        Point::new(rectangle.x0, rectangle.y0),
        Point::new(rectangle.x1, rectangle.y0),
        Point::new(rectangle.x1, rectangle.y1),
        Point::new(rectangle.x0, rectangle.y1),
    ]);
    points.iter().copied().collect::<BTreeSet<_>>() == corners
        && (0..points.len()).all(|index| {
            let first = points[index];
            let second = points[(index + 1) % points.len()];
            first.x == second.x || first.y == second.y
        })
}

/// Exact vertical slab validator without a coordinate-cell array.
#[derive(Clone, Copy, Debug, Default)]
pub struct SparseSlabValidator;

impl SparseSlabValidator {
    /// Validates a dissection using polygon and rectangle interval sweeps.
    ///
    /// # Errors
    ///
    /// Returns the first exact geometry, coverage, or area error.
    pub fn validate(
        &self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<SparseSlabMetrics, PolygonValidationError> {
        let mut rectangle_area = 0_i128;
        let mut x_coordinates = polygon
            .loops()
            .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
            .collect::<BTreeSet<_>>();
        let mut events = BTreeMap::<i64, Vec<(bool, usize)>>::new();
        for (index, rectangle) in rectangles.iter().copied().enumerate() {
            if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
                return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
            }
            rectangle_area = rectangle_area
                .checked_add(rectangle.area())
                .ok_or(PolygonValidationError::AreaOverflow)?;
            x_coordinates.extend([rectangle.x0, rectangle.x1]);
            events.entry(rectangle.x0).or_default().push((true, index));
            events.entry(rectangle.x1).or_default().push((false, index));
        }
        let polygon_area_twice = polygon
            .twice_signed_area()
            .map_err(PolygonValidationError::Polygon)?;
        if rectangle_area
            .checked_mul(2)
            .ok_or(PolygonValidationError::AreaOverflow)?
            != polygon_area_twice
        {
            return Err(PolygonValidationError::AreaMismatch {
                polygon_area_twice,
                rectangle_area_twice: rectangle_area * 2,
            });
        }
        let x_coordinates = x_coordinates.into_iter().collect::<Vec<_>>();
        let horizontal_boundary = polygon
            .loops()
            .flat_map(rect_core::OrthogonalLoop::edges)
            .filter(|(first, second)| first.y == second.y)
            .collect::<Vec<_>>();
        let mut active = BTreeSet::new();
        let mut metrics = SparseSlabMetrics {
            owned_bytes: x_coordinates.len() * std::mem::size_of::<i64>()
                + horizontal_boundary.len() * std::mem::size_of::<(Point, Point)>(),
            ..SparseSlabMetrics::default()
        };
        for pair in x_coordinates.windows(2) {
            let x = pair[0];
            if let Some(changes) = events.get(&x) {
                for &(starts, index) in changes {
                    if starts {
                        active.insert(index);
                    } else {
                        active.remove(&index);
                    }
                }
            }
            if pair[0] == pair[1] {
                continue;
            }
            metrics.slab_count += 1;
            let doubled_x = i128::from(pair[0]) + i128::from(pair[1]);
            let mut crossings = horizontal_boundary
                .iter()
                .filter_map(|&(first, second)| {
                    let left = 2 * i128::from(first.x.min(second.x));
                    let right = 2 * i128::from(first.x.max(second.x));
                    (left < doubled_x && doubled_x < right).then_some(first.y)
                })
                .collect::<Vec<_>>();
            crossings.sort_unstable();
            crossings.dedup();
            metrics.polygon_interval_events += crossings.len();
            let expected = crossings
                .chunks_exact(2)
                .map(|pair| (pair[0], pair[1]))
                .collect::<Vec<_>>();
            let mut coverage = active
                .iter()
                .copied()
                .map(|index| {
                    let rectangle = rectangles[index];
                    (rectangle.y0, rectangle.y1, index)
                })
                .collect::<Vec<_>>();
            coverage.sort_unstable();
            metrics.rectangle_interval_events += coverage.len();
            let unions = rectangle_unions(&coverage, doubled_x)?;
            compare_slab_intervals(&expected, &unions, doubled_x)?;
        }
        Ok(metrics)
    }
}

fn rectangle_unions(
    coverage: &[(i64, i64, usize)],
    doubled_x: i128,
) -> Result<Vec<(i64, i64, usize)>, PolygonValidationError> {
    let mut unions = Vec::<(i64, i64, usize)>::new();
    for &(bottom, top, index) in coverage {
        if let Some(last) = unions.last_mut() {
            if bottom < last.1 {
                return Err(PolygonValidationError::Overlap {
                    first: last.2,
                    second: index,
                    point: DoubledPoint::new(
                        doubled_x,
                        i128::from(bottom) + i128::from(last.1.min(top)),
                    ),
                });
            }
            if bottom == last.1 {
                last.1 = top;
                continue;
            }
        }
        unions.push((bottom, top, index));
    }
    Ok(unions)
}

fn compare_slab_intervals(
    expected: &[(i64, i64)],
    actual: &[(i64, i64, usize)],
    doubled_x: i128,
) -> Result<(), PolygonValidationError> {
    let mut expected_index = 0;
    let mut actual_index = 0;
    let mut expected_start = expected.first().map(|interval| interval.0);
    let mut actual_start = actual.first().map(|interval| interval.0);
    while let (Some(polygon_bottom), Some(rectangle_bottom)) = (expected_start, actual_start) {
        let polygon_top = expected[expected_index].1;
        let (rectangle_top, rectangle_id) = (actual[actual_index].1, actual[actual_index].2);
        if rectangle_bottom < polygon_bottom {
            return Err(PolygonValidationError::OutsidePolygon {
                rectangle: rectangle_id,
                point: DoubledPoint::new(
                    doubled_x,
                    i128::from(rectangle_bottom) + i128::from(rectangle_top.min(polygon_bottom)),
                ),
            });
        }
        if polygon_bottom < rectangle_bottom {
            return Err(PolygonValidationError::UncoveredInterior {
                point: DoubledPoint::new(
                    doubled_x,
                    i128::from(polygon_bottom) + i128::from(polygon_top.min(rectangle_bottom)),
                ),
            });
        }
        match polygon_top.cmp(&rectangle_top) {
            std::cmp::Ordering::Equal => {
                expected_index += 1;
                actual_index += 1;
                expected_start = expected.get(expected_index).map(|interval| interval.0);
                actual_start = actual.get(actual_index).map(|interval| interval.0);
            }
            std::cmp::Ordering::Less => {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: rectangle_id,
                    point: DoubledPoint::new(
                        doubled_x,
                        i128::from(polygon_top) + i128::from(rectangle_top),
                    ),
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(PolygonValidationError::UncoveredInterior {
                    point: DoubledPoint::new(
                        doubled_x,
                        i128::from(rectangle_top) + i128::from(polygon_top),
                    ),
                });
            }
        }
    }
    if let Some(&(bottom, top)) = expected.get(expected_index) {
        return Err(PolygonValidationError::UncoveredInterior {
            point: DoubledPoint::new(doubled_x, i128::from(bottom) + i128::from(top)),
        });
    }
    if let Some(&(bottom, top, rectangle)) = actual.get(actual_index) {
        return Err(PolygonValidationError::OutsidePolygon {
            rectangle,
            point: DoubledPoint::new(doubled_x, i128::from(bottom) + i128::from(top)),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rect_core::{
        CoordinateRect, OrthogonalLoop, Point, PreparedPolygonContext, RectilinearPolygon,
    };

    use crate::polygon::{HorizontalCutSegment, VerticalCutSegment};
    use crate::polygon_arrangement::PreparedCoordinateArrangement;

    use super::{SparseOrthogonalSubdivision, SparseSlabValidator};

    #[test]
    fn sparse_subdivision_recovers_single_rectangle_without_dense_cells() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 3),
                Point::new(0, 3),
            ]),
            vec![],
        )
        .unwrap();
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let subdivision =
            SparseOrthogonalSubdivision::new(&prepared, &BTreeSet::new(), &BTreeSet::new())
                .unwrap();
        assert_eq!(
            subdivision.recover_rectangles(&polygon).unwrap(),
            vec![CoordinateRect::new(0, 0, 4, 3).unwrap()]
        );
        SparseSlabValidator
            .validate(&polygon, &[CoordinateRect::new(0, 0, 4, 3).unwrap()])
            .unwrap();
    }

    #[test]
    fn sparse_subdivision_matches_dense_on_t_junction_and_crossing_cuts() {
        let l_shape = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 1),
                Point::new(1, 1),
                Point::new(1, 4),
                Point::new(0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let rectangle = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 4),
                Point::new(0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let cases = [
            (
                l_shape,
                BTreeSet::new(),
                BTreeSet::from([VerticalCutSegment::new(1, 0, 1).unwrap()]),
            ),
            (
                rectangle,
                BTreeSet::from([HorizontalCutSegment::new(0, 4, 2).unwrap()]),
                BTreeSet::from([VerticalCutSegment::new(2, 0, 4).unwrap()]),
            ),
        ];
        for (polygon, horizontal, vertical) in cases {
            let prepared = PreparedPolygonContext::new(&polygon).unwrap();
            let sparse = SparseOrthogonalSubdivision::new(&prepared, &horizontal, &vertical)
                .unwrap()
                .recover_rectangles(&polygon)
                .unwrap();
            let mut dense =
                PreparedCoordinateArrangement::new(&prepared, &horizontal, &vertical).unwrap();
            let dense = dense.recover_rectangles().unwrap();
            assert_eq!(sparse, dense);
            SparseSlabValidator.validate(&polygon, &sparse).unwrap();
        }
    }

    #[test]
    fn sparse_subdivision_accepts_all_ordinary_hole_bridge_topologies() {
        let outer = || {
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(20, 0),
                Point::new(20, 20),
                Point::new(0, 20),
            ])
        };
        let clockwise_rectangle = |left, bottom, right, top| {
            OrthogonalLoop::new(vec![
                Point::new(left, bottom),
                Point::new(left, top),
                Point::new(right, top),
                Point::new(right, bottom),
            ])
        };
        let cases = [
            (
                "same-boundary-component",
                RectilinearPolygon::new(outer(), vec![]).unwrap(),
                BTreeSet::new(),
                BTreeSet::from([VerticalCutSegment::new(10, 0, 20).unwrap()]),
            ),
            (
                "outer-to-hole",
                RectilinearPolygon::new(outer(), vec![clockwise_rectangle(6, 6, 10, 10)]).unwrap(),
                BTreeSet::from([HorizontalCutSegment::new(0, 6, 8).unwrap()]),
                BTreeSet::new(),
            ),
            (
                "hole-to-hole",
                RectilinearPolygon::new(
                    outer(),
                    vec![
                        clockwise_rectangle(3, 6, 7, 10),
                        clockwise_rectangle(13, 6, 17, 10),
                    ],
                )
                .unwrap(),
                BTreeSet::from([HorizontalCutSegment::new(7, 13, 8).unwrap()]),
                BTreeSet::new(),
            ),
        ];
        for (name, polygon, horizontal, vertical) in cases {
            let prepared = PreparedPolygonContext::new(&polygon).unwrap();
            let subdivision = SparseOrthogonalSubdivision::new(&prepared, &horizontal, &vertical)
                .unwrap_or_else(|error| panic!("{name}: {error}"));
            assert!(subdivision.metrics.vertex_count > 0, "{name}");
            assert!(subdivision.metrics.half_edge_count > 0, "{name}");
            assert!(!subdivision.faces.is_empty(), "{name}");
        }
    }
}
