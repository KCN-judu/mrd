//! Sparse planar subdivision and slab validation for polygon completion.
//!
//! This module deliberately never materializes the Cartesian product of the
//! x and y coordinate sets.  The coordinate arrangement in
//! `polygon_arrangement` remains the independent dense oracle.

use std::collections::{BTreeMap, BTreeSet};

use rect_core::{
    CoordinateRect, DoubledPoint, MemoryEstimate, Point, PreparedPolygonContext, RectilinearPolygon,
};
use serde::{Deserialize, Serialize};

use crate::polygon::{HorizontalCutSegment, PolygonSgError, VerticalCutSegment};

pub mod subdivision;
pub mod validator;

/// Final-geometry recovery implementation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonRecoveryBackend {
    /// Preserved coordinate-compressed flood-fill oracle.
    DenseCoordinateArrangement,
    /// Sparse half-edge subdivision and face walk.
    #[default]
    SparseSubdivision,
    /// Selects one backend from cheap coordinate and segment estimates.
    Auto,
}

impl PolygonRecoveryBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::DenseCoordinateArrangement => "dense-arrangement",
            Self::SparseSubdivision => "sparse-subdivision",
            Self::Auto => "auto",
        }
    }
}

/// Public policy name used by v1.3 crossover evidence.
pub type PolygonRecoveryPolicy = PolygonRecoveryBackend;

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
    pub builder_backend: String,
    pub input_segment_count: usize,
    pub horizontal_segment_count: usize,
    pub vertical_segment_count: usize,
    pub sweep_event_count: usize,
    pub active_set_insertions: usize,
    pub active_set_removals: usize,
    pub range_queries: usize,
    pub candidate_pair_tests: usize,
    pub reported_intersections: usize,
    pub t_junction_count: usize,
    pub endpoint_contact_count: usize,
    pub atomic_segment_count: usize,
    pub materialized_split_coordinates: usize,
    pub vertex_count: usize,
    pub half_edge_count: usize,
    pub face_count: usize,
    pub junction_count: usize,
    pub owned_bytes: usize,
    pub memory_estimate: MemoryEstimate,
}

/// Canonical positive-length atomic segment emitted by a subdivision builder.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct SubdivisionAtomicSegment {
    pub first: Point,
    pub second: Point,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Segment {
    first: Point,
    second: Point,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SegmentProvenance {
    Boundary,
    Cut,
}

fn normalize_collinear_segments(
    sourced: Vec<(Segment, SegmentProvenance)>,
) -> Result<Vec<Segment>, PolygonSgError> {
    let mut by_line = BTreeMap::<(bool, i64), Vec<(Segment, SegmentProvenance)>>::new();
    for item @ (segment, _) in sourced {
        by_line
            .entry((segment.horizontal(), segment.line()))
            .or_default()
            .push(item);
    }
    let mut normalized = BTreeSet::new();
    for line in by_line.values_mut() {
        line.sort_unstable_by_key(|(segment, provenance)| {
            (segment.low(), segment.high(), *provenance)
        });
        let mut previous: Option<(Segment, SegmentProvenance)> = None;
        for &(segment, provenance) in line.iter() {
            if let Some((prior, prior_provenance)) = previous {
                // Coincident boundary/cut records are the same embedded edge;
                // polygon-interior classification remains sourced from the
                // prepared boundary index. Partial overlaps are ambiguous.
                if segment == prior {
                    continue;
                }
                if segment.low() < prior.high() {
                    return Err(PolygonSgError::SparseSubdivision {
                        message: format!(
                            "conflicting collinear segment overlap between {prior:?} ({prior_provenance:?}) and {segment:?} ({provenance:?})"
                        ),
                    });
                }
            }
            normalized.insert(segment);
            previous = Some((segment, provenance));
        }
    }
    Ok(normalized.into_iter().collect())
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

fn initial_split_coordinates(segments: &[Segment]) -> Vec<BTreeSet<i64>> {
    segments
        .iter()
        .map(|segment| BTreeSet::from([segment.low(), segment.high()]))
        .collect()
}

fn record_intersection(
    segments: &[Segment],
    split_coordinates: &mut [BTreeSet<i64>],
    horizontal_id: usize,
    vertical_id: usize,
    point: Point,
    junctions: &mut BTreeSet<Point>,
    metrics: &mut SparseSubdivisionMetrics,
) {
    split_coordinates[horizontal_id].insert(point.x);
    split_coordinates[vertical_id].insert(point.y);
    junctions.insert(point);
    metrics.reported_intersections += 1;
    let horizontal = segments[horizontal_id];
    let vertical = segments[vertical_id];
    let horizontal_endpoint = point.x == horizontal.low() || point.x == horizontal.high();
    let vertical_endpoint = point.y == vertical.low() || point.y == vertical.high();
    if horizontal_endpoint && vertical_endpoint {
        metrics.endpoint_contact_count += 1;
    } else if horizontal_endpoint || vertical_endpoint {
        metrics.t_junction_count += 1;
    }
}

/// Sparse embedded orthogonal graph built from boundary and final cuts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SparseOrthogonalSubdivision {
    pub vertices: Vec<SubdivisionVertex>,
    pub half_edges: Vec<SubdivisionHalfEdge>,
    pub faces: Vec<SubdivisionFace>,
    pub split_junctions: Vec<Point>,
    pub atomic_segments: Vec<SubdivisionAtomicSegment>,
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
        Self::new_with_backend(
            prepared,
            horizontal_cuts,
            vertical_cuts,
            subdivision::Backend::Experiment,
        )
    }

    /// Builds the same canonical subdivision with an explicit intersection
    /// reporting backend.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for malformed segments, failed sparse graph
    /// construction, or exact arithmetic overflow.
    #[allow(clippy::too_many_lines)]
    pub fn new_with_backend(
        prepared: &PreparedPolygonContext,
        horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
        vertical_cuts: &BTreeSet<VerticalCutSegment>,
        backend: subdivision::Backend,
    ) -> Result<Self, PolygonSgError> {
        let mut segments = Vec::new();
        for boundary_loop in prepared.polygon().loops() {
            for (first, second) in boundary_loop.edges() {
                segments.push((Segment::new(first, second)?, SegmentProvenance::Boundary));
            }
        }
        for cut in horizontal_cuts {
            segments.push((
                Segment::new(Point::new(cut.left, cut.y), Point::new(cut.right, cut.y))?,
                SegmentProvenance::Cut,
            ));
        }
        for cut in vertical_cuts {
            segments.push((
                Segment::new(Point::new(cut.x, cut.bottom), Point::new(cut.x, cut.top))?,
                SegmentProvenance::Cut,
            ));
        }
        let segments = normalize_collinear_segments(segments)?;

        let (split_coordinates, split_junctions, mut metrics) =
            subdivision::split(backend, &segments);
        let mut atomic_edges = BTreeSet::new();
        for (segment, coordinates) in segments.iter().zip(&split_coordinates) {
            let coordinates = coordinates.iter().copied().collect::<Vec<_>>();
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
        metrics.atomic_segment_count = atomic_edges.len();
        let atomic_segments = atomic_edges
            .iter()
            .map(|segment| SubdivisionAtomicSegment {
                first: segment.first,
                second: segment.second,
            })
            .collect::<Vec<_>>();

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
        metrics.vertex_count = vertices.len();
        metrics.half_edge_count = half_edges.len();
        metrics.face_count = faces.len();
        metrics.junction_count = junction_count;
        let split_junctions = split_junctions.into_iter().collect::<Vec<_>>();
        let face_boundary_payload = faces
            .iter()
            .map(|face| face.boundary.len() * std::mem::size_of::<SubdivisionHalfEdgeId>())
            .sum::<usize>();
        let face_boundary_capacity = faces
            .iter()
            .map(|face| {
                (face.boundary.capacity() - face.boundary.len())
                    * std::mem::size_of::<SubdivisionHalfEdgeId>()
            })
            .sum::<usize>();
        metrics.memory_estimate = MemoryEstimate {
            retained_payload_bytes: vertices.len() * std::mem::size_of::<SubdivisionVertex>()
                + half_edges.len() * std::mem::size_of::<SubdivisionHalfEdge>()
                + faces.len() * std::mem::size_of::<SubdivisionFace>()
                + face_boundary_payload
                + split_junctions.len() * std::mem::size_of::<Point>()
                + atomic_segments.len() * std::mem::size_of::<SubdivisionAtomicSegment>(),
            retained_collection_capacity_bytes: (vertices.capacity() - vertices.len())
                * std::mem::size_of::<SubdivisionVertex>()
                + (half_edges.capacity() - half_edges.len())
                    * std::mem::size_of::<SubdivisionHalfEdge>()
                + (faces.capacity() - faces.len()) * std::mem::size_of::<SubdivisionFace>()
                + face_boundary_capacity
                + (split_junctions.capacity() - split_junctions.len())
                    * std::mem::size_of::<Point>()
                + (atomic_segments.capacity() - atomic_segments.len())
                    * std::mem::size_of::<SubdivisionAtomicSegment>(),
            retained_container_estimate: faces.len()
                * std::mem::size_of::<Vec<SubdivisionHalfEdgeId>>(),
            peak_temporary_payload_bytes: metrics.materialized_split_coordinates
                * std::mem::size_of::<i64>()
                + vertex_ids.len() * std::mem::size_of::<(Point, SubdivisionVertexId)>()
                + outgoing.len()
                    * std::mem::size_of::<(
                        SubdivisionVertexId,
                        BTreeMap<Direction, SubdivisionHalfEdgeId>,
                    )>(),
            unmeasured_allocator_overhead: true,
        };
        metrics.owned_bytes = metrics.memory_estimate.retained_total_estimate();
        let metrics = SparseSubdivisionMetrics {
            vertex_count: vertices.len(),
            half_edge_count: half_edges.len(),
            face_count: faces.len(),
            junction_count,
            ..metrics
        };
        Ok(Self {
            vertices,
            half_edges,
            faces,
            split_junctions,
            atomic_segments,
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rect_core::{
        CoordinateRect, OrthogonalLoop, Point, PreparedPolygonContext, RectilinearPolygon,
    };

    use crate::polygon::{HorizontalCutSegment, VerticalCutSegment};
    use crate::polygon_arrangement;

    use super::{SparseOrthogonalSubdivision, subdivision, validator};

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
        validator::Validator
            .validate(&polygon, &[CoordinateRect::new(0, 0, 4, 3).unwrap()])
            .unwrap();
    }

    #[test]
    fn event_validator_matches_reference_and_performs_no_slab_rescans() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 4),
                Point::new(0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let valid = [
            CoordinateRect::new(0, 0, 2, 4).unwrap(),
            CoordinateRect::new(2, 0, 4, 4).unwrap(),
        ];
        let reference = validator::Validator
            .validate_with_backend(&polygon, &valid, validator::Backend::Oracle)
            .unwrap();
        let event = validator::Validator
            .validate_with_backend(&polygon, &valid, validator::Backend::Experiment)
            .unwrap();
        assert!(reference.boundary_edge_scans > 0);
        assert!(reference.active_rectangle_resorts > 0);
        assert_eq!(event.boundary_edge_scans, 0);
        assert_eq!(event.active_rectangle_resorts, 0);
        assert!(event.segment_tree_node_visits > 0);
        assert!(event.root_checks > 0);

        let invalid = [
            vec![CoordinateRect {
                x0: 0,
                y0: 0,
                x1: 0,
                y1: 4,
            }],
            vec![CoordinateRect::new(0, 0, 3, 4).unwrap()],
            vec![
                CoordinateRect::new(0, 0, 3, 4).unwrap(),
                CoordinateRect::new(2, 0, 3, 4).unwrap(),
            ],
            vec![
                CoordinateRect::new(0, 0, 3, 4).unwrap(),
                CoordinateRect::new(3, 0, 5, 2).unwrap(),
            ],
        ];
        for rectangles in invalid {
            let reference = validator::Validator
                .validate_with_backend(&polygon, &rectangles, validator::Backend::Oracle)
                .unwrap_err();
            let event = validator::Validator
                .validate_with_backend(&polygon, &rectangles, validator::Backend::Experiment)
                .unwrap_err();
            assert_eq!(
                std::mem::discriminant(&reference),
                std::mem::discriminant(&event),
                "reference={reference:?}, event={event:?}"
            );
        }
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
                polygon_arrangement::Arrangement::new(&prepared, &horizontal, &vertical).unwrap();
            let dense = dense.recover_rectangles().unwrap();
            assert_eq!(sparse, dense);
            validator::Validator.validate(&polygon, &sparse).unwrap();
        }
    }

    #[test]
    fn orthogonal_sweep_matches_range_scan_junctions_and_atomic_segments() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(8, 0),
                Point::new(8, 8),
                Point::new(0, 8),
            ]),
            vec![],
        )
        .unwrap();
        let horizontal = BTreeSet::from([
            HorizontalCutSegment::new(0, 8, 2).unwrap(),
            HorizontalCutSegment::new(0, 4, 4).unwrap(),
            HorizontalCutSegment::new(2, 8, 6).unwrap(),
        ]);
        let vertical = BTreeSet::from([
            VerticalCutSegment::new(2, 0, 6).unwrap(),
            VerticalCutSegment::new(4, 2, 8).unwrap(),
            VerticalCutSegment::new(8, 0, 8).unwrap(),
        ]);
        let prepared = PreparedPolygonContext::new(&polygon).unwrap();
        let reference = SparseOrthogonalSubdivision::new_with_backend(
            &prepared,
            &horizontal,
            &vertical,
            subdivision::Backend::Oracle,
        )
        .unwrap();
        let sweep = SparseOrthogonalSubdivision::new_with_backend(
            &prepared,
            &horizontal,
            &vertical,
            subdivision::Backend::Experiment,
        )
        .unwrap();
        assert_eq!(reference.split_junctions, sweep.split_junctions);
        assert_eq!(reference.atomic_segments, sweep.atomic_segments);
        assert_eq!(reference.vertices, sweep.vertices);
        assert_eq!(reference.half_edges, sweep.half_edges);
        assert_eq!(reference.faces, sweep.faces);
        assert_eq!(sweep.metrics.candidate_pair_tests, 0);
        assert!(sweep.metrics.t_junction_count > 0);
        assert!(sweep.metrics.endpoint_contact_count > 0);
        assert_eq!(
            reference
                .recover_rectangles(&polygon)
                .map_err(|error| error.to_string()),
            sweep
                .recover_rectangles(&polygon)
                .map_err(|error| error.to_string())
        );
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
