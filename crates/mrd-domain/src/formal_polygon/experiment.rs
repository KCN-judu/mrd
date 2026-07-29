//! Section 10 Step 1(a)--(d) effective-chord construction.

use std::collections::BTreeMap;

use crate::{
    Boundary, Coord, DoubledPoint, HorizontalChord, HorizontalChordId, OrthogonalEdgeIndex, Point,
    VerticalChord, VerticalChordId,
};

use super::{
    FormalBoundaryIncidence, FormalChordAxis, FormalChordConstructionMetrics,
    FormalChordConstructionRecord, FormalChordConstructionResult, FormalChordEndpoints,
    FormalDirection, FormalFamilies, FormalPolygonError, FormalRectilinearPolygon,
    FormalVertexGeometry, FormalVertexId, build_vertex_geometry,
};

/// Constructs every effective chord using the source algorithm.
///
/// # Errors
///
/// Returns a structured validation or generated-chord error.
pub fn effective_chords(
    polygon: &FormalRectilinearPolygon,
) -> Result<FormalChordConstructionResult, FormalPolygonError> {
    let incidence = polygon.incidence()?;
    let geometry = build_vertex_geometry(polygon, &incidence);
    construct(polygon, &incidence, &geometry)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceChordSpan {
    first: FormalVertexId,
    second: FormalVertexId,
    merged_vertices: Vec<FormalVertexId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AxisChordCandidate {
    span: SourceChordSpan,
    fixed: Coord,
    low: Coord,
    high: Coord,
    valid: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AxisSweepEvent {
    starts: Vec<usize>,
    ends: Vec<usize>,
    orthogonal_ranges: Vec<(Coord, Coord)>,
}

fn construct(
    polygon: &FormalRectilinearPolygon,
    incidence: &FormalBoundaryIncidence,
    geometry: &[FormalVertexGeometry],
) -> Result<FormalChordConstructionResult, FormalPolygonError> {
    let mut metrics = FormalChordConstructionMetrics::default();
    let topological_index = OrthogonalEdgeIndex::new(&Boundary::from_polygon(&polygon.region));
    let horizontal = construct_axis_chords(
        &topological_index,
        incidence,
        geometry,
        FormalChordAxis::Horizontal,
        &mut metrics,
    );
    let vertical = construct_axis_chords(
        &topological_index,
        incidence,
        geometry,
        FormalChordAxis::Vertical,
        &mut metrics,
    );
    metrics.output_horizontal_chords = horizontal.len();
    metrics.output_vertical_chords = vertical.len();

    let horizontal_endpoints = horizontal
        .iter()
        .map(|span| FormalChordEndpoints {
            first: span.first,
            second: span.second,
        })
        .collect::<Vec<_>>();
    let vertical_endpoints = vertical
        .iter()
        .map(|span| FormalChordEndpoints {
            first: span.first,
            second: span.second,
        })
        .collect::<Vec<_>>();
    let horizontal_chords = horizontal
        .iter()
        .enumerate()
        .map(|(index, span)| {
            let first = incidence.vertices[span.first.0].point;
            let second = incidence.vertices[span.second.0].point;
            HorizontalChord::new(HorizontalChordId(index), first.x, second.x, first.y).map_err(
                |_| FormalPolygonError::GeneratedChordInvalid {
                    start: first,
                    end: second,
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let vertical_chords = vertical
        .iter()
        .enumerate()
        .map(|(index, span)| {
            let first = incidence.vertices[span.first.0].point;
            let second = incidence.vertices[span.second.0].point;
            VerticalChord::new(VerticalChordId(index), first.x, first.y, second.y).map_err(|_| {
                FormalPolygonError::GeneratedChordInvalid {
                    start: first,
                    end: second,
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let records = horizontal
        .iter()
        .map(|span| source_record(FormalChordAxis::Horizontal, span))
        .chain(
            vertical
                .iter()
                .map(|span| source_record(FormalChordAxis::Vertical, span)),
        )
        .collect();
    Ok(FormalChordConstructionResult {
        families: FormalFamilies {
            horizontal: horizontal_chords,
            vertical: vertical_chords,
            horizontal_endpoints,
            vertical_endpoints,
            candidate_pair_count: metrics.step_a_adjacent_pair_tests,
        },
        metrics,
        records,
    })
}

fn construct_axis_chords(
    topological_index: &OrthogonalEdgeIndex,
    incidence: &FormalBoundaryIncidence,
    geometry: &[FormalVertexGeometry],
    axis: FormalChordAxis,
    metrics: &mut FormalChordConstructionMetrics,
) -> Vec<SourceChordSpan> {
    let mut candidates = build_step_a_candidates(topological_index, incidence, axis, metrics);
    invalidate_orthogonal_crossings(axis, incidence, &mut candidates, metrics);

    let mut valid_by_line = BTreeMap::<Coord, Vec<SourceChordSpan>>::new();
    for candidate in candidates.into_iter().filter(|candidate| candidate.valid) {
        metrics.step_a_open_interior_chords += 1;
        valid_by_line
            .entry(candidate.fixed)
            .or_default()
            .push(candidate.span);
    }
    let mut output = Vec::new();
    for spans in valid_by_line.values_mut() {
        spans.sort_by_key(|span| {
            axis_variable_coordinate(axis, incidence.vertices[span.first.0].point)
        });
        let mut retained = Vec::new();
        for span in spans.drain(..) {
            if endpoint_has_two_orthogonal_segments(axis, span.first, incidence)
                || endpoint_has_two_orthogonal_segments(axis, span.second, incidence)
            {
                metrics.step_b_two_orthogonal_deletions += 1;
                continue;
            }
            retained.push(span);
        }

        let mut merged = Vec::<SourceChordSpan>::new();
        for span in retained {
            if let Some(previous) = merged.last_mut()
                && previous.second == span.first
                && !geometry[span.first.0].isolated
            {
                previous.second = span.second;
                previous.merged_vertices.push(span.first);
                previous.merged_vertices.extend(span.merged_vertices);
                metrics.step_c_nonisolated_merges += 1;
            } else {
                merged.push(span);
            }
        }
        for span in merged {
            if !source_endpoint_is_valid(axis, &geometry[span.first.0])
                || !source_endpoint_is_valid(axis, &geometry[span.second.0])
            {
                metrics.step_d_endpoint_deletions += 1;
                continue;
            }
            output.push(span);
        }
    }
    output.sort_by_key(|span| {
        let first = incidence.vertices[span.first.0].point;
        let second = incidence.vertices[span.second.0].point;
        (
            axis_fixed_coordinate(axis, first),
            axis_variable_coordinate(axis, first),
            axis_variable_coordinate(axis, second),
        )
    });
    output
}

fn build_step_a_candidates(
    topological_index: &OrthogonalEdgeIndex,
    incidence: &FormalBoundaryIncidence,
    axis: FormalChordAxis,
    metrics: &mut FormalChordConstructionMetrics,
) -> Vec<AxisChordCandidate> {
    let mut lines = BTreeMap::<Coord, Vec<FormalVertexId>>::new();
    for vertex in &incidence.vertices {
        lines
            .entry(axis_fixed_coordinate(axis, vertex.point))
            .or_default()
            .push(vertex.id);
    }
    metrics.axis_line_count += lines.len();
    let mut candidates = Vec::new();
    for vertices in lines.values_mut() {
        vertices.sort_by_key(|&vertex| {
            axis_variable_coordinate(axis, incidence.vertices[vertex.0].point)
        });
        for pair in vertices.windows(2) {
            metrics.step_a_adjacent_pair_tests += 1;
            let first = pair[0];
            let second = pair[1];
            let first_point = incidence.vertices[first.0].point;
            let second_point = incidence.vertices[second.0].point;
            metrics.step_a_point_location_queries += 1;
            if !topological_index.contains_doubled_point_by_parity(axis_midpoint(
                axis,
                first_point,
                second_point,
            )) {
                continue;
            }
            if elementary_segment_connects(first, second, incidence) {
                metrics.step_a_collinear_boundary_rejections += 1;
                continue;
            }
            candidates.push(AxisChordCandidate {
                span: SourceChordSpan {
                    first,
                    second,
                    merged_vertices: Vec::new(),
                },
                fixed: axis_fixed_coordinate(axis, first_point),
                low: axis_variable_coordinate(axis, first_point),
                high: axis_variable_coordinate(axis, second_point),
                valid: true,
            });
            metrics.step_a_candidate_insertions += 1;
        }
    }
    candidates
}

fn source_record(axis: FormalChordAxis, span: &SourceChordSpan) -> FormalChordConstructionRecord {
    FormalChordConstructionRecord {
        axis,
        endpoints: FormalChordEndpoints {
            first: span.first,
            second: span.second,
        },
        merged_vertices: span.merged_vertices.clone(),
    }
}

fn axis_fixed_coordinate(axis: FormalChordAxis, point: Point) -> Coord {
    match axis {
        FormalChordAxis::Horizontal => point.y,
        FormalChordAxis::Vertical => point.x,
    }
}

fn axis_variable_coordinate(axis: FormalChordAxis, point: Point) -> Coord {
    match axis {
        FormalChordAxis::Horizontal => point.x,
        FormalChordAxis::Vertical => point.y,
    }
}

fn axis_midpoint(axis: FormalChordAxis, first: Point, second: Point) -> DoubledPoint {
    match axis {
        FormalChordAxis::Horizontal => DoubledPoint::new(
            i128::from(first.x) + i128::from(second.x),
            2 * i128::from(first.y),
        ),
        FormalChordAxis::Vertical => DoubledPoint::new(
            2 * i128::from(first.x),
            i128::from(first.y) + i128::from(second.y),
        ),
    }
}

fn elementary_segment_connects(
    first: FormalVertexId,
    second: FormalVertexId,
    incidence: &FormalBoundaryIncidence,
) -> bool {
    incidence.vertices[first.0]
        .incident_segments
        .iter()
        .any(|segment_id| {
            let segment = &incidence.elementary_segments[segment_id.0];
            (segment.start == first && segment.end == second)
                || (segment.start == second && segment.end == first)
        })
}

fn invalidate_orthogonal_crossings(
    axis: FormalChordAxis,
    incidence: &FormalBoundaryIncidence,
    candidates: &mut [AxisChordCandidate],
    metrics: &mut FormalChordConstructionMetrics,
) {
    let mut events = BTreeMap::<Coord, AxisSweepEvent>::new();
    for (candidate_id, candidate) in candidates.iter().enumerate() {
        events
            .entry(candidate.low)
            .or_default()
            .starts
            .push(candidate_id);
        events
            .entry(candidate.high)
            .or_default()
            .ends
            .push(candidate_id);
    }
    for segment in &incidence.elementary_segments {
        let first = incidence.vertices[segment.start.0].point;
        let second = incidence.vertices[segment.end.0].point;
        let segment_axis = if first.y == second.y {
            FormalChordAxis::Horizontal
        } else {
            FormalChordAxis::Vertical
        };
        if segment_axis == axis {
            continue;
        }
        let coordinate = axis_variable_coordinate(axis, first);
        let low = axis_fixed_coordinate(axis, first).min(axis_fixed_coordinate(axis, second));
        let high = axis_fixed_coordinate(axis, first).max(axis_fixed_coordinate(axis, second));
        events
            .entry(coordinate)
            .or_default()
            .orthogonal_ranges
            .push((low, high));
    }

    let mut active = BTreeMap::<Coord, usize>::new();
    for event in events.values_mut() {
        event.ends.sort_unstable();
        event.starts.sort_unstable();
        event.orthogonal_ranges.sort_unstable();
        for &candidate_id in &event.ends {
            if candidates[candidate_id].valid {
                active.remove(&candidates[candidate_id].fixed);
            }
        }
        for &(low, high) in &event.orthogonal_ranges {
            metrics.step_a_orthogonal_segment_queries += 1;
            let crossed = active
                .range(low..=high)
                .map(|(&fixed, &candidate_id)| (fixed, candidate_id))
                .collect::<Vec<_>>();
            metrics.step_a_reported_boundary_crossings += crossed.len();
            for (fixed, candidate_id) in crossed {
                active.remove(&fixed);
                candidates[candidate_id].valid = false;
                metrics.step_a_candidate_removals += 1;
            }
        }
        for &candidate_id in &event.starts {
            if candidates[candidate_id].valid {
                let replaced = active.insert(candidates[candidate_id].fixed, candidate_id);
                debug_assert!(replaced.is_none(), "adjacent open candidates are disjoint");
            }
        }
    }
}

fn endpoint_has_two_orthogonal_segments(
    axis: FormalChordAxis,
    vertex: FormalVertexId,
    incidence: &FormalBoundaryIncidence,
) -> bool {
    incidence.vertices[vertex.0]
        .incident_segments
        .iter()
        .filter(|segment_id| {
            let segment = &incidence.elementary_segments[segment_id.0];
            let first = incidence.vertices[segment.start.0].point;
            let second = incidence.vertices[segment.end.0].point;
            let segment_axis = if first.y == second.y {
                FormalChordAxis::Horizontal
            } else {
                FormalChordAxis::Vertical
            };
            segment_axis != axis
        })
        .take(2)
        .count()
        == 2
}

fn source_endpoint_is_valid(axis: FormalChordAxis, vertex: &FormalVertexGeometry) -> bool {
    vertex.local_nonconvexity_measure > 0
        && (vertex.isolated
            || vertex.incident_directions.iter().any(|direction| {
                matches!(
                    (axis, direction),
                    (
                        FormalChordAxis::Horizontal,
                        FormalDirection::East | FormalDirection::West
                    ) | (
                        FormalChordAxis::Vertical,
                        FormalDirection::North | FormalDirection::South
                    )
                )
            }))
}
