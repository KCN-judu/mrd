//! Explicit Soltan--Gorpinevich oracle for ordinary grid-cell polygons.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashSet, VecDeque};
use std::time::Instant;

use rect_core::{
    Boundary, BoundaryError, BoundaryVertexId, Certificate, Coord, Diagnostics, DissectionResult,
    ExactRatio, GeometryError, GridComponent, GridRect, HorizontalChord, HorizontalChordId, Point,
    PreparedComponentContext, PreparedContextError, PreparedGridComponent, ValidationError,
    VerticalChord, VerticalChordId, closed_chords_intersect, validate_dissection,
};
use rect_graph::{BipartiteGraph, Matching, VertexCover, hopcroft_karp, minimum_vertex_cover};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct SgAnalysis {
    pub boundary: Boundary,
    pub prepared: PreparedGridComponent,
    pub horizontal_chords: Vec<HorizontalChord>,
    pub vertical_chords: Vec<VerticalChord>,
    pub conflict_graph: BipartiteGraph,
    pub matching: Matching,
    pub vertex_cover: VertexCover,
    pub selected_horizontal: Vec<bool>,
    pub selected_vertical: Vec<bool>,
    pub optimum_rectangle_count: usize,
}

#[derive(Clone, Debug)]
pub struct SgGeometry {
    pub boundary: Boundary,
    pub prepared: PreparedGridComponent,
    pub horizontal_chords: Vec<HorizontalChord>,
    pub vertical_chords: Vec<VerticalChord>,
    pub horizontal_interior_run_count: Option<usize>,
    pub vertical_interior_run_count: Option<usize>,
    pub candidate_reflex_pair_count: Option<usize>,
    pub prepared_component_build_microseconds: u128,
    pub boundary_extraction_microseconds: u128,
    pub reflex_grouping_microseconds: u128,
    pub effective_chord_enumeration_microseconds: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EffectiveChordFamilies {
    pub horizontal: Vec<HorizontalChord>,
    pub vertical: Vec<VerticalChord>,
    pub horizontal_interior_run_count: Option<usize>,
    pub vertical_interior_run_count: Option<usize>,
    pub candidate_reflex_pair_count: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ChordRef {
    Horizontal(HorizontalChordId),
    Vertical(VerticalChordId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ChordBoundaryEndpoints {
    pub first: BoundaryVertexId,
    pub second: BoundaryVertexId,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct EffectiveChordGeometry<C> {
    pub chord: C,
    pub endpoints: ChordBoundaryEndpoints,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CleanHoleFreeCertificate {
    pub eligible: bool,
    pub outer_loop_count: usize,
    pub hole_count: usize,
    pub all_chords_proper: bool,
    pub distinct_boundary_endpoints: bool,
    pub rejection_reasons: Vec<CleanRejectionReason>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum CleanRejectionReason {
    MultipleOuterLoops {
        count: usize,
    },
    HasHole {
        count: usize,
    },
    UnsupportedOrnamentModel,
    NonProperHorizontalChord(HorizontalChordId),
    NonProperVerticalChord(VerticalChordId),
    EndpointNotOnBoundary,
    SharedBoundaryEndpoint {
        first: ChordRef,
        second: ChordRef,
        endpoint: BoundaryVertexId,
    },
}

/// Returns deterministic boundary identities for a horizontal chord.
///
/// # Errors
///
/// Returns [`SgError::EndpointNotOnBoundary`] when either endpoint is not a
/// normalized boundary vertex.
pub fn horizontal_chord_endpoints(
    boundary: &Boundary,
    chord: HorizontalChord,
) -> Result<ChordBoundaryEndpoints, SgError> {
    let first = boundary
        .vertex_id(Point::new(chord.left(), chord.y()))
        .ok_or(SgError::EndpointNotOnBoundary)?;
    let second = boundary
        .vertex_id(Point::new(chord.right(), chord.y()))
        .ok_or(SgError::EndpointNotOnBoundary)?;
    Ok(ChordBoundaryEndpoints { first, second })
}

/// Returns deterministic boundary identities for a vertical chord.
///
/// # Errors
///
/// Returns [`SgError::EndpointNotOnBoundary`] when either endpoint is not a
/// normalized boundary vertex.
pub fn vertical_chord_endpoints(
    boundary: &Boundary,
    chord: VerticalChord,
) -> Result<ChordBoundaryEndpoints, SgError> {
    let first = boundary
        .vertex_id(Point::new(chord.x(), chord.bottom()))
        .ok_or(SgError::EndpointNotOnBoundary)?;
    let second = boundary
        .vertex_id(Point::new(chord.x(), chord.top()))
        .ok_or(SgError::EndpointNotOnBoundary)?;
    Ok(ChordBoundaryEndpoints { first, second })
}

/// Tests strict cyclic alternation on one normalized boundary loop.
#[must_use]
pub fn endpoints_alternate(
    first: ChordBoundaryEndpoints,
    second: ChordBoundaryEndpoints,
    loop_len: usize,
) -> bool {
    if loop_len == 0
        || first.first.loop_id != first.second.loop_id
        || first.first.loop_id != second.first.loop_id
        || second.first.loop_id != second.second.loop_id
        || [first.first, first.second, second.first, second.second]
            .iter()
            .enumerate()
            .any(|(index, endpoint)| {
                [first.first, first.second, second.first, second.second]
                    .iter()
                    .skip(index + 1)
                    .any(|other| endpoint == other)
            })
    {
        return false;
    }
    let between = |start: usize, end: usize, point: usize| {
        let end_distance = (end + loop_len - start) % loop_len;
        let point_distance = (point + loop_len - start) % loop_len;
        point_distance > 0 && point_distance < end_distance
    };
    between(
        first.first.cyclic_index,
        first.second.cyclic_index,
        second.first.cyclic_index,
    ) != between(
        first.first.cyclic_index,
        first.second.cyclic_index,
        second.second.cyclic_index,
    )
}

/// Classifies the ordinary finite-grid component under Definition 9.1.
///
/// The supported input model has no ornament object; the classifier therefore
/// records no ornament rejection for a value that cannot express ornaments.
#[must_use]
pub fn classify_clean_hole_free<C>(
    component: &GridComponent<C>,
    boundary: &Boundary,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
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
    let mut endpoint_entries = Vec::<(ChordRef, ChordBoundaryEndpoints)>::new();
    let mut all_chords_proper = true;
    for &chord in horizontal_chords {
        let proper = horizontal_chord_is_proper(component, boundary, chord);
        all_chords_proper &= proper;
        if !proper {
            rejection_reasons.push(CleanRejectionReason::NonProperHorizontalChord(chord.id()));
        }
        if let Ok(endpoints) = horizontal_chord_endpoints(boundary, chord) {
            endpoint_entries.push((ChordRef::Horizontal(chord.id()), endpoints));
        } else {
            rejection_reasons.push(CleanRejectionReason::EndpointNotOnBoundary);
        }
    }
    for &chord in vertical_chords {
        let proper = vertical_chord_is_proper(component, boundary, chord);
        all_chords_proper &= proper;
        if !proper {
            rejection_reasons.push(CleanRejectionReason::NonProperVerticalChord(chord.id()));
        }
        if let Ok(endpoints) = vertical_chord_endpoints(boundary, chord) {
            endpoint_entries.push((ChordRef::Vertical(chord.id()), endpoints));
        } else {
            rejection_reasons.push(CleanRejectionReason::EndpointNotOnBoundary);
        }
    }
    let mut distinct_boundary_endpoints = true;
    for first in 0..endpoint_entries.len() {
        for second in first + 1..endpoint_entries.len() {
            for endpoint in [
                endpoint_entries[first].1.first,
                endpoint_entries[first].1.second,
            ] {
                if endpoint == endpoint_entries[second].1.first
                    || endpoint == endpoint_entries[second].1.second
                {
                    distinct_boundary_endpoints = false;
                    rejection_reasons.push(CleanRejectionReason::SharedBoundaryEndpoint {
                        first: endpoint_entries[first].0,
                        second: endpoint_entries[second].0,
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

fn horizontal_chord_is_proper<C>(
    component: &GridComponent<C>,
    boundary: &Boundary,
    chord: HorizontalChord,
) -> bool {
    horizontal_chord_endpoints(boundary, chord).is_ok()
        && usize::try_from(chord.left()).is_ok()
        && usize::try_from(chord.right()).is_ok()
        && usize::try_from(chord.y()).is_ok()
        && (chord.left()..chord.right()).all(|x| {
            let Ok(x) = usize::try_from(x) else {
                return false;
            };
            let Ok(y) = usize::try_from(chord.y()) else {
                return false;
            };
            y > 0 && component.contains_cell(x, y - 1) && component.contains_cell(x, y)
        })
}

fn vertical_chord_is_proper<C>(
    component: &GridComponent<C>,
    boundary: &Boundary,
    chord: VerticalChord,
) -> bool {
    vertical_chord_endpoints(boundary, chord).is_ok()
        && usize::try_from(chord.x()).is_ok()
        && usize::try_from(chord.bottom()).is_ok()
        && usize::try_from(chord.top()).is_ok()
        && (chord.bottom()..chord.top()).all(|y| {
            let Ok(x) = usize::try_from(chord.x()) else {
                return false;
            };
            let Ok(y) = usize::try_from(y) else {
                return false;
            };
            x > 0 && component.contains_cell(x - 1, y) && component.contains_cell(x, y)
        })
}

pub trait EffectiveChordEnumerator {
    /// Enumerates the effective chord families for a supported component.
    ///
    /// # Errors
    ///
    /// Returns [`SgError`] when a chord coordinate cannot be represented or
    /// chord construction fails.
    fn enumerate<C>(
        &self,
        component: &GridComponent<C>,
        boundary: &Boundary,
    ) -> Result<EffectiveChordFamilies, SgError>;

    /// Enumerates chords from geometry already prepared for this solve.
    ///
    /// # Errors
    ///
    /// Returns [`SgError`] under the same conditions as [`Self::enumerate`].
    fn enumerate_prepared<C>(
        &self,
        context: &PreparedComponentContext<'_, C>,
    ) -> Result<EffectiveChordFamilies, SgError> {
        self.enumerate(context.component, &context.boundary)
    }

    fn name(&self) -> &'static str;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReferencePairwiseEnumerator;

#[derive(Clone, Copy, Debug, Default)]
pub struct GridInteriorRunEnumerator;

/// Extracts the supported boundary and effective chord families without
/// constructing the conflict graph or running a matching algorithm.
///
/// # Errors
///
/// Returns [`SgError`] when boundary extraction, topology validation, or
/// effective-chord enumeration fails.
pub fn analyze_geometry<C>(component: &GridComponent<C>) -> Result<SgGeometry, SgError> {
    analyze_geometry_with(component, &ReferencePairwiseEnumerator)
}

/// Extracts geometry with a selected effective-chord enumerator.
///
/// # Errors
///
/// Returns [`SgError`] when boundary extraction or enumeration fails.
pub fn analyze_geometry_with<C, E: EffectiveChordEnumerator>(
    component: &GridComponent<C>,
    enumerator: &E,
) -> Result<SgGeometry, SgError> {
    let context = PreparedComponentContext::new(component)?;
    analyze_prepared_geometry(context, enumerator)
}

/// Extracts geometry from a context prepared exactly once for this solve.
///
/// # Errors
///
/// Returns [`SgError`] when topology validation or enumeration fails.
pub fn analyze_prepared_geometry<C, E: EffectiveChordEnumerator>(
    context: PreparedComponentContext<'_, C>,
    enumerator: &E,
) -> Result<SgGeometry, SgError> {
    if context.boundary.outer_loop_count() != 1 {
        return Err(SgError::UnsupportedBoundaryTopology {
            outer_loops: context.boundary.outer_loop_count(),
        });
    }
    let enumeration_started = Instant::now();
    let families = enumerator.enumerate_prepared(&context)?;
    let effective_chord_enumeration_microseconds = enumeration_started.elapsed().as_micros();
    Ok(SgGeometry {
        boundary: context.boundary,
        prepared: context.prepared,
        horizontal_chords: families.horizontal,
        vertical_chords: families.vertical,
        horizontal_interior_run_count: families.horizontal_interior_run_count,
        vertical_interior_run_count: families.vertical_interior_run_count,
        candidate_reflex_pair_count: families.candidate_reflex_pair_count,
        prepared_component_build_microseconds: context.prepared_component_build_microseconds,
        boundary_extraction_microseconds: context.boundary_extraction_microseconds,
        reflex_grouping_microseconds: context.reflex_grouping_microseconds,
        effective_chord_enumeration_microseconds,
    })
}

/// Builds and verifies the complete explicit classical reduction.
///
/// # Errors
///
/// Returns [`SgError`] when a boundary, chord, graph, or formula invariant fails.
pub fn analyze<C>(component: &GridComponent<C>) -> Result<SgAnalysis, SgError> {
    analyze_with(component, &ReferencePairwiseEnumerator)
}

/// Builds the complete explicit reduction with a selected chord enumerator.
///
/// # Errors
///
/// Returns [`SgError`] when geometry, graph construction, matching, or
/// certificate validation fails.
pub fn analyze_with<C, E: EffectiveChordEnumerator>(
    component: &GridComponent<C>,
    enumerator: &E,
) -> Result<SgAnalysis, SgError> {
    let geometry = analyze_geometry_with(component, enumerator)?;
    let boundary = geometry.boundary;
    let prepared = geometry.prepared;
    let horizontal_chords = geometry.horizontal_chords;
    let vertical_chords = geometry.vertical_chords;
    let conflict_graph = build_conflict_graph(&horizontal_chords, &vertical_chords)?;
    let matching = hopcroft_karp(&conflict_graph);
    let vertex_cover = minimum_vertex_cover(&conflict_graph, &matching);
    let selected_horizontal = vertex_cover.left.iter().map(|selected| !selected).collect();
    let selected_vertical = vertex_cover
        .right
        .iter()
        .map(|selected| !selected)
        .collect();
    let chord_count = horizontal_chords.len() + vertical_chords.len();
    let independent_count = chord_count
        .checked_sub(matching.size)
        .ok_or(SgError::FormulaUnderflow)?;
    let base = boundary
        .reflex_vertices
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_sub(boundary.hole_count()))
        .ok_or(SgError::FormulaUnderflow)?;
    let optimum_rectangle_count = base
        .checked_sub(independent_count)
        .ok_or(SgError::FormulaUnderflow)?;

    let analysis = SgAnalysis {
        boundary,
        prepared,
        horizontal_chords,
        vertical_chords,
        conflict_graph,
        matching,
        vertex_cover,
        selected_horizontal,
        selected_vertical,
        optimum_rectangle_count,
    };
    validate_analysis(component, &analysis)?;
    Ok(analysis)
}

/// Solves a supported grid component through explicit SG matching and completion.
///
/// # Errors
///
/// Returns [`SgError`] when reduction, certificate, completion, or output validation fails.
pub fn solve<C>(component: &GridComponent<C>) -> Result<DissectionResult, SgError> {
    let started = Instant::now();
    let analysis = analyze(component)?;
    let analyzed_at = Instant::now();
    let completion = complete_with_backend(
        component,
        &analysis.horizontal_chords,
        &analysis.vertical_chords,
        &analysis.selected_horizontal,
        &analysis.selected_vertical,
        &ReferenceRescanCompletion,
    )?;
    let completed_at = Instant::now();
    if completion.rectangles.len() != analysis.optimum_rectangle_count {
        return Err(SgError::CompletionCount {
            expected: analysis.optimum_rectangle_count,
            actual: completion.rectangles.len(),
        });
    }

    let selected_horizontal = analysis
        .selected_horizontal
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let selected_vertical = analysis
        .selected_vertical
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let matching_edges = analysis
        .matching
        .left_to_right
        .iter()
        .enumerate()
        .filter_map(|(left, right)| right.map(|right| (left, right)))
        .collect::<Vec<_>>();

    let result = DissectionResult {
        optimum_rectangle_count: analysis.optimum_rectangle_count,
        rectangles: completion.rectangles,
        diagnostics: Diagnostics {
            cell_count: component.cell_count(),
            boundary_complexity: analysis.boundary.boundary_complexity(),
            outer_loop_count: analysis.boundary.outer_loop_count(),
            hole_count: analysis.boundary.hole_count(),
            reflex_vertex_count: analysis.boundary.reflex_vertices.len(),
            horizontal_chord_count: analysis.horizontal_chords.len(),
            vertical_chord_count: analysis.vertical_chords.len(),
            total_chord_count: analysis.horizontal_chords.len() + analysis.vertical_chords.len(),
            explicit_conflict_edge_count: Some(analysis.conflict_graph.edge_count()),
            conflict_edge_density: ExactRatio::new(
                analysis.conflict_graph.edge_count() as u128,
                (analysis.horizontal_chords.len() as u128)
                    * (analysis.vertical_chords.len() as u128),
            ),
            maximum_matching_size: analysis.matching.size,
            minimum_vertex_cover_size: analysis.vertex_cover.size,
            output_rectangle_count: analysis.optimum_rectangle_count,
            phase_microseconds: completion_phase_timings(
                analyzed_at.duration_since(started).as_micros(),
                completed_at.duration_since(analyzed_at).as_micros(),
                &completion.metrics,
            ),
            ..completion_diagnostics(&completion.metrics, ReferenceRescanCompletion.name())
        },
        certificate: Some(Certificate {
            kind: "soltan-gorpinevich-explicit".to_owned(),
            payload: json!({
                "horizontal_chords": analysis.horizontal_chords,
                "vertical_chords": analysis.vertical_chords,
                "matching_edges": matching_edges,
                "cover_left": analysis.vertex_cover.left,
                "cover_right": analysis.vertex_cover.right,
                "selected_horizontal": selected_horizontal,
                "selected_vertical": selected_vertical,
                "formula": {
                    "reflex_vertices": analysis.boundary.reflex_vertices.len(),
                    "holes": analysis.boundary.hole_count(),
                    "effective_chords": analysis.horizontal_chords.len() + analysis.vertical_chords.len(),
                    "matching": analysis.matching.size,
                    "rectangles": analysis.optimum_rectangle_count,
                }
            }),
        }),
    };
    validate_dissection(component, &result)?;
    Ok(result)
}

fn completion_diagnostics(metrics: &CompletionMetrics, backend: &str) -> Diagnostics {
    Diagnostics {
        completion_backend: Some(backend.to_owned()),
        selected_chord_cut_materialization_microseconds: Some(
            metrics.selected_chord_cut_materialization_microseconds,
        ),
        horizontal_simple_chord_completion_microseconds: Some(
            metrics.horizontal_simple_chord_completion_microseconds,
        ),
        vertical_simple_chord_completion_microseconds: Some(
            metrics.vertical_simple_chord_completion_microseconds,
        ),
        rectangle_recovery_microseconds: Some(metrics.rectangle_recovery_microseconds),
        final_output_validation_microseconds: Some(metrics.final_output_validation_microseconds),
        initial_horizontal_unit_cut_count: Some(metrics.initial_horizontal_unit_cut_count),
        initial_vertical_unit_cut_count: Some(metrics.initial_vertical_unit_cut_count),
        added_horizontal_unit_cut_count: Some(metrics.added_horizontal_unit_cut_count),
        added_vertical_unit_cut_count: Some(metrics.added_vertical_unit_cut_count),
        horizontal_simple_chord_count: Some(metrics.horizontal_simple_chord_count),
        vertical_simple_chord_count: Some(metrics.vertical_simple_chord_count),
        completion_candidate_queries: Some(metrics.concave_candidate_queries),
        completion_full_grid_scans: Some(metrics.full_grid_vertex_scans),
        completion_candidate_revalidations: Some(metrics.candidate_revalidations),
        completion_stale_candidates: Some(metrics.stale_candidate_count),
        completion_ray_extension_unit_steps: Some(metrics.ray_extension_unit_steps),
        rectangle_recovery_component_visits: Some(metrics.rectangle_recovery_component_visits),
        ..Diagnostics::default()
    }
}

fn completion_phase_timings(
    prefix_microseconds: u128,
    aggregate_microseconds: u128,
    metrics: &CompletionMetrics,
) -> BTreeMap<String, u128> {
    [
        ("boundary_chords_matching".to_owned(), prefix_microseconds),
        ("geometric_completion".to_owned(), aggregate_microseconds),
        (
            "selected_chord_cut_materialization".to_owned(),
            metrics.selected_chord_cut_materialization_microseconds,
        ),
        (
            "horizontal_simple_chord_completion".to_owned(),
            metrics.horizontal_simple_chord_completion_microseconds,
        ),
        (
            "vertical_simple_chord_completion".to_owned(),
            metrics.vertical_simple_chord_completion_microseconds,
        ),
        (
            "rectangle_recovery".to_owned(),
            metrics.rectangle_recovery_microseconds,
        ),
        (
            "final_output_validation".to_owned(),
            metrics.final_output_validation_microseconds,
        ),
    ]
    .into_iter()
    .collect()
}

/// Enumerates Definition 7 effective chords for an ordinary grid-cell polygon.
///
/// # Errors
///
/// Returns [`SgError`] when exact coordinates cannot be converted or chord
/// construction fails.
pub fn enumerate_effective_chords<C>(
    component: &GridComponent<C>,
    boundary: &Boundary,
) -> Result<(Vec<HorizontalChord>, Vec<VerticalChord>), SgError> {
    let families = ReferencePairwiseEnumerator.enumerate(component, boundary)?;
    Ok((families.horizontal, families.vertical))
}

impl EffectiveChordEnumerator for ReferencePairwiseEnumerator {
    fn enumerate<C>(
        &self,
        component: &GridComponent<C>,
        boundary: &Boundary,
    ) -> Result<EffectiveChordFamilies, SgError> {
        let points = boundary
            .reflex_vertices
            .iter()
            .map(|vertex| vertex.point)
            .collect::<Vec<_>>();
        let mut horizontal_records = BTreeSet::new();
        let mut vertical_records = BTreeSet::new();
        for first_index in 0..points.len() {
            for second_index in (first_index + 1)..points.len() {
                let first = points[first_index];
                let second = points[second_index];
                if first.y == second.y {
                    let left = first.x.min(second.x);
                    let right = first.x.max(second.x);
                    if horizontal_open_interval_is_interior(component, left, right, first.y)? {
                        horizontal_records.insert((first.y, left, right));
                    }
                }
                if first.x == second.x {
                    let bottom = first.y.min(second.y);
                    let top = first.y.max(second.y);
                    if vertical_open_interval_is_interior(component, first.x, bottom, top)? {
                        vertical_records.insert((first.x, bottom, top));
                    }
                }
            }
        }

        let horizontal_chords = horizontal_records
            .into_iter()
            .enumerate()
            .map(|(index, (y, left, right))| {
                HorizontalChord::new(HorizontalChordId(index), left, right, y)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let vertical_chords = vertical_records
            .into_iter()
            .enumerate()
            .map(|(index, (x, bottom, top))| {
                VerticalChord::new(VerticalChordId(index), x, bottom, top)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EffectiveChordFamilies {
            horizontal: horizontal_chords,
            vertical: vertical_chords,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(points.len().saturating_sub(1) * points.len() / 2),
        })
    }

    fn name(&self) -> &'static str {
        "reference-pairwise"
    }
}

impl EffectiveChordEnumerator for GridInteriorRunEnumerator {
    fn enumerate<C>(
        &self,
        component: &GridComponent<C>,
        _boundary: &Boundary,
    ) -> Result<EffectiveChordFamilies, SgError> {
        let context = PreparedComponentContext::new(component)?;
        self.enumerate_prepared(&context)
    }

    fn enumerate_prepared<C>(
        &self,
        context: &PreparedComponentContext<'_, C>,
    ) -> Result<EffectiveChordFamilies, SgError> {
        let mut horizontal_records = BTreeSet::new();
        let mut vertical_records = BTreeSet::new();
        let mut horizontal_interior_run_count = 0;
        let mut vertical_interior_run_count = 0;
        let mut candidate_reflex_pair_count = 0;
        for (&y, xs) in &context.reflex_by_row {
            let y_index = coordinate_to_usize(y)?;
            let Some(local_y) = y_index.checked_sub(context.prepared.y0) else {
                continue;
            };
            let Some(runs) = context.prepared.horizontal_interior_runs.get(local_y) else {
                continue;
            };
            horizontal_interior_run_count += runs.len();
            for &(left_run, right_run) in runs {
                let left_run = Coord::try_from(left_run)
                    .map_err(|_| SgError::CoordinateConversion { value: Coord::MAX })?;
                let right_run = Coord::try_from(right_run)
                    .map_err(|_| SgError::CoordinateConversion { value: Coord::MAX })?;
                let begin = xs.partition_point(|&x| x < left_run);
                let end = xs.partition_point(|&x| x <= right_run);
                let aligned = &xs[begin..end];
                for (index, &left) in aligned.iter().enumerate() {
                    for &right in &aligned[index + 1..] {
                        candidate_reflex_pair_count += 1;
                        horizontal_records.insert((y, left, right));
                    }
                }
            }
        }
        for (&x, ys) in &context.reflex_by_column {
            let x_index = coordinate_to_usize(x)?;
            let Some(local_x) = x_index.checked_sub(context.prepared.x0) else {
                continue;
            };
            let Some(runs) = context.prepared.vertical_interior_runs.get(local_x) else {
                continue;
            };
            vertical_interior_run_count += runs.len();
            for &(bottom_run, top_run) in runs {
                let bottom_run = Coord::try_from(bottom_run)
                    .map_err(|_| SgError::CoordinateConversion { value: Coord::MAX })?;
                let top_run = Coord::try_from(top_run)
                    .map_err(|_| SgError::CoordinateConversion { value: Coord::MAX })?;
                let begin = ys.partition_point(|&y| y < bottom_run);
                let end = ys.partition_point(|&y| y <= top_run);
                let aligned = &ys[begin..end];
                for (index, &bottom) in aligned.iter().enumerate() {
                    for &top in &aligned[index + 1..] {
                        candidate_reflex_pair_count += 1;
                        vertical_records.insert((x, bottom, top));
                    }
                }
            }
        }
        let horizontal = horizontal_records
            .into_iter()
            .enumerate()
            .map(|(index, (y, left, right))| {
                HorizontalChord::new(HorizontalChordId(index), left, right, y)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let vertical = vertical_records
            .into_iter()
            .enumerate()
            .map(|(index, (x, bottom, top))| {
                VerticalChord::new(VerticalChordId(index), x, bottom, top)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EffectiveChordFamilies {
            horizontal,
            vertical,
            horizontal_interior_run_count: Some(horizontal_interior_run_count),
            vertical_interior_run_count: Some(vertical_interior_run_count),
            candidate_reflex_pair_count: Some(candidate_reflex_pair_count),
        })
    }

    fn name(&self) -> &'static str {
        "grid-interior-runs"
    }
}

fn horizontal_open_interval_is_interior<C>(
    component: &GridComponent<C>,
    left: Coord,
    right: Coord,
    y: Coord,
) -> Result<bool, SgError> {
    let left = coordinate_to_usize(left)?;
    let right = coordinate_to_usize(right)?;
    let y = coordinate_to_usize(y)?;
    if y == 0 {
        return Ok(false);
    }
    Ok((left..right).all(|x| component.contains_cell(x, y - 1) && component.contains_cell(x, y)))
}

fn vertical_open_interval_is_interior<C>(
    component: &GridComponent<C>,
    x: Coord,
    bottom: Coord,
    top: Coord,
) -> Result<bool, SgError> {
    let x = coordinate_to_usize(x)?;
    let bottom = coordinate_to_usize(bottom)?;
    let top = coordinate_to_usize(top)?;
    if x == 0 {
        return Ok(false);
    }
    Ok((bottom..top).all(|y| component.contains_cell(x - 1, y) && component.contains_cell(x, y)))
}

fn coordinate_to_usize(value: Coord) -> Result<usize, SgError> {
    usize::try_from(value).map_err(|_| SgError::CoordinateConversion { value })
}

/// Builds all closed horizontal--vertical chord intersections explicitly.
///
/// # Errors
///
/// Returns [`SgError`] if an endpoint cannot be inserted into declared graph dimensions.
pub fn build_conflict_graph(
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
) -> Result<BipartiteGraph, SgError> {
    let mut graph = BipartiteGraph::new(horizontal_chords.len(), vertical_chords.len());
    for (left, &horizontal) in horizontal_chords.iter().enumerate() {
        for (right, &vertical) in vertical_chords.iter().enumerate() {
            if closed_chords_intersect(horizontal, vertical) {
                graph.add_edge(left, right)?;
            }
        }
    }
    Ok(graph)
}

/// Checks chord validity, cover completeness, independence, and Konig equality.
///
/// # Errors
///
/// Returns [`SgError`] on the first invalid certificate or geometric object.
pub fn validate_analysis<C>(
    component: &GridComponent<C>,
    analysis: &SgAnalysis,
) -> Result<(), SgError> {
    let reflex_points = analysis
        .boundary
        .reflex_vertices
        .iter()
        .map(|vertex| vertex.point)
        .collect::<HashSet<_>>();
    for &chord in &analysis.horizontal_chords {
        if !reflex_points.contains(&Point::new(chord.left(), chord.y()))
            || !reflex_points.contains(&Point::new(chord.right(), chord.y()))
            || !horizontal_open_interval_is_interior(
                component,
                chord.left(),
                chord.right(),
                chord.y(),
            )?
        {
            return Err(SgError::InvalidEffectiveChord);
        }
    }
    for &chord in &analysis.vertical_chords {
        if !reflex_points.contains(&Point::new(chord.x(), chord.bottom()))
            || !reflex_points.contains(&Point::new(chord.x(), chord.top()))
            || !vertical_open_interval_is_interior(
                component,
                chord.x(),
                chord.bottom(),
                chord.top(),
            )?
        {
            return Err(SgError::InvalidEffectiveChord);
        }
    }
    for (left, right) in analysis.conflict_graph.edges() {
        if !analysis.vertex_cover.left[left] && !analysis.vertex_cover.right[right] {
            return Err(SgError::UncoveredConflictEdge { left, right });
        }
        if analysis.selected_horizontal[left] && analysis.selected_vertical[right] {
            return Err(SgError::NonIndependentSelection { left, right });
        }
    }
    if analysis.matching.size != analysis.vertex_cover.size {
        return Err(SgError::MatchingCoverMismatch {
            matching: analysis.matching.size,
            cover: analysis.vertex_cover.size,
        });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct HorizontalUnitCut {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct VerticalUnitCut {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompletionMetrics {
    pub selected_chord_cut_materialization_microseconds: u128,
    pub horizontal_simple_chord_completion_microseconds: u128,
    pub vertical_simple_chord_completion_microseconds: u128,
    pub rectangle_recovery_microseconds: u128,
    pub final_output_validation_microseconds: u128,
    pub initial_horizontal_unit_cut_count: usize,
    pub initial_vertical_unit_cut_count: usize,
    pub added_horizontal_unit_cut_count: usize,
    pub added_vertical_unit_cut_count: usize,
    pub horizontal_simple_chord_count: usize,
    pub vertical_simple_chord_count: usize,
    pub concave_candidate_queries: usize,
    pub full_grid_vertex_scans: usize,
    pub candidate_revalidations: usize,
    pub stale_candidate_count: usize,
    pub ray_extension_unit_steps: usize,
    pub rectangle_recovery_component_visits: usize,
    pub rectangle_recovery_queue_pushes: usize,
    pub rectangle_recovery_region_count: usize,
    pub rectangle_recovery_allocations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompletionResult {
    pub rectangles: Vec<GridRect>,
    pub selected_horizontal_unit_cuts: Vec<HorizontalUnitCut>,
    pub selected_vertical_unit_cuts: Vec<VerticalUnitCut>,
    pub added_horizontal_unit_cuts: Vec<HorizontalUnitCut>,
    pub added_vertical_unit_cuts: Vec<VerticalUnitCut>,
    pub metrics: CompletionMetrics,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RectangleRecoveryResult {
    pub rectangles: Vec<GridRect>,
    pub cell_visits: usize,
    pub queue_pushes: usize,
    pub region_count: usize,
    pub allocations: usize,
}

pub trait RectangleRecoveryBackend {
    /// Recovers canonical rectangles from prepared occupancy and dense cuts.
    ///
    /// # Errors
    ///
    /// Returns [`SgError`] when a cut region is not rectangular.
    fn recover(
        &self,
        prepared: &PreparedGridComponent,
        cuts: &DenseCutGrid,
    ) -> Result<RectangleRecoveryResult, SgError>;

    fn name(&self) -> &'static str;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReferenceHashBfsRecovery;

#[derive(Clone, Copy, Debug, Default)]
pub struct DenseGridRecovery;

pub trait GeometricCompletionBackend {
    /// Completes the selected effective chords into a rectangular dissection.
    ///
    /// # Errors
    ///
    /// Returns [`SgError`] for invalid selections or completion invariants.
    fn complete<C>(
        &self,
        component: &GridComponent<C>,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<CompletionResult, SgError>;

    /// Completes using occupancy and runs already prepared for this solve.
    ///
    /// # Errors
    ///
    /// Returns [`SgError`] under the same conditions as [`Self::complete`].
    fn complete_prepared<C>(
        &self,
        component: &GridComponent<C>,
        _prepared: &PreparedGridComponent,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<CompletionResult, SgError> {
        self.complete(
            component,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
        )
    }

    fn name(&self) -> &'static str;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ReferenceRescanCompletion;

#[derive(Clone, Copy, Debug, Default)]
pub struct IndexedFrontierCompletion;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompletionBackendKind {
    ReferenceRescan,
    IndexedFrontier,
}

impl CompletionBackendKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferenceRescan => "reference-rescan",
            Self::IndexedFrontier => "indexed-frontier",
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ReferenceOrderedCuts {
    horizontal: BTreeSet<HorizontalUnitCut>,
    vertical: BTreeSet<VerticalUnitCut>,
}

impl ReferenceOrderedCuts {
    fn from_selection(
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<Self, SgError> {
        let mut cuts = Self::default();
        for (index, &selected) in selected_horizontal.iter().enumerate() {
            if selected {
                let chord = horizontal_chords[index];
                let left = coordinate_to_usize(chord.left())?;
                let right = coordinate_to_usize(chord.right())?;
                let y = coordinate_to_usize(chord.y())?;
                cuts.horizontal
                    .extend((left..right).map(|x| HorizontalUnitCut { x, y }));
            }
        }
        for (index, &selected) in selected_vertical.iter().enumerate() {
            if selected {
                let chord = vertical_chords[index];
                let x = coordinate_to_usize(chord.x())?;
                let bottom = coordinate_to_usize(chord.bottom())?;
                let top = coordinate_to_usize(chord.top())?;
                cuts.vertical
                    .extend((bottom..top).map(|y| VerticalUnitCut { x, y }));
            }
        }
        Ok(cuts)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DenseCutGrid {
    x0: usize,
    y0: usize,
    width: usize,
    height: usize,
    horizontal: Vec<bool>,
    vertical: Vec<bool>,
}

impl DenseCutGrid {
    fn from_selection(
        prepared: &PreparedGridComponent,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<Self, SgError> {
        let mut cuts = Self {
            x0: prepared.x0,
            y0: prepared.y0,
            width: prepared.width(),
            height: prepared.height(),
            horizontal: vec![false; prepared.width() * (prepared.height() + 1)],
            vertical: vec![false; (prepared.width() + 1) * prepared.height()],
        };
        for (index, &selected) in selected_horizontal.iter().enumerate() {
            if selected {
                let chord = horizontal_chords[index];
                let y = coordinate_to_usize(chord.y())?;
                for x in coordinate_to_usize(chord.left())?..coordinate_to_usize(chord.right())? {
                    cuts.insert_horizontal(HorizontalUnitCut { x, y });
                }
            }
        }
        for (index, &selected) in selected_vertical.iter().enumerate() {
            if selected {
                let chord = vertical_chords[index];
                let x = coordinate_to_usize(chord.x())?;
                for y in coordinate_to_usize(chord.bottom())?..coordinate_to_usize(chord.top())? {
                    cuts.insert_vertical(VerticalUnitCut { x, y });
                }
            }
        }
        Ok(cuts)
    }

    fn contains_horizontal(&self, x: usize, y: usize) -> bool {
        x >= self.x0
            && x < self.x0 + self.width
            && y >= self.y0
            && y <= self.y0 + self.height
            && self.horizontal[(y - self.y0) * self.width + x - self.x0]
    }

    fn contains_vertical(&self, x: usize, y: usize) -> bool {
        x >= self.x0
            && x <= self.x0 + self.width
            && y >= self.y0
            && y < self.y0 + self.height
            && self.vertical[(y - self.y0) * (self.width + 1) + x - self.x0]
    }

    fn insert_horizontal(&mut self, cut: HorizontalUnitCut) -> bool {
        if cut.x < self.x0
            || cut.x >= self.x0 + self.width
            || cut.y < self.y0
            || cut.y > self.y0 + self.height
        {
            return false;
        }
        let index = (cut.y - self.y0) * self.width + cut.x - self.x0;
        let inserted = !self.horizontal[index];
        self.horizontal[index] = true;
        inserted
    }

    fn insert_vertical(&mut self, cut: VerticalUnitCut) -> bool {
        if cut.x < self.x0
            || cut.x > self.x0 + self.width
            || cut.y < self.y0
            || cut.y >= self.y0 + self.height
        {
            return false;
        }
        let index = (cut.y - self.y0) * (self.width + 1) + cut.x - self.x0;
        let inserted = !self.vertical[index];
        self.vertical[index] = true;
        inserted
    }

    fn horizontal_cuts(&self) -> Vec<HorizontalUnitCut> {
        let mut cuts = self
            .horizontal
            .iter()
            .enumerate()
            .filter_map(|(index, &present)| {
                present.then_some(HorizontalUnitCut {
                    x: self.x0 + index % self.width,
                    y: self.y0 + index / self.width,
                })
            })
            .collect::<Vec<_>>();
        cuts.sort_unstable();
        cuts
    }

    fn vertical_cuts(&self) -> Vec<VerticalUnitCut> {
        let mut cuts = self
            .vertical
            .iter()
            .enumerate()
            .filter_map(|(index, &present)| {
                present.then_some(VerticalUnitCut {
                    x: self.x0 + index % (self.width + 1),
                    y: self.y0 + index / (self.width + 1),
                })
            })
            .collect::<Vec<_>>();
        cuts.sort_unstable();
        cuts
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
    const fn order(self) -> usize {
        match self {
            Self::East => 0,
            Self::North => 1,
            Self::West => 2,
            Self::South => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FrontierCandidate {
    y: usize,
    x: usize,
    direction_order: usize,
    generation: u64,
    direction: Direction,
}

#[derive(Clone, Debug)]
struct CompletionState<'a> {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    width: usize,
    height: usize,
    prepared: &'a PreparedGridComponent,
    cuts: DenseCutGrid,
    generations: Vec<u64>,
    frontier: BinaryHeap<Reverse<FrontierCandidate>>,
}

impl<'a> CompletionState<'a> {
    fn new(prepared: &'a PreparedGridComponent, cuts: DenseCutGrid) -> Self {
        let (x0, y0, x1, y1) = (prepared.x0, prepared.y0, prepared.x1, prepared.y1);
        let width = x1 - x0;
        let height = y1 - y0;
        Self {
            x0,
            y0,
            x1,
            y1,
            width,
            height,
            prepared,
            cuts,
            generations: vec![0; (width + 1) * (height + 1)],
            frontier: BinaryHeap::new(),
        }
    }

    fn contains_cell(&self, x: usize, y: usize) -> bool {
        x >= self.x0
            && x < self.x1
            && y >= self.y0
            && y < self.y1
            && self.prepared.contains_cell(x, y)
    }

    fn vertex_index(&self, x: usize, y: usize) -> usize {
        (y - self.y0) * (self.width + 1) + x - self.x0
    }

    fn horizontal_cut(&self, x: usize, y: usize) -> bool {
        x >= self.x0
            && x < self.x1
            && y >= self.y0
            && y <= self.y1
            && self.cuts.contains_horizontal(x, y)
    }

    fn vertical_cut(&self, x: usize, y: usize) -> bool {
        x >= self.x0
            && x <= self.x1
            && y >= self.y0
            && y < self.y1
            && self.cuts.contains_vertical(x, y)
    }

    fn local_quadrants(&self, x: usize, y: usize) -> [bool; 4] {
        [
            x > 0 && y > 0 && self.contains_cell(x - 1, y - 1),
            y > 0 && self.contains_cell(x, y - 1),
            self.contains_cell(x, y),
            x > 0 && self.contains_cell(x - 1, y),
        ]
    }

    fn local_blocked_rays(&self, inside: [bool; 4], x: usize, y: usize) -> [bool; 4] {
        [
            self.horizontal_cut(x, y) || inside[1] != inside[2],
            self.vertical_cut(x, y) || inside[2] != inside[3],
            (x > 0 && self.horizontal_cut(x - 1, y)) || inside[3] != inside[0],
            (y > 0 && self.vertical_cut(x, y - 1)) || inside[0] != inside[1],
        ]
    }

    fn candidate_valid(
        &self,
        x: usize,
        y: usize,
        direction: Direction,
        metrics: &mut CompletionMetrics,
    ) -> bool {
        metrics.concave_candidate_queries += 1;
        let inside = self.local_quadrants(x, y);
        let blocked = self.local_blocked_rays(inside, x, y);
        if !blocked.iter().any(|&value| value) {
            return false;
        }
        let (roots, sizes) = local_angle_components(inside, blocked);
        let (ray, first, second) = match direction {
            Direction::East => (0, 1, 2),
            Direction::North => (1, 2, 3),
            Direction::West => (2, 3, 0),
            Direction::South => (3, 0, 1),
        };
        inside[first]
            && inside[second]
            && !blocked[ray]
            && roots[first] == roots[second]
            && sizes[roots[first]] >= 3
    }

    fn enqueue_vertex(
        &mut self,
        x: usize,
        y: usize,
        horizontal: bool,
        metrics: &mut CompletionMetrics,
    ) {
        if x < self.x0 || x > self.x1 || y < self.y0 || y > self.y1 {
            return;
        }
        let index = self.vertex_index(x, y);
        let generation = self.generations[index];
        for direction in [
            Direction::East,
            Direction::North,
            Direction::West,
            Direction::South,
        ] {
            if direction.is_horizontal() == horizontal
                && self.candidate_valid(x, y, direction, metrics)
            {
                self.frontier.push(Reverse(FrontierCandidate {
                    y,
                    x,
                    direction_order: direction.order(),
                    generation,
                    direction,
                }));
            }
        }
    }

    fn refresh_vertex(
        &mut self,
        x: usize,
        y: usize,
        horizontal: bool,
        metrics: &mut CompletionMetrics,
    ) {
        if x < self.x0 || x > self.x1 || y < self.y0 || y > self.y1 {
            return;
        }
        let index = self.vertex_index(x, y);
        self.generations[index] = self.generations[index].wrapping_add(1);
        self.enqueue_vertex(x, y, horizontal, metrics);
    }

    fn initialize_frontier(&mut self, horizontal: bool, metrics: &mut CompletionMetrics) {
        self.frontier.clear();
        self.generations.fill(0);
        metrics.full_grid_vertex_scans += 1;
        for y in self.y0..=self.y1 {
            for x in self.x0..=self.x1 {
                self.enqueue_vertex(x, y, horizontal, metrics);
            }
        }
    }

    fn pop_candidate(&mut self, metrics: &mut CompletionMetrics) -> Option<FrontierCandidate> {
        while let Some(Reverse(candidate)) = self.frontier.pop() {
            metrics.candidate_revalidations += 1;
            let generation = self.generations[self.vertex_index(candidate.x, candidate.y)];
            if candidate.generation != generation
                || !self.candidate_valid(candidate.x, candidate.y, candidate.direction, metrics)
            {
                metrics.stale_candidate_count += 1;
                continue;
            }
            return Some(candidate);
        }
        None
    }

    fn perpendicular_boundary_at(&self, point: (usize, usize), horizontal_chord: bool) -> bool {
        let (x, y) = point;
        let inside = self.local_quadrants(x, y);
        if horizontal_chord {
            self.vertical_cut(x, y)
                || (y > 0 && self.vertical_cut(x, y - 1))
                || inside[2] != inside[3]
                || inside[0] != inside[1]
        } else {
            self.horizontal_cut(x, y)
                || (x > 0 && self.horizontal_cut(x - 1, y))
                || inside[1] != inside[2]
                || inside[3] != inside[0]
        }
    }
}

impl Direction {
    const fn is_horizontal(self) -> bool {
        matches!(self, Self::East | Self::West)
    }
}

/// Performs the classical horizontal-then-vertical simple-chord completion.
///
/// # Errors
///
/// Returns [`SgError`] for dimension mismatches, invalid simple chords,
/// nontermination, or nonrectangular final regions.
pub fn complete_with_selected_chords<C>(
    component: &GridComponent<C>,
    analysis: &SgAnalysis,
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
) -> Result<Vec<GridRect>, SgError> {
    complete_with_chord_families(
        component,
        &analysis.horizontal_chords,
        &analysis.vertical_chords,
        selected_horizontal,
        selected_vertical,
    )
}

/// Performs geometric completion from chord families without requiring an
/// explicit conflict graph or matching object.
///
/// # Errors
///
/// Returns [`SgError`] for dimension mismatches, invalid simple chords,
/// nontermination, or nonrectangular final regions.
pub fn complete_with_chord_families<C>(
    component: &GridComponent<C>,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
) -> Result<Vec<GridRect>, SgError> {
    Ok(complete_with_backend(
        component,
        horizontal_chords,
        vertical_chords,
        selected_horizontal,
        selected_vertical,
        &ReferenceRescanCompletion,
    )?
    .rectangles)
}

/// Performs geometric completion with an explicitly selected backend.
///
/// # Errors
///
/// Returns [`SgError`] for dimension mismatches or completion invariants.
pub fn complete_with_backend<C, B: GeometricCompletionBackend>(
    component: &GridComponent<C>,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
    backend: &B,
) -> Result<CompletionResult, SgError> {
    backend.complete(
        component,
        horizontal_chords,
        vertical_chords,
        selected_horizontal,
        selected_vertical,
    )
}

/// Performs geometric completion while reusing prepared component geometry.
///
/// # Errors
///
/// Returns [`SgError`] for dimension mismatches or completion invariants.
pub fn complete_with_prepared_backend<C, B: GeometricCompletionBackend>(
    component: &GridComponent<C>,
    prepared: &PreparedGridComponent,
    horizontal_chords: &[HorizontalChord],
    vertical_chords: &[VerticalChord],
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
    backend: &B,
) -> Result<CompletionResult, SgError> {
    backend.complete_prepared(
        component,
        prepared,
        horizontal_chords,
        vertical_chords,
        selected_horizontal,
        selected_vertical,
    )
}

impl GeometricCompletionBackend for ReferenceRescanCompletion {
    fn complete<C>(
        &self,
        component: &GridComponent<C>,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<CompletionResult, SgError> {
        if selected_horizontal.len() != horizontal_chords.len()
            || selected_vertical.len() != vertical_chords.len()
        {
            return Err(SgError::SelectionLengthMismatch);
        }
        let started = Instant::now();
        let mut cuts = ReferenceOrderedCuts::from_selection(
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
        )?;
        let selected_at = Instant::now();
        let selected_horizontal_unit_cuts = cuts.horizontal.iter().copied().collect::<Vec<_>>();
        let selected_vertical_unit_cuts = cuts.vertical.iter().copied().collect::<Vec<_>>();
        let mut metrics = CompletionMetrics {
            selected_chord_cut_materialization_microseconds: selected_at
                .duration_since(started)
                .as_micros(),
            initial_horizontal_unit_cut_count: cuts.horizontal.len(),
            initial_vertical_unit_cut_count: cuts.vertical.len(),
            ..CompletionMetrics::default()
        };
        complete_axis(component, &mut cuts, true, &mut metrics)?;
        let horizontal_at = Instant::now();
        metrics.horizontal_simple_chord_completion_microseconds =
            horizontal_at.duration_since(selected_at).as_micros();
        complete_axis(component, &mut cuts, false, &mut metrics)?;
        let vertical_at = Instant::now();
        metrics.vertical_simple_chord_completion_microseconds =
            vertical_at.duration_since(horizontal_at).as_micros();
        let rectangles = rectangles_from_cuts(component, &cuts, &mut metrics)?;
        let rectangles_at = Instant::now();
        metrics.rectangle_recovery_microseconds =
            rectangles_at.duration_since(vertical_at).as_micros();
        validate_completion_rectangles(component, &rectangles)?;
        metrics.final_output_validation_microseconds = rectangles_at.elapsed().as_micros();
        let added_horizontal_unit_cuts = cuts
            .horizontal
            .iter()
            .filter(|cut| selected_horizontal_unit_cuts.binary_search(cut).is_err())
            .copied()
            .collect::<Vec<_>>();
        let added_vertical_unit_cuts = cuts
            .vertical
            .iter()
            .filter(|cut| selected_vertical_unit_cuts.binary_search(cut).is_err())
            .copied()
            .collect::<Vec<_>>();
        metrics.added_horizontal_unit_cut_count = added_horizontal_unit_cuts.len();
        metrics.added_vertical_unit_cut_count = added_vertical_unit_cuts.len();
        Ok(CompletionResult {
            rectangles,
            selected_horizontal_unit_cuts,
            selected_vertical_unit_cuts,
            added_horizontal_unit_cuts,
            added_vertical_unit_cuts,
            metrics,
        })
    }

    fn name(&self) -> &'static str {
        "reference-rescan"
    }
}

impl GeometricCompletionBackend for IndexedFrontierCompletion {
    fn complete<C>(
        &self,
        component: &GridComponent<C>,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<CompletionResult, SgError> {
        let prepared = PreparedGridComponent::from_component(component)
            .map_err(|_| SgError::CompletionDidNotTerminate)?;
        self.complete_prepared(
            component,
            &prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
        )
    }

    fn complete_prepared<C>(
        &self,
        component: &GridComponent<C>,
        prepared: &PreparedGridComponent,
        horizontal_chords: &[HorizontalChord],
        vertical_chords: &[VerticalChord],
        selected_horizontal: &[bool],
        selected_vertical: &[bool],
    ) -> Result<CompletionResult, SgError> {
        if selected_horizontal.len() != horizontal_chords.len()
            || selected_vertical.len() != vertical_chords.len()
        {
            return Err(SgError::SelectionLengthMismatch);
        }
        let started = Instant::now();
        let cuts = DenseCutGrid::from_selection(
            prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
        )?;
        let selected_at = Instant::now();
        let selected_horizontal_unit_cuts = cuts.horizontal_cuts();
        let selected_vertical_unit_cuts = cuts.vertical_cuts();
        let mut metrics = CompletionMetrics {
            selected_chord_cut_materialization_microseconds: selected_at
                .duration_since(started)
                .as_micros(),
            initial_horizontal_unit_cut_count: selected_horizontal_unit_cuts.len(),
            initial_vertical_unit_cut_count: selected_vertical_unit_cuts.len(),
            ..CompletionMetrics::default()
        };
        let mut state = CompletionState::new(prepared, cuts);
        complete_indexed_axis(&mut state, true, &mut metrics)?;
        let horizontal_at = Instant::now();
        metrics.horizontal_simple_chord_completion_microseconds =
            horizontal_at.duration_since(selected_at).as_micros();
        complete_indexed_axis(&mut state, false, &mut metrics)?;
        let vertical_at = Instant::now();
        metrics.vertical_simple_chord_completion_microseconds =
            vertical_at.duration_since(horizontal_at).as_micros();
        let recovery = DenseGridRecovery.recover(prepared, &state.cuts)?;
        metrics.rectangle_recovery_component_visits = recovery.cell_visits;
        metrics.rectangle_recovery_queue_pushes = recovery.queue_pushes;
        metrics.rectangle_recovery_region_count = recovery.region_count;
        metrics.rectangle_recovery_allocations = recovery.allocations;
        let rectangles = recovery.rectangles;
        let rectangles_at = Instant::now();
        metrics.rectangle_recovery_microseconds =
            rectangles_at.duration_since(vertical_at).as_micros();
        validate_completion_rectangles(component, &rectangles)?;
        metrics.final_output_validation_microseconds = rectangles_at.elapsed().as_micros();
        let all_horizontal_unit_cuts = state.cuts.horizontal_cuts();
        let all_vertical_unit_cuts = state.cuts.vertical_cuts();
        let added_horizontal_unit_cuts = all_horizontal_unit_cuts
            .iter()
            .filter(|cut| selected_horizontal_unit_cuts.binary_search(cut).is_err())
            .copied()
            .collect::<Vec<_>>();
        let added_vertical_unit_cuts = all_vertical_unit_cuts
            .iter()
            .filter(|cut| selected_vertical_unit_cuts.binary_search(cut).is_err())
            .copied()
            .collect::<Vec<_>>();
        metrics.added_horizontal_unit_cut_count = added_horizontal_unit_cuts.len();
        metrics.added_vertical_unit_cut_count = added_vertical_unit_cuts.len();
        Ok(CompletionResult {
            rectangles,
            selected_horizontal_unit_cuts,
            selected_vertical_unit_cuts,
            added_horizontal_unit_cuts,
            added_vertical_unit_cuts,
            metrics,
        })
    }

    fn name(&self) -> &'static str {
        "indexed-frontier"
    }
}

fn complete_indexed_axis(
    state: &mut CompletionState,
    horizontal: bool,
    metrics: &mut CompletionMetrics,
) -> Result<(), SgError> {
    state.initialize_frontier(horizontal, metrics);
    let maximum_unit_cuts = state
        .width
        .checked_mul(state.height + 1)
        .and_then(|horizontal_slots| {
            (state.width + 1)
                .checked_mul(state.height)
                .and_then(|vertical_slots| horizontal_slots.checked_add(vertical_slots))
        })
        .ok_or(SgError::CompletionDidNotTerminate)?;
    for _ in 0..=maximum_unit_cuts {
        let Some(candidate) = state.pop_candidate(metrics) else {
            return Ok(());
        };
        let added = extend_indexed_simple_chord(state, candidate, horizontal, metrics);
        if added == 0 {
            return Err(SgError::InvalidSimpleChord {
                point: (candidate.x, candidate.y),
            });
        }
        if horizontal {
            metrics.horizontal_simple_chord_count += 1;
        } else {
            metrics.vertical_simple_chord_count += 1;
        }
    }
    Err(SgError::CompletionDidNotTerminate)
}

fn extend_indexed_simple_chord(
    state: &mut CompletionState,
    candidate: FrontierCandidate,
    horizontal: bool,
    metrics: &mut CompletionMetrics,
) -> usize {
    let mut horizontal_additions = Vec::new();
    let mut vertical_additions = Vec::new();
    let (mut x, mut y) = (candidate.x, candidate.y);
    loop {
        metrics.ray_extension_unit_steps += 1;
        let next = match candidate.direction {
            Direction::East => {
                if y == 0
                    || !state.contains_cell(x, y - 1)
                    || !state.contains_cell(x, y)
                    || state.horizontal_cut(x, y)
                {
                    break;
                }
                horizontal_additions.push(HorizontalUnitCut { x, y });
                x += 1;
                (x, y)
            }
            Direction::West => {
                if x == 0 || y == 0 {
                    break;
                }
                let unit_x = x - 1;
                if !state.contains_cell(unit_x, y - 1)
                    || !state.contains_cell(unit_x, y)
                    || state.horizontal_cut(unit_x, y)
                {
                    break;
                }
                horizontal_additions.push(HorizontalUnitCut { x: unit_x, y });
                x -= 1;
                (x, y)
            }
            Direction::North => {
                if x == 0
                    || !state.contains_cell(x - 1, y)
                    || !state.contains_cell(x, y)
                    || state.vertical_cut(x, y)
                {
                    break;
                }
                vertical_additions.push(VerticalUnitCut { x, y });
                y += 1;
                (x, y)
            }
            Direction::South => {
                if x == 0 || y == 0 {
                    break;
                }
                let unit_y = y - 1;
                if !state.contains_cell(x - 1, unit_y)
                    || !state.contains_cell(x, unit_y)
                    || state.vertical_cut(x, unit_y)
                {
                    break;
                }
                vertical_additions.push(VerticalUnitCut { x, y: unit_y });
                y -= 1;
                (x, y)
            }
        };
        if state.perpendicular_boundary_at(next, candidate.direction.is_horizontal()) {
            break;
        }
    }
    let added = horizontal_additions.len() + vertical_additions.len();
    let mut affected = Vec::new();
    for cut in horizontal_additions {
        state.cuts.insert_horizontal(cut);
        affected.push((cut.x, cut.y));
        affected.push((cut.x + 1, cut.y));
    }
    for cut in vertical_additions {
        state.cuts.insert_vertical(cut);
        affected.push((cut.x, cut.y));
        affected.push((cut.x, cut.y + 1));
    }
    affected.sort_unstable();
    affected.dedup();
    for (affected_x, affected_y) in affected {
        state.refresh_vertex(affected_x, affected_y, horizontal, metrics);
    }
    added
}

fn validate_completion_rectangles<C>(
    component: &GridComponent<C>,
    rectangles: &[GridRect],
) -> Result<(), SgError> {
    validate_dissection(
        component,
        &DissectionResult {
            optimum_rectangle_count: rectangles.len(),
            rectangles: rectangles.to_vec(),
            diagnostics: Diagnostics::default(),
            certificate: None,
        },
    )?;
    Ok(())
}

fn complete_axis<C>(
    component: &GridComponent<C>,
    cuts: &mut ReferenceOrderedCuts,
    horizontal: bool,
    metrics: &mut CompletionMetrics,
) -> Result<(), SgError> {
    let horizontal_height = component
        .grid_height
        .checked_add(1)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let vertical_width = component
        .grid_width
        .checked_add(1)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let horizontal_slots = component
        .grid_width
        .checked_mul(horizontal_height)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let vertical_slots = vertical_width
        .checked_mul(component.grid_height)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    let maximum_unit_cuts = horizontal_slots
        .checked_add(vertical_slots)
        .ok_or(SgError::CompletionDidNotTerminate)?;
    for _ in 0..=maximum_unit_cuts {
        let Some((point, direction)) = find_concave_ray(component, cuts, horizontal, metrics)
        else {
            return Ok(());
        };
        let added = extend_simple_chord(component, cuts, point, direction, metrics);
        if added == 0 {
            return Err(SgError::InvalidSimpleChord { point });
        }
        if horizontal {
            metrics.horizontal_simple_chord_count += 1;
        } else {
            metrics.vertical_simple_chord_count += 1;
        }
    }
    Err(SgError::CompletionDidNotTerminate)
}

fn find_concave_ray<C>(
    component: &GridComponent<C>,
    cuts: &ReferenceOrderedCuts,
    horizontal: bool,
    metrics: &mut CompletionMetrics,
) -> Option<((usize, usize), Direction)> {
    metrics.full_grid_vertex_scans += 1;
    for y in 0..=component.grid_height {
        for x in 0..=component.grid_width {
            let inside = local_quadrants(component, x, y);
            let blocked = local_blocked_rays(cuts, inside, x, y);
            if !blocked.iter().any(|&value| value) {
                continue;
            }
            let (roots, sizes) = local_angle_components(inside, blocked);
            for (direction, first, second) in [
                (Direction::East, 1, 2),
                (Direction::North, 2, 3),
                (Direction::West, 3, 0),
                (Direction::South, 0, 1),
            ] {
                if direction.is_horizontal() != horizontal {
                    continue;
                }
                metrics.concave_candidate_queries += 1;
                let ray_index = match direction {
                    Direction::East => 0,
                    Direction::North => 1,
                    Direction::West => 2,
                    Direction::South => 3,
                };
                if inside[first]
                    && inside[second]
                    && !blocked[ray_index]
                    && roots[first] == roots[second]
                    && sizes[roots[first]] >= 3
                {
                    return Some(((x, y), direction));
                }
            }
        }
    }
    None
}

fn local_quadrants<C>(component: &GridComponent<C>, x: usize, y: usize) -> [bool; 4] {
    [
        x > 0 && y > 0 && component.contains_cell(x - 1, y - 1),
        y > 0 && component.contains_cell(x, y - 1),
        component.contains_cell(x, y),
        x > 0 && component.contains_cell(x - 1, y),
    ]
}

fn local_blocked_rays(
    cuts: &ReferenceOrderedCuts,
    inside: [bool; 4],
    x: usize,
    y: usize,
) -> [bool; 4] {
    [
        cuts.horizontal.contains(&HorizontalUnitCut { x, y }) || inside[1] != inside[2],
        cuts.vertical.contains(&VerticalUnitCut { x, y }) || inside[2] != inside[3],
        (x > 0 && cuts.horizontal.contains(&HorizontalUnitCut { x: x - 1, y }))
            || inside[3] != inside[0],
        (y > 0 && cuts.vertical.contains(&VerticalUnitCut { x, y: y - 1 }))
            || inside[0] != inside[1],
    ]
}

fn local_angle_components(inside: [bool; 4], blocked: [bool; 4]) -> ([usize; 4], [usize; 4]) {
    let mut roots = [0, 1, 2, 3];
    for (ray, first, second) in [(0, 1, 2), (1, 2, 3), (2, 3, 0), (3, 0, 1)] {
        if inside[first] && inside[second] && !blocked[ray] {
            union_roots(&mut roots, first, second);
        }
    }
    for index in 0..4 {
        roots[index] = find_root(&roots, index);
    }
    let mut sizes = [0; 4];
    for index in 0..4 {
        if inside[index] {
            sizes[roots[index]] += 1;
        }
    }
    (roots, sizes)
}

fn find_root(roots: &[usize; 4], mut index: usize) -> usize {
    while roots[index] != index {
        index = roots[index];
    }
    index
}

fn union_roots(roots: &mut [usize; 4], first: usize, second: usize) {
    let first_root = find_root(roots, first);
    let second_root = find_root(roots, second);
    if first_root != second_root {
        roots[second_root] = first_root;
    }
}

fn extend_simple_chord<C>(
    component: &GridComponent<C>,
    cuts: &mut ReferenceOrderedCuts,
    point: (usize, usize),
    direction: Direction,
    metrics: &mut CompletionMetrics,
) -> usize {
    let mut horizontal_additions = Vec::new();
    let mut vertical_additions = Vec::new();
    let (mut x, mut y) = point;
    loop {
        metrics.ray_extension_unit_steps += 1;
        let next = match direction {
            Direction::East => {
                if y == 0
                    || !component.contains_cell(x, y - 1)
                    || !component.contains_cell(x, y)
                    || cuts.horizontal.contains(&HorizontalUnitCut { x, y })
                {
                    break;
                }
                horizontal_additions.push(HorizontalUnitCut { x, y });
                x += 1;
                (x, y)
            }
            Direction::West => {
                if x == 0 || y == 0 {
                    break;
                }
                let unit_x = x - 1;
                if !component.contains_cell(unit_x, y - 1)
                    || !component.contains_cell(unit_x, y)
                    || cuts
                        .horizontal
                        .contains(&HorizontalUnitCut { x: unit_x, y })
                {
                    break;
                }
                horizontal_additions.push(HorizontalUnitCut { x: unit_x, y });
                x -= 1;
                (x, y)
            }
            Direction::North => {
                if x == 0
                    || !component.contains_cell(x - 1, y)
                    || !component.contains_cell(x, y)
                    || cuts.vertical.contains(&VerticalUnitCut { x, y })
                {
                    break;
                }
                vertical_additions.push(VerticalUnitCut { x, y });
                y += 1;
                (x, y)
            }
            Direction::South => {
                if x == 0 || y == 0 {
                    break;
                }
                let unit_y = y - 1;
                if !component.contains_cell(x - 1, unit_y)
                    || !component.contains_cell(x, unit_y)
                    || cuts.vertical.contains(&VerticalUnitCut { x, y: unit_y })
                {
                    break;
                }
                vertical_additions.push(VerticalUnitCut { x, y: unit_y });
                y -= 1;
                (x, y)
            }
        };
        if perpendicular_boundary_at(component, cuts, next, direction.is_horizontal()) {
            break;
        }
    }
    let added = horizontal_additions.len() + vertical_additions.len();
    cuts.horizontal.extend(horizontal_additions);
    cuts.vertical.extend(vertical_additions);
    added
}

fn perpendicular_boundary_at<C>(
    component: &GridComponent<C>,
    cuts: &ReferenceOrderedCuts,
    point: (usize, usize),
    horizontal_chord: bool,
) -> bool {
    let (x, y) = point;
    let inside = local_quadrants(component, x, y);
    if horizontal_chord {
        cuts.vertical.contains(&VerticalUnitCut { x, y })
            || (y > 0 && cuts.vertical.contains(&VerticalUnitCut { x, y: y - 1 }))
            || inside[2] != inside[3]
            || inside[0] != inside[1]
    } else {
        cuts.horizontal.contains(&HorizontalUnitCut { x, y })
            || (x > 0 && cuts.horizontal.contains(&HorizontalUnitCut { x: x - 1, y }))
            || inside[1] != inside[2]
            || inside[3] != inside[0]
    }
}

fn rectangles_from_cuts<C>(
    component: &GridComponent<C>,
    cuts: &ReferenceOrderedCuts,
    metrics: &mut CompletionMetrics,
) -> Result<Vec<GridRect>, SgError> {
    let cell_set = component.cells.iter().copied().collect::<HashSet<_>>();
    let mut unseen = cell_set.clone();
    let mut rectangles = Vec::new();
    metrics.rectangle_recovery_allocations += 4;
    while let Some(&seed) = unseen.iter().next() {
        unseen.remove(&seed);
        let mut queue = VecDeque::from([seed]);
        let mut region = vec![seed];
        metrics.rectangle_recovery_queue_pushes += 1;
        metrics.rectangle_recovery_region_count += 1;
        while let Some(cell) = queue.pop_front() {
            metrics.rectangle_recovery_component_visits += 1;
            for neighbor in uncut_neighbors(cell, component, cuts) {
                if unseen.remove(&neighbor) {
                    queue.push_back(neighbor);
                    region.push(neighbor);
                    metrics.rectangle_recovery_queue_pushes += 1;
                }
            }
        }
        let x0 = region.iter().map(|cell| cell.x).min().unwrap();
        let y0 = region.iter().map(|cell| cell.y).min().unwrap();
        let x1 = region.iter().map(|cell| cell.x + 1).max().unwrap();
        let y1 = region.iter().map(|cell| cell.y + 1).max().unwrap();
        let rectangle = GridRect::new(x0, y0, x1, y1)?;
        if rectangle.area() != region.len()
            || !(y0..y1).all(|y| (x0..x1).all(|x| cell_set.contains(&rect_core::Cell { x, y })))
        {
            return Err(SgError::NonRectangularCompletionRegion {
                seed_x: seed.x,
                seed_y: seed.y,
            });
        }
        rectangles.push(rectangle);
    }
    rectangles.sort_unstable();
    Ok(rectangles)
}

impl RectangleRecoveryBackend for DenseGridRecovery {
    fn recover(
        &self,
        prepared: &PreparedGridComponent,
        cuts: &DenseCutGrid,
    ) -> Result<RectangleRecoveryResult, SgError> {
        dense_rectangles_from_cuts(prepared, cuts)
    }

    fn name(&self) -> &'static str {
        "dense-grid"
    }
}

impl RectangleRecoveryBackend for ReferenceHashBfsRecovery {
    fn recover(
        &self,
        prepared: &PreparedGridComponent,
        cuts: &DenseCutGrid,
    ) -> Result<RectangleRecoveryResult, SgError> {
        hash_rectangles_from_dense_cuts(prepared, cuts)
    }

    fn name(&self) -> &'static str {
        "reference-hash-bfs"
    }
}

fn dense_rectangles_from_cuts(
    prepared: &PreparedGridComponent,
    cuts: &DenseCutGrid,
) -> Result<RectangleRecoveryResult, SgError> {
    let width = prepared.width();
    let height = prepared.height();
    let mut visited = vec![false; width * height];
    let mut queue = VecDeque::<usize>::new();
    let mut rectangles = Vec::new();
    let mut result = RectangleRecoveryResult {
        allocations: 3,
        ..RectangleRecoveryResult::default()
    };

    for seed in 0..prepared.occupancy.len() {
        if !prepared.occupancy[seed] || visited[seed] {
            continue;
        }
        visited[seed] = true;
        queue.push_back(seed);
        result.queue_pushes += 1;
        result.region_count += 1;
        let seed_x = seed % width;
        let seed_y = seed / width;
        let (mut left, mut top, mut right, mut bottom) = (seed_x, seed_y, seed_x + 1, seed_y + 1);
        let mut region_size = 0;

        while let Some(index) = queue.pop_front() {
            result.cell_visits += 1;
            region_size += 1;
            let local_x = index % width;
            let local_y = index / width;
            left = left.min(local_x);
            top = top.min(local_y);
            right = right.max(local_x + 1);
            bottom = bottom.max(local_y + 1);
            let x = prepared.x0 + local_x;
            let y = prepared.y0 + local_y;

            let mut visit = |neighbor: usize| {
                if prepared.occupancy[neighbor] && !visited[neighbor] {
                    visited[neighbor] = true;
                    queue.push_back(neighbor);
                    result.queue_pushes += 1;
                }
            };
            if local_x > 0 && !cuts.contains_vertical(x, y) {
                visit(index - 1);
            }
            if local_x + 1 < width && !cuts.contains_vertical(x + 1, y) {
                visit(index + 1);
            }
            if local_y > 0 && !cuts.contains_horizontal(x, y) {
                visit(index - width);
            }
            if local_y + 1 < height && !cuts.contains_horizontal(x, y + 1) {
                visit(index + width);
            }
        }

        let x0 = prepared.x0 + left;
        let y0 = prepared.y0 + top;
        let x1 = prepared.x0 + right;
        let y1 = prepared.y0 + bottom;
        let rectangle = GridRect::new(x0, y0, x1, y1)?;
        if rectangle.area() != region_size
            || prepared.occupied_cell_count(x0, y0, x1, y1) != region_size
        {
            return Err(SgError::NonRectangularCompletionRegion {
                seed_x: prepared.x0 + seed_x,
                seed_y: prepared.y0 + seed_y,
            });
        }
        rectangles.push(rectangle);
    }
    rectangles.sort_unstable();
    result.rectangles = rectangles;
    Ok(result)
}

fn hash_rectangles_from_dense_cuts(
    prepared: &PreparedGridComponent,
    cuts: &DenseCutGrid,
) -> Result<RectangleRecoveryResult, SgError> {
    let occupied = prepared
        .occupancy
        .iter()
        .enumerate()
        .filter_map(|(index, &present)| present.then_some(index))
        .collect::<HashSet<_>>();
    let mut unseen = occupied.clone();
    let mut queue = VecDeque::new();
    let mut region = Vec::new();
    let mut result = RectangleRecoveryResult {
        allocations: 5,
        ..RectangleRecoveryResult::default()
    };
    while let Some(&seed) = unseen.iter().next() {
        unseen.remove(&seed);
        queue.push_back(seed);
        region.clear();
        result.queue_pushes += 1;
        result.region_count += 1;
        while let Some(index) = queue.pop_front() {
            result.cell_visits += 1;
            region.push(index);
            let local_x = index % prepared.width();
            let local_y = index / prepared.width();
            let x = prepared.x0 + local_x;
            let y = prepared.y0 + local_y;
            let candidates = [
                (local_x > 0 && !cuts.contains_vertical(x, y)).then_some(index.wrapping_sub(1)),
                (local_x + 1 < prepared.width() && !cuts.contains_vertical(x + 1, y))
                    .then_some(index + 1),
                (local_y > 0 && !cuts.contains_horizontal(x, y))
                    .then_some(index.wrapping_sub(prepared.width())),
                (local_y + 1 < prepared.height() && !cuts.contains_horizontal(x, y + 1))
                    .then_some(index + prepared.width()),
            ];
            for neighbor in candidates.into_iter().flatten() {
                if unseen.remove(&neighbor) {
                    queue.push_back(neighbor);
                    result.queue_pushes += 1;
                }
            }
        }
        let left = region
            .iter()
            .map(|index| index % prepared.width())
            .min()
            .unwrap();
        let top = region
            .iter()
            .map(|index| index / prepared.width())
            .min()
            .unwrap();
        let right = region
            .iter()
            .map(|index| index % prepared.width() + 1)
            .max()
            .unwrap();
        let bottom = region
            .iter()
            .map(|index| index / prepared.width() + 1)
            .max()
            .unwrap();
        let rectangle = GridRect::new(
            prepared.x0 + left,
            prepared.y0 + top,
            prepared.x0 + right,
            prepared.y0 + bottom,
        )?;
        if rectangle.area() != region.len()
            || prepared.occupied_cell_count(rectangle.x0, rectangle.y0, rectangle.x1, rectangle.y1)
                != region.len()
        {
            return Err(SgError::NonRectangularCompletionRegion {
                seed_x: prepared.x0 + seed % prepared.width(),
                seed_y: prepared.y0 + seed / prepared.width(),
            });
        }
        result.rectangles.push(rectangle);
    }
    result.rectangles.sort_unstable();
    Ok(result)
}

fn uncut_neighbors<C>(
    cell: rect_core::Cell,
    component: &GridComponent<C>,
    cuts: &ReferenceOrderedCuts,
) -> impl Iterator<Item = rect_core::Cell> {
    let mut neighbors = Vec::with_capacity(4);
    if cell.x > 0
        && component.contains_cell(cell.x - 1, cell.y)
        && !cuts.vertical.contains(&VerticalUnitCut {
            x: cell.x,
            y: cell.y,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x - 1,
            y: cell.y,
        });
    }
    if component.contains_cell(cell.x + 1, cell.y)
        && !cuts.vertical.contains(&VerticalUnitCut {
            x: cell.x + 1,
            y: cell.y,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x + 1,
            y: cell.y,
        });
    }
    if cell.y > 0
        && component.contains_cell(cell.x, cell.y - 1)
        && !cuts.horizontal.contains(&HorizontalUnitCut {
            x: cell.x,
            y: cell.y,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x,
            y: cell.y - 1,
        });
    }
    if component.contains_cell(cell.x, cell.y + 1)
        && !cuts.horizontal.contains(&HorizontalUnitCut {
            x: cell.x,
            y: cell.y + 1,
        })
    {
        neighbors.push(rect_core::Cell {
            x: cell.x,
            y: cell.y + 1,
        });
    }
    neighbors.into_iter()
}

#[derive(Debug, Error)]
pub enum SgError {
    #[error(transparent)]
    PreparedContext(#[from] PreparedContextError),
    #[error(transparent)]
    Boundary(#[from] BoundaryError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
    #[error(transparent)]
    Graph(#[from] rect_graph::GraphError),
    #[error("solver produced an invalid dissection: {0}")]
    InvalidOutput(#[from] ValidationError),
    #[error("grid coordinate {value} cannot be represented as usize")]
    CoordinateConversion { value: Coord },
    #[error("effective chord endpoint is not a normalized boundary vertex")]
    EndpointNotOnBoundary,
    #[error("ordinary grid component unexpectedly has {outer_loops} outer loops")]
    UnsupportedBoundaryTopology { outer_loops: usize },
    #[error("rectangle-count formula underflowed; geometric invariants are inconsistent")]
    FormulaUnderflow,
    #[error("an enumerated effective chord violates Definition 7 in the grid model")]
    InvalidEffectiveChord,
    #[error("minimum vertex cover does not cover conflict edge ({left}, {right})")]
    UncoveredConflictEdge { left: usize, right: usize },
    #[error("selected chord family contains conflicting pair ({left}, {right})")]
    NonIndependentSelection { left: usize, right: usize },
    #[error("matching size {matching} differs from minimum vertex-cover size {cover}")]
    MatchingCoverMismatch { matching: usize, cover: usize },
    #[error("geometric completion could not extend a simple chord from {point:?}")]
    InvalidSimpleChord { point: (usize, usize) },
    #[error("geometric completion did not terminate within the finite unit-cut bound")]
    CompletionDidNotTerminate,
    #[error("completion region containing ({seed_x}, {seed_y}) is not a rectangle")]
    NonRectangularCompletionRegion { seed_x: usize, seed_y: usize },
    #[error("completion produced {actual} rectangles, formula requires {expected}")]
    CompletionCount { expected: usize, actual: usize },
    #[error("selected-chord bit vectors do not match the chord-family dimensions")]
    SelectionLengthMismatch,
}

#[cfg(test)]
mod tests {
    use rect_core::{ColorGrid, GridComponent, validate_dissection, validate_dissection_prepared};
    use rect_oracle_exact_cover as exact_cover;

    use super::{
        ChordBoundaryEndpoints, CleanRejectionReason, DenseCutGrid, DenseGridRecovery,
        EffectiveChordEnumerator, GridInteriorRunEnumerator, IndexedFrontierCompletion,
        RectangleRecoveryBackend, ReferenceHashBfsRecovery, ReferencePairwiseEnumerator,
        ReferenceRescanCompletion, analyze, analyze_geometry, classify_clean_hole_free,
        complete_with_backend, complete_with_chord_families, endpoints_alternate, solve,
    };

    fn foreground_component(width: usize, height: usize, cells: Vec<bool>) -> GridComponent<bool> {
        ColorGrid::new(width, height, cells)
            .unwrap()
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
            .max_by_key(GridComponent::cell_count)
            .unwrap()
    }

    fn endpoint(loop_index: usize, index: usize) -> rect_core::BoundaryVertexId {
        rect_core::BoundaryVertexId {
            loop_id: rect_core::BoundaryLoopId(loop_index),
            cyclic_index: index,
        }
    }

    #[test]
    fn endpoint_alternation_handles_wraparound_and_nested_intervals() {
        let crossing = ChordBoundaryEndpoints {
            first: endpoint(0, 7),
            second: endpoint(0, 2),
        };
        let alternating = ChordBoundaryEndpoints {
            first: endpoint(0, 0),
            second: endpoint(0, 5),
        };
        let nested = ChordBoundaryEndpoints {
            first: endpoint(0, 0),
            second: endpoint(0, 1),
        };
        assert!(endpoints_alternate(crossing, alternating, 8));
        assert!(!endpoints_alternate(crossing, nested, 8));
        assert!(!endpoints_alternate(
            crossing,
            ChordBoundaryEndpoints {
                first: endpoint(0, 7),
                second: endpoint(0, 4),
            },
            8
        ));
        assert!(!endpoints_alternate(
            crossing,
            ChordBoundaryEndpoints {
                first: endpoint(1, 0),
                second: endpoint(1, 2),
            },
            8
        ));
    }

    #[test]
    fn clean_classifier_rejects_ordinary_holes() {
        let component = foreground_component(
            3,
            3,
            vec![true, true, true, true, false, true, true, true, true],
        );
        let geometry = analyze_geometry(&component).unwrap();
        let certificate = classify_clean_hole_free(
            &component,
            &geometry.boundary,
            &geometry.horizontal_chords,
            &geometry.vertical_chords,
        );
        assert!(!certificate.eligible);
        assert!(
            certificate
                .rejection_reasons
                .contains(&CleanRejectionReason::HasHole { count: 1 })
        );
        assert!(certificate.all_chords_proper);
        assert!(certificate.distinct_boundary_endpoints);
    }

    #[test]
    fn agrees_on_l_tromino_plus_and_ring() {
        let cases = [
            (2, 2, vec![true, true, true, false]),
            (
                3,
                3,
                vec![false, true, false, true, true, true, false, true, false],
            ),
            (
                3,
                3,
                vec![true, true, true, true, false, true, true, true, true],
            ),
        ];
        for (width, height, cells) in cases {
            let component = foreground_component(width, height, cells);
            let expected = exact_cover::solve(&component).unwrap();
            let actual = solve(&component).unwrap();
            assert_eq!(
                actual.optimum_rectangle_count,
                expected.optimum_rectangle_count
            );
            validate_dissection(&component, &actual).unwrap();
        }
    }

    #[test]
    fn reference_completion_reports_real_scan_and_recovery_metrics() {
        let component = foreground_component(
            3,
            3,
            vec![false, true, false, true, true, true, false, true, false],
        );
        let analysis = analyze(&component).unwrap();
        let completion = complete_with_backend(
            &component,
            &analysis.horizontal_chords,
            &analysis.vertical_chords,
            &analysis.selected_horizontal,
            &analysis.selected_vertical,
            &ReferenceRescanCompletion,
        )
        .unwrap();
        let legacy = complete_with_chord_families(
            &component,
            &analysis.horizontal_chords,
            &analysis.vertical_chords,
            &analysis.selected_horizontal,
            &analysis.selected_vertical,
        )
        .unwrap();
        assert_eq!(completion.rectangles, legacy);
        assert_eq!(
            completion.metrics.rectangle_recovery_component_visits,
            component.cell_count()
        );
        assert!(completion.metrics.full_grid_vertex_scans >= 2);
        assert_eq!(
            completion.metrics.added_horizontal_unit_cut_count,
            completion.added_horizontal_unit_cuts.len()
        );
        assert_eq!(
            completion.metrics.added_vertical_unit_cut_count,
            completion.added_vertical_unit_cuts.len()
        );
    }

    fn assert_completion_backends_equal(component: &GridComponent<bool>) {
        let analysis = analyze(component).unwrap();
        let reference = complete_with_backend(
            component,
            &analysis.horizontal_chords,
            &analysis.vertical_chords,
            &analysis.selected_horizontal,
            &analysis.selected_vertical,
            &ReferenceRescanCompletion,
        )
        .unwrap();
        let indexed = complete_with_backend(
            component,
            &analysis.horizontal_chords,
            &analysis.vertical_chords,
            &analysis.selected_horizontal,
            &analysis.selected_vertical,
            &IndexedFrontierCompletion,
        )
        .unwrap();
        assert_eq!(
            reference.selected_horizontal_unit_cuts,
            indexed.selected_horizontal_unit_cuts
        );
        assert_eq!(
            reference.selected_vertical_unit_cuts,
            indexed.selected_vertical_unit_cuts
        );
        assert_eq!(
            reference.added_horizontal_unit_cuts,
            indexed.added_horizontal_unit_cuts
        );
        assert_eq!(
            reference.added_vertical_unit_cuts,
            indexed.added_vertical_unit_cuts
        );
        assert_eq!(reference.rectangles, indexed.rectangles);
        assert_eq!(indexed.metrics.full_grid_vertex_scans, 2);
        let mut dense_cuts = DenseCutGrid::from_selection(
            &analysis.prepared,
            &analysis.horizontal_chords,
            &analysis.vertical_chords,
            &analysis.selected_horizontal,
            &analysis.selected_vertical,
        )
        .unwrap();
        for cut in indexed.added_horizontal_unit_cuts {
            dense_cuts.insert_horizontal(cut);
        }
        for cut in indexed.added_vertical_unit_cuts {
            dense_cuts.insert_vertical(cut);
        }
        let hash_recovery = ReferenceHashBfsRecovery
            .recover(&analysis.prepared, &dense_cuts)
            .unwrap();
        let dense_recovery = DenseGridRecovery
            .recover(&analysis.prepared, &dense_cuts)
            .unwrap();
        assert_eq!(hash_recovery.rectangles, dense_recovery.rectangles);
        let result = rect_core::DissectionResult {
            optimum_rectangle_count: dense_recovery.rectangles.len(),
            rectangles: dense_recovery.rectangles,
            diagnostics: rect_core::Diagnostics::default(),
            certificate: None,
        };
        assert_eq!(validate_dissection(component, &result), Ok(()));
        assert_eq!(
            validate_dissection_prepared(&analysis.prepared, &result),
            Ok(())
        );
    }

    #[test]
    fn completion_backends_match_through_three_by_three() {
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(3, 3, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                assert_completion_backends_equal(&component);
            }
        }
    }

    #[test]
    fn completion_backends_match_through_four_by_four() {
        for mask in 1_u32..(1_u32 << 16) {
            let cells = (0..16)
                .map(|index| mask & (1_u32 << index) != 0)
                .collect::<Vec<_>>();
            for component in ColorGrid::new(4, 4, cells)
                .unwrap()
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                assert_completion_backends_equal(&component);
            }
        }
    }

    #[test]
    fn exhaustively_agrees_through_three_by_three() {
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            let grid = ColorGrid::new(3, 3, cells).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let expected = exact_cover::solve(&component).unwrap();
                let actual = solve(&component).unwrap_or_else(|error| {
                    panic!("mask {mask:#05x}, component {:?}: {error}", component.cells)
                });
                assert_eq!(
                    actual.optimum_rectangle_count, expected.optimum_rectangle_count,
                    "mask {mask:#05x}, component {:?}",
                    component.cells
                );
            }
        }
    }

    #[test]
    fn grid_runs_match_reference_chord_sets_through_three_by_three() {
        for mask in 1_u16..(1_u16 << 9) {
            let cells = (0..9)
                .map(|index| mask & (1_u16 << index) != 0)
                .collect::<Vec<_>>();
            let grid = ColorGrid::new(3, 3, cells).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = rect_core::Boundary::from_component(&component).unwrap();
                let reference = ReferencePairwiseEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                let optimized = GridInteriorRunEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                assert_eq!(
                    reference.horizontal, optimized.horizontal,
                    "mask {mask:#05x}"
                );
                assert_eq!(reference.vertical, optimized.vertical, "mask {mask:#05x}");
            }
        }
    }

    #[test]
    fn grid_runs_match_reference_chord_sets_through_four_by_four() {
        for mask in 1_u32..(1_u32 << 16) {
            let cells = (0..16)
                .map(|index| mask & (1_u32 << index) != 0)
                .collect::<Vec<_>>();
            let grid = ColorGrid::new(4, 4, cells).unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = rect_core::Boundary::from_component(&component).unwrap();
                let reference = ReferencePairwiseEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                let optimized = GridInteriorRunEnumerator
                    .enumerate(&component, &boundary)
                    .unwrap();
                assert_eq!(
                    reference.horizontal, optimized.horizontal,
                    "mask {mask:#06x}"
                );
                assert_eq!(reference.vertical, optimized.vertical, "mask {mask:#06x}");
            }
        }
    }

    #[test]
    fn grid_runs_match_reference_on_one_hundred_thousand_connected_regions() {
        const CASES: usize = 100_000;
        const SEED: u64 = 0x6d72_642d_7630_3300;
        let mut random = SplitMix64::new(SEED);
        for case in 0..CASES {
            let width = 5 + random.index(12);
            let height = 5 + random.index(12);
            let mut cells = vec![false; width * height];
            let mut x = random.index(width);
            let mut y = random.index(height);
            cells[y * width + x] = true;
            let steps = 8 + random.index(width * height * 3);
            for _ in 0..steps {
                match random.index(4) {
                    0 if x > 0 => x -= 1,
                    1 if x + 1 < width => x += 1,
                    2 if y > 0 => y -= 1,
                    3 if y + 1 < height => y += 1,
                    _ => {}
                }
                cells[y * width + x] = true;
            }
            let grid = ColorGrid::new(width, height, cells.clone()).unwrap();
            let component = grid
                .four_connected_components()
                .into_iter()
                .find(|component| component.color)
                .unwrap();
            let boundary = rect_core::Boundary::from_component(&component).unwrap();
            let reference = ReferencePairwiseEnumerator
                .enumerate(&component, &boundary)
                .unwrap();
            let optimized = GridInteriorRunEnumerator
                .enumerate(&component, &boundary)
                .unwrap();
            assert_eq!(
                reference.horizontal, optimized.horizontal,
                "horizontal: seed={SEED:#018x}, case={case}, width={width}, height={height}, cells={cells:?}"
            );
            assert_eq!(
                reference.vertical, optimized.vertical,
                "seed={SEED:#018x}, case={case}, width={width}, height={height}, cells={cells:?}"
            );
            let selected_horizontal = vec![false; reference.horizontal.len()];
            let selected_vertical = vec![false; reference.vertical.len()];
            let reference_completion = complete_with_backend(
                &component,
                &reference.horizontal,
                &reference.vertical,
                &selected_horizontal,
                &selected_vertical,
                &ReferenceRescanCompletion,
            )
            .unwrap();
            let indexed_completion = complete_with_backend(
                &component,
                &reference.horizontal,
                &reference.vertical,
                &selected_horizontal,
                &selected_vertical,
                &IndexedFrontierCompletion,
            )
            .unwrap();
            assert_eq!(
                reference_completion.added_horizontal_unit_cuts,
                indexed_completion.added_horizontal_unit_cuts,
                "completion horizontal: seed={SEED:#018x}, case={case}"
            );
            assert_eq!(
                reference_completion.added_vertical_unit_cuts,
                indexed_completion.added_vertical_unit_cuts,
                "completion vertical: seed={SEED:#018x}, case={case}"
            );
            assert_eq!(
                reference_completion.rectangles, indexed_completion.rectangles,
                "completion rectangles: seed={SEED:#018x}, case={case}"
            );
        }
    }

    struct SplitMix64 {
        state: u64,
    }

    impl SplitMix64 {
        const fn new(seed: u64) -> Self {
            Self { state: seed }
        }

        fn next(&mut self) -> u64 {
            self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut value = self.state;
            value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            value ^ (value >> 31)
        }

        fn index(&mut self, upper: usize) -> usize {
            usize::try_from(self.next() % u64::try_from(upper).unwrap()).unwrap()
        }
    }
}
