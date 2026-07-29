//! Pairwise Definition 7 effective-chord oracle.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DoubledPoint, HorizontalChord, HorizontalChordId, Point, VerticalChord, VerticalChordId,
};

use super::{
    FormalBoundaryIncidence, FormalChordEndpoints, FormalDirection, FormalEffectiveChordFamilies,
    FormalPolygonError, FormalRectilinearPolygon, FormalVertexGeometry, build_vertex_geometry,
};

/// Enumerates every formal effective chord by testing all eligible pairs.
///
/// # Errors
///
/// Returns a structured validation or coordinate-geometry error.
pub fn effective_chords(
    polygon: &FormalRectilinearPolygon,
) -> Result<FormalEffectiveChordFamilies, FormalPolygonError> {
    let incidence = polygon.incidence()?;
    let geometry = build_vertex_geometry(polygon, &incidence);
    enumerate(polygon, &incidence, &geometry)
}

fn enumerate(
    polygon: &FormalRectilinearPolygon,
    incidence: &FormalBoundaryIncidence,
    geometry: &[FormalVertexGeometry],
) -> Result<FormalEffectiveChordFamilies, FormalPolygonError> {
    let candidates = geometry
        .iter()
        .filter(|vertex| vertex.local_nonconvexity_measure > 0)
        .collect::<Vec<_>>();
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();
    let mut candidate_pair_count = 0;
    for first_index in 0..candidates.len() {
        for second_index in first_index + 1..candidates.len() {
            let first = candidates[first_index];
            let second = candidates[second_index];
            if first.point.x != second.point.x && first.point.y != second.point.y {
                continue;
            }
            candidate_pair_count += 1;
            let (first, second) = if first.point < second.point {
                (first, second)
            } else {
                (second, first)
            };
            if definition7_effective_chord(polygon, incidence, first, second) {
                if first.point.y == second.point.y {
                    horizontal.push((
                        first.point.y,
                        first.point.x,
                        second.point.x,
                        first.vertex,
                        second.vertex,
                    ));
                } else {
                    vertical.push((
                        first.point.x,
                        first.point.y,
                        second.point.y,
                        first.vertex,
                        second.vertex,
                    ));
                }
            }
        }
    }
    horizontal.sort_unstable();
    vertical.sort_unstable();
    let horizontal_endpoints = horizontal
        .iter()
        .map(|&(_, _, _, first, second)| FormalChordEndpoints { first, second })
        .collect::<Vec<_>>();
    let vertical_endpoints = vertical
        .iter()
        .map(|&(_, _, _, first, second)| FormalChordEndpoints { first, second })
        .collect::<Vec<_>>();
    let horizontal = horizontal
        .into_iter()
        .enumerate()
        .map(|(index, (y, left, right, _, _))| {
            HorizontalChord::new(HorizontalChordId(index), left, right, y).map_err(|_| {
                FormalPolygonError::GeneratedChordInvalid {
                    start: Point::new(left, y),
                    end: Point::new(right, y),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let vertical = vertical
        .into_iter()
        .enumerate()
        .map(|(index, (x, bottom, top, _, _))| {
            VerticalChord::new(VerticalChordId(index), x, bottom, top).map_err(|_| {
                FormalPolygonError::GeneratedChordInvalid {
                    start: Point::new(x, bottom),
                    end: Point::new(x, top),
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(FormalEffectiveChordFamilies {
        horizontal,
        vertical,
        horizontal_endpoints,
        vertical_endpoints,
        candidate_pair_count,
    })
}

fn definition7_effective_chord(
    polygon: &FormalRectilinearPolygon,
    incidence: &FormalBoundaryIncidence,
    first: &FormalVertexGeometry,
    second: &FormalVertexGeometry,
) -> bool {
    let horizontal = first.point.y == second.point.y;
    if !endpoint_supports_chord(first, horizontal) || !endpoint_supports_chord(second, horizontal) {
        return false;
    }
    definition7_open_interval(polygon, incidence, first.point, second.point, horizontal)
}

fn endpoint_supports_chord(vertex: &FormalVertexGeometry, horizontal: bool) -> bool {
    if vertex.isolated {
        return true;
    }
    vertex.incident_directions.iter().any(|direction| {
        matches!(
            (horizontal, direction),
            (true, FormalDirection::East | FormalDirection::West)
                | (false, FormalDirection::North | FormalDirection::South)
        )
    })
}

fn definition7_open_interval(
    polygon: &FormalRectilinearPolygon,
    incidence: &FormalBoundaryIncidence,
    first: Point,
    second: Point,
    horizontal: bool,
) -> bool {
    let (start, end, fixed) = if horizontal {
        (first.x, second.x, first.y)
    } else {
        (first.y, second.y, first.x)
    };
    let mut breakpoints = BTreeSet::from([start, end]);
    let vertex_ids = incidence
        .vertices
        .iter()
        .map(|vertex| (vertex.point, vertex.id))
        .collect::<BTreeMap<_, _>>();

    for segment in &incidence.elementary_segments {
        let segment_start = incidence.vertices[segment.start.0].point;
        let segment_end = incidence.vertices[segment.end.0].point;
        let segment_horizontal = segment_start.y == segment_end.y;
        if segment_horizontal == horizontal {
            let same_line = if horizontal {
                segment_start.y == fixed
            } else {
                segment_start.x == fixed
            };
            if same_line {
                let segment_low = if horizontal {
                    segment_start.x.min(segment_end.x)
                } else {
                    segment_start.y.min(segment_end.y)
                };
                let segment_high = if horizontal {
                    segment_start.x.max(segment_end.x)
                } else {
                    segment_start.y.max(segment_end.y)
                };
                if start.max(segment_low) < end.min(segment_high) {
                    return false;
                }
            }
            continue;
        }

        let (coordinate, segment_low, segment_high) = if horizontal {
            (
                segment_start.x,
                segment_start.y.min(segment_end.y),
                segment_start.y.max(segment_end.y),
            )
        } else {
            (
                segment_start.y,
                segment_start.x.min(segment_end.x),
                segment_start.x.max(segment_end.x),
            )
        };
        if start < coordinate && coordinate < end && segment_low <= fixed && fixed <= segment_high {
            let point = if horizontal {
                Point::new(coordinate, fixed)
            } else {
                Point::new(fixed, coordinate)
            };
            let Some(&vertex_id) = vertex_ids.get(&point) else {
                return false;
            };
            let orthogonal_count = incidence.vertices[vertex_id.0]
                .incident_segments
                .iter()
                .filter(|segment_id| {
                    let segment = &incidence.elementary_segments[segment_id.0];
                    let segment_start = incidence.vertices[segment.start.0].point;
                    let segment_end = incidence.vertices[segment.end.0].point;
                    (segment_start.y == segment_end.y) != horizontal
                })
                .count();
            if orthogonal_count != 1 {
                return false;
            }
            breakpoints.insert(coordinate);
        }
    }

    for &point in &polygon.ornament.isolated_points {
        let (coordinate, point_fixed) = if horizontal {
            (point.x, point.y)
        } else {
            (point.y, point.x)
        };
        if point_fixed == fixed && start < coordinate && coordinate < end {
            return false;
        }
    }

    let breakpoints = breakpoints.into_iter().collect::<Vec<_>>();
    for pair in breakpoints.windows(2) {
        let variable = i128::from(pair[0]) + i128::from(pair[1]);
        let fixed = 2 * i128::from(fixed);
        let probe = if horizontal {
            DoubledPoint::new(variable, fixed)
        } else {
            DoubledPoint::new(fixed, variable)
        };
        if !polygon.contains_doubled_point_strict(probe) {
            return false;
        }
    }
    true
}
