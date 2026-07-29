use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Coord, DoubledPoint, Point};

pub mod experiment;
pub mod oracle;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PolygonLoopId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct PolygonVertexId {
    pub loop_id: PolygonLoopId,
    pub cyclic_index: usize,
}

/// One implicitly closed normalized orthogonal boundary loop.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OrthogonalLoop {
    pub vertices: Vec<Point>,
}

impl OrthogonalLoop {
    #[must_use]
    pub const fn new(vertices: Vec<Point>) -> Self {
        Self { vertices }
    }

    /// Computes the exact signed doubled area.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonError::AreaOverflow`] if accumulation overflows `i128`.
    pub fn twice_signed_area(&self) -> Result<i128, PolygonError> {
        twice_signed_area(&self.vertices)
    }

    pub fn edges(&self) -> impl Iterator<Item = (Point, Point)> + '_ {
        (0..self.vertices.len()).map(|index| {
            (
                self.vertices[index],
                self.vertices[(index + 1) % self.vertices.len()],
            )
        })
    }
}

/// One connected ordinary rectilinear polygon with ordinary two-dimensional holes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RectilinearPolygon {
    pub outer: OrthogonalLoop,
    #[serde(default)]
    pub holes: Vec<OrthogonalLoop>,
}

/// Interchangeable exact structural validators for ordinary polygons.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Backend {
    /// The v0.9 explicit segment-pair audit.
    #[default]
    #[serde(rename = "reference-quadratic")]
    Oracle,
    /// The indexed deterministic orthogonal sweep introduced in v1.0.
    #[serde(rename = "orthogonal-sweep")]
    Experiment,
}

impl Backend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Oracle => "reference-quadratic",
            Self::Experiment => "orthogonal-sweep",
        }
    }
}

/// Exact polygon-validator interface retained by both reference and indexed paths.
pub trait Validator {
    /// Audits one already normalized polygon.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic structural or topological failure.
    fn validate(&self, polygon: &RectilinearPolygon) -> Result<(), PolygonError>;

    fn name(&self) -> &'static str;
}

impl RectilinearPolygon {
    /// Normalizes and validates an ordinary polygon.
    ///
    /// Outer orientation is canonicalized counter-clockwise and holes clockwise.
    /// Hole order and every cyclic start vertex are deterministic.
    ///
    /// # Errors
    ///
    /// Returns a structured [`PolygonError`] for malformed or unsupported input.
    pub fn new(outer: OrthogonalLoop, holes: Vec<OrthogonalLoop>) -> Result<Self, PolygonError> {
        let polygon = Self::normalize_unvalidated(outer, holes)?;
        oracle::Validator.validate(&polygon)?;
        Ok(polygon)
    }

    pub(crate) fn normalize_unvalidated(
        outer: OrthogonalLoop,
        holes: Vec<OrthogonalLoop>,
    ) -> Result<Self, PolygonError> {
        let outer = normalize_loop(outer.vertices, false, PolygonLoopId(0))?;
        let mut holes = holes
            .into_iter()
            .enumerate()
            .map(|(index, boundary_loop)| {
                normalize_loop(boundary_loop.vertices, true, PolygonLoopId(index + 1))
            })
            .collect::<Result<Vec<_>, _>>()?;
        holes.sort_by(|first, second| first.vertices.cmp(&second.vertices));
        Ok(Self { outer, holes })
    }

    /// Re-normalizes an existing value. This operation is idempotent.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonError`] under the same contract as [`Self::new`].
    pub fn normalized(&self) -> Result<Self, PolygonError> {
        Self::new(self.outer.clone(), self.holes.clone())
    }

    /// Audits the normalized ordinary-polygon contract.
    ///
    /// # Errors
    ///
    /// Returns the first deterministic structural or topological failure.
    pub fn validate(&self) -> Result<(), PolygonError> {
        validate_loop(&self.outer, false, PolygonLoopId(0))?;
        for (index, boundary_loop) in self.holes.iter().enumerate() {
            validate_loop(boundary_loop, true, PolygonLoopId(index + 1))?;
        }

        for (hole_index, hole) in self.holes.iter().enumerate() {
            if loops_intersect(&self.outer, hole) {
                return Err(PolygonError::HoleIntersectsOuter {
                    hole: PolygonLoopId(hole_index + 1),
                });
            }
            let probe = DoubledPoint::from_point(hole.vertices[0]);
            if !point_in_loop_strict(&self.outer, probe) {
                return Err(PolygonError::HoleOutsideOuter {
                    hole: PolygonLoopId(hole_index + 1),
                });
            }
        }

        for first in 0..self.holes.len() {
            for second in first + 1..self.holes.len() {
                if loops_intersect(&self.holes[first], &self.holes[second]) {
                    return Err(PolygonError::HoleIntersectsHole {
                        first: PolygonLoopId(first + 1),
                        second: PolygonLoopId(second + 1),
                    });
                }
                let first_probe = DoubledPoint::from_point(self.holes[first].vertices[0]);
                let second_probe = DoubledPoint::from_point(self.holes[second].vertices[0]);
                if point_in_loop_strict(&self.holes[first], second_probe)
                    || point_in_loop_strict(&self.holes[second], first_probe)
                {
                    return Err(PolygonError::NestedHole {
                        first: PolygonLoopId(first + 1),
                        second: PolygonLoopId(second + 1),
                    });
                }
            }
        }

        let area = self.twice_signed_area()?;
        if area <= 0 {
            return Err(PolygonError::DisconnectedInterior);
        }
        Ok(())
    }

    /// Returns the exact positive doubled area of the formal interior.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonError::AreaOverflow`] if accumulation overflows `i128`.
    pub fn twice_signed_area(&self) -> Result<i128, PolygonError> {
        self.holes
            .iter()
            .try_fold(self.outer.twice_signed_area()?, |sum, boundary_loop| {
                sum.checked_add(boundary_loop.twice_signed_area()?)
                    .ok_or(PolygonError::AreaOverflow)
            })
    }

    #[must_use]
    pub fn boundary_complexity(&self) -> usize {
        self.outer.vertices.len()
            + self
                .holes
                .iter()
                .map(|boundary_loop| boundary_loop.vertices.len())
                .sum::<usize>()
    }

    #[must_use]
    pub fn hole_vertex_count(&self) -> usize {
        self.holes
            .iter()
            .map(|boundary_loop| boundary_loop.vertices.len())
            .sum()
    }

    pub fn loops(&self) -> impl Iterator<Item = &OrthogonalLoop> {
        std::iter::once(&self.outer).chain(self.holes.iter())
    }

    /// Exact strict-interior test for a doubled-coordinate point.
    #[must_use]
    pub fn contains_doubled_point_strict(&self, point: DoubledPoint) -> bool {
        point_in_loop_strict(&self.outer, point)
            && self
                .holes
                .iter()
                .all(|boundary_loop| !point_in_loop_or_boundary(boundary_loop, point))
    }

    /// Returns true exactly when the open horizontal segment is strictly interior.
    #[must_use]
    pub fn contains_open_horizontal_segment(
        &self,
        left: Coord,
        right: Coord,
        doubled_y: i128,
    ) -> bool {
        if left >= right {
            return false;
        }
        let left_twice = 2 * i128::from(left);
        let right_twice = 2 * i128::from(right);
        if self.loops().flat_map(OrthogonalLoop::edges).any(|edge| {
            horizontal_open_segment_meets_edge(left_twice, right_twice, doubled_y, edge)
        }) {
            return false;
        }
        self.contains_doubled_point_strict(DoubledPoint::new(
            i128::from(left) + i128::from(right),
            doubled_y,
        ))
    }

    /// Returns true exactly when the open vertical segment is strictly interior.
    #[must_use]
    pub fn contains_open_vertical_segment(
        &self,
        doubled_x: i128,
        bottom: Coord,
        top: Coord,
    ) -> bool {
        if bottom >= top {
            return false;
        }
        let bottom_twice = 2 * i128::from(bottom);
        let top_twice = 2 * i128::from(top);
        if self
            .loops()
            .flat_map(OrthogonalLoop::edges)
            .any(|edge| vertical_open_segment_meets_edge(doubled_x, bottom_twice, top_twice, edge))
        {
            return false;
        }
        self.contains_doubled_point_strict(DoubledPoint::new(
            doubled_x,
            i128::from(bottom) + i128::from(top),
        ))
    }
}

/// Exact predicates required by boundary-native chord and completion algorithms.
pub trait RectilinearDomain {
    fn contains_doubled_point_strict(&self, point: DoubledPoint) -> bool;
    fn contains_open_horizontal_segment(&self, left: Coord, right: Coord, doubled_y: i128) -> bool;
    fn contains_open_vertical_segment(&self, doubled_x: i128, bottom: Coord, top: Coord) -> bool;
    /// Returns exact signed doubled area.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonError::AreaOverflow`] when exact accumulation cannot
    /// be represented in `i128`.
    fn area_twice(&self) -> Result<i128, PolygonError>;
}

impl RectilinearDomain for RectilinearPolygon {
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
        self.twice_signed_area()
    }
}

fn normalize_loop(
    mut vertices: Vec<Point>,
    is_hole: bool,
    loop_id: PolygonLoopId,
) -> Result<OrthogonalLoop, PolygonError> {
    if vertices.first() == vertices.last() && !vertices.is_empty() {
        vertices.pop();
    }
    if vertices.len() < 4 {
        return Err(PolygonError::TooFewVertices {
            loop_id,
            count: vertices.len(),
        });
    }
    validate_raw_edges(&vertices, loop_id)?;

    loop {
        let mut remove = None;
        for index in 0..vertices.len() {
            let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
            let current = vertices[index];
            let next = vertices[(index + 1) % vertices.len()];
            let incoming = (current.x - previous.x, current.y - previous.y);
            let outgoing = (next.x - current.x, next.y - current.y);
            let cross = i128::from(incoming.0) * i128::from(outgoing.1)
                - i128::from(incoming.1) * i128::from(outgoing.0);
            if cross == 0 {
                let dot = i128::from(incoming.0) * i128::from(outgoing.0)
                    + i128::from(incoming.1) * i128::from(outgoing.1);
                if dot <= 0 {
                    return Err(PolygonError::UnsupportedDegenerateBoundary {
                        loop_id,
                        vertex: current,
                    });
                }
                remove = Some(index);
                break;
            }
        }
        let Some(index) = remove else {
            break;
        };
        vertices.remove(index);
        if vertices.len() < 4 {
            return Err(PolygonError::TooFewVertices {
                loop_id,
                count: vertices.len(),
            });
        }
    }

    let mut area = twice_signed_area(&vertices)?;
    if area == 0 {
        return Err(PolygonError::UnsupportedDegenerateBoundary {
            loop_id,
            vertex: vertices[0],
        });
    }
    if (is_hole && area > 0) || (!is_hole && area < 0) {
        vertices.reverse();
        area = -area;
    }
    debug_assert!((is_hole && area < 0) || (!is_hole && area > 0));
    let canonical = vertices
        .iter()
        .enumerate()
        .min_by_key(|(_, point)| **point)
        .map(|(index, _)| index)
        .ok_or(PolygonError::TooFewVertices { loop_id, count: 0 })?;
    vertices.rotate_left(canonical);
    Ok(OrthogonalLoop { vertices })
}

fn validate_raw_edges(vertices: &[Point], loop_id: PolygonLoopId) -> Result<(), PolygonError> {
    for index in 0..vertices.len() {
        let first = vertices[index];
        let second = vertices[(index + 1) % vertices.len()];
        if first == second {
            return Err(PolygonError::ZeroLengthEdge { loop_id, index });
        }
        if first.x != second.x && first.y != second.y {
            return Err(PolygonError::NonAxisAlignedEdge { loop_id, index });
        }
    }
    Ok(())
}

fn validate_loop(
    boundary_loop: &OrthogonalLoop,
    is_hole: bool,
    loop_id: PolygonLoopId,
) -> Result<(), PolygonError> {
    if boundary_loop.vertices.len() < 4 {
        return Err(PolygonError::TooFewVertices {
            loop_id,
            count: boundary_loop.vertices.len(),
        });
    }
    validate_raw_edges(&boundary_loop.vertices, loop_id)?;
    for index in 0..boundary_loop.vertices.len() {
        let previous = boundary_loop.vertices
            [(index + boundary_loop.vertices.len() - 1) % boundary_loop.vertices.len()];
        let current = boundary_loop.vertices[index];
        let next = boundary_loop.vertices[(index + 1) % boundary_loop.vertices.len()];
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
    }
    let area = boundary_loop.twice_signed_area()?;
    if (is_hole && area >= 0) || (!is_hole && area <= 0) {
        return Err(PolygonError::WrongOrientation { loop_id, area });
    }

    let mut vertices = HashMap::new();
    for (index, &point) in boundary_loop.vertices.iter().enumerate() {
        if let Some(first) = vertices.insert(point, index) {
            return Err(PolygonError::DuplicateVertex {
                loop_id,
                first,
                second: index,
                point,
            });
        }
    }
    let mut edges = HashMap::new();
    let segments = boundary_loop.edges().collect::<Vec<_>>();
    for (index, &(first, second)) in segments.iter().enumerate() {
        let key = if first < second {
            (first, second)
        } else {
            (second, first)
        };
        if let Some(previous) = edges.insert(key, index) {
            return Err(PolygonError::DuplicateEdge {
                loop_id,
                first: previous,
                second: index,
            });
        }
    }
    for first in 0..segments.len() {
        for second in first + 1..segments.len() {
            if second == first + 1 || (first == 0 && second + 1 == segments.len()) {
                continue;
            }
            if let Some(kind) = segment_intersection_kind(segments[first], segments[second]) {
                return Err(match kind {
                    SegmentIntersectionKind::Proper => PolygonError::SelfIntersection {
                        loop_id,
                        first_edge: first,
                        second_edge: second,
                    },
                    SegmentIntersectionKind::Touch | SegmentIntersectionKind::Overlap => {
                        PolygonError::NonAdjacentBoundaryTouch {
                            loop_id,
                            first_edge: first,
                            second_edge: second,
                        }
                    }
                });
            }
        }
    }
    Ok(())
}

fn twice_signed_area(vertices: &[Point]) -> Result<i128, PolygonError> {
    let mut area = 0_i128;
    for index in 0..vertices.len() {
        let first = vertices[index];
        let second = vertices[(index + 1) % vertices.len()];
        let term = i128::from(first.x)
            .checked_mul(i128::from(second.y))
            .and_then(|value| {
                i128::from(second.x)
                    .checked_mul(i128::from(first.y))
                    .and_then(|other| value.checked_sub(other))
            })
            .ok_or(PolygonError::AreaOverflow)?;
        area = area.checked_add(term).ok_or(PolygonError::AreaOverflow)?;
    }
    Ok(area)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SegmentIntersectionKind {
    Proper,
    Touch,
    Overlap,
}

fn segment_intersection_kind(
    first: (Point, Point),
    second: (Point, Point),
) -> Option<SegmentIntersectionKind> {
    let (a, b) = ordered_segment(first);
    let (c, d) = ordered_segment(second);
    let first_horizontal = a.y == b.y;
    let second_horizontal = c.y == d.y;
    if first_horizontal == second_horizontal {
        let same_line = if first_horizontal {
            a.y == c.y
        } else {
            a.x == c.x
        };
        if !same_line {
            return None;
        }
        let (a0, a1, c0, c1) = if first_horizontal {
            (a.x, b.x, c.x, d.x)
        } else {
            (a.y, b.y, c.y, d.y)
        };
        let start = a0.max(c0);
        let end = a1.min(c1);
        return (start < end)
            .then_some(SegmentIntersectionKind::Overlap)
            .or_else(|| (start == end).then_some(SegmentIntersectionKind::Touch));
    }
    let (horizontal, vertical) = if first_horizontal {
        ((a, b), (c, d))
    } else {
        ((c, d), (a, b))
    };
    let intersects = horizontal.0.x <= vertical.0.x
        && vertical.0.x <= horizontal.1.x
        && vertical.0.y <= horizontal.0.y
        && horizontal.0.y <= vertical.1.y;
    if !intersects {
        return None;
    }
    let point = Point::new(vertical.0.x, horizontal.0.y);
    let endpoint = point == horizontal.0
        || point == horizontal.1
        || point == vertical.0
        || point == vertical.1;
    Some(if endpoint {
        SegmentIntersectionKind::Touch
    } else {
        SegmentIntersectionKind::Proper
    })
}

fn ordered_segment(segment: (Point, Point)) -> (Point, Point) {
    if segment.0 <= segment.1 {
        segment
    } else {
        (segment.1, segment.0)
    }
}

fn loops_intersect(first: &OrthogonalLoop, second: &OrthogonalLoop) -> bool {
    first.edges().any(|first_edge| {
        second
            .edges()
            .any(|second_edge| segment_intersection_kind(first_edge, second_edge).is_some())
    })
}

fn point_on_segment_doubled(point: DoubledPoint, edge: (Point, Point)) -> bool {
    let (first, second) = ordered_segment(edge);
    if first.y == second.y {
        point.y == 2 * i128::from(first.y)
            && 2 * i128::from(first.x) <= point.x
            && point.x <= 2 * i128::from(second.x)
    } else {
        point.x == 2 * i128::from(first.x)
            && 2 * i128::from(first.y) <= point.y
            && point.y <= 2 * i128::from(second.y)
    }
}

fn point_in_loop_or_boundary(boundary_loop: &OrthogonalLoop, point: DoubledPoint) -> bool {
    boundary_loop
        .edges()
        .any(|edge| point_on_segment_doubled(point, edge))
        || point_in_loop_strict(boundary_loop, point)
}

fn point_in_loop_strict(boundary_loop: &OrthogonalLoop, point: DoubledPoint) -> bool {
    if boundary_loop
        .edges()
        .any(|edge| point_on_segment_doubled(point, edge))
    {
        return false;
    }
    let mut inside = false;
    for (first, second) in boundary_loop.edges() {
        if first.x != second.x {
            continue;
        }
        let low = 2 * i128::from(first.y.min(second.y));
        let high = 2 * i128::from(first.y.max(second.y));
        if low <= point.y && point.y < high && 2 * i128::from(first.x) > point.x {
            inside = !inside;
        }
    }
    inside
}

fn horizontal_open_segment_meets_edge(
    left_twice: i128,
    right_twice: i128,
    doubled_y: i128,
    edge: (Point, Point),
) -> bool {
    let (first, second) = ordered_segment(edge);
    if first.y == second.y {
        if doubled_y != 2 * i128::from(first.y) {
            return false;
        }
        let start = left_twice.max(2 * i128::from(first.x));
        let end = right_twice.min(2 * i128::from(second.x));
        start < end && start < right_twice && end > left_twice
    } else {
        let x = 2 * i128::from(first.x);
        left_twice < x
            && x < right_twice
            && 2 * i128::from(first.y) <= doubled_y
            && doubled_y <= 2 * i128::from(second.y)
    }
}

fn vertical_open_segment_meets_edge(
    doubled_x: i128,
    bottom_twice: i128,
    top_twice: i128,
    edge: (Point, Point),
) -> bool {
    let (first, second) = ordered_segment(edge);
    if first.x == second.x {
        if doubled_x != 2 * i128::from(first.x) {
            return false;
        }
        let start = bottom_twice.max(2 * i128::from(first.y));
        let end = top_twice.min(2 * i128::from(second.y));
        start < end && start < top_twice && end > bottom_twice
    } else {
        let y = 2 * i128::from(first.y);
        bottom_twice < y
            && y < top_twice
            && 2 * i128::from(first.x) <= doubled_x
            && doubled_x <= 2 * i128::from(second.x)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq, Serialize, Deserialize)]
pub enum PolygonError {
    #[error("loop {loop_id:?} edge {index} is not axis aligned")]
    NonAxisAlignedEdge {
        loop_id: PolygonLoopId,
        index: usize,
    },
    #[error("loop {loop_id:?} edge {index} has zero length")]
    ZeroLengthEdge {
        loop_id: PolygonLoopId,
        index: usize,
    },
    #[error(
        "loop {loop_id:?} has {count} vertices after normalization; at least four are required"
    )]
    TooFewVertices {
        loop_id: PolygonLoopId,
        count: usize,
    },
    #[error("loop {loop_id:?} edges {first_edge} and {second_edge} intersect properly")]
    SelfIntersection {
        loop_id: PolygonLoopId,
        first_edge: usize,
        second_edge: usize,
    },
    #[error("loop {loop_id:?} nonadjacent edges {first_edge} and {second_edge} touch or overlap")]
    NonAdjacentBoundaryTouch {
        loop_id: PolygonLoopId,
        first_edge: usize,
        second_edge: usize,
    },
    #[error("loop {loop_id:?} repeats vertex {point:?} at {first} and {second}")]
    DuplicateVertex {
        loop_id: PolygonLoopId,
        first: usize,
        second: usize,
        point: Point,
    },
    #[error("loop {loop_id:?} repeats edges {first} and {second}")]
    DuplicateEdge {
        loop_id: PolygonLoopId,
        first: usize,
        second: usize,
    },
    #[error("hole {hole:?} is not strictly inside the outer loop")]
    HoleOutsideOuter { hole: PolygonLoopId },
    #[error("hole {hole:?} intersects or touches the outer loop")]
    HoleIntersectsOuter { hole: PolygonLoopId },
    #[error("holes {first:?} and {second:?} intersect or touch")]
    HoleIntersectsHole {
        first: PolygonLoopId,
        second: PolygonLoopId,
    },
    #[error("holes {first:?} and {second:?} are nested")]
    NestedHole {
        first: PolygonLoopId,
        second: PolygonLoopId,
    },
    #[error("loop {loop_id:?} has wrong normalized orientation with doubled area {area}")]
    WrongOrientation { loop_id: PolygonLoopId, area: i128 },
    #[error("exact signed-area arithmetic overflowed i128")]
    AreaOverflow,
    #[error("the accepted ordinary polygon model requires one connected formal interior")]
    DisconnectedInterior,
    #[error("loop {loop_id:?} has an unsupported degenerate boundary at {vertex:?}")]
    UnsupportedDegenerateBoundary {
        loop_id: PolygonLoopId,
        vertex: Point,
    },
}

#[cfg(test)]
mod tests {
    use super::{OrthogonalLoop, PolygonError, RectilinearPolygon};
    use crate::{DoubledPoint, Point};

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    #[test]
    fn normalization_is_canonical_and_idempotent() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(10, 10),
                Point::new(10, 0),
                Point::new(5, 0),
                Point::new(0, 0),
                Point::new(0, 10),
                Point::new(10, 10),
            ]),
            vec![OrthogonalLoop::new(vec![
                Point::new(2, 2),
                Point::new(2, 4),
                Point::new(4, 4),
                Point::new(4, 2),
            ])],
        )
        .unwrap();
        assert_eq!(polygon.outer.vertices[0], Point::new(0, 0));
        assert!(polygon.outer.twice_signed_area().unwrap() > 0);
        assert!(polygon.holes[0].twice_signed_area().unwrap() < 0);
        assert_eq!(polygon.normalized().unwrap(), polygon);
    }

    #[test]
    fn exact_interior_predicates_exclude_holes_and_boundary() {
        let polygon = RectilinearPolygon::new(
            rectangle(0, 0, 1_000_000_000, 20),
            vec![rectangle(4, 4, 8, 8)],
        )
        .unwrap();
        assert!(polygon.contains_doubled_point_strict(DoubledPoint::new(3, 3)));
        assert!(!polygon.contains_doubled_point_strict(DoubledPoint::new(10, 10)));
        assert!(!polygon.contains_doubled_point_strict(DoubledPoint::new(0, 1)));
        assert!(polygon.contains_open_horizontal_segment(1, 3, 2));
        assert!(!polygon.contains_open_horizontal_segment(1, 9, 10));
    }

    #[test]
    fn rejects_boundary_touches_and_nested_holes() {
        let touching =
            RectilinearPolygon::new(rectangle(0, 0, 10, 10), vec![rectangle(0, 2, 4, 4)]);
        assert!(matches!(
            touching,
            Err(PolygonError::HoleIntersectsOuter { .. })
        ));

        let nested = RectilinearPolygon::new(
            rectangle(0, 0, 20, 20),
            vec![rectangle(2, 2, 12, 12), rectangle(4, 4, 6, 6)],
        );
        assert!(matches!(nested, Err(PolygonError::NestedHole { .. })));
    }

    #[test]
    fn rejects_non_axis_aligned_and_self_intersecting_loops() {
        let diagonal = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(2, 1),
                Point::new(2, 2),
                Point::new(0, 2),
            ]),
            vec![],
        );
        assert!(matches!(
            diagonal,
            Err(PolygonError::NonAxisAlignedEdge { .. })
        ));

        let crossing = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 4),
                Point::new(1, 4),
                Point::new(1, -1),
                Point::new(0, -1),
            ]),
            vec![],
        );
        assert!(matches!(
            crossing,
            Err(PolygonError::SelfIntersection { .. })
        ));
    }

    #[test]
    fn rejects_zero_length_and_too_few_vertices() {
        let zero = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(4, 4),
                Point::new(0, 4),
                Point::new(0, 4),
            ]),
            vec![],
        );
        assert!(matches!(zero, Err(PolygonError::ZeroLengthEdge { .. })));

        let too_few = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![Point::new(0, 0), Point::new(4, 0), Point::new(0, 4)]),
            vec![],
        );
        assert!(matches!(too_few, Err(PolygonError::TooFewVertices { .. })));
    }

    #[test]
    fn rejects_invalid_hole_placement_and_orientation() {
        let outside =
            RectilinearPolygon::new(rectangle(0, 0, 10, 10), vec![rectangle(12, 2, 14, 4)]);
        assert!(matches!(
            outside,
            Err(PolygonError::HoleOutsideOuter { .. })
        ));

        let intersecting = RectilinearPolygon::new(
            rectangle(0, 0, 20, 20),
            vec![rectangle(2, 2, 8, 8), rectangle(6, 6, 12, 12)],
        );
        assert!(matches!(
            intersecting,
            Err(PolygonError::HoleIntersectsHole { .. })
        ));

        let clockwise = rectangle(0, 0, 10, 10);
        let mut reversed = clockwise.vertices;
        reversed.reverse();
        let wrong_orientation = RectilinearPolygon {
            outer: OrthogonalLoop::new(reversed),
            holes: vec![],
        };
        assert!(matches!(
            wrong_orientation.validate(),
            Err(PolygonError::WrongOrientation { .. })
        ));
    }

    #[test]
    fn rejects_unsupported_degenerate_and_duplicate_boundaries() {
        let degenerate = RectilinearPolygon {
            outer: OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(4, 0),
                Point::new(6, 0),
                Point::new(6, 4),
                Point::new(0, 4),
            ]),
            holes: vec![],
        };
        assert!(matches!(
            degenerate.validate(),
            Err(PolygonError::UnsupportedDegenerateBoundary { .. })
        ));

        let duplicate = RectilinearPolygon {
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
        };
        let duplicate_error = duplicate.validate().unwrap_err();
        assert!(
            matches!(duplicate_error, PolygonError::DuplicateVertex { .. }),
            "unexpected error: {duplicate_error:?}"
        );
    }
}
