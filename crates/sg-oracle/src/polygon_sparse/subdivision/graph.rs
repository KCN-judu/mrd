//! Shared sparse subdivision model and deterministic half-edge construction.

use std::collections::{BTreeMap, BTreeSet};

use mrd_domain::{
    CoordinateRect, DoubledPoint, MemoryEstimate, Point, PreparedPolygonContext, RectilinearPolygon,
};
use serde::{Deserialize, Serialize};

use crate::polygon::{HorizontalCutSegment, PolygonSgError, VerticalCutSegment};

use super::{Backend, split};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VertexId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HalfEdgeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FaceId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Vertex {
    pub id: VertexId,
    pub point: Point,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HalfEdge {
    pub id: HalfEdgeId,
    pub origin: VertexId,
    pub destination: VertexId,
    pub twin: HalfEdgeId,
    pub next: HalfEdgeId,
    pub previous: HalfEdgeId,
    pub face: FaceId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Face {
    pub id: FaceId,
    pub boundary: Vec<HalfEdgeId>,
    pub signed_area_twice: i128,
    pub polygon_interior_on_left: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Metrics {
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
pub struct AtomicSegment {
    pub first: Point,
    pub second: Point,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Segment {
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

    pub(super) const fn horizontal(self) -> bool {
        self.first.y == self.second.y
    }

    pub(super) const fn low(self) -> i64 {
        if self.horizontal() {
            self.first.x
        } else {
            self.first.y
        }
    }

    pub(super) const fn high(self) -> i64 {
        if self.horizontal() {
            self.second.x
        } else {
            self.second.y
        }
    }

    pub(super) const fn line(self) -> i64 {
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

pub(super) fn initial_split_coordinates(segments: &[Segment]) -> Vec<BTreeSet<i64>> {
    segments
        .iter()
        .map(|segment| BTreeSet::from([segment.low(), segment.high()]))
        .collect()
}

pub(super) fn record_intersection(
    segments: &[Segment],
    split_coordinates: &mut [BTreeSet<i64>],
    horizontal_id: usize,
    vertical_id: usize,
    point: Point,
    junctions: &mut BTreeSet<Point>,
    metrics: &mut Metrics,
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
pub struct Graph {
    pub vertices: Vec<Vertex>,
    pub half_edges: Vec<HalfEdge>,
    pub faces: Vec<Face>,
    pub split_junctions: Vec<Point>,
    pub atomic_segments: Vec<AtomicSegment>,
    pub metrics: Metrics,
}

impl Graph {
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
        Self::with_backend(
            prepared,
            horizontal_cuts,
            vertical_cuts,
            Backend::Experiment,
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
    pub fn with_backend(
        prepared: &PreparedPolygonContext,
        horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
        vertical_cuts: &BTreeSet<VerticalCutSegment>,
        backend: Backend,
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

        let (split_coordinates, split_junctions, mut metrics) = split(backend, &segments);
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
            .map(|segment| AtomicSegment {
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
            .map(|(id, point)| (point, VertexId(id)))
            .collect::<BTreeMap<_, _>>();
        let vertices = vertex_points
            .iter()
            .copied()
            .enumerate()
            .map(|(id, point)| Vertex {
                id: VertexId(id),
                point,
            })
            .collect::<Vec<_>>();
        let mut half_edges = Vec::with_capacity(atomic_edges.len() * 2);
        let mut outgoing = BTreeMap::<VertexId, BTreeMap<Direction, HalfEdgeId>>::new();
        for edge in atomic_edges {
            let first = vertex_ids[&edge.first];
            let second = vertex_ids[&edge.second];
            let forward = HalfEdgeId(half_edges.len());
            let backward = HalfEdgeId(half_edges.len() + 1);
            half_edges.push(HalfEdge {
                id: forward,
                origin: first,
                destination: second,
                twin: backward,
                next: forward,
                previous: forward,
                face: FaceId(usize::MAX),
            });
            half_edges.push(HalfEdge {
                id: backward,
                origin: second,
                destination: first,
                twin: forward,
                next: backward,
                previous: backward,
                face: FaceId(usize::MAX),
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
            half_edges[next].previous = HalfEdgeId(index);
        }

        let mut faces = Vec::new();
        for seed in 0..half_edges.len() {
            if half_edges[seed].face.0 != usize::MAX {
                continue;
            }
            let id = FaceId(faces.len());
            let mut boundary = Vec::new();
            let mut current = HalfEdgeId(seed);
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
            faces.push(Face {
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
            .map(|face| face.boundary.len() * std::mem::size_of::<HalfEdgeId>())
            .sum::<usize>();
        let face_boundary_capacity = faces
            .iter()
            .map(|face| {
                (face.boundary.capacity() - face.boundary.len()) * std::mem::size_of::<HalfEdgeId>()
            })
            .sum::<usize>();
        metrics.memory_estimate = MemoryEstimate {
            retained_payload_bytes: vertices.len() * std::mem::size_of::<Vertex>()
                + half_edges.len() * std::mem::size_of::<HalfEdge>()
                + faces.len() * std::mem::size_of::<Face>()
                + face_boundary_payload
                + split_junctions.len() * std::mem::size_of::<Point>()
                + atomic_segments.len() * std::mem::size_of::<AtomicSegment>(),
            retained_collection_capacity_bytes: (vertices.capacity() - vertices.len())
                * std::mem::size_of::<Vertex>()
                + (half_edges.capacity() - half_edges.len()) * std::mem::size_of::<HalfEdge>()
                + (faces.capacity() - faces.len()) * std::mem::size_of::<Face>()
                + face_boundary_capacity
                + (split_junctions.capacity() - split_junctions.len())
                    * std::mem::size_of::<Point>()
                + (atomic_segments.capacity() - atomic_segments.len())
                    * std::mem::size_of::<AtomicSegment>(),
            retained_container_estimate: faces.len() * std::mem::size_of::<Vec<HalfEdgeId>>(),
            peak_temporary_payload_bytes: metrics.materialized_split_coordinates
                * std::mem::size_of::<i64>()
                + vertex_ids.len() * std::mem::size_of::<(Point, VertexId)>()
                + outgoing.len()
                    * std::mem::size_of::<(VertexId, BTreeMap<Direction, HalfEdgeId>)>(),
            unmeasured_allocator_overhead: true,
        };
        metrics.owned_bytes = metrics.memory_estimate.retained_total_estimate();
        let metrics = Metrics {
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
