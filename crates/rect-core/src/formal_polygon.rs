use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Deserializer, Serialize, de};
use thiserror::Error;

use crate::{
    Coord, DoubledPoint, OrthogonalLoop, Point, PolygonError, PolygonLoopId, RectilinearDomain,
    RectilinearPolygon,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FormalVertexId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ElementarySegmentId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FormalBoundaryComponentId(pub usize);

/// One closed nonzero axis-aligned segment in the ornament.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct OrnamentSegment {
    pub start: Point,
    pub end: Point,
}

impl OrnamentSegment {
    /// Creates a segment with endpoints in canonical lexicographic order.
    ///
    /// # Errors
    ///
    /// Returns [`FormalPolygonError`] for a zero-length or non-axis-aligned segment.
    pub fn new(start: Point, end: Point) -> Result<Self, FormalPolygonError> {
        canonical_segment(start, end, 0)
    }

    #[must_use]
    pub const fn is_horizontal(self) -> bool {
        self.start.y == self.end.y
    }

    #[must_use]
    pub const fn is_vertical(self) -> bool {
        self.start.x == self.end.x
    }
}

/// The source paper's finite family of isolated points and closed segments.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Ornament {
    #[serde(default)]
    pub isolated_points: Vec<Point>,
    #[serde(default)]
    pub segments: Vec<OrnamentSegment>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormalBoundarySource {
    Topological {
        loop_id: PolygonLoopId,
        edge_index: usize,
    },
    Ornament {
        segment_index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalVertex {
    pub id: FormalVertexId,
    pub point: Point,
    pub isolated: bool,
    pub incident_segments: Vec<ElementarySegmentId>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ElementarySegment {
    pub id: ElementarySegmentId,
    pub start: FormalVertexId,
    pub end: FormalVertexId,
    pub sources: Vec<FormalBoundarySource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormalBoundaryComponentKind {
    Exterior,
    Hole,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FormalBoundaryDimension {
    Point,
    Segment,
    Topological,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalBoundaryComponent {
    pub id: FormalBoundaryComponentId,
    pub kind: FormalBoundaryComponentKind,
    pub dimension: FormalBoundaryDimension,
    pub vertices: Vec<FormalVertexId>,
    pub elementary_segments: Vec<ElementarySegmentId>,
    pub topological_loops: Vec<PolygonLoopId>,
    pub ornament_segments: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FormalBoundaryIncidence {
    pub vertices: Vec<FormalVertex>,
    pub elementary_segments: Vec<ElementarySegment>,
    pub components: Vec<FormalBoundaryComponent>,
}

impl FormalBoundaryIncidence {
    pub fn formal_holes(&self) -> impl Iterator<Item = &FormalBoundaryComponent> {
        self.components
            .iter()
            .filter(|component| component.kind == FormalBoundaryComponentKind::Hole)
    }
}

/// A source-faithful formal boundary over a validated ordinary topological region.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FormalRectilinearPolygon {
    region: RectilinearPolygon,
    ornament: Ornament,
}

impl FormalRectilinearPolygon {
    /// Normalizes and validates the topological region and ornament.
    ///
    /// # Errors
    ///
    /// Returns a structured error when either the ordinary region or one of
    /// Soltan--Gorpinevich's ornament conditions is violated.
    pub fn new(region: RectilinearPolygon, ornament: Ornament) -> Result<Self, FormalPolygonError> {
        let region = RectilinearPolygon::new(region.outer, region.holes)?;
        let ornament = normalize_ornament(&region, ornament)?;
        let polygon = Self { region, ornament };
        polygon.validate()?;
        Ok(polygon)
    }

    #[must_use]
    pub const fn region(&self) -> &RectilinearPolygon {
        &self.region
    }

    #[must_use]
    pub const fn ornament(&self) -> &Ornament {
        &self.ornament
    }

    /// Re-normalizes an existing value. This operation is idempotent.
    ///
    /// # Errors
    ///
    /// Returns the same structured errors as [`Self::new`].
    pub fn normalized(&self) -> Result<Self, FormalPolygonError> {
        Self::new(self.region.clone(), self.ornament.clone())
    }

    /// Validates the exact formal-boundary representation contract.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic region, containment, incidence, or
    /// ornament-intersection error.
    pub fn validate(&self) -> Result<(), FormalPolygonError> {
        self.region.validate()?;
        validate_canonical_ornament(&self.region, &self.ornament)
    }

    /// Derives vertices, elementary segments, and formal-boundary components.
    ///
    /// # Errors
    ///
    /// Returns a structured validation error if this value was constructed
    /// outside [`Self::new`] and violates its invariants.
    pub fn incidence(&self) -> Result<FormalBoundaryIncidence, FormalPolygonError> {
        self.validate()?;
        Ok(build_incidence(&self.region, &self.ornament))
    }

    /// Returns the ordinary region's exact doubled area. Removing a finite
    /// union of points and segments does not change area.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonError::AreaOverflow`] if exact accumulation overflows.
    pub fn area_twice(&self) -> Result<i128, PolygonError> {
        self.region.twice_signed_area()
    }

    /// Exact strict-formal-interior test for a doubled-coordinate point.
    #[must_use]
    pub fn contains_doubled_point_strict(&self, point: DoubledPoint) -> bool {
        self.region.contains_doubled_point_strict(point)
            && !point_on_ornament_doubled(point, &self.ornament)
    }

    /// Returns true exactly when the open horizontal segment is contained in
    /// the formal interior, including exclusion of every ornament contact.
    #[must_use]
    pub fn contains_open_horizontal_segment(
        &self,
        left: Coord,
        right: Coord,
        doubled_y: i128,
    ) -> bool {
        self.region
            .contains_open_horizontal_segment(left, right, doubled_y)
            && !horizontal_open_segment_meets_ornament(left, right, doubled_y, &self.ornament)
    }

    /// Returns true exactly when the open vertical segment is contained in the
    /// formal interior, including exclusion of every ornament contact.
    #[must_use]
    pub fn contains_open_vertical_segment(
        &self,
        doubled_x: i128,
        bottom: Coord,
        top: Coord,
    ) -> bool {
        self.region
            .contains_open_vertical_segment(doubled_x, bottom, top)
            && !vertical_open_segment_meets_ornament(doubled_x, bottom, top, &self.ornament)
    }
}

impl RectilinearDomain for FormalRectilinearPolygon {
    fn contains_doubled_point_strict(&self, point: DoubledPoint) -> bool {
        self.contains_doubled_point_strict(point)
    }

    fn contains_open_horizontal_segment(&self, left: Coord, right: Coord, doubled_y: i128) -> bool {
        self.contains_open_horizontal_segment(left, right, doubled_y)
    }

    fn contains_open_vertical_segment(&self, doubled_x: i128, bottom: Coord, top: Coord) -> bool {
        self.contains_open_vertical_segment(doubled_x, bottom, top)
    }

    fn area_twice(&self) -> Result<i128, PolygonError> {
        self.area_twice()
    }
}

impl<'de> Deserialize<'de> for FormalRectilinearPolygon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RawFormalPolygon {
            region: RectilinearPolygon,
            #[serde(default)]
            ornament: Ornament,
        }

        let raw = RawFormalPolygon::deserialize(deserializer)?;
        Self::new(raw.region, raw.ornament).map_err(de::Error::custom)
    }
}

fn normalize_ornament(
    region: &RectilinearPolygon,
    ornament: Ornament,
) -> Result<Ornament, FormalPolygonError> {
    let mut indexed_points = ornament
        .isolated_points
        .into_iter()
        .enumerate()
        .collect::<Vec<_>>();
    indexed_points.sort_by_key(|(_, point)| *point);
    for pair in indexed_points.windows(2) {
        if pair[0].1 == pair[1].1 {
            return Err(FormalPolygonError::DuplicateIsolatedPoint {
                first: pair[0].0,
                second: pair[1].0,
                point: pair[0].1,
            });
        }
    }
    let isolated_points = indexed_points
        .into_iter()
        .map(|(_, point)| point)
        .collect::<Vec<_>>();

    let mut indexed_segments = ornament
        .segments
        .into_iter()
        .enumerate()
        .map(|(index, segment)| {
            canonical_segment(segment.start, segment.end, index).map(|segment| (index, segment))
        })
        .collect::<Result<Vec<_>, _>>()?;
    indexed_segments.sort_by_key(|(_, segment)| *segment);
    for pair in indexed_segments.windows(2) {
        if pair[0].1 == pair[1].1 {
            return Err(FormalPolygonError::DuplicateOrnamentSegment {
                first: pair[0].0,
                second: pair[1].0,
                segment: pair[0].1,
            });
        }
    }
    let segments = indexed_segments
        .into_iter()
        .map(|(_, segment)| segment)
        .collect::<Vec<_>>();
    let normalized = Ornament {
        isolated_points,
        segments,
    };
    validate_canonical_ornament(region, &normalized)?;
    Ok(normalized)
}

fn canonical_segment(
    start: Point,
    end: Point,
    index: usize,
) -> Result<OrnamentSegment, FormalPolygonError> {
    if start == end {
        return Err(FormalPolygonError::ZeroLengthOrnamentSegment {
            index,
            point: start,
        });
    }
    if start.x != end.x && start.y != end.y {
        return Err(FormalPolygonError::NonAxisAlignedOrnamentSegment { index, start, end });
    }
    let (start, end) = if start < end {
        (start, end)
    } else {
        (end, start)
    };
    Ok(OrnamentSegment { start, end })
}

fn validate_canonical_ornament(
    region: &RectilinearPolygon,
    ornament: &Ornament,
) -> Result<(), FormalPolygonError> {
    for (index, &point) in ornament.isolated_points.iter().enumerate() {
        if !region.contains_doubled_point_strict(DoubledPoint::from_point(point)) {
            return Err(FormalPolygonError::IsolatedPointOutsideInterior { index, point });
        }
        for (segment_index, &segment) in ornament.segments.iter().enumerate() {
            if point_on_segment(point, segment) {
                return Err(FormalPolygonError::IsolatedPointOnSegment {
                    point_index: index,
                    segment_index,
                    point,
                });
            }
        }
    }

    for (index, &segment) in ornament.segments.iter().enumerate() {
        if segment.start >= segment.end {
            return Err(FormalPolygonError::NonCanonicalOrnamentSegment { index, segment });
        }
        for endpoint in [segment.start, segment.end] {
            if !point_in_region_closed(region, endpoint) {
                return Err(FormalPolygonError::OrnamentEndpointOutsideRegion {
                    index,
                    point: endpoint,
                });
            }
        }
        let interior_is_valid = if segment.is_horizontal() {
            region.contains_open_horizontal_segment(
                segment.start.x,
                segment.end.x,
                2 * i128::from(segment.start.y),
            )
        } else {
            region.contains_open_vertical_segment(
                2 * i128::from(segment.start.x),
                segment.start.y,
                segment.end.y,
            )
        };
        if !interior_is_valid {
            return Err(FormalPolygonError::OrnamentInteriorOutsideRegion { index, segment });
        }
    }

    for first in 0..ornament.segments.len() {
        for second in first + 1..ornament.segments.len() {
            match segment_intersection(ornament.segments[first], ornament.segments[second]) {
                SegmentIntersection::None => {}
                SegmentIntersection::Overlap => {
                    return Err(FormalPolygonError::OverlappingOrnamentSegments { first, second });
                }
                SegmentIntersection::Point(point) => {
                    let first_endpoint = is_endpoint(point, ornament.segments[first]);
                    let second_endpoint = is_endpoint(point, ornament.segments[second]);
                    if !first_endpoint || !second_endpoint {
                        return Err(FormalPolygonError::NonVertexOrnamentIntersection {
                            first,
                            second,
                            point,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn build_incidence(region: &RectilinearPolygon, ornament: &Ornament) -> FormalBoundaryIncidence {
    let mut points = region
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().copied())
        .chain(
            ornament
                .segments
                .iter()
                .flat_map(|segment| [segment.start, segment.end]),
        )
        .chain(ornament.isolated_points.iter().copied())
        .collect::<BTreeSet<_>>();

    for segment in &ornament.segments {
        for boundary_loop in region.loops() {
            for edge in boundary_loop.edges() {
                for endpoint in [segment.start, segment.end] {
                    if point_on_raw_segment(endpoint, edge) {
                        points.insert(endpoint);
                    }
                }
            }
        }
    }

    let point_list = points.into_iter().collect::<Vec<_>>();
    let point_ids = point_list
        .iter()
        .enumerate()
        .map(|(index, &point)| (point, FormalVertexId(index)))
        .collect::<BTreeMap<_, _>>();
    let mut elementary = BTreeMap::<(Point, Point), BTreeSet<FormalBoundarySource>>::new();

    for (loop_index, boundary_loop) in region.loops().enumerate() {
        for (edge_index, edge) in boundary_loop.edges().enumerate() {
            split_source_segment(
                edge.0,
                edge.1,
                FormalBoundarySource::Topological {
                    loop_id: PolygonLoopId(loop_index),
                    edge_index,
                },
                &point_list,
                &mut elementary,
            );
        }
    }
    for (segment_index, segment) in ornament.segments.iter().enumerate() {
        split_source_segment(
            segment.start,
            segment.end,
            FormalBoundarySource::Ornament { segment_index },
            &point_list,
            &mut elementary,
        );
    }

    let elementary_segments = elementary
        .into_iter()
        .enumerate()
        .map(|(index, ((start, end), sources))| ElementarySegment {
            id: ElementarySegmentId(index),
            start: point_ids[&start],
            end: point_ids[&end],
            sources: sources.into_iter().collect(),
        })
        .collect::<Vec<_>>();
    let mut incident = vec![Vec::new(); point_list.len()];
    for segment in &elementary_segments {
        incident[segment.start.0].push(segment.id);
        incident[segment.end.0].push(segment.id);
    }
    let vertices = point_list
        .into_iter()
        .enumerate()
        .map(|(index, point)| FormalVertex {
            id: FormalVertexId(index),
            point,
            isolated: incident[index].is_empty(),
            incident_segments: incident[index].clone(),
        })
        .collect::<Vec<_>>();
    let components = build_components(&vertices, &elementary_segments);
    FormalBoundaryIncidence {
        vertices,
        elementary_segments,
        components,
    }
}

fn split_source_segment(
    first: Point,
    second: Point,
    source: FormalBoundarySource,
    vertices: &[Point],
    output: &mut BTreeMap<(Point, Point), BTreeSet<FormalBoundarySource>>,
) {
    let (start, end) = ordered_points(first, second);
    let mut split_points = vertices
        .iter()
        .copied()
        .filter(|&point| point_on_raw_segment(point, (start, end)))
        .collect::<Vec<_>>();
    split_points.sort_unstable();
    for pair in split_points.windows(2) {
        if pair[0] != pair[1] {
            output.entry((pair[0], pair[1])).or_default().insert(source);
        }
    }
}

fn build_components(
    vertices: &[FormalVertex],
    segments: &[ElementarySegment],
) -> Vec<FormalBoundaryComponent> {
    let mut adjacency = vec![Vec::<(FormalVertexId, ElementarySegmentId)>::new(); vertices.len()];
    for segment in segments {
        adjacency[segment.start.0].push((segment.end, segment.id));
        adjacency[segment.end.0].push((segment.start, segment.id));
    }
    let mut seen = vec![false; vertices.len()];
    let mut raw_components = Vec::new();
    for start in 0..vertices.len() {
        if seen[start] {
            continue;
        }
        seen[start] = true;
        let mut queue = VecDeque::from([FormalVertexId(start)]);
        let mut component_vertices = BTreeSet::new();
        let mut component_segments = BTreeSet::new();
        let mut topological_loops = BTreeSet::new();
        let mut ornament_segments = BTreeSet::new();
        while let Some(vertex) = queue.pop_front() {
            component_vertices.insert(vertex);
            for &(neighbor, segment_id) in &adjacency[vertex.0] {
                component_segments.insert(segment_id);
                for source in &segments[segment_id.0].sources {
                    match *source {
                        FormalBoundarySource::Topological { loop_id, .. } => {
                            topological_loops.insert(loop_id);
                        }
                        FormalBoundarySource::Ornament { segment_index } => {
                            ornament_segments.insert(segment_index);
                        }
                    }
                }
                if !seen[neighbor.0] {
                    seen[neighbor.0] = true;
                    queue.push_back(neighbor);
                }
            }
        }
        let kind = if topological_loops.contains(&PolygonLoopId(0)) {
            FormalBoundaryComponentKind::Exterior
        } else {
            FormalBoundaryComponentKind::Hole
        };
        let dimension = if !topological_loops.is_empty() {
            FormalBoundaryDimension::Topological
        } else if !component_segments.is_empty() {
            FormalBoundaryDimension::Segment
        } else {
            FormalBoundaryDimension::Point
        };
        raw_components.push(FormalBoundaryComponent {
            id: FormalBoundaryComponentId(0),
            kind,
            dimension,
            vertices: component_vertices.into_iter().collect(),
            elementary_segments: component_segments.into_iter().collect(),
            topological_loops: topological_loops.into_iter().collect(),
            ornament_segments: ornament_segments.into_iter().collect(),
        });
    }
    raw_components.sort_by_key(|component| {
        (
            component.kind != FormalBoundaryComponentKind::Exterior,
            component.vertices.first().copied(),
        )
    });
    for (index, component) in raw_components.iter_mut().enumerate() {
        component.id = FormalBoundaryComponentId(index);
    }
    raw_components
}

fn point_in_region_closed(region: &RectilinearPolygon, point: Point) -> bool {
    region.contains_doubled_point_strict(DoubledPoint::from_point(point))
        || region
            .loops()
            .flat_map(OrthogonalLoop::edges)
            .any(|edge| point_on_raw_segment(point, edge))
}

fn point_on_ornament_doubled(point: DoubledPoint, ornament: &Ornament) -> bool {
    ornament.isolated_points.iter().any(|candidate| {
        point.x == 2 * i128::from(candidate.x) && point.y == 2 * i128::from(candidate.y)
    }) || ornament.segments.iter().any(|segment| {
        if segment.is_horizontal() {
            point.y == 2 * i128::from(segment.start.y)
                && 2 * i128::from(segment.start.x) <= point.x
                && point.x <= 2 * i128::from(segment.end.x)
        } else {
            point.x == 2 * i128::from(segment.start.x)
                && 2 * i128::from(segment.start.y) <= point.y
                && point.y <= 2 * i128::from(segment.end.y)
        }
    })
}

fn horizontal_open_segment_meets_ornament(
    left: Coord,
    right: Coord,
    doubled_y: i128,
    ornament: &Ornament,
) -> bool {
    if left >= right {
        return false;
    }
    let left_twice = 2 * i128::from(left);
    let right_twice = 2 * i128::from(right);
    ornament.isolated_points.iter().any(|point| {
        doubled_y == 2 * i128::from(point.y)
            && left_twice < 2 * i128::from(point.x)
            && 2 * i128::from(point.x) < right_twice
    }) || ornament.segments.iter().any(|segment| {
        if segment.is_horizontal() {
            doubled_y == 2 * i128::from(segment.start.y)
                && left_twice.max(2 * i128::from(segment.start.x))
                    < right_twice.min(2 * i128::from(segment.end.x))
        } else {
            left_twice < 2 * i128::from(segment.start.x)
                && 2 * i128::from(segment.start.x) < right_twice
                && 2 * i128::from(segment.start.y) <= doubled_y
                && doubled_y <= 2 * i128::from(segment.end.y)
        }
    })
}

fn vertical_open_segment_meets_ornament(
    doubled_x: i128,
    bottom: Coord,
    top: Coord,
    ornament: &Ornament,
) -> bool {
    if bottom >= top {
        return false;
    }
    let bottom_twice = 2 * i128::from(bottom);
    let top_twice = 2 * i128::from(top);
    ornament.isolated_points.iter().any(|point| {
        doubled_x == 2 * i128::from(point.x)
            && bottom_twice < 2 * i128::from(point.y)
            && 2 * i128::from(point.y) < top_twice
    }) || ornament.segments.iter().any(|segment| {
        if segment.is_vertical() {
            doubled_x == 2 * i128::from(segment.start.x)
                && bottom_twice.max(2 * i128::from(segment.start.y))
                    < top_twice.min(2 * i128::from(segment.end.y))
        } else {
            bottom_twice < 2 * i128::from(segment.start.y)
                && 2 * i128::from(segment.start.y) < top_twice
                && 2 * i128::from(segment.start.x) <= doubled_x
                && doubled_x <= 2 * i128::from(segment.end.x)
        }
    })
}

fn point_on_segment(point: Point, segment: OrnamentSegment) -> bool {
    point_on_raw_segment(point, (segment.start, segment.end))
}

fn point_on_raw_segment(point: Point, segment: (Point, Point)) -> bool {
    let (start, end) = ordered_points(segment.0, segment.1);
    if start.y == end.y {
        point.y == start.y && start.x <= point.x && point.x <= end.x
    } else {
        point.x == start.x && start.y <= point.y && point.y <= end.y
    }
}

fn is_endpoint(point: Point, segment: OrnamentSegment) -> bool {
    point == segment.start || point == segment.end
}

fn ordered_points(first: Point, second: Point) -> (Point, Point) {
    if first < second {
        (first, second)
    } else {
        (second, first)
    }
}

enum SegmentIntersection {
    None,
    Point(Point),
    Overlap,
}

fn segment_intersection(first: OrnamentSegment, second: OrnamentSegment) -> SegmentIntersection {
    if first.is_horizontal() == second.is_horizontal() {
        let same_line = if first.is_horizontal() {
            first.start.y == second.start.y
        } else {
            first.start.x == second.start.x
        };
        if !same_line {
            return SegmentIntersection::None;
        }
        let (first_start, first_end, second_start, second_end) = if first.is_horizontal() {
            (first.start.x, first.end.x, second.start.x, second.end.x)
        } else {
            (first.start.y, first.end.y, second.start.y, second.end.y)
        };
        let start = first_start.max(second_start);
        let end = first_end.min(second_end);
        if start > end {
            return SegmentIntersection::None;
        }
        if start < end {
            return SegmentIntersection::Overlap;
        }
        return SegmentIntersection::Point(if first.is_horizontal() {
            Point::new(start, first.start.y)
        } else {
            Point::new(first.start.x, start)
        });
    }
    let (horizontal, vertical) = if first.is_horizontal() {
        (first, second)
    } else {
        (second, first)
    };
    if horizontal.start.x <= vertical.start.x
        && vertical.start.x <= horizontal.end.x
        && vertical.start.y <= horizontal.start.y
        && horizontal.start.y <= vertical.end.y
    {
        SegmentIntersection::Point(Point::new(vertical.start.x, horizontal.start.y))
    } else {
        SegmentIntersection::None
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq, Serialize, Deserialize)]
pub enum FormalPolygonError {
    #[error(transparent)]
    Polygon(#[from] PolygonError),
    #[error("ornament segment {index} has zero length at {point:?}")]
    ZeroLengthOrnamentSegment { index: usize, point: Point },
    #[error("ornament segment {index} from {start:?} to {end:?} is not axis aligned")]
    NonAxisAlignedOrnamentSegment {
        index: usize,
        start: Point,
        end: Point,
    },
    #[error("ornament segment {index} is not in canonical endpoint order: {segment:?}")]
    NonCanonicalOrnamentSegment {
        index: usize,
        segment: OrnamentSegment,
    },
    #[error("isolated ornament point {point:?} is repeated at indices {first} and {second}")]
    DuplicateIsolatedPoint {
        first: usize,
        second: usize,
        point: Point,
    },
    #[error("ornament segment {segment:?} is repeated at indices {first} and {second}")]
    DuplicateOrnamentSegment {
        first: usize,
        second: usize,
        segment: OrnamentSegment,
    },
    #[error(
        "isolated ornament point {index} at {point:?} is not in the strict topological interior"
    )]
    IsolatedPointOutsideInterior { index: usize, point: Point },
    #[error("isolated ornament point {point_index} at {point:?} lies on segment {segment_index}")]
    IsolatedPointOnSegment {
        point_index: usize,
        segment_index: usize,
        point: Point,
    },
    #[error("ornament segment {index} endpoint {point:?} lies outside the topological region")]
    OrnamentEndpointOutsideRegion { index: usize, point: Point },
    #[error(
        "the open interior of ornament segment {index} is not contained in the topological interior: {segment:?}"
    )]
    OrnamentInteriorOutsideRegion {
        index: usize,
        segment: OrnamentSegment,
    },
    #[error("ornament segments {first} and {second} overlap in positive length")]
    OverlappingOrnamentSegments { first: usize, second: usize },
    #[error("ornament segments {first} and {second} meet at non-vertex point {point:?}")]
    NonVertexOrnamentIntersection {
        first: usize,
        second: usize,
        point: Point,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        FormalBoundaryComponentKind, FormalBoundaryDimension, FormalPolygonError,
        FormalRectilinearPolygon, Ornament, OrnamentSegment,
    };
    use crate::{OrthogonalLoop, Point, RectilinearPolygon};

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    fn region() -> RectilinearPolygon {
        RectilinearPolygon::new(
            rectangle(0, 0, 20, 20),
            vec![rectangle(2, 2, 4, 4), rectangle(16, 16, 18, 18)],
        )
        .unwrap()
    }

    #[test]
    fn normalizes_and_round_trips_exactly() {
        let polygon = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![Point::new(14, 14), Point::new(6, 6)],
                segments: vec![
                    OrnamentSegment {
                        start: Point::new(10, 10),
                        end: Point::new(8, 10),
                    },
                    OrnamentSegment {
                        start: Point::new(10, 12),
                        end: Point::new(10, 10),
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(polygon.normalized().unwrap(), polygon);
        assert_eq!(polygon.ornament().isolated_points[0], Point::new(6, 6));
        assert!(
            polygon
                .ornament()
                .segments
                .iter()
                .all(|segment| segment.start < segment.end)
        );
        let serialized = serde_json::to_string(&polygon).unwrap();
        let deserialized: FormalRectilinearPolygon = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, polygon);
        let permuted = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![Point::new(6, 6), Point::new(14, 14)],
                segments: vec![
                    OrnamentSegment {
                        start: Point::new(10, 10),
                        end: Point::new(10, 12),
                    },
                    OrnamentSegment {
                        start: Point::new(8, 10),
                        end: Point::new(10, 10),
                    },
                ],
            },
        )
        .unwrap();
        assert_eq!(permuted, polygon);
        assert_eq!(permuted.incidence().unwrap(), polygon.incidence().unwrap());
    }

    #[test]
    fn derives_point_segment_topological_holes_and_exterior_incidence() {
        let polygon = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![Point::new(6, 6)],
                segments: vec![
                    OrnamentSegment::new(Point::new(8, 8), Point::new(12, 8)).unwrap(),
                    OrnamentSegment::new(Point::new(12, 8), Point::new(12, 12)).unwrap(),
                    OrnamentSegment::new(Point::new(0, 10), Point::new(2, 10)).unwrap(),
                ],
            },
        )
        .unwrap();
        let incidence = polygon.incidence().unwrap();
        assert_eq!(
            incidence
                .components
                .iter()
                .filter(|component| component.kind == FormalBoundaryComponentKind::Exterior)
                .count(),
            1
        );
        let dimensions = incidence
            .formal_holes()
            .map(|component| component.dimension)
            .collect::<Vec<_>>();
        assert_eq!(
            dimensions
                .iter()
                .filter(|&&dimension| dimension == FormalBoundaryDimension::Point)
                .count(),
            1
        );
        assert_eq!(
            dimensions
                .iter()
                .filter(|&&dimension| dimension == FormalBoundaryDimension::Segment)
                .count(),
            1
        );
        assert_eq!(
            dimensions
                .iter()
                .filter(|&&dimension| dimension == FormalBoundaryDimension::Topological)
                .count(),
            2
        );
        assert!(incidence.vertices.iter().any(|vertex| vertex.isolated));
        for segment in &incidence.elementary_segments {
            assert_ne!(segment.start, segment.end);
        }
    }

    #[test]
    fn empty_ornament_preserves_ordinary_polygon() {
        let ordinary = region();
        let formal = FormalRectilinearPolygon::new(ordinary.clone(), Ornament::default()).unwrap();
        assert_eq!(formal.region(), &ordinary);
        assert_eq!(
            formal.area_twice().unwrap(),
            ordinary.twice_signed_area().unwrap()
        );
        let incidence = formal.incidence().unwrap();
        assert_eq!(incidence.formal_holes().count(), ordinary.holes.len());
        assert_eq!(
            incidence.elementary_segments.len(),
            ordinary.boundary_complexity()
        );
    }

    #[test]
    fn formal_interior_predicates_exclude_points_and_segments() {
        let polygon = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![Point::new(6, 6)],
                segments: vec![
                    OrnamentSegment::new(Point::new(8, 10), Point::new(12, 10)).unwrap(),
                ],
            },
        )
        .unwrap();
        assert!(!polygon.contains_doubled_point_strict(crate::DoubledPoint::new(12, 12)));
        assert!(!polygon.contains_doubled_point_strict(crate::DoubledPoint::new(20, 20)));
        assert!(polygon.contains_doubled_point_strict(crate::DoubledPoint::new(14, 14)));
        assert!(!polygon.contains_open_horizontal_segment(5, 7, 12));
        assert!(!polygon.contains_open_vertical_segment(20, 8, 12));
        assert!(polygon.contains_open_horizontal_segment(5, 7, 14));
    }

    #[test]
    fn rejects_every_source_ornament_violation() {
        let outside_point = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![Point::new(30, 30)],
                segments: vec![],
            },
        );
        assert!(matches!(
            outside_point,
            Err(FormalPolygonError::IsolatedPointOutsideInterior { .. })
        ));

        let crosses_hole = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![],
                segments: vec![OrnamentSegment::new(Point::new(1, 3), Point::new(5, 3)).unwrap()],
            },
        );
        assert!(matches!(
            crosses_hole,
            Err(FormalPolygonError::OrnamentInteriorOutsideRegion { .. })
        ));

        let crossing = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![],
                segments: vec![
                    OrnamentSegment::new(Point::new(6, 10), Point::new(14, 10)).unwrap(),
                    OrnamentSegment::new(Point::new(10, 6), Point::new(10, 14)).unwrap(),
                ],
            },
        );
        assert!(matches!(
            crossing,
            Err(FormalPolygonError::NonVertexOrnamentIntersection { .. })
        ));

        let point_on_segment = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![Point::new(10, 10)],
                segments: vec![
                    OrnamentSegment::new(Point::new(8, 10), Point::new(12, 10)).unwrap(),
                ],
            },
        );
        assert!(matches!(
            point_on_segment,
            Err(FormalPolygonError::IsolatedPointOnSegment { .. })
        ));
    }

    #[test]
    fn reports_canonicalization_and_intersection_errors_structurally() {
        let zero = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![],
                segments: vec![OrnamentSegment {
                    start: Point::new(6, 6),
                    end: Point::new(6, 6),
                }],
            },
        );
        assert!(matches!(
            zero,
            Err(FormalPolygonError::ZeroLengthOrnamentSegment { index: 0, .. })
        ));

        let diagonal = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![],
                segments: vec![OrnamentSegment {
                    start: Point::new(6, 6),
                    end: Point::new(8, 8),
                }],
            },
        );
        assert!(matches!(
            diagonal,
            Err(FormalPolygonError::NonAxisAlignedOrnamentSegment { index: 0, .. })
        ));

        let duplicate_point = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![Point::new(6, 6), Point::new(6, 6)],
                segments: vec![],
            },
        );
        assert!(matches!(
            duplicate_point,
            Err(FormalPolygonError::DuplicateIsolatedPoint { .. })
        ));

        let duplicate_segment = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![],
                segments: vec![
                    OrnamentSegment {
                        start: Point::new(6, 8),
                        end: Point::new(10, 8),
                    },
                    OrnamentSegment {
                        start: Point::new(10, 8),
                        end: Point::new(6, 8),
                    },
                ],
            },
        );
        assert!(matches!(
            duplicate_segment,
            Err(FormalPolygonError::DuplicateOrnamentSegment { .. })
        ));

        let overlap = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![],
                segments: vec![
                    OrnamentSegment::new(Point::new(6, 8), Point::new(10, 8)).unwrap(),
                    OrnamentSegment::new(Point::new(8, 8), Point::new(12, 8)).unwrap(),
                ],
            },
        );
        assert!(matches!(
            overlap,
            Err(FormalPolygonError::OverlappingOrnamentSegments { .. })
        ));

        let endpoint_outside = FormalRectilinearPolygon::new(
            region(),
            Ornament {
                isolated_points: vec![],
                segments: vec![
                    OrnamentSegment::new(Point::new(-1, 10), Point::new(6, 10)).unwrap(),
                ],
            },
        );
        assert!(matches!(
            endpoint_outside,
            Err(FormalPolygonError::OrnamentEndpointOutsideRegion { .. })
        ));
    }
}
