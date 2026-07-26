//! Exact boundary-native reference algorithms for ordinary rectilinear polygons.

use std::collections::{BTreeSet, VecDeque};

use rect_core::{
    Boundary, BoundaryIndex, BoundaryIndexError, BoundaryVertexId, CoordinateRect, DoubledPoint,
    GeometryError, HorizontalChord, HorizontalChordId, Point, PolygonError, RectilinearPolygon,
    VerticalChord, VerticalChordId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::EffectiveChordFamilies;
use crate::{
    ChordRef, CleanHoleFreeCertificate, CleanRejectionReason, EffectiveChordEndpointIndex,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct GeneralPolygonPairwiseEnumerator;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HorizontalCutSegment {
    pub left: i64,
    pub right: i64,
    pub y: i64,
}

impl HorizontalCutSegment {
    fn new(left: i64, right: i64, y: i64) -> Result<Self, PolygonSgError> {
        if left >= right {
            return Err(PolygonSgError::InvalidSimpleChord {
                start: Point::new(left, y),
            });
        }
        Ok(Self { left, right, y })
    }

    fn from_chord(chord: HorizontalChord) -> Self {
        Self {
            left: chord.left(),
            right: chord.right(),
            y: chord.y(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VerticalCutSegment {
    pub x: i64,
    pub bottom: i64,
    pub top: i64,
}

impl VerticalCutSegment {
    fn new(x: i64, bottom: i64, top: i64) -> Result<Self, PolygonSgError> {
        if bottom >= top {
            return Err(PolygonSgError::InvalidSimpleChord {
                start: Point::new(x, bottom),
            });
        }
        Ok(Self { x, bottom, top })
    }

    fn from_chord(chord: VerticalChord) -> Self {
        Self {
            x: chord.x(),
            bottom: chord.bottom(),
            top: chord.top(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonCompletionMetrics {
    pub horizontal_candidate_queries: usize,
    pub vertical_candidate_queries: usize,
    pub horizontal_simple_chord_count: usize,
    pub vertical_simple_chord_count: usize,
    pub coordinate_compression_x_count: usize,
    pub coordinate_compression_y_count: usize,
    pub atomic_cell_count: usize,
    pub rectangle_recovery_visits: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonCompletionResult {
    pub rectangles: Vec<CoordinateRect>,
    pub selected_horizontal_cuts: Vec<HorizontalCutSegment>,
    pub selected_vertical_cuts: Vec<VerticalCutSegment>,
    pub added_horizontal_cuts: Vec<HorizontalCutSegment>,
    pub added_vertical_cuts: Vec<VerticalCutSegment>,
    pub metrics: PolygonCompletionMetrics,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CoordinateCompressedCompletion;

impl CoordinateCompressedCompletion {
    /// Completes a selected admissible effective-chord family into rectangles.
    ///
    /// The reference policy inserts selected chords, then horizontal simple
    /// chords, then vertical simple chords. Rectangle recovery is sensitive to
    /// the coordinate arrangement, not to coordinate magnitude.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid selections, incomplete rays, or
    /// nonrectangular recovered regions.
    pub fn complete(
        &self,
        polygon: &RectilinearPolygon,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<PolygonCompletionResult, PolygonSgError> {
        if horizontal_chords.len() != selected_horizontal.len()
            || vertical_chords.len() != selected_vertical.len()
        {
            return Err(PolygonSgError::SelectionLengthMismatch);
        }
        let polygon = polygon.normalized()?;
        let selected_horizontal_cuts = horizontal_chords
            .iter()
            .zip(selected_horizontal)
            .filter_map(|(&chord, &selected)| {
                selected.then_some(HorizontalCutSegment::from_chord(chord))
            })
            .collect::<Vec<_>>();
        let selected_vertical_cuts = vertical_chords
            .iter()
            .zip(selected_vertical)
            .filter_map(|(&chord, &selected)| {
                selected.then_some(VerticalCutSegment::from_chord(chord))
            })
            .collect::<Vec<_>>();
        let mut horizontal_cuts = selected_horizontal_cuts
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut vertical_cuts = selected_vertical_cuts
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut added_horizontal_cuts = Vec::new();
        let mut added_vertical_cuts = Vec::new();
        let mut metrics = PolygonCompletionMetrics::default();

        complete_polygon_axis(
            &polygon,
            &mut horizontal_cuts,
            &mut vertical_cuts,
            true,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;
        complete_polygon_axis(
            &polygon,
            &mut horizontal_cuts,
            &mut vertical_cuts,
            false,
            &mut added_horizontal_cuts,
            &mut added_vertical_cuts,
            &mut metrics,
        )?;

        added_horizontal_cuts.sort_unstable();
        added_vertical_cuts.sort_unstable();

        let recovery = recover_coordinate_rectangles(&polygon, &horizontal_cuts, &vertical_cuts)?;
        metrics.coordinate_compression_x_count = recovery.x_count;
        metrics.coordinate_compression_y_count = recovery.y_count;
        metrics.atomic_cell_count = recovery.atomic_cell_count;
        metrics.rectangle_recovery_visits = recovery.visits;
        validate_polygon_dissection(&polygon, &recovery.rectangles)?;
        Ok(PolygonCompletionResult {
            rectangles: recovery.rectangles,
            selected_horizontal_cuts,
            selected_vertical_cuts,
            added_horizontal_cuts,
            added_vertical_cuts,
            metrics,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "coordinate-compressed"
    }
}

impl GeneralPolygonPairwiseEnumerator {
    /// Enumerates every Definition 7 effective chord for an ordinary polygon.
    ///
    /// This is an exact `O(r^2 n)` reference implementation, not the general
    /// Soltan--Gorpinevich sweep-line algorithm.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] when normalization, boundary indexing, or
    /// exact chord construction fails.
    pub fn enumerate(
        &self,
        polygon: &RectilinearPolygon,
    ) -> Result<EffectiveChordFamilies, PolygonSgError> {
        let polygon = polygon.normalized()?;
        let boundary = Boundary::from_polygon(&polygon);
        let boundary_index = BoundaryIndex::new(&boundary)?;
        let points = boundary
            .reflex_vertices
            .iter()
            .map(|vertex| vertex.point)
            .collect::<Vec<_>>();
        let mut horizontal = BTreeSet::new();
        let mut vertical = BTreeSet::new();

        for first_index in 0..points.len() {
            for second_index in first_index + 1..points.len() {
                let first = points[first_index];
                let second = points[second_index];
                if first.y == second.y {
                    let left = first.x.min(second.x);
                    let right = first.x.max(second.x);
                    if endpoint_has_collinear_edge(
                        &boundary,
                        &boundary_index,
                        Point::new(left, first.y),
                        true,
                    )? && endpoint_has_collinear_edge(
                        &boundary,
                        &boundary_index,
                        Point::new(right, first.y),
                        true,
                    )? && horizontal_satisfies_definition_7(
                        &polygon,
                        &boundary,
                        &boundary_index,
                        left,
                        right,
                        first.y,
                    )? {
                        horizontal.insert((first.y, left, right));
                    }
                }
                if first.x == second.x {
                    let bottom = first.y.min(second.y);
                    let top = first.y.max(second.y);
                    if endpoint_has_collinear_edge(
                        &boundary,
                        &boundary_index,
                        Point::new(first.x, bottom),
                        false,
                    )? && endpoint_has_collinear_edge(
                        &boundary,
                        &boundary_index,
                        Point::new(first.x, top),
                        false,
                    )? && vertical_satisfies_definition_7(
                        &polygon,
                        &boundary,
                        &boundary_index,
                        first.x,
                        bottom,
                        top,
                    )? {
                        vertical.insert((first.x, bottom, top));
                    }
                }
            }
        }

        Ok(EffectiveChordFamilies {
            horizontal: horizontal
                .into_iter()
                .enumerate()
                .map(|(index, (y, left, right))| {
                    HorizontalChord::new(HorizontalChordId(index), left, right, y)
                })
                .collect::<Result<Vec<_>, _>>()?,
            vertical: vertical
                .into_iter()
                .enumerate()
                .map(|(index, (x, bottom, top))| {
                    VerticalChord::new(VerticalChordId(index), x, bottom, top)
                })
                .collect::<Result<Vec<_>, _>>()?,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(points.len().saturating_sub(1) * points.len() / 2),
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "general-polygon-pairwise"
    }
}

/// Classifies a boundary-native polygon for the clean hole-free path-tree
/// representation without consulting grid occupancy.
#[must_use]
pub fn classify_clean_polygon(
    polygon: &RectilinearPolygon,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    endpoint_index: &EffectiveChordEndpointIndex,
) -> CleanHoleFreeCertificate {
    let mut rejection_reasons = Vec::new();
    let outer_loop_count = boundary.outer_loop_count();
    let hole_count = boundary.hole_count();
    if outer_loop_count != 1 {
        rejection_reasons.push(CleanRejectionReason::MultipleOuterLoops {
            count: outer_loop_count,
        });
    }
    if hole_count != 0 {
        rejection_reasons.push(CleanRejectionReason::HasHole { count: hole_count });
    }
    let mut endpoint_owners = std::collections::HashMap::<BoundaryVertexId, Vec<ChordRef>>::new();
    let mut all_chords_proper = true;
    for (index, &chord) in horizontal_chords.iter().enumerate() {
        let proper = polygon.contains_open_horizontal_segment(
            chord.left(),
            chord.right(),
            2 * i128::from(chord.y()),
        );
        all_chords_proper &= proper;
        if !proper {
            rejection_reasons.push(CleanRejectionReason::NonProperHorizontalChord(chord.id()));
        }
        if let Some(endpoints) = endpoint_index.horizontal.get(index) {
            endpoint_owners
                .entry(endpoints.first)
                .or_default()
                .push(ChordRef::Horizontal(chord.id()));
            endpoint_owners
                .entry(endpoints.second)
                .or_default()
                .push(ChordRef::Horizontal(chord.id()));
        } else {
            all_chords_proper = false;
            rejection_reasons.push(CleanRejectionReason::EndpointNotOnBoundary);
        }
    }
    for (index, &chord) in vertical_chords.iter().enumerate() {
        let proper = polygon.contains_open_vertical_segment(
            2 * i128::from(chord.x()),
            chord.bottom(),
            chord.top(),
        );
        all_chords_proper &= proper;
        if !proper {
            rejection_reasons.push(CleanRejectionReason::NonProperVerticalChord(chord.id()));
        }
        if let Some(endpoints) = endpoint_index.vertical.get(index) {
            endpoint_owners
                .entry(endpoints.first)
                .or_default()
                .push(ChordRef::Vertical(chord.id()));
            endpoint_owners
                .entry(endpoints.second)
                .or_default()
                .push(ChordRef::Vertical(chord.id()));
        } else {
            all_chords_proper = false;
            rejection_reasons.push(CleanRejectionReason::EndpointNotOnBoundary);
        }
    }
    let distinct_boundary_endpoints = endpoint_owners.values().all(|owners| owners.len() <= 1);
    if !distinct_boundary_endpoints {
        for (endpoint, owners) in endpoint_owners {
            for first in 0..owners.len() {
                for second in first + 1..owners.len() {
                    rejection_reasons.push(CleanRejectionReason::SharedBoundaryEndpoint {
                        first: owners[first],
                        second: owners[second],
                        endpoint,
                    });
                }
            }
        }
    }
    CleanHoleFreeCertificate {
        eligible: rejection_reasons.is_empty(),
        outer_loop_count,
        hole_count,
        all_chords_proper,
        distinct_boundary_endpoints,
        rejection_reasons,
    }
}

fn endpoint_has_collinear_edge(
    boundary: &Boundary,
    boundary_index: &BoundaryIndex,
    point: Point,
    horizontal: bool,
) -> Result<bool, PolygonSgError> {
    let id = boundary_index
        .vertex_id(point)
        .ok_or(PolygonSgError::EndpointNotOnBoundary { point })?;
    let (previous, current, next) = incident_vertices(boundary, id)?;
    Ok(if horizontal {
        previous.y == current.y || next.y == current.y
    } else {
        previous.x == current.x || next.x == current.x
    })
}

fn horizontal_satisfies_definition_7(
    polygon: &RectilinearPolygon,
    boundary: &Boundary,
    boundary_index: &BoundaryIndex,
    left: i64,
    right: i64,
    y: i64,
) -> Result<bool, PolygonSgError> {
    let mut breaks = BTreeSet::from([2 * i128::from(left), 2 * i128::from(right)]);
    for boundary_loop in &boundary.loops {
        for index in 0..boundary_loop.vertices.len() {
            let first = boundary_loop.vertices[index];
            let second = boundary_loop.vertices[(index + 1) % boundary_loop.vertices.len()];
            if first.y == second.y {
                if first.y == y
                    && left.max(first.x.min(second.x)) < right.min(first.x.max(second.x))
                {
                    return Ok(false);
                }
                continue;
            }
            let edge_bottom = first.y.min(second.y);
            let edge_top = first.y.max(second.y);
            if left < first.x && first.x < right && edge_bottom <= y && y <= edge_top {
                let point = Point::new(first.x, y);
                let Some(vertex_id) = boundary_index.vertex_id(point) else {
                    return Ok(false);
                };
                if orthogonal_incident_edge_count(boundary, vertex_id, true)? != 1 {
                    return Ok(false);
                }
                breaks.insert(2 * i128::from(first.x));
            }
        }
    }
    all_horizontal_subintervals_are_interior(polygon, &breaks, y)
}

fn vertical_satisfies_definition_7(
    polygon: &RectilinearPolygon,
    boundary: &Boundary,
    boundary_index: &BoundaryIndex,
    x: i64,
    bottom: i64,
    top: i64,
) -> Result<bool, PolygonSgError> {
    let mut breaks = BTreeSet::from([2 * i128::from(bottom), 2 * i128::from(top)]);
    for boundary_loop in &boundary.loops {
        for index in 0..boundary_loop.vertices.len() {
            let first = boundary_loop.vertices[index];
            let second = boundary_loop.vertices[(index + 1) % boundary_loop.vertices.len()];
            if first.x == second.x {
                if first.x == x
                    && bottom.max(first.y.min(second.y)) < top.min(first.y.max(second.y))
                {
                    return Ok(false);
                }
                continue;
            }
            let edge_left = first.x.min(second.x);
            let edge_right = first.x.max(second.x);
            if bottom < first.y && first.y < top && edge_left <= x && x <= edge_right {
                let point = Point::new(x, first.y);
                let Some(vertex_id) = boundary_index.vertex_id(point) else {
                    return Ok(false);
                };
                if orthogonal_incident_edge_count(boundary, vertex_id, false)? != 1 {
                    return Ok(false);
                }
                breaks.insert(2 * i128::from(first.y));
            }
        }
    }
    all_vertical_subintervals_are_interior(polygon, &breaks, x)
}

fn all_horizontal_subintervals_are_interior(
    polygon: &RectilinearPolygon,
    breaks: &BTreeSet<i128>,
    y: i64,
) -> Result<bool, PolygonSgError> {
    let coordinates = breaks.iter().copied().collect::<Vec<_>>();
    for pair in coordinates.windows(2) {
        let doubled_x = pair[0]
            .checked_add(pair[1])
            .and_then(|sum| sum.checked_div(2))
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        if !polygon.contains_doubled_point_strict(DoubledPoint::new(doubled_x, 2 * i128::from(y))) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn all_vertical_subintervals_are_interior(
    polygon: &RectilinearPolygon,
    breaks: &BTreeSet<i128>,
    x: i64,
) -> Result<bool, PolygonSgError> {
    let coordinates = breaks.iter().copied().collect::<Vec<_>>();
    for pair in coordinates.windows(2) {
        let doubled_y = pair[0]
            .checked_add(pair[1])
            .and_then(|sum| sum.checked_div(2))
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        if !polygon.contains_doubled_point_strict(DoubledPoint::new(2 * i128::from(x), doubled_y)) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn orthogonal_incident_edge_count(
    boundary: &Boundary,
    vertex_id: BoundaryVertexId,
    horizontal_chord: bool,
) -> Result<usize, PolygonSgError> {
    let (previous, current, next) = incident_vertices(boundary, vertex_id)?;
    Ok([previous, next]
        .into_iter()
        .filter(|neighbor| {
            if horizontal_chord {
                neighbor.x == current.x
            } else {
                neighbor.y == current.y
            }
        })
        .count())
}

fn incident_vertices(
    boundary: &Boundary,
    vertex_id: BoundaryVertexId,
) -> Result<(Point, Point, Point), PolygonSgError> {
    let boundary_loop = boundary
        .loops
        .get(vertex_id.loop_id.0)
        .ok_or(PolygonSgError::InvalidBoundaryVertexId(vertex_id))?;
    let len = boundary_loop.vertices.len();
    let current = boundary_loop
        .vertices
        .get(vertex_id.cyclic_index)
        .copied()
        .ok_or(PolygonSgError::InvalidBoundaryVertexId(vertex_id))?;
    Ok((
        boundary_loop.vertices[(vertex_id.cyclic_index + len - 1) % len],
        current,
        boundary_loop.vertices[(vertex_id.cyclic_index + 1) % len],
    ))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PolygonDirection {
    East,
    North,
    West,
    South,
}

impl PolygonDirection {
    const fn is_horizontal(self) -> bool {
        matches!(self, Self::East | Self::West)
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_polygon_axis(
    polygon: &RectilinearPolygon,
    horizontal_cuts: &mut BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &mut BTreeSet<VerticalCutSegment>,
    horizontal: bool,
    added_horizontal: &mut Vec<HorizontalCutSegment>,
    added_vertical: &mut Vec<VerticalCutSegment>,
    metrics: &mut PolygonCompletionMetrics,
) -> Result<(), PolygonSgError> {
    let coordinate_bound = polygon
        .boundary_complexity()
        .checked_add(horizontal_cuts.len().saturating_mul(2))
        .and_then(|value| value.checked_add(vertical_cuts.len().saturating_mul(2)))
        .and_then(|value| value.checked_mul(value))
        .and_then(|value| value.checked_mul(4))
        .ok_or(PolygonSgError::CoordinateOverflow)?;
    for _ in 0..=coordinate_bound {
        let Some((point, direction)) =
            find_polygon_concave_ray(polygon, horizontal_cuts, vertical_cuts, horizontal, metrics)
        else {
            return Ok(());
        };
        let stop = find_polygon_ray_stop(polygon, horizontal_cuts, vertical_cuts, point, direction)
            .ok_or(PolygonSgError::UnboundedSimpleChord { start: point })?;
        match direction {
            PolygonDirection::East | PolygonDirection::West => {
                let segment =
                    HorizontalCutSegment::new(point.x.min(stop.x), point.x.max(stop.x), point.y)?;
                if !horizontal_cuts.insert(segment) {
                    return Err(PolygonSgError::InvalidSimpleChord { start: point });
                }
                added_horizontal.push(segment);
                metrics.horizontal_simple_chord_count += 1;
            }
            PolygonDirection::North | PolygonDirection::South => {
                let segment =
                    VerticalCutSegment::new(point.x, point.y.min(stop.y), point.y.max(stop.y))?;
                if !vertical_cuts.insert(segment) {
                    return Err(PolygonSgError::InvalidSimpleChord { start: point });
                }
                added_vertical.push(segment);
                metrics.vertical_simple_chord_count += 1;
            }
        }
    }
    Err(PolygonSgError::CompletionDidNotTerminate)
}

fn find_polygon_concave_ray(
    polygon: &RectilinearPolygon,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
    horizontal: bool,
    metrics: &mut PolygonCompletionMetrics,
) -> Option<(Point, PolygonDirection)> {
    let candidates = polygon_candidate_points(polygon, horizontal_cuts, vertical_cuts);
    for point in candidates {
        let inside = polygon_local_quadrants(polygon, point);
        let blocked = polygon_local_blocked_rays(horizontal_cuts, vertical_cuts, inside, point);
        if !blocked.iter().any(|&value| value) {
            continue;
        }
        let (roots, sizes) = polygon_local_angle_components(inside, blocked);
        for (direction, ray, first, second) in [
            (PolygonDirection::East, 0, 1, 2),
            (PolygonDirection::North, 1, 2, 3),
            (PolygonDirection::West, 2, 3, 0),
            (PolygonDirection::South, 3, 0, 1),
        ] {
            if direction.is_horizontal() != horizontal {
                continue;
            }
            if horizontal {
                metrics.horizontal_candidate_queries += 1;
            } else {
                metrics.vertical_candidate_queries += 1;
            }
            if inside[first]
                && inside[second]
                && !blocked[ray]
                && roots[first] == roots[second]
                && sizes[roots[first]] >= 3
            {
                return Some((point, direction));
            }
        }
    }
    None
}

fn polygon_candidate_points(
    polygon: &RectilinearPolygon,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
) -> BTreeSet<Point> {
    let mut points = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().copied())
        .collect::<BTreeSet<_>>();
    for segment in horizontal_cuts {
        points.insert(Point::new(segment.left, segment.y));
        points.insert(Point::new(segment.right, segment.y));
    }
    for segment in vertical_cuts {
        points.insert(Point::new(segment.x, segment.bottom));
        points.insert(Point::new(segment.x, segment.top));
    }
    for horizontal in horizontal_cuts {
        for vertical in vertical_cuts {
            if horizontal.left <= vertical.x
                && vertical.x <= horizontal.right
                && vertical.bottom <= horizontal.y
                && horizontal.y <= vertical.top
            {
                points.insert(Point::new(vertical.x, horizontal.y));
            }
        }
    }
    points
}

fn polygon_local_quadrants(polygon: &RectilinearPolygon, point: Point) -> [bool; 4] {
    let x = 2 * i128::from(point.x);
    let y = 2 * i128::from(point.y);
    [
        polygon.contains_doubled_point_strict(DoubledPoint::new(x - 1, y - 1)),
        polygon.contains_doubled_point_strict(DoubledPoint::new(x + 1, y - 1)),
        polygon.contains_doubled_point_strict(DoubledPoint::new(x + 1, y + 1)),
        polygon.contains_doubled_point_strict(DoubledPoint::new(x - 1, y + 1)),
    ]
}

fn polygon_local_blocked_rays(
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
    inside: [bool; 4],
    point: Point,
) -> [bool; 4] {
    let east_cut = horizontal_cuts
        .iter()
        .any(|cut| cut.y == point.y && cut.left <= point.x && point.x < cut.right);
    let north_cut = vertical_cuts
        .iter()
        .any(|cut| cut.x == point.x && cut.bottom <= point.y && point.y < cut.top);
    let west_cut = horizontal_cuts
        .iter()
        .any(|cut| cut.y == point.y && cut.left < point.x && point.x <= cut.right);
    let south_cut = vertical_cuts
        .iter()
        .any(|cut| cut.x == point.x && cut.bottom < point.y && point.y <= cut.top);
    [
        east_cut || inside[1] != inside[2],
        north_cut || inside[2] != inside[3],
        west_cut || inside[3] != inside[0],
        south_cut || inside[0] != inside[1],
    ]
}

fn polygon_local_angle_components(
    inside: [bool; 4],
    blocked: [bool; 4],
) -> ([usize; 4], [usize; 4]) {
    let mut roots = [0, 1, 2, 3];
    for (ray, first, second) in [(0, 1, 2), (1, 2, 3), (2, 3, 0), (3, 0, 1)] {
        if inside[first] && inside[second] && !blocked[ray] {
            polygon_union_roots(&mut roots, first, second);
        }
    }
    for index in 0..4 {
        roots[index] = polygon_find_root(&roots, index);
    }
    let mut sizes = [0; 4];
    for index in 0..4 {
        if inside[index] {
            sizes[roots[index]] += 1;
        }
    }
    (roots, sizes)
}

fn polygon_find_root(roots: &[usize; 4], mut index: usize) -> usize {
    while roots[index] != index {
        index = roots[index];
    }
    index
}

fn polygon_union_roots(roots: &mut [usize; 4], first: usize, second: usize) {
    let first_root = polygon_find_root(roots, first);
    let second_root = polygon_find_root(roots, second);
    if first_root != second_root {
        roots[second_root] = first_root;
    }
}

fn find_polygon_ray_stop(
    polygon: &RectilinearPolygon,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
    point: Point,
    direction: PolygonDirection,
) -> Option<Point> {
    let mut coordinates = Vec::new();
    for boundary_loop in polygon.loops() {
        for (first, second) in boundary_loop.edges() {
            collect_boundary_stop(&mut coordinates, point, direction, first, second);
        }
    }
    match direction {
        PolygonDirection::East => {
            coordinates.extend(vertical_cuts.iter().filter_map(|cut| {
                (cut.x > point.x && cut.bottom <= point.y && point.y <= cut.top).then_some(cut.x)
            }));
            coordinates.extend(
                horizontal_cuts
                    .iter()
                    .filter_map(|cut| (cut.y == point.y && cut.left > point.x).then_some(cut.left)),
            );
            coordinates
                .into_iter()
                .min()
                .map(|x| Point::new(x, point.y))
        }
        PolygonDirection::West => {
            coordinates.extend(vertical_cuts.iter().filter_map(|cut| {
                (cut.x < point.x && cut.bottom <= point.y && point.y <= cut.top).then_some(cut.x)
            }));
            coordinates.extend(
                horizontal_cuts.iter().filter_map(|cut| {
                    (cut.y == point.y && cut.right < point.x).then_some(cut.right)
                }),
            );
            coordinates
                .into_iter()
                .max()
                .map(|x| Point::new(x, point.y))
        }
        PolygonDirection::North => {
            coordinates.extend(horizontal_cuts.iter().filter_map(|cut| {
                (cut.y > point.y && cut.left <= point.x && point.x <= cut.right).then_some(cut.y)
            }));
            coordinates.extend(vertical_cuts.iter().filter_map(|cut| {
                (cut.x == point.x && cut.bottom > point.y).then_some(cut.bottom)
            }));
            coordinates
                .into_iter()
                .min()
                .map(|y| Point::new(point.x, y))
        }
        PolygonDirection::South => {
            coordinates.extend(horizontal_cuts.iter().filter_map(|cut| {
                (cut.y < point.y && cut.left <= point.x && point.x <= cut.right).then_some(cut.y)
            }));
            coordinates.extend(
                vertical_cuts
                    .iter()
                    .filter_map(|cut| (cut.x == point.x && cut.top < point.y).then_some(cut.top)),
            );
            coordinates
                .into_iter()
                .max()
                .map(|y| Point::new(point.x, y))
        }
    }
}

fn collect_boundary_stop(
    coordinates: &mut Vec<i64>,
    point: Point,
    direction: PolygonDirection,
    first: Point,
    second: Point,
) {
    let left = first.x.min(second.x);
    let right = first.x.max(second.x);
    let bottom = first.y.min(second.y);
    let top = first.y.max(second.y);
    match direction {
        PolygonDirection::East if first.x == second.x => {
            if first.x > point.x && bottom <= point.y && point.y <= top {
                coordinates.push(first.x);
            }
        }
        PolygonDirection::East if first.y == second.y => {
            if first.y == point.y && left > point.x {
                coordinates.push(left);
            }
        }
        PolygonDirection::West if first.x == second.x => {
            if first.x < point.x && bottom <= point.y && point.y <= top {
                coordinates.push(first.x);
            }
        }
        PolygonDirection::West if first.y == second.y => {
            if first.y == point.y && right < point.x {
                coordinates.push(right);
            }
        }
        PolygonDirection::North if first.y == second.y => {
            if first.y > point.y && left <= point.x && point.x <= right {
                coordinates.push(first.y);
            }
        }
        PolygonDirection::North if first.x == second.x => {
            if first.x == point.x && bottom > point.y {
                coordinates.push(bottom);
            }
        }
        PolygonDirection::South if first.y == second.y => {
            if first.y < point.y && left <= point.x && point.x <= right {
                coordinates.push(first.y);
            }
        }
        PolygonDirection::South if first.x == second.x => {
            if first.x == point.x && top < point.y {
                coordinates.push(top);
            }
        }
        _ => {}
    }
}

struct PolygonRecovery {
    rectangles: Vec<CoordinateRect>,
    x_count: usize,
    y_count: usize,
    atomic_cell_count: usize,
    visits: usize,
}

fn recover_coordinate_rectangles(
    polygon: &RectilinearPolygon,
    horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
    vertical_cuts: &BTreeSet<VerticalCutSegment>,
) -> Result<PolygonRecovery, PolygonSgError> {
    let mut xs = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
        .collect::<BTreeSet<_>>();
    let mut ys = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
        .collect::<BTreeSet<_>>();
    for cut in horizontal_cuts {
        xs.extend([cut.left, cut.right]);
        ys.insert(cut.y);
    }
    for cut in vertical_cuts {
        xs.insert(cut.x);
        ys.extend([cut.bottom, cut.top]);
    }
    let xs = xs.into_iter().collect::<Vec<_>>();
    let ys = ys.into_iter().collect::<Vec<_>>();
    let width = xs.len().saturating_sub(1);
    let height = ys.len().saturating_sub(1);
    let atomic_cell_count = width
        .checked_mul(height)
        .ok_or(PolygonSgError::CoordinateOverflow)?;
    let occupied = (0..atomic_cell_count)
        .map(|index| {
            let x = index % width;
            let y = index / width;
            polygon.contains_doubled_point_strict(DoubledPoint::new(
                i128::from(xs[x]) + i128::from(xs[x + 1]),
                i128::from(ys[y]) + i128::from(ys[y + 1]),
            ))
        })
        .collect::<Vec<_>>();
    let mut region_ids = vec![usize::MAX; atomic_cell_count];
    let mut queue = VecDeque::new();
    let mut rectangles = Vec::new();
    let mut visits = 0;
    for seed in 0..atomic_cell_count {
        if !occupied[seed] || region_ids[seed] != usize::MAX {
            continue;
        }
        let region_id = rectangles.len();
        region_ids[seed] = region_id;
        queue.push_back(seed);
        let (mut left, mut right) = (seed % width, seed % width + 1);
        let (mut bottom, mut top) = (seed / width, seed / width + 1);
        while let Some(index) = queue.pop_front() {
            visits += 1;
            let x = index % width;
            let y = index / width;
            left = left.min(x);
            right = right.max(x + 1);
            bottom = bottom.min(y);
            top = top.max(y + 1);
            let mut visit = |neighbor: usize| {
                if occupied[neighbor] && region_ids[neighbor] == usize::MAX {
                    region_ids[neighbor] = region_id;
                    queue.push_back(neighbor);
                }
            };
            if x > 0 && !vertical_barrier_covers(vertical_cuts, xs[x], ys[y], ys[y + 1]) {
                visit(index - 1);
            }
            if x + 1 < width && !vertical_barrier_covers(vertical_cuts, xs[x + 1], ys[y], ys[y + 1])
            {
                visit(index + 1);
            }
            if y > 0 && !horizontal_barrier_covers(horizontal_cuts, ys[y], xs[x], xs[x + 1]) {
                visit(index - width);
            }
            if y + 1 < height
                && !horizontal_barrier_covers(horizontal_cuts, ys[y + 1], xs[x], xs[x + 1])
            {
                visit(index + width);
            }
        }
        if !(bottom..top).all(|y| (left..right).all(|x| region_ids[y * width + x] == region_id)) {
            return Err(PolygonSgError::NonRectangularCompletionRegion {
                point: Point::new(xs[seed % width], ys[seed / width]),
            });
        }
        rectangles.push(CoordinateRect::new(
            xs[left], ys[bottom], xs[right], ys[top],
        )?);
    }
    rectangles.sort_unstable();
    Ok(PolygonRecovery {
        rectangles,
        x_count: xs.len(),
        y_count: ys.len(),
        atomic_cell_count,
        visits,
    })
}

fn vertical_barrier_covers(
    cuts: &BTreeSet<VerticalCutSegment>,
    x: i64,
    bottom: i64,
    top: i64,
) -> bool {
    cuts.iter()
        .any(|cut| cut.x == x && cut.bottom <= bottom && top <= cut.top)
}

fn horizontal_barrier_covers(
    cuts: &BTreeSet<HorizontalCutSegment>,
    y: i64,
    left: i64,
    right: i64,
) -> bool {
    cuts.iter()
        .any(|cut| cut.y == y && cut.left <= left && right <= cut.right)
}

/// Validates an exact coordinate-rectangle partition of an ordinary polygon.
///
/// # Errors
///
/// Returns [`PolygonValidationError`] for invalid rectangles, overlap,
/// uncovered polygon area, or coverage outside the polygon.
pub fn validate_polygon_dissection(
    polygon: &RectilinearPolygon,
    rectangles: &[CoordinateRect],
) -> Result<(), PolygonValidationError> {
    let mut xs = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
        .collect::<BTreeSet<_>>();
    let mut ys = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
        .collect::<BTreeSet<_>>();
    let mut rectangle_area = 0_i128;
    for (index, rectangle) in rectangles.iter().copied().enumerate() {
        if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
            return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
        }
        xs.extend([rectangle.x0, rectangle.x1]);
        ys.extend([rectangle.y0, rectangle.y1]);
        rectangle_area = rectangle_area
            .checked_add(rectangle.area())
            .ok_or(PolygonValidationError::AreaOverflow)?;
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
    let xs = xs.into_iter().collect::<Vec<_>>();
    let ys = ys.into_iter().collect::<Vec<_>>();
    for y in 0..ys.len().saturating_sub(1) {
        for x in 0..xs.len().saturating_sub(1) {
            let point = DoubledPoint::new(
                i128::from(xs[x]) + i128::from(xs[x + 1]),
                i128::from(ys[y]) + i128::from(ys[y + 1]),
            );
            let inside = polygon.contains_doubled_point_strict(point);
            let covering = rectangles
                .iter()
                .enumerate()
                .filter_map(|(index, rectangle)| {
                    rectangle
                        .contains_doubled_point_strict(point)
                        .then_some(index)
                })
                .collect::<Vec<_>>();
            if covering.len() > 1 {
                return Err(PolygonValidationError::Overlap {
                    first: covering[0],
                    second: covering[1],
                    point,
                });
            }
            if inside && covering.is_empty() {
                return Err(PolygonValidationError::UncoveredInterior { point });
            }
            if !inside && !covering.is_empty() {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: covering[0],
                    point,
                });
            }
        }
    }
    Ok(())
}

/// Validates the declared optimum count in addition to exact geometry.
///
/// # Errors
///
/// Returns [`PolygonValidationError::DeclaredCount`] on count mismatch, then
/// applies [`validate_polygon_dissection`].
pub fn validate_polygon_dissection_count(
    polygon: &RectilinearPolygon,
    declared: usize,
    rectangles: &[CoordinateRect],
) -> Result<(), PolygonValidationError> {
    if declared != rectangles.len() {
        return Err(PolygonValidationError::DeclaredCount {
            declared,
            actual: rectangles.len(),
        });
    }
    validate_polygon_dissection(polygon, rectangles)
}

#[derive(Clone, Debug, Eq, Error, PartialEq, Serialize, Deserialize)]
pub enum PolygonValidationError {
    #[error(transparent)]
    Polygon(PolygonError),
    #[error("declared rectangle count {declared} differs from actual count {actual}")]
    DeclaredCount { declared: usize, actual: usize },
    #[error("coordinate rectangle {rectangle} has non-positive area")]
    NonPositiveRectangle { rectangle: usize },
    #[error("coordinate rectangle {rectangle} covers outside point {point:?}")]
    OutsidePolygon {
        rectangle: usize,
        point: DoubledPoint,
    },
    #[error("coordinate rectangles {first} and {second} overlap near {point:?}")]
    Overlap {
        first: usize,
        second: usize,
        point: DoubledPoint,
    },
    #[error("polygon interior is uncovered near {point:?}")]
    UncoveredInterior { point: DoubledPoint },
    #[error(
        "rectangle area {rectangle_area_twice} does not equal polygon area {polygon_area_twice}"
    )]
    AreaMismatch {
        polygon_area_twice: i128,
        rectangle_area_twice: i128,
    },
    #[error("exact rectangle-area arithmetic overflowed i128")]
    AreaOverflow,
}

#[derive(Debug, Error)]
pub enum PolygonSgError {
    #[error(transparent)]
    Polygon(#[from] PolygonError),
    #[error(transparent)]
    BoundaryIndex(#[from] BoundaryIndexError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error("effective chord endpoint {point:?} is not a normalized boundary vertex")]
    EndpointNotOnBoundary { point: Point },
    #[error("invalid normalized boundary vertex identity {0:?}")]
    InvalidBoundaryVertexId(BoundaryVertexId),
    #[error("doubled-coordinate arithmetic overflowed")]
    CoordinateOverflow,
    #[error("effective-chord selection vectors have the wrong length")]
    SelectionLengthMismatch,
    #[error("simple chord from {start:?} is empty or duplicates an existing cut")]
    InvalidSimpleChord { start: Point },
    #[error("simple chord from {start:?} did not reach a boundary or existing cut")]
    UnboundedSimpleChord { start: Point },
    #[error("boundary-native completion did not terminate")]
    CompletionDidNotTerminate,
    #[error("completion region at {point:?} is not a coordinate rectangle")]
    NonRectangularCompletionRegion { point: Point },
    #[error(transparent)]
    Validation(#[from] PolygonValidationError),
}

#[cfg(test)]
mod tests {
    use rect_core::{Boundary, ColorGrid, OrthogonalLoop, Point, RectilinearPolygon};

    use crate::{EffectiveChordEnumerator, GridInteriorRunEnumerator};

    use super::GeneralPolygonPairwiseEnumerator;

    #[test]
    fn grid_derived_polygon_chords_match_on_all_3x3_masks() {
        let enumerator = GeneralPolygonPairwiseEnumerator;
        let mut compared = 0;
        for mask in 1_u16..1 << 9 {
            let grid =
                ColorGrid::new(3, 3, (0..9).map(|bit| mask & (1 << bit) != 0).collect()).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = Boundary::from_component(&component).unwrap();
                let Ok(polygon) = boundary.to_polygon() else {
                    continue;
                };
                let grid_families = GridInteriorRunEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                let polygon_families = enumerator.enumerate(&polygon).unwrap();
                assert_eq!(grid_families.horizontal, polygon_families.horizontal);
                assert_eq!(grid_families.vertical, polygon_families.vertical);
                compared += 1;
            }
        }
        assert!(compared > 100);
    }

    #[test]
    fn rejects_collinear_boundary_overlap_and_hole_interior() {
        let notch = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(8, 0),
                Point::new(8, 8),
                Point::new(5, 8),
                Point::new(5, 3),
                Point::new(3, 3),
                Point::new(3, 8),
                Point::new(0, 8),
            ]),
            vec![],
        )
        .unwrap();
        let families = GeneralPolygonPairwiseEnumerator.enumerate(&notch).unwrap();
        assert!(families.horizontal.is_empty());
        assert!(families.vertical.is_empty());

        let with_hole = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(12, 0),
                Point::new(12, 10),
                Point::new(0, 10),
            ]),
            vec![OrthogonalLoop::new(vec![
                Point::new(4, 3),
                Point::new(4, 7),
                Point::new(8, 7),
                Point::new(8, 3),
            ])],
        )
        .unwrap();
        let families = GeneralPolygonPairwiseEnumerator
            .enumerate(&with_hole)
            .unwrap();
        assert!(families.horizontal.is_empty());
        assert!(families.vertical.is_empty());
    }

    #[test]
    fn coordinate_completion_handles_large_gaps_without_rasterization() {
        let polygon = RectilinearPolygon::new(
            OrthogonalLoop::new(vec![
                Point::new(0, 0),
                Point::new(1_000_000_000, 0),
                Point::new(1_000_000_000, 10),
                Point::new(0, 10),
            ]),
            vec![],
        )
        .unwrap();
        let completion = super::CoordinateCompressedCompletion
            .complete(&polygon, &[], &[], &[], &[])
            .unwrap();
        assert_eq!(completion.rectangles.len(), 1);
        assert_eq!(completion.metrics.coordinate_compression_x_count, 2);
        assert_eq!(completion.metrics.coordinate_compression_y_count, 2);
        assert_eq!(completion.metrics.atomic_cell_count, 1);
    }

    #[test]
    fn coordinate_completion_dissects_an_l_shape() {
        let polygon = RectilinearPolygon::new(
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
        let completion = super::CoordinateCompressedCompletion
            .complete(&polygon, &[], &[], &[], &[])
            .unwrap();
        assert_eq!(completion.rectangles.len(), 2);
        super::validate_polygon_dissection(&polygon, &completion.rectangles).unwrap();
    }
}
