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

/// Selects how orthogonal subdivision intersections are reported.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubdivisionBuilderBackend {
    /// Preserved v1.2 horizontal-range scan Oracle.
    ReferenceRangeScan,
    /// Output-sensitive closed-endpoint x sweep.
    #[default]
    OrthogonalSweep,
}

impl SubdivisionBuilderBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferenceRangeScan => "reference-range-scan",
            Self::OrthogonalSweep => "orthogonal-sweep",
        }
    }
}

/// Selects the sparse dissection validator implementation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SparseValidatorBackend {
    /// Preserved v1.2 slab-rescan Oracle.
    ReferenceSlabRescan,
    /// Event-driven y segment tree.
    #[default]
    EventSegmentTree,
}

impl SparseValidatorBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferenceSlabRescan => "reference-slab-rescan",
            Self::EventSegmentTree => "event-segment-tree",
        }
    }
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
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SparseSlabMetrics {
    pub validator_backend: String,
    pub x_event_count: usize,
    pub y_coordinate_count: usize,
    pub range_add_count: usize,
    pub parity_toggle_count: usize,
    pub segment_tree_node_visits: usize,
    pub root_checks: usize,
    pub boundary_edge_scans: usize,
    pub active_rectangle_resorts: usize,
    pub slab_count: usize,
    pub polygon_interval_events: usize,
    pub rectangle_interval_events: usize,
    pub owned_bytes: usize,
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
                if segment == prior && provenance == prior_provenance {
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

fn reference_range_scan_splits(
    segments: &[Segment],
) -> (
    Vec<BTreeSet<i64>>,
    BTreeSet<Point>,
    SparseSubdivisionMetrics,
) {
    let mut split_coordinates = initial_split_coordinates(segments);
    let mut junctions = BTreeSet::new();
    let mut vertical_by_x = BTreeMap::<i64, Vec<usize>>::new();
    let mut horizontal_ids = Vec::new();
    for (id, segment) in segments.iter().copied().enumerate() {
        if segment.horizontal() {
            horizontal_ids.push(id);
        } else {
            vertical_by_x.entry(segment.line()).or_default().push(id);
        }
    }
    let mut metrics = SparseSubdivisionMetrics {
        builder_backend: SubdivisionBuilderBackend::ReferenceRangeScan
            .name()
            .to_owned(),
        input_segment_count: segments.len(),
        horizontal_segment_count: horizontal_ids.len(),
        vertical_segment_count: segments.len() - horizontal_ids.len(),
        ..SparseSubdivisionMetrics::default()
    };
    for horizontal_id in horizontal_ids {
        let horizontal = segments[horizontal_id];
        for (&x, vertical_ids) in vertical_by_x.range(horizontal.low()..=horizontal.high()) {
            for &vertical_id in vertical_ids {
                metrics.candidate_pair_tests += 1;
                let vertical = segments[vertical_id];
                if vertical.low() <= horizontal.line() && horizontal.line() <= vertical.high() {
                    record_intersection(
                        segments,
                        &mut split_coordinates,
                        horizontal_id,
                        vertical_id,
                        Point::new(x, horizontal.line()),
                        &mut junctions,
                        &mut metrics,
                    );
                }
            }
        }
    }
    metrics.materialized_split_coordinates = split_coordinates.iter().map(BTreeSet::len).sum();
    (split_coordinates, junctions, metrics)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum IntersectionEventKind {
    HorizontalStart,
    VerticalQuery,
    HorizontalEnd,
}

fn orthogonal_sweep_splits(
    segments: &[Segment],
) -> (
    Vec<BTreeSet<i64>>,
    BTreeSet<Point>,
    SparseSubdivisionMetrics,
) {
    let mut split_coordinates = initial_split_coordinates(segments);
    let mut junctions = BTreeSet::new();
    let mut events = Vec::with_capacity(segments.len().saturating_mul(2));
    let mut horizontal_count = 0;
    for (id, segment) in segments.iter().copied().enumerate() {
        if segment.horizontal() {
            horizontal_count += 1;
            events.push((segment.low(), IntersectionEventKind::HorizontalStart, id));
            events.push((segment.high(), IntersectionEventKind::HorizontalEnd, id));
        } else {
            events.push((segment.line(), IntersectionEventKind::VerticalQuery, id));
        }
    }
    events.sort_unstable();
    let mut active = BTreeSet::<(i64, usize)>::new();
    let mut metrics = SparseSubdivisionMetrics {
        builder_backend: SubdivisionBuilderBackend::OrthogonalSweep.name().to_owned(),
        input_segment_count: segments.len(),
        horizontal_segment_count: horizontal_count,
        vertical_segment_count: segments.len() - horizontal_count,
        sweep_event_count: events.len(),
        ..SparseSubdivisionMetrics::default()
    };
    for (x, kind, id) in events {
        let segment = segments[id];
        match kind {
            IntersectionEventKind::HorizontalStart => {
                active.insert((segment.line(), id));
                metrics.active_set_insertions += 1;
            }
            IntersectionEventKind::VerticalQuery => {
                metrics.range_queries += 1;
                let intersections = active
                    .range((segment.low(), 0)..=(segment.high(), usize::MAX))
                    .copied()
                    .collect::<Vec<_>>();
                for (y, horizontal_id) in intersections {
                    record_intersection(
                        segments,
                        &mut split_coordinates,
                        horizontal_id,
                        id,
                        Point::new(x, y),
                        &mut junctions,
                        &mut metrics,
                    );
                }
            }
            IntersectionEventKind::HorizontalEnd => {
                active.remove(&(segment.line(), id));
                metrics.active_set_removals += 1;
            }
        }
    }
    metrics.materialized_split_coordinates = split_coordinates.iter().map(BTreeSet::len).sum();
    (split_coordinates, junctions, metrics)
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
            SubdivisionBuilderBackend::OrthogonalSweep,
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
        backend: SubdivisionBuilderBackend,
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

        let (split_coordinates, split_junctions, mut metrics) = match backend {
            SubdivisionBuilderBackend::ReferenceRangeScan => reference_range_scan_splits(&segments),
            SubdivisionBuilderBackend::OrthogonalSweep => orthogonal_sweep_splits(&segments),
        };
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
        metrics.owned_bytes = vertices.len() * std::mem::size_of::<SubdivisionVertex>()
            + half_edges.len() * std::mem::size_of::<SubdivisionHalfEdge>()
            + faces
                .iter()
                .map(|face| face.boundary.len() * std::mem::size_of::<SubdivisionHalfEdgeId>())
                .sum::<usize>();
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
            split_junctions: split_junctions.into_iter().collect(),
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
        self.validate_with_backend(
            polygon,
            rectangles,
            SparseValidatorBackend::EventSegmentTree,
        )
    }

    /// Validates with an explicitly selected sparse backend.
    ///
    /// # Errors
    ///
    /// Returns the first exact geometry, coverage, or area error.
    pub fn validate_with_backend(
        &self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
        backend: SparseValidatorBackend,
    ) -> Result<SparseSlabMetrics, PolygonValidationError> {
        match backend {
            SparseValidatorBackend::ReferenceSlabRescan => {
                self.validate_reference(polygon, rectangles)
            }
            SparseValidatorBackend::EventSegmentTree => {
                self.validate_event_tree(polygon, rectangles)
            }
        }
    }

    fn validate_reference(
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
            validator_backend: SparseValidatorBackend::ReferenceSlabRescan
                .name()
                .to_owned(),
            x_event_count: x_coordinates.len(),
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
            metrics.boundary_edge_scans += horizontal_boundary.len();
            metrics.active_rectangle_resorts += 1;
            metrics.root_checks += 1;
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

    fn validate_event_tree(
        &self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<SparseSlabMetrics, PolygonValidationError> {
        let polygon_area_twice = polygon
            .twice_signed_area()
            .map_err(PolygonValidationError::Polygon)?;
        let mut rectangle_area = 0_i128;
        let mut y_coordinates = polygon
            .loops()
            .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
            .collect::<BTreeSet<_>>();
        let mut events = BTreeMap::<i64, Vec<SlabEvent>>::new();
        for boundary_loop in polygon.loops() {
            for (first, second) in boundary_loop.edges() {
                if first.y != second.y {
                    continue;
                }
                let left = first.x.min(second.x);
                let right = first.x.max(second.x);
                events
                    .entry(left)
                    .or_default()
                    .push(SlabEvent::PolygonToggle(first.y));
                events
                    .entry(right)
                    .or_default()
                    .push(SlabEvent::PolygonToggle(first.y));
            }
        }
        for (index, rectangle) in rectangles.iter().copied().enumerate() {
            if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
                return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
            }
            rectangle_area = rectangle_area
                .checked_add(rectangle.area())
                .ok_or(PolygonValidationError::AreaOverflow)?;
            y_coordinates.extend([rectangle.y0, rectangle.y1]);
            events
                .entry(rectangle.x0)
                .or_default()
                .push(SlabEvent::RectangleStart {
                    bottom: rectangle.y0,
                    top: rectangle.y1,
                });
            events
                .entry(rectangle.x1)
                .or_default()
                .push(SlabEvent::RectangleEnd {
                    bottom: rectangle.y0,
                    top: rectangle.y1,
                });
        }
        let rectangle_area_twice = rectangle_area
            .checked_mul(2)
            .ok_or(PolygonValidationError::AreaOverflow)?;
        if rectangle_area_twice != polygon_area_twice {
            return Err(PolygonValidationError::AreaMismatch {
                polygon_area_twice,
                rectangle_area_twice,
            });
        }

        let y_coordinates = y_coordinates.into_iter().collect::<Vec<_>>();
        let mut tree = ValidationSegmentTree::new(y_coordinates.len().saturating_sub(1));
        let x_coordinates = events.keys().copied().collect::<Vec<_>>();
        let mut metrics = SparseSlabMetrics {
            validator_backend: SparseValidatorBackend::EventSegmentTree.name().to_owned(),
            x_event_count: events.values().map(Vec::len).sum(),
            y_coordinate_count: y_coordinates.len(),
            owned_bytes: y_coordinates.capacity() * std::mem::size_of::<i64>()
                + tree.owned_bytes_estimate(),
            ..SparseSlabMetrics::default()
        };
        for pair in x_coordinates.windows(2) {
            let x = pair[0];
            let Some(changes) = events.get_mut(&x) else {
                continue;
            };
            changes.sort_unstable();
            for event in changes.iter().copied() {
                match event {
                    SlabEvent::PolygonToggle(y) => {
                        let start = y_coordinates
                            .binary_search(&y)
                            .map_err(|_| PolygonValidationError::AreaOverflow)?;
                        if start + 1 < y_coordinates.len() {
                            tree.toggle(start, y_coordinates.len() - 2, &mut metrics);
                            metrics.parity_toggle_count += 1;
                        }
                    }
                    SlabEvent::RectangleEnd { bottom, top } => {
                        update_rectangle_coverage(
                            &mut tree,
                            &y_coordinates,
                            bottom,
                            top,
                            -1,
                            &mut metrics,
                        )?;
                    }
                    SlabEvent::RectangleStart { bottom, top } => {
                        update_rectangle_coverage(
                            &mut tree,
                            &y_coordinates,
                            bottom,
                            top,
                            1,
                            &mut metrics,
                        )?;
                    }
                }
            }
            if pair[0] == pair[1] || tree.leaf_count == 0 {
                continue;
            }
            metrics.slab_count += 1;
            metrics.root_checks += 1;
            let doubled_x = i128::from(pair[0]) + i128::from(pair[1]);
            if let Some(leaf) = tree.first_leaf(ValidationViolation::Overlap, &mut metrics) {
                let point = slab_witness(doubled_x, &y_coordinates, leaf);
                let covering = covering_rectangles(rectangles, point);
                return Err(PolygonValidationError::Overlap {
                    first: covering[0],
                    second: covering[1],
                    point,
                });
            }
            if let Some(leaf) = tree.first_leaf(ValidationViolation::Uncovered, &mut metrics) {
                return Err(PolygonValidationError::UncoveredInterior {
                    point: slab_witness(doubled_x, &y_coordinates, leaf),
                });
            }
            if let Some(leaf) = tree.first_leaf(ValidationViolation::Outside, &mut metrics) {
                let point = slab_witness(doubled_x, &y_coordinates, leaf);
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: covering_rectangles(rectangles, point)[0],
                    point,
                });
            }
        }
        Ok(metrics)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SlabEvent {
    PolygonToggle(i64),
    RectangleEnd { bottom: i64, top: i64 },
    RectangleStart { bottom: i64, top: i64 },
}

#[derive(Clone, Copy, Debug, Default)]
struct ValidationNode {
    present: [bool; 2],
    minimum: [i32; 2],
    maximum: [i32; 2],
    lazy_add: i32,
    lazy_toggle: bool,
}

#[derive(Clone, Copy)]
enum ValidationViolation {
    Overlap,
    Uncovered,
    Outside,
}

struct ValidationSegmentTree {
    nodes: Vec<ValidationNode>,
    leaf_count: usize,
}

impl ValidationSegmentTree {
    fn new(leaf_count: usize) -> Self {
        let mut tree = Self {
            nodes: vec![ValidationNode::default(); leaf_count.saturating_mul(4).max(1)],
            leaf_count,
        };
        if leaf_count > 0 {
            tree.build(1, 0, leaf_count - 1);
        }
        tree
    }

    fn build(&mut self, node: usize, start: usize, end: usize) {
        if start == end {
            self.nodes[node].present[0] = true;
            return;
        }
        let middle = start + (end - start) / 2;
        self.build(node * 2, start, middle);
        self.build(node * 2 + 1, middle + 1, end);
        self.pull(node);
    }

    fn owned_bytes_estimate(&self) -> usize {
        self.nodes.capacity() * std::mem::size_of::<ValidationNode>()
    }

    fn add(&mut self, low: usize, high: usize, delta: i32, metrics: &mut SparseSlabMetrics) {
        self.update_add(1, 0, self.leaf_count - 1, low, high, delta, metrics);
    }

    fn toggle(&mut self, low: usize, high: usize, metrics: &mut SparseSlabMetrics) {
        self.update_toggle(1, 0, self.leaf_count - 1, low, high, metrics);
    }

    fn apply_add(&mut self, node: usize, delta: i32) {
        for parity in 0..2 {
            if self.nodes[node].present[parity] {
                self.nodes[node].minimum[parity] += delta;
                self.nodes[node].maximum[parity] += delta;
            }
        }
        self.nodes[node].lazy_add += delta;
    }

    fn apply_toggle(&mut self, node: usize) {
        self.nodes[node].present.swap(0, 1);
        self.nodes[node].minimum.swap(0, 1);
        self.nodes[node].maximum.swap(0, 1);
        self.nodes[node].lazy_toggle = !self.nodes[node].lazy_toggle;
    }

    fn push(&mut self, node: usize) {
        if self.nodes[node].lazy_toggle {
            self.apply_toggle(node * 2);
            self.apply_toggle(node * 2 + 1);
            self.nodes[node].lazy_toggle = false;
        }
        let delta = self.nodes[node].lazy_add;
        if delta != 0 {
            self.apply_add(node * 2, delta);
            self.apply_add(node * 2 + 1, delta);
            self.nodes[node].lazy_add = 0;
        }
    }

    fn pull(&mut self, node: usize) {
        for parity in 0..2 {
            let left = self.nodes[node * 2];
            let right = self.nodes[node * 2 + 1];
            self.nodes[node].present[parity] = left.present[parity] || right.present[parity];
            self.nodes[node].minimum[parity] = if left.present[parity] && right.present[parity] {
                left.minimum[parity].min(right.minimum[parity])
            } else if left.present[parity] {
                left.minimum[parity]
            } else {
                right.minimum[parity]
            };
            self.nodes[node].maximum[parity] = if left.present[parity] && right.present[parity] {
                left.maximum[parity].max(right.maximum[parity])
            } else if left.present[parity] {
                left.maximum[parity]
            } else {
                right.maximum[parity]
            };
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn update_add(
        &mut self,
        node: usize,
        start: usize,
        end: usize,
        low: usize,
        high: usize,
        delta: i32,
        metrics: &mut SparseSlabMetrics,
    ) {
        metrics.segment_tree_node_visits += 1;
        if low <= start && end <= high {
            self.apply_add(node, delta);
            return;
        }
        self.push(node);
        let middle = start + (end - start) / 2;
        if low <= middle {
            self.update_add(node * 2, start, middle, low, high, delta, metrics);
        }
        if high > middle {
            self.update_add(node * 2 + 1, middle + 1, end, low, high, delta, metrics);
        }
        self.pull(node);
    }

    fn update_toggle(
        &mut self,
        node: usize,
        start: usize,
        end: usize,
        low: usize,
        high: usize,
        metrics: &mut SparseSlabMetrics,
    ) {
        metrics.segment_tree_node_visits += 1;
        if low <= start && end <= high {
            self.apply_toggle(node);
            return;
        }
        self.push(node);
        let middle = start + (end - start) / 2;
        if low <= middle {
            self.update_toggle(node * 2, start, middle, low, high, metrics);
        }
        if high > middle {
            self.update_toggle(node * 2 + 1, middle + 1, end, low, high, metrics);
        }
        self.pull(node);
    }

    fn violates(node: ValidationNode, violation: ValidationViolation) -> bool {
        match violation {
            ValidationViolation::Overlap => {
                (node.present[0] && node.maximum[0] > 1) || (node.present[1] && node.maximum[1] > 1)
            }
            ValidationViolation::Uncovered => node.present[1] && node.minimum[1] == 0,
            ValidationViolation::Outside => node.present[0] && node.maximum[0] > 0,
        }
    }

    fn first_leaf(
        &mut self,
        violation: ValidationViolation,
        metrics: &mut SparseSlabMetrics,
    ) -> Option<usize> {
        Self::violates(self.nodes[1], violation)
            .then(|| self.find_first(1, 0, self.leaf_count - 1, violation, metrics))
    }

    fn find_first(
        &mut self,
        node: usize,
        start: usize,
        end: usize,
        violation: ValidationViolation,
        metrics: &mut SparseSlabMetrics,
    ) -> usize {
        metrics.segment_tree_node_visits += 1;
        if start == end {
            return start;
        }
        self.push(node);
        let middle = start + (end - start) / 2;
        if Self::violates(self.nodes[node * 2], violation) {
            self.find_first(node * 2, start, middle, violation, metrics)
        } else {
            self.find_first(node * 2 + 1, middle + 1, end, violation, metrics)
        }
    }
}

fn update_rectangle_coverage(
    tree: &mut ValidationSegmentTree,
    y_coordinates: &[i64],
    bottom: i64,
    top: i64,
    delta: i32,
    metrics: &mut SparseSlabMetrics,
) -> Result<(), PolygonValidationError> {
    let low = y_coordinates
        .binary_search(&bottom)
        .map_err(|_| PolygonValidationError::AreaOverflow)?;
    let high = y_coordinates
        .binary_search(&top)
        .map_err(|_| PolygonValidationError::AreaOverflow)?;
    if low < high {
        tree.add(low, high - 1, delta, metrics);
        metrics.range_add_count += 1;
    }
    Ok(())
}

fn slab_witness(doubled_x: i128, y_coordinates: &[i64], leaf: usize) -> DoubledPoint {
    DoubledPoint::new(
        doubled_x,
        i128::from(y_coordinates[leaf]) + i128::from(y_coordinates[leaf + 1]),
    )
}

fn covering_rectangles(rectangles: &[CoordinateRect], point: DoubledPoint) -> Vec<usize> {
    rectangles
        .iter()
        .enumerate()
        .filter_map(|(index, rectangle)| {
            (2 * i128::from(rectangle.x0) < point.x
                && point.x < 2 * i128::from(rectangle.x1)
                && 2 * i128::from(rectangle.y0) < point.y
                && point.y < 2 * i128::from(rectangle.y1))
            .then_some(index)
        })
        .collect()
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

    use super::{SparseOrthogonalSubdivision, SparseSlabValidator, SubdivisionBuilderBackend};

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
            SubdivisionBuilderBackend::ReferenceRangeScan,
        )
        .unwrap();
        let sweep = SparseOrthogonalSubdivision::new_with_backend(
            &prepared,
            &horizontal,
            &vertical,
            SubdivisionBuilderBackend::OrthogonalSweep,
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
