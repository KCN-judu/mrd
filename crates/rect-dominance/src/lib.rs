pub mod biclique;
pub mod compressed_flow;
pub mod embedding;
pub mod formal;
pub mod path_tree;

pub use formal::{
    FormalAdmissibleAnalysis, FormalAdmissibleError, FormalCompletionAnalysis, FormalStep2Segment,
    FormalStep2Transformation, analyze_formal_admissible_family, complete_formal_polygon,
};

use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::time::Instant;

use biclique::{Error, Partition};
use compressed_flow::{CompressedFlowError, solve_biclique_flow};
use embedding::{DominanceEmbedding, EmbeddingError};
pub use path_tree::{GapBackend, PathTreeOrientation, PathTreeOrientationPolicy, RegionBackend};
use path_tree::{
    PathTreeError, build_best_path_tree_partition_with_backend, build_boundary_path_tree_partition,
    build_path_tree_partition_with_orientation_policy_and_options,
};
use rect_core::{
    Certificate, Diagnostics, DissectionResult, ExactRatio, ExecutionTrace, GridComponent,
    PolygonDissectionResult, PolygonGeometryBackend, PreparedComponentContext,
    PreparedGridComponent, PreparedPolygonContext, PreparedPolygonError, RectilinearPolygon,
    ValidationError, polygon, validate_dissection, validate_dissection_prepared,
};
use rect_graph::{DinicBackend, FlowBackendKind, MaxFlowBackend, hopcroft_karp};
use rect_oracle_sg::{
    CompletionBackendKind, CompletionMetrics, CoordinateCompressedCompletion,
    EffectiveChordEndpointIndex, EffectiveChordEnumerator, GeneralPolygonPairwiseEnumerator,
    GridInteriorRunEnumerator, IndexedFrontierCompletion, IndexedPolygonCompletion,
    IndexedPolygonPairwiseEnumerator, PolygonCutIndexBackend, PolygonDissectionValidatorBackend,
    PolygonRecoveryBackend, PolygonSgError, PreparedCoordinateArrangement,
    ReferencePairwiseEnumerator, ReferenceRescanCompletion, SgError,
    SoltanGorpinevichSweepEnumerator, SparseValidatorBackend, SubdivisionBuilderBackend,
    analyze_prepared_geometry, audit_sweep_provenance, classify_clean_polygon,
    complete_with_prepared_backend, validate_polygon_dissection_count,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DominanceMode {
    ExplicitEdges,
    Compact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerificationMode {
    FullyAudited,
    CompactOnly,
}

fn completion_diagnostics(metrics: &CompletionMetrics) -> Diagnostics {
    Diagnostics {
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
        rectangle_recovery_queue_pushes: Some(metrics.rectangle_recovery_queue_pushes),
        rectangle_recovery_region_count: Some(metrics.rectangle_recovery_region_count),
        rectangle_recovery_allocations: Some(metrics.rectangle_recovery_allocations),
        ..Diagnostics::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChordEnumerator {
    ReferencePairwise,
    GridInteriorRuns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictRepresentationBackend {
    GeneralDominance4D,
    CleanHoleFreePathTree,
    Auto,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonChordBackend {
    ReferencePairwise,
    #[default]
    IndexedPairwise,
    SoltanGorpinevichSweep,
}

impl PolygonChordBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ReferencePairwise => "reference-pairwise",
            Self::IndexedPairwise => "indexed-pairwise",
            Self::SoltanGorpinevichSweep => "sg-sweep",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonCompletionBackend {
    CoordinateReference,
    #[default]
    IndexedFrontier,
}

impl PolygonCompletionBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CoordinateReference => "coordinate-reference",
            Self::IndexedFrontier => "indexed-frontier",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolygonArrangementBackend {
    Reference,
    #[default]
    Indexed,
}

impl PolygonArrangementBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::Indexed => "indexed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonSolveOptions {
    pub verification_mode: VerificationMode,
    pub geometry_backend: PolygonGeometryBackend,
    pub validation_backend: polygon::Backend,
    pub chord_backend: PolygonChordBackend,
    pub completion_backend: PolygonCompletionBackend,
    pub cut_index_backend: PolygonCutIndexBackend,
    pub recovery_backend: PolygonRecoveryBackend,
    pub dissection_validator_backend: PolygonDissectionValidatorBackend,
    pub subdivision_builder_backend: SubdivisionBuilderBackend,
    pub sparse_validator_backend: SparseValidatorBackend,
    /// Legacy dense/reference arrangement selector retained for old callers.
    pub arrangement_backend: PolygonArrangementBackend,
    pub representation: ConflictRepresentationBackend,
}

impl Default for PolygonSolveOptions {
    fn default() -> Self {
        Self {
            verification_mode: VerificationMode::CompactOnly,
            geometry_backend: PolygonGeometryBackend::Indexed,
            validation_backend: polygon::Backend::Experiment,
            chord_backend: PolygonChordBackend::SoltanGorpinevichSweep,
            completion_backend: PolygonCompletionBackend::IndexedFrontier,
            cut_index_backend: PolygonCutIndexBackend::DynamicStabbing,
            recovery_backend: PolygonRecoveryBackend::SparseSubdivision,
            dissection_validator_backend: PolygonDissectionValidatorBackend::SparseSlab,
            subdivision_builder_backend: SubdivisionBuilderBackend::OrthogonalSweep,
            sparse_validator_backend: SparseValidatorBackend::EventSegmentTree,
            arrangement_backend: PolygonArrangementBackend::Indexed,
            representation: ConflictRepresentationBackend::GeneralDominance4D,
        }
    }
}

impl ConflictRepresentationBackend {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::GeneralDominance4D => "dominance-4d",
            Self::CleanHoleFreePathTree => "path-tree",
            Self::Auto => "auto",
        }
    }
}

/// Solves a normalized ordinary polygon with the conservative 4D production
/// representation.
///
/// # Errors
///
/// Returns [`DominanceError`] when polygon validation, chord enumeration,
/// compact matching, completion, or final validation fails.
pub fn solve_polygon(
    polygon: &RectilinearPolygon,
) -> Result<PolygonDissectionResult, DominanceError> {
    solve_polygon_with_representation(polygon, ConflictRepresentationBackend::GeneralDominance4D)
}

/// Solves a boundary-native ordinary polygon with an explicit compact
/// representation choice.
///
/// `Auto` selects the path-tree partition only for a clean hole-free polygon;
/// otherwise it falls back to 4D. The path-tree branch also solves the 4D
/// partition and requires equal matching values.
///
/// # Errors
///
/// Returns [`DominanceError`] for any geometry, representation, flow,
/// completion, or certificate mismatch.
#[allow(clippy::too_many_lines)]
pub fn solve_polygon_with_representation(
    polygon: &RectilinearPolygon,
    representation: ConflictRepresentationBackend,
) -> Result<PolygonDissectionResult, DominanceError> {
    solve_polygon_with_options(
        polygon,
        PolygonSolveOptions {
            representation,
            ..PolygonSolveOptions::default()
        },
    )
}

/// Solves a polygon with independently selected exact geometry backends.
///
/// In `FullyAudited` mode, reference and indexed chord/completion paths are
/// both run and their complete geometric outputs must agree.
///
/// # Errors
///
/// Returns [`DominanceError`] for backend disagreement or any solver failure.
#[allow(clippy::too_many_lines)]
pub fn solve_polygon_with_options(
    polygon: &RectilinearPolygon,
    options: PolygonSolveOptions,
) -> Result<PolygonDissectionResult, DominanceError> {
    let started = Instant::now();
    let prepared = PreparedPolygonContext::new_with_validator(polygon, options.validation_backend)?;
    let polygon = prepared.polygon();
    let boundary = prepared.boundary();
    let boundary_index = prepared.boundary_index();
    let (families, chord_metrics, sweep_certificate) = match options.verification_mode {
        VerificationMode::FullyAudited => {
            let reference =
                GeneralPolygonPairwiseEnumerator.enumerate_prepared_with_metrics(&prepared)?;
            let indexed = IndexedPolygonPairwiseEnumerator.enumerate_prepared(&prepared)?;
            let sweep = SoltanGorpinevichSweepEnumerator.enumerate_prepared(&prepared)?;
            audit_sweep_provenance(&prepared, &sweep)?;
            if reference.families.horizontal != indexed.families.horizontal
                || reference.families.vertical != indexed.families.vertical
                || reference.families.horizontal != sweep.families.horizontal
                || reference.families.vertical != sweep.families.vertical
            {
                return Err(DominanceError::ChordFamilyMismatch);
            }
            match options.chord_backend {
                PolygonChordBackend::ReferencePairwise => {
                    (reference.families, Some(reference.metrics), None)
                }
                PolygonChordBackend::IndexedPairwise => {
                    (indexed.families, Some(indexed.metrics), None)
                }
                PolygonChordBackend::SoltanGorpinevichSweep => {
                    (sweep.families, Some(sweep.metrics), sweep.sweep_certificate)
                }
            }
        }
        VerificationMode::CompactOnly => match options.chord_backend {
            PolygonChordBackend::ReferencePairwise => {
                let reference =
                    GeneralPolygonPairwiseEnumerator.enumerate_prepared_with_metrics(&prepared)?;
                (reference.families, Some(reference.metrics), None)
            }
            PolygonChordBackend::IndexedPairwise => {
                let indexed = IndexedPolygonPairwiseEnumerator.enumerate_prepared(&prepared)?;
                (indexed.families, Some(indexed.metrics), None)
            }
            PolygonChordBackend::SoltanGorpinevichSweep => {
                let sweep = SoltanGorpinevichSweepEnumerator.enumerate_prepared(&prepared)?;
                (sweep.families, Some(sweep.metrics), sweep.sweep_certificate)
            }
        },
    };
    let endpoint_index =
        EffectiveChordEndpointIndex::new(boundary_index, &families.horizontal, &families.vertical)?;
    let geometry_at = Instant::now();
    let embedding = DominanceEmbedding::new(&families.horizontal, &families.vertical)?;
    let four_d_construction = match options.verification_mode {
        VerificationMode::FullyAudited => Partition::comparability_theorem_8_audited(&embedding)?,
        VerificationMode::CompactOnly => biclique::experiment::construct(&embedding)?,
    };
    let four_d_partition = four_d_construction.partition;
    four_d_partition.verify_dominance_blocks(&embedding)?;
    let four_d_flow = solve_biclique_flow(
        embedding.horizontal.len(),
        embedding.vertical.len(),
        &four_d_partition,
        &DinicBackend,
    )?;
    let clean_certificate = classify_clean_polygon(
        polygon,
        boundary,
        &families.horizontal,
        &families.vertical,
        &endpoint_index,
    );
    let mut path_tree_orientation = None;
    let (selected_partition, selected_flow, representation_name) = match options.representation {
        ConflictRepresentationBackend::CleanHoleFreePathTree
        | ConflictRepresentationBackend::Auto
            if clean_certificate.eligible =>
        {
            let vertical = build_boundary_path_tree_partition(
                boundary,
                &families.horizontal,
                &families.vertical,
                clean_certificate.clone(),
                PathTreeOrientation::VerticalTreeHorizontalPaths,
                Some(&endpoint_index),
                GapBackend::Experiment,
            )?;
            let horizontal = build_boundary_path_tree_partition(
                boundary,
                &families.horizontal,
                &families.vertical,
                clean_certificate.clone(),
                PathTreeOrientation::HorizontalTreeVerticalPaths,
                Some(&endpoint_index),
                GapBackend::Experiment,
            )?;
            let selected = if vertical.biclique_partition.total_vertex_occurrences()
                <= horizontal.biclique_partition.total_vertex_occurrences()
            {
                vertical
            } else {
                horizontal
            };
            path_tree_orientation = Some(selected.orientation.name().to_owned());
            let partition = selected.biclique_partition;
            let flow = solve_biclique_flow(
                embedding.horizontal.len(),
                embedding.vertical.len(),
                &partition,
                &DinicBackend,
            )?;
            if flow.flow.value != four_d_flow.flow.value {
                return Err(DominanceError::PathTreeMatchingMismatch {
                    path_tree: flow.flow.value,
                    four_d: four_d_flow.flow.value,
                });
            }
            (
                partition,
                flow,
                ConflictRepresentationBackend::CleanHoleFreePathTree.name(),
            )
        }
        ConflictRepresentationBackend::CleanHoleFreePathTree => {
            return Err(DominanceError::PathTreeIneligible(clean_certificate));
        }
        ConflictRepresentationBackend::GeneralDominance4D | ConflictRepresentationBackend::Auto => {
            (
                four_d_partition.clone(),
                four_d_flow.clone(),
                ConflictRepresentationBackend::GeneralDominance4D.name(),
            )
        }
    };
    let flow_at = Instant::now();
    let flow_value = usize::try_from(selected_flow.flow.value)
        .map_err(|_| DominanceError::FlowValueConversion)?;
    let selected_horizontal = selected_flow
        .vertex_cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = selected_flow
        .vertex_cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let total_chord_count = families.horizontal.len() + families.vertical.len();
    let independent_count = total_chord_count
        .checked_sub(flow_value)
        .ok_or(DominanceError::FormulaUnderflow)?;
    let optimum_rectangle_count = boundary
        .reflex_vertices
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_sub(boundary.hole_count()))
        .and_then(|value| value.checked_sub(independent_count))
        .ok_or(DominanceError::FormulaUnderflow)?;
    let completion = match options.verification_mode {
        VerificationMode::FullyAudited => {
            let reference = CoordinateCompressedCompletion.complete_prepared(
                &prepared,
                &families.horizontal,
                &families.vertical,
                &selected_horizontal,
                &selected_vertical,
            )?;
            let indexed = IndexedPolygonCompletion.complete_prepared_with_geometry_backends(
                &prepared,
                &families.horizontal,
                &families.vertical,
                &selected_horizontal,
                &selected_vertical,
                options.cut_index_backend,
                options.recovery_backend,
                options.dissection_validator_backend,
                options.subdivision_builder_backend,
                options.sparse_validator_backend,
            )?;
            let line_map = IndexedPolygonCompletion.complete_prepared_with_geometry_backends(
                &prepared,
                &families.horizontal,
                &families.vertical,
                &selected_horizontal,
                &selected_vertical,
                PolygonCutIndexBackend::ReferenceLineMaps,
                options.recovery_backend,
                options.dissection_validator_backend,
                SubdivisionBuilderBackend::ReferenceRangeScan,
                SparseValidatorBackend::ReferenceSlabRescan,
            )?;
            if reference.selected_horizontal_cuts != indexed.selected_horizontal_cuts
                || reference.selected_vertical_cuts != indexed.selected_vertical_cuts
                || reference.added_horizontal_cuts != indexed.added_horizontal_cuts
                || reference.added_vertical_cuts != indexed.added_vertical_cuts
                || reference.rectangles != indexed.rectangles
                || line_map.selected_horizontal_cuts != indexed.selected_horizontal_cuts
                || line_map.selected_vertical_cuts != indexed.selected_vertical_cuts
                || line_map.added_horizontal_cuts != indexed.added_horizontal_cuts
                || line_map.added_vertical_cuts != indexed.added_vertical_cuts
                || line_map.rectangles != indexed.rectangles
            {
                return Err(DominanceError::PolygonCompletionMismatch);
            }
            match options.completion_backend {
                PolygonCompletionBackend::CoordinateReference => reference,
                PolygonCompletionBackend::IndexedFrontier => indexed,
            }
        }
        VerificationMode::CompactOnly => match options.completion_backend {
            PolygonCompletionBackend::CoordinateReference => CoordinateCompressedCompletion
                .complete_prepared(
                    &prepared,
                    &families.horizontal,
                    &families.vertical,
                    &selected_horizontal,
                    &selected_vertical,
                )?,
            PolygonCompletionBackend::IndexedFrontier => IndexedPolygonCompletion
                .complete_prepared_with_geometry_backends(
                    &prepared,
                    &families.horizontal,
                    &families.vertical,
                    &selected_horizontal,
                    &selected_vertical,
                    options.cut_index_backend,
                    options.recovery_backend,
                    options.dissection_validator_backend,
                    options.subdivision_builder_backend,
                    options.sparse_validator_backend,
                )?,
        },
    };
    if completion.rectangles.len() != optimum_rectangle_count {
        return Err(DominanceError::CompletionCount {
            expected: optimum_rectangle_count,
            actual: completion.rectangles.len(),
        });
    }
    if options.verification_mode == VerificationMode::FullyAudited {
        validate_polygon_dissection_count(polygon, optimum_rectangle_count, &completion.rectangles)
            .map_err(PolygonSgError::from)?;
    }
    if options.verification_mode == VerificationMode::FullyAudited {
        let horizontal_cuts = completion
            .selected_horizontal_cuts
            .iter()
            .chain(&completion.added_horizontal_cuts)
            .copied()
            .collect::<BTreeSet<_>>();
        let vertical_cuts = completion
            .selected_vertical_cuts
            .iter()
            .chain(&completion.added_vertical_cuts)
            .copied()
            .collect::<BTreeSet<_>>();
        let arrangement =
            PreparedCoordinateArrangement::new(&prepared, &horizontal_cuts, &vertical_cuts)?;
        arrangement
            .validate_rectangles(polygon, &completion.rectangles)
            .map_err(PolygonSgError::from)?;
    }
    let completed_at = Instant::now();
    let dense_arrangement_used = options.verification_mode == VerificationMode::FullyAudited
        || options.completion_backend == PolygonCompletionBackend::CoordinateReference
        || completion.metrics.selected_recovery_backend == "dense-arrangement"
        || options.dissection_validator_backend
            == PolygonDissectionValidatorBackend::DenseArrangement;
    Ok(PolygonDissectionResult {
        optimum_rectangle_count,
        rectangles: completion.rectangles,
        diagnostics: Diagnostics {
            input_model: Some("rectilinear-polygon".to_owned()),
            polygon_outer_vertices: Some(polygon.outer.vertices.len()),
            polygon_hole_count: Some(polygon.holes.len()),
            polygon_hole_vertices: Some(polygon.hole_vertex_count()),
            polygon_validation_backend: Some(options.validation_backend.name().to_owned()),
            polygon_geometry_backend: Some(options.geometry_backend.name().to_owned()),
            polygon_chord_enumerator: Some(options.chord_backend.name().to_owned()),
            coordinate_compression_x_count: Some(completion.metrics.coordinate_compression_x_count),
            coordinate_compression_y_count: Some(completion.metrics.coordinate_compression_y_count),
            atomic_cell_count: Some(completion.metrics.atomic_cell_count),
            polygon_completion_backend: Some(options.completion_backend.name().to_owned()),
            polygon_arrangement_backend: Some(options.recovery_backend.name().to_owned()),
            polygon_selected_recovery_backend: Some(
                completion.metrics.selected_recovery_backend.clone(),
            ),
            dense_recovery_retained_byte_estimate: Some(
                completion.metrics.dense_recovery_retained_byte_estimate,
            ),
            sparse_recovery_retained_upper_estimate: Some(
                completion.metrics.sparse_recovery_retained_upper_estimate,
            ),
            polygon_validator_backend: Some(options.dissection_validator_backend.name().to_owned()),
            polygon_cut_index_backend: Some(options.cut_index_backend.name().to_owned()),
            polygon_prepare_build_count: Some(prepared.metrics().polygon_prepare_build_count),
            polygon_normalization_count: Some(prepared.metrics().polygon_normalization_count),
            polygon_validation_count: Some(prepared.metrics().polygon_validation_count),
            polygon_boundary_build_count: Some(prepared.metrics().polygon_boundary_build_count),
            polygon_boundary_index_build_count: Some(
                prepared.metrics().polygon_boundary_index_build_count,
            ),
            polygon_edge_index_build_count: Some(prepared.metrics().polygon_edge_index_build_count),
            polygon_prepare_microseconds: Some(prepared.metrics().polygon_prepare_microseconds),
            polygon_prepare_owned_bytes: Some(prepared.metrics().polygon_prepare_owned_bytes),
            polygon_boundary_edge_visits: chord_metrics
                .as_ref()
                .map(|metrics| metrics.polygon_boundary_edge_visits),
            polygon_point_location_queries: chord_metrics
                .as_ref()
                .map(|metrics| metrics.polygon_point_location_queries),
            polygon_segment_reporting_queries: chord_metrics
                .as_ref()
                .map(|metrics| metrics.polygon_segment_reporting_queries),
            polygon_reported_boundary_intersections: chord_metrics
                .as_ref()
                .map(|metrics| metrics.polygon_reported_boundary_intersections),
            polygon_aligned_reflex_candidate_pairs: chord_metrics
                .as_ref()
                .map(|metrics| metrics.polygon_aligned_reflex_candidate_pairs),
            polygon_unaligned_reflex_pair_checks: chord_metrics
                .as_ref()
                .map(|metrics| metrics.polygon_unaligned_reflex_pair_checks),
            polygon_definition7_full_boundary_scans: chord_metrics
                .as_ref()
                .map(|metrics| metrics.polygon_definition7_full_boundary_scans),
            sweep_horizontal_event_count: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_horizontal_event_count),
            sweep_vertical_event_count: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_vertical_event_count),
            sweep_status_insertions: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_status_insertions),
            sweep_status_deletions: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_status_deletions),
            sweep_status_queries: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_status_queries),
            sweep_auxiliary_tree_operations: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_auxiliary_tree_operations),
            sweep_output_horizontal_chords: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_output_horizontal_chords),
            sweep_output_vertical_chords: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_output_vertical_chords),
            sweep_duplicate_output_count: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_duplicate_output_count),
            sweep_aligned_pair_iterations: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_aligned_pair_iterations),
            sweep_all_pair_iterations: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_all_pair_iterations),
            sweep_definition7_fallback_checks: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_definition7_fallback_checks),
            sweep_full_boundary_scans: chord_metrics
                .as_ref()
                .map(|metrics| metrics.sweep_full_boundary_scans),
            polygon_completion_candidate_rebuilds: Some(
                completion.metrics.completion_global_candidate_rebuilds,
            ),
            polygon_completion_cut_pair_tests: Some(completion.metrics.completion_cut_pair_tests),
            polygon_completion_intersections_reported: Some(
                completion.metrics.completion_intersections_reported,
            ),
            polygon_completion_candidate_insertions: Some(
                completion.metrics.completion_candidate_insertions,
            ),
            polygon_completion_candidate_revalidations: Some(
                completion.metrics.completion_candidate_revalidations,
            ),
            polygon_completion_stale_candidates: Some(
                completion.metrics.completion_stale_candidates,
            ),
            polygon_completion_boundary_ray_queries: Some(
                completion.metrics.completion_boundary_ray_queries,
            ),
            polygon_completion_cut_ray_queries: Some(completion.metrics.completion_cut_ray_queries),
            polygon_completion_full_boundary_scans: Some(
                completion.metrics.completion_full_boundary_scans,
            ),
            polygon_completion_full_cut_scans: Some(completion.metrics.completion_full_cut_scans),
            polygon_arrangement_point_location_queries: Some(
                completion.metrics.arrangement_point_location_queries,
            ),
            polygon_arrangement_boundary_edge_visits: Some(
                completion.metrics.arrangement_boundary_edge_visits,
            ),
            polygon_arrangement_span_writes: Some(completion.metrics.arrangement_span_writes),
            polygon_validator_rectangle_cell_tests: Some(
                completion.metrics.polygon_validator_rectangle_cell_tests,
            ),
            cut_index_insertions: Some(completion.metrics.cut_index.insertions),
            cut_index_canonical_node_insertions: Some(
                completion.metrics.cut_index.canonical_node_insertions,
            ),
            cut_index_stabbing_queries: Some(completion.metrics.cut_index.stabbing_queries),
            cut_index_tree_node_visits: Some(completion.metrics.cut_index.tree_node_visits),
            cut_index_ordered_set_queries: Some(completion.metrics.cut_index.ordered_set_queries),
            cut_index_reported_intersections: Some(
                completion.metrics.cut_index.reported_intersections,
            ),
            cut_index_coordinate_line_scans: Some(
                completion.metrics.cut_index.coordinate_line_scans,
            ),
            cut_index_interval_scans: Some(completion.metrics.cut_index.interval_scans),
            cut_index_logical_tree_node_count: Some(
                completion.metrics.cut_index.logical_tree_node_count,
            ),
            cut_index_materialized_tree_node_count: Some(
                completion.metrics.cut_index.materialized_tree_node_count,
            ),
            cut_index_ordered_set_entry_count: Some(
                completion.metrics.cut_index.ordered_set_entry_count,
            ),
            cut_index_owned_bytes: Some(completion.metrics.cut_index.owned_bytes),
            cut_index_memory_estimate: Some(completion.metrics.cut_index.memory_estimate),
            sparse_subdivision_vertex_count: Some(completion.metrics.sparse_subdivision_vertices),
            sparse_subdivision_half_edge_count: Some(
                completion.metrics.sparse_subdivision_half_edges,
            ),
            sparse_subdivision_face_count: Some(completion.metrics.sparse_subdivision_faces),
            sparse_subdivision_junction_count: Some(
                completion.metrics.sparse_subdivision_junctions,
            ),
            sparse_subdivision_owned_bytes: Some(completion.metrics.sparse_subdivision_owned_bytes),
            sparse_subdivision_memory_estimate: Some(
                completion.metrics.sparse_subdivision.memory_estimate,
            ),
            subdivision_builder_backend: Some(
                completion
                    .metrics
                    .sparse_subdivision
                    .builder_backend
                    .clone(),
            ),
            subdivision_input_segment_count: Some(
                completion.metrics.sparse_subdivision.input_segment_count,
            ),
            subdivision_horizontal_segment_count: Some(
                completion
                    .metrics
                    .sparse_subdivision
                    .horizontal_segment_count,
            ),
            subdivision_vertical_segment_count: Some(
                completion.metrics.sparse_subdivision.vertical_segment_count,
            ),
            subdivision_sweep_event_count: Some(
                completion.metrics.sparse_subdivision.sweep_event_count,
            ),
            subdivision_active_set_insertions: Some(
                completion.metrics.sparse_subdivision.active_set_insertions,
            ),
            subdivision_active_set_removals: Some(
                completion.metrics.sparse_subdivision.active_set_removals,
            ),
            subdivision_range_queries: Some(completion.metrics.sparse_subdivision.range_queries),
            subdivision_candidate_pair_tests: Some(
                completion.metrics.sparse_subdivision.candidate_pair_tests,
            ),
            subdivision_reported_intersections: Some(
                completion.metrics.sparse_subdivision.reported_intersections,
            ),
            subdivision_t_junction_count: Some(
                completion.metrics.sparse_subdivision.t_junction_count,
            ),
            subdivision_endpoint_contact_count: Some(
                completion.metrics.sparse_subdivision.endpoint_contact_count,
            ),
            subdivision_atomic_segment_count: Some(
                completion.metrics.sparse_subdivision.atomic_segment_count,
            ),
            subdivision_materialized_split_coordinates: Some(
                completion
                    .metrics
                    .sparse_subdivision
                    .materialized_split_coordinates,
            ),
            sparse_validator_slab_count: Some(completion.metrics.sparse_validator_slab_count),
            sparse_validator_backend: Some(
                completion
                    .metrics
                    .sparse_validator
                    .validator_backend
                    .clone(),
            ),
            validator_x_event_count: Some(completion.metrics.sparse_validator.x_event_count),
            validator_y_coordinate_count: Some(
                completion.metrics.sparse_validator.y_coordinate_count,
            ),
            validator_range_add_count: Some(completion.metrics.sparse_validator.range_add_count),
            validator_parity_toggle_count: Some(
                completion.metrics.sparse_validator.parity_toggle_count,
            ),
            validator_segment_tree_node_visits: Some(
                completion.metrics.sparse_validator.segment_tree_node_visits,
            ),
            validator_root_checks: Some(completion.metrics.sparse_validator.root_checks),
            validator_boundary_edge_scans: Some(
                completion.metrics.sparse_validator.boundary_edge_scans,
            ),
            validator_active_rectangle_resorts: Some(
                completion.metrics.sparse_validator.active_rectangle_resorts,
            ),
            validator_owned_bytes: Some(completion.metrics.sparse_validator.owned_bytes),
            validator_memory_estimate: Some(completion.metrics.sparse_validator.memory_estimate),
            raster_oracle_used: Some(false),
            boundary_complexity: boundary.boundary_complexity(),
            outer_loop_count: boundary.outer_loop_count(),
            hole_count: boundary.hole_count(),
            reflex_vertex_count: boundary.reflex_vertices.len(),
            horizontal_chord_count: families.horizontal.len(),
            vertical_chord_count: families.vertical.len(),
            total_chord_count,
            explicit_conflict_edge_count: None,
            biclique_count: selected_partition.blocks.len(),
            biclique_total_vertex_occurrences: selected_partition.total_vertex_occurrences(),
            compressed_network_vertex_count: selected_flow.network_vertex_count,
            compressed_network_arc_count: selected_flow.network_arc_count,
            maximum_matching_size: flow_value,
            minimum_vertex_cover_size: selected_flow.vertex_cover.size,
            output_rectangle_count: optimum_rectangle_count,
            conflict_representation: Some(representation_name.to_owned()),
            clean_hole_free_eligible: Some(clean_certificate.eligible),
            path_tree_orientation,
            path_tree_orientation_policy: Some("build-both".to_owned()),
            phase_microseconds: [
                (
                    "polygon_prepare".to_owned(),
                    prepared.metrics().polygon_prepare_microseconds,
                ),
                (
                    "polygon_geometry".to_owned(),
                    geometry_at.duration_since(started).as_micros(),
                ),
                (
                    "polygon_sweep_horizontal".to_owned(),
                    chord_metrics
                        .as_ref()
                        .map_or(0, |metrics| metrics.sweep_horizontal_microseconds),
                ),
                (
                    "polygon_sweep_vertical".to_owned(),
                    chord_metrics
                        .as_ref()
                        .map_or(0, |metrics| metrics.sweep_vertical_microseconds),
                ),
                (
                    "compact_matching".to_owned(),
                    flow_at.duration_since(geometry_at).as_micros(),
                ),
                (
                    "polygon_completion_validation".to_owned(),
                    completed_at.duration_since(flow_at).as_micros(),
                ),
                (
                    "polygon_selected_cut_materialization".to_owned(),
                    completion.metrics.selected_cut_materialization_microseconds,
                ),
                (
                    "polygon_horizontal_completion".to_owned(),
                    completion.metrics.horizontal_completion_microseconds,
                ),
                (
                    "polygon_vertical_completion".to_owned(),
                    completion.metrics.vertical_completion_microseconds,
                ),
                (
                    "polygon_rectangle_recovery".to_owned(),
                    completion.metrics.rectangle_recovery_microseconds,
                ),
                (
                    "polygon_final_validation".to_owned(),
                    completion.metrics.final_validation_microseconds,
                ),
            ]
            .into_iter()
            .collect(),
            execution_trace: ExecutionTrace {
                compact_structure_check_called: true,
                dense_atomic_cells_materialized: dense_arrangement_used,
                dense_occupied_array_materialized: dense_arrangement_used,
                dense_horizontal_barrier_array_materialized: dense_arrangement_used,
                dense_vertical_barrier_array_materialized: dense_arrangement_used,
                dense_coverage_difference_array_materialized: dense_arrangement_used,
                ..ExecutionTrace::default()
            },
            owned_allocation_estimates: BTreeMap::from([
                (
                    "polygon_prepared_context".to_owned(),
                    prepared.metrics().polygon_prepare_owned_bytes,
                ),
                (
                    "polygon_arrangement".to_owned(),
                    completion.metrics.arrangement_owned_bytes,
                ),
                (
                    "polygon_cut_index".to_owned(),
                    completion.metrics.cut_index.owned_bytes,
                ),
                (
                    "polygon_sparse_subdivision".to_owned(),
                    completion.metrics.sparse_subdivision_owned_bytes,
                ),
            ]),
            ..Diagnostics::default()
        },
        certificate: Some(Certificate {
            kind: "boundary-native-polygon-compact".to_owned(),
            payload: json!({
                "horizontal_chords": families.horizontal,
                "vertical_chords": families.vertical,
                "selected_horizontal": selected_horizontal,
                "selected_vertical": selected_vertical,
                "selected_horizontal_cuts": completion.selected_horizontal_cuts,
                "selected_vertical_cuts": completion.selected_vertical_cuts,
                "added_horizontal_cuts": completion.added_horizontal_cuts,
                "added_vertical_cuts": completion.added_vertical_cuts,
                "flow_value": flow_value,
                "representation": representation_name,
                "sweep_certificate": sweep_certificate,
            }),
        }),
    })
}

/// Solves with an explicit conflict-representation backend.
///
/// Existing solver wrappers continue to use the general four-dimensional
/// representation. `Auto` selects the geometry-derived path/tree backend only
/// when the clean certificate is valid.
///
/// # Errors
///
/// Returns [`DominanceError`] when eligibility, representation, flow,
/// completion, or validation invariants fail.
pub fn solve_with_representation<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
    representation: ConflictRepresentationBackend,
    enumerator: ChordEnumerator,
    completion_backend: CompletionBackendKind,
) -> Result<DissectionResult, DominanceError> {
    solve_with_representation_and_region_dual(
        component,
        mode,
        representation,
        enumerator,
        completion_backend,
        match mode {
            VerificationMode::FullyAudited => RegionBackend::Oracle,
            VerificationMode::CompactOnly => RegionBackend::Experiment,
        },
    )
}

/// Solver entry point with an explicit path-tree region-dual backend.
///
/// # Errors
///
/// Returns [`DominanceError`] when geometry, representation, flow,
/// completion, or validation invariants fail.
pub fn solve_with_representation_and_region_dual<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
    representation: ConflictRepresentationBackend,
    enumerator: ChordEnumerator,
    completion_backend: CompletionBackendKind,
    region_dual: RegionBackend,
) -> Result<DissectionResult, DominanceError> {
    solve_with_representation_and_region_dual_and_orientation_policy(
        component,
        mode,
        representation,
        enumerator,
        completion_backend,
        region_dual,
        default_orientation_policy(mode),
    )
}

/// Solver entry point with an explicit path-tree orientation policy.
///
/// # Errors
///
/// Returns [`DominanceError`] when geometry, representation, flow,
/// completion, or validation invariants fail.
pub fn solve_with_representation_and_region_dual_and_orientation_policy<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
    representation: ConflictRepresentationBackend,
    enumerator: ChordEnumerator,
    completion_backend: CompletionBackendKind,
    region_dual: RegionBackend,
    orientation_policy: PathTreeOrientationPolicy,
) -> Result<DissectionResult, DominanceError> {
    solve_with_representation_and_path_tree_options(
        component,
        mode,
        representation,
        enumerator,
        completion_backend,
        region_dual,
        orientation_policy,
        GapBackend::Experiment,
    )
}

/// Solver entry point with explicit path-tree construction backends.
///
/// This is primarily intended for differential verification. Existing public
/// wrappers retain the production [`GapBackend::Experiment`]
/// default.
///
/// # Errors
///
/// Returns [`DominanceError`] when geometry, representation, flow,
/// completion, or validation invariants fail.
#[allow(clippy::too_many_arguments)]
pub fn solve_with_representation_and_path_tree_options<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
    representation: ConflictRepresentationBackend,
    enumerator: ChordEnumerator,
    completion_backend: CompletionBackendKind,
    region_dual: RegionBackend,
    orientation_policy: PathTreeOrientationPolicy,
    gap_backend: GapBackend,
) -> Result<DissectionResult, DominanceError> {
    match representation {
        ConflictRepresentationBackend::GeneralDominance4D => {
            solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                component,
                mode,
                enumerator,
                completion_backend,
            )
            .map(|result| annotate_general_representation(result, None))
        }
        ConflictRepresentationBackend::CleanHoleFreePathTree => solve_path_tree_dispatch(
            component,
            mode,
            enumerator,
            completion_backend,
            region_dual,
            orientation_policy,
            gap_backend,
        ),
        ConflictRepresentationBackend::Auto => {
            let geometry = match enumerator {
                ChordEnumerator::ReferencePairwise => {
                    rect_oracle_sg::analyze_geometry_with(component, &ReferencePairwiseEnumerator)?
                }
                ChordEnumerator::GridInteriorRuns => {
                    rect_oracle_sg::analyze_geometry_with(component, &GridInteriorRunEnumerator)?
                }
            };
            let certificate = rect_oracle_sg::classify_clean_hole_free_with_endpoint_index(
                component,
                &geometry.boundary,
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
                &geometry.endpoint_index,
            );
            if certificate.eligible {
                solve_path_tree_with_geometry(
                    component,
                    &geometry,
                    &certificate,
                    PathTreeSolveOptions {
                        mode,
                        completion_backend,
                        region_dual,
                        orientation_policy,
                        gap_backend,
                    },
                )
            } else {
                solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                    component,
                    mode,
                    enumerator,
                    completion_backend,
                )
                .map(|result| annotate_general_representation(result, Some(false)))
            }
        }
    }
}

const fn default_orientation_policy(mode: VerificationMode) -> PathTreeOrientationPolicy {
    match mode {
        // BoundEstimate remains an explicit benchmark policy; the expanded
        // v0.8 witness population contains positive-regret cases.
        VerificationMode::FullyAudited | VerificationMode::CompactOnly => {
            PathTreeOrientationPolicy::BuildBothExact
        }
    }
}

#[derive(Clone, Copy)]
struct PathTreeSolveOptions {
    mode: VerificationMode,
    completion_backend: CompletionBackendKind,
    region_dual: RegionBackend,
    orientation_policy: PathTreeOrientationPolicy,
    gap_backend: GapBackend,
}

const fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn annotate_general_representation(
    mut result: DissectionResult,
    clean_hole_free_eligible: Option<bool>,
) -> DissectionResult {
    result.diagnostics.conflict_representation = Some(
        ConflictRepresentationBackend::GeneralDominance4D
            .name()
            .to_owned(),
    );
    result.diagnostics.clean_hole_free_eligible = clean_hole_free_eligible;
    result
}

/// Solves with either the fully audited compact pipeline or the compact-only
/// execution path that never materializes the conflict edge set.
///
/// # Errors
///
/// Returns [`DominanceError`] when a construction, flow, completion, or output
/// invariant fails.
pub fn solve_with_verification_mode<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
) -> Result<DissectionResult, DominanceError> {
    solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
        component,
        mode,
        ChordEnumerator::GridInteriorRuns,
        match mode {
            VerificationMode::FullyAudited => CompletionBackendKind::ReferenceRescan,
            VerificationMode::CompactOnly => CompletionBackendKind::IndexedFrontier,
        },
    )
}

/// Solves with an explicit verification mode and effective-chord enumerator.
///
/// # Errors
///
/// Returns [`DominanceError`] when geometry, dominance, flow, completion, or
/// output validation fails.
pub fn solve_with_verification_mode_and_chord_enumerator<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
    enumerator: ChordEnumerator,
) -> Result<DissectionResult, DominanceError> {
    solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
        component,
        mode,
        enumerator,
        CompletionBackendKind::ReferenceRescan,
    )
}

/// Solves with explicit verification, chord-enumerator, and completion backends.
///
/// # Errors
///
/// Returns [`DominanceError`] when geometry, flow, completion, or validation
/// invariants fail.
pub fn solve_with_verification_mode_and_chord_enumerator_and_completion_backend<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
    enumerator: ChordEnumerator,
    completion_backend: CompletionBackendKind,
) -> Result<DissectionResult, DominanceError> {
    match mode {
        VerificationMode::FullyAudited => match enumerator {
            ChordEnumerator::ReferencePairwise => solve_fully_audited_with(
                component,
                DominanceMode::Compact,
                &ReferencePairwiseEnumerator,
                completion_backend,
            ),
            ChordEnumerator::GridInteriorRuns => solve_fully_audited_with(
                component,
                DominanceMode::Compact,
                &GridInteriorRunEnumerator,
                completion_backend,
            ),
        },
        VerificationMode::CompactOnly => match enumerator {
            ChordEnumerator::ReferencePairwise => {
                solve_compact_only_with(component, &ReferencePairwiseEnumerator, completion_backend)
            }
            ChordEnumerator::GridInteriorRuns => {
                solve_compact_only_with(component, &GridInteriorRunEnumerator, completion_backend)
            }
        },
    }
}

/// Solves through the paper embedding and either C0 or Theorem 8 bicliques.
///
/// # Errors
///
/// Returns [`DominanceError`] when any geometric equivalence, biclique,
/// matching/flow, cover, completion, or output invariant fails.
#[allow(clippy::too_many_lines)]
pub fn solve<C>(
    component: &GridComponent<C>,
    mode: DominanceMode,
) -> Result<DissectionResult, DominanceError> {
    solve_with_flow_backend(component, mode, FlowBackendKind::Dinic)
}

/// Solves through the fully audited paper embedding with a selected exact
/// integral max-flow backend.
///
/// # Errors
///
/// Returns [`DominanceError`] when any geometric equivalence, biclique,
/// matching/flow, cover, completion, or output invariant fails.
pub fn solve_with_flow_backend<C>(
    component: &GridComponent<C>,
    mode: DominanceMode,
    backend: FlowBackendKind,
) -> Result<DissectionResult, DominanceError> {
    solve_fully_audited_with_backend(
        component,
        mode,
        &ReferencePairwiseEnumerator,
        CompletionBackendKind::ReferenceRescan,
        &backend,
    )
}

#[allow(clippy::too_many_lines)]
fn solve_fully_audited_with<C, E: EffectiveChordEnumerator>(
    component: &GridComponent<C>,
    mode: DominanceMode,
    enumerator: &E,
    completion_backend: CompletionBackendKind,
) -> Result<DissectionResult, DominanceError> {
    solve_fully_audited_with_backend(
        component,
        mode,
        enumerator,
        completion_backend,
        &DinicBackend,
    )
}

#[allow(clippy::too_many_lines)]
fn solve_fully_audited_with_backend<C, E: EffectiveChordEnumerator, B: MaxFlowBackend>(
    component: &GridComponent<C>,
    mode: DominanceMode,
    enumerator: &E,
    completion_backend: CompletionBackendKind,
    backend: &B,
) -> Result<DissectionResult, DominanceError> {
    let started = Instant::now();
    let sg_analysis = rect_oracle_sg::analyze_with(component, enumerator)?;
    if enumerator.name() == "grid-interior-runs" {
        let reference = rect_oracle_sg::analyze_geometry(component)?;
        if reference.horizontal_chords != sg_analysis.horizontal_chords
            || reference.vertical_chords != sg_analysis.vertical_chords
        {
            return Err(DominanceError::ChordFamilyMismatch);
        }
    }
    let geometry_at = Instant::now();
    let embedding =
        DominanceEmbedding::new(&sg_analysis.horizontal_chords, &sg_analysis.vertical_chords)?;
    embedding.assert_pairwise_equivalence(
        &sg_analysis.horizontal_chords,
        &sg_analysis.vertical_chords,
    )?;
    let dominance_graph = embedding.explicit_graph()?;
    let sg_edges = sg_analysis.conflict_graph.edges().collect::<Vec<_>>();
    let dominance_edges = dominance_graph.edges().collect::<Vec<_>>();
    if dominance_edges != sg_edges {
        return Err(DominanceError::ExplicitGraphMismatch);
    }
    let embedding_at = Instant::now();

    let partition = match mode {
        DominanceMode::ExplicitEdges => Partition::from_explicit_edges(&dominance_graph),
        DominanceMode::Compact => Partition::comparability_theorem_8_audited(&embedding)?.partition,
    };
    partition.verify_exact_partition(&dominance_graph)?;
    let biclique_certificate = partition.certificate(&dominance_graph);
    let bicliques_at = Instant::now();
    let flow_solution = solve_biclique_flow(
        embedding.horizontal.len(),
        embedding.vertical.len(),
        &partition,
        backend,
    )?;
    let flow_value = usize::try_from(flow_solution.flow.value)
        .map_err(|_| DominanceError::FlowValueConversion)?;
    if flow_value != sg_analysis.matching.size {
        return Err(DominanceError::MatchingFlowMismatch {
            matching: sg_analysis.matching.size,
            flow: flow_value,
        });
    }
    for (left, right) in dominance_graph.edges() {
        if !flow_solution.vertex_cover.left[left] && !flow_solution.vertex_cover.right[right] {
            return Err(DominanceError::UncoveredConflictEdge { left, right });
        }
    }
    let selected_horizontal = flow_solution
        .vertex_cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = flow_solution
        .vertex_cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let flow_at = Instant::now();
    let completion = complete_selected(
        component,
        &sg_analysis.prepared,
        &sg_analysis.horizontal_chords,
        &sg_analysis.vertical_chords,
        &selected_horizontal,
        &selected_vertical,
        completion_backend,
    )?;
    if completion.rectangles.len() != sg_analysis.optimum_rectangle_count {
        return Err(DominanceError::CompletionCount {
            expected: sg_analysis.optimum_rectangle_count,
            actual: completion.rectangles.len(),
        });
    }
    let completed_at = Instant::now();

    let selected_horizontal_indices = selected_horizontal
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let selected_vertical_indices = selected_vertical
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let horizontal_count = embedding.horizontal.len();
    let vertical_count = embedding.vertical.len();
    let total_chord_count = horizontal_count + vertical_count;
    let explicit_edge_count = dominance_graph.edge_count();
    let biclique_total_size = partition.total_vertex_occurrences();
    let c0_network_vertex_count = 2_usize
        .checked_add(total_chord_count)
        .and_then(|value| value.checked_add(explicit_edge_count))
        .ok_or(DominanceError::MetricOverflow)?;
    let c0_network_arc_count = explicit_edge_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(total_chord_count))
        .ok_or(DominanceError::MetricOverflow)?;
    let (compressed_network_vertex_count, compressed_network_arc_count) = match mode {
        DominanceMode::ExplicitEdges => (0, 0),
        DominanceMode::Compact => (
            flow_solution.network_vertex_count,
            flow_solution.network_arc_count,
        ),
    };
    let result = DissectionResult {
        optimum_rectangle_count: sg_analysis.optimum_rectangle_count,
        rectangles: completion.rectangles,
        diagnostics: Diagnostics {
            cell_count: component.cell_count(),
            boundary_complexity: sg_analysis.boundary.boundary_complexity(),
            outer_loop_count: sg_analysis.boundary.outer_loop_count(),
            hole_count: sg_analysis.boundary.hole_count(),
            reflex_vertex_count: sg_analysis.boundary.reflex_vertices.len(),
            horizontal_chord_count: horizontal_count,
            vertical_chord_count: vertical_count,
            total_chord_count,
            explicit_conflict_edge_count: Some(explicit_edge_count),
            conflict_edge_density: ExactRatio::new(
                explicit_edge_count as u128,
                (horizontal_count as u128) * (vertical_count as u128),
            ),
            biclique_count: partition.blocks.len(),
            biclique_total_vertex_occurrences: biclique_total_size,
            biclique_size_per_chord: ExactRatio::new(
                biclique_total_size as u128,
                total_chord_count as u128,
            ),
            biclique_size_per_explicit_edge: ExactRatio::new(
                biclique_total_size as u128,
                explicit_edge_count as u128,
            ),
            c0_network_vertex_count,
            c0_network_arc_count,
            compressed_network_vertex_count,
            compressed_network_arc_count,
            maximum_matching_size: flow_value,
            minimum_vertex_cover_size: flow_solution.vertex_cover.size,
            output_rectangle_count: sg_analysis.optimum_rectangle_count,
            phase_microseconds: [
                (
                    "boundary_effective_chords".to_owned(),
                    geometry_at.duration_since(started).as_micros(),
                ),
                (
                    "dominance_embedding".to_owned(),
                    embedding_at.duration_since(geometry_at).as_micros(),
                ),
                (
                    "biclique_partition".to_owned(),
                    bicliques_at.duration_since(embedding_at).as_micros(),
                ),
                (
                    "compressed_flow".to_owned(),
                    flow_at.duration_since(bicliques_at).as_micros(),
                ),
                (
                    "geometric_completion".to_owned(),
                    completed_at.duration_since(flow_at).as_micros(),
                ),
                (
                    "selected_chord_cut_materialization".to_owned(),
                    completion
                        .metrics
                        .selected_chord_cut_materialization_microseconds,
                ),
                (
                    "horizontal_simple_chord_completion".to_owned(),
                    completion
                        .metrics
                        .horizontal_simple_chord_completion_microseconds,
                ),
                (
                    "vertical_simple_chord_completion".to_owned(),
                    completion
                        .metrics
                        .vertical_simple_chord_completion_microseconds,
                ),
                (
                    "rectangle_recovery".to_owned(),
                    completion.metrics.rectangle_recovery_microseconds,
                ),
                (
                    "final_output_validation".to_owned(),
                    completion.metrics.final_output_validation_microseconds,
                ),
            ]
            .into_iter()
            .collect(),
            peak_memory_bytes: None,
            execution_trace: ExecutionTrace {
                pairwise_embedding_audit_called: true,
                explicit_conflict_graph_built: true,
                hopcroft_karp_called: true,
                c0_partition_built: matches!(mode, DominanceMode::ExplicitEdges),
                full_edge_partition_audit_called: true,
                compact_structure_check_called: true,
                ..ExecutionTrace::default()
            },
            effective_chord_enumerator: Some(enumerator.name().to_owned()),
            effective_chord_enumeration_microseconds: Some(
                geometry_at.duration_since(started).as_micros(),
            ),
            conflict_representation: Some(
                ConflictRepresentationBackend::GeneralDominance4D
                    .name()
                    .to_owned(),
            ),
            emitted_chord_count: Some(total_chord_count),
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: None,
            owned_allocation_estimates: BTreeMap::new(),
            completion_backend: Some(completion_backend.name().to_owned()),
            ..completion_diagnostics(&completion.metrics)
        },
        certificate: Some(Certificate {
            kind: match mode {
                DominanceMode::ExplicitEdges => "dominance-c0",
                DominanceMode::Compact => "dominance-compact",
            }
            .to_owned(),
            payload: json!({
                "verification_mode": VerificationMode::FullyAudited,
                "embedding": embedding,
                "biclique_partition": biclique_certificate,
                "flow_value": flow_value,
                "compressed_network_vertex_count": flow_solution.network_vertex_count,
                "compressed_network_arc_count": flow_solution.network_arc_count,
                "internal_capacity": flow_solution.internal_capacity,
                "internal_cut_arc_count": flow_solution.internal_cut_arc_count,
                "min_cut_source_side": flow_solution.flow.source_side,
                "cover_left": flow_solution.vertex_cover.left,
                "cover_right": flow_solution.vertex_cover.right,
                "selected_horizontal": selected_horizontal_indices,
                "selected_vertical": selected_vertical_indices,
            }),
        }),
    };
    validate_dissection(component, &result)?;
    Ok(result)
}

#[allow(clippy::too_many_lines)]
fn solve_compact_only_with<C, E: EffectiveChordEnumerator>(
    component: &GridComponent<C>,
    enumerator: &E,
    completion_backend: CompletionBackendKind,
) -> Result<DissectionResult, DominanceError> {
    let started = Instant::now();
    let context = PreparedComponentContext::new(component).map_err(SgError::from)?;
    let geometry = analyze_prepared_geometry(context, enumerator)?;
    let geometry_at = Instant::now();
    let embedding =
        DominanceEmbedding::new(&geometry.horizontal_chords, &geometry.vertical_chords)?;
    let embedding_at = Instant::now();
    let partition = biclique::experiment::construct(&embedding)?.partition;
    partition.verify_dominance_blocks(&embedding)?;
    let bicliques_at = Instant::now();
    let flow_solution = solve_biclique_flow(
        embedding.horizontal.len(),
        embedding.vertical.len(),
        &partition,
        &DinicBackend,
    )?;
    let flow_value = usize::try_from(flow_solution.flow.value)
        .map_err(|_| DominanceError::FlowValueConversion)?;
    let selected_horizontal = flow_solution
        .vertex_cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = flow_solution
        .vertex_cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let flow_at = Instant::now();

    let horizontal_count = embedding.horizontal.len();
    let vertical_count = embedding.vertical.len();
    let total_chord_count = horizontal_count
        .checked_add(vertical_count)
        .ok_or(DominanceError::MetricOverflow)?;
    let independent_count = total_chord_count
        .checked_sub(flow_value)
        .ok_or(DominanceError::FormulaUnderflow)?;
    let formula_base = geometry
        .boundary
        .reflex_vertices
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_sub(geometry.boundary.hole_count()))
        .ok_or(DominanceError::FormulaUnderflow)?;
    let optimum_rectangle_count = formula_base
        .checked_sub(independent_count)
        .ok_or(DominanceError::FormulaUnderflow)?;
    let completion = complete_selected(
        component,
        &geometry.prepared,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        &selected_horizontal,
        &selected_vertical,
        completion_backend,
    )?;
    if completion.rectangles.len() != optimum_rectangle_count {
        return Err(DominanceError::CompletionCount {
            expected: optimum_rectangle_count,
            actual: completion.rectangles.len(),
        });
    }
    let completed_at = Instant::now();

    let selected_horizontal_indices = selected_horizontal
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let selected_vertical_indices = selected_vertical
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let biclique_total_size = partition.total_vertex_occurrences();
    let mut owned_allocation_estimates = BTreeMap::new();
    owned_allocation_estimates.insert(
        "chord_vectors".to_owned(),
        total_chord_count * size_of::<rect_core::HorizontalChord>(),
    );
    owned_allocation_estimates.insert(
        "boundary_index".to_owned(),
        geometry.boundary_index.owned_bytes_estimate(),
    );
    owned_allocation_estimates.insert(
        "endpoint_tables".to_owned(),
        geometry.endpoint_index.owned_bytes_estimate(),
    );
    owned_allocation_estimates.insert(
        "embedding_point_arrays".to_owned(),
        total_chord_count * size_of::<embedding::DominancePoint>(),
    );
    owned_allocation_estimates.insert(
        "biclique_vectors".to_owned(),
        partition.blocks.len() * size_of::<biclique::Block>()
            + biclique_total_size * size_of::<usize>(),
    );
    owned_allocation_estimates.insert(
        "flow_graph_storage".to_owned(),
        (flow_solution.network_vertex_count + flow_solution.network_arc_count) * size_of::<usize>(),
    );
    owned_allocation_estimates.insert(
        "certificate_payload".to_owned(),
        total_chord_count * size_of::<embedding::DominancePoint>()
            + partition.blocks.len() * size_of::<biclique::Block>()
            + biclique_total_size * size_of::<usize>()
            + (flow_solution.flow.source_side.len()
                + flow_solution.vertex_cover.left.len()
                + flow_solution.vertex_cover.right.len()
                + selected_horizontal_indices.len()
                + selected_vertical_indices.len())
                * size_of::<usize>(),
    );
    owned_allocation_estimates.insert(
        "completion_output_vectors".to_owned(),
        completion.selected_horizontal_unit_cuts.len()
            * size_of::<rect_oracle_sg::HorizontalUnitCut>()
            + completion.selected_vertical_unit_cuts.len()
                * size_of::<rect_oracle_sg::VerticalUnitCut>()
            + completion.added_horizontal_unit_cuts.len()
                * size_of::<rect_oracle_sg::HorizontalUnitCut>()
            + completion.added_vertical_unit_cuts.len()
                * size_of::<rect_oracle_sg::VerticalUnitCut>()
            + completion.rectangles.len() * size_of::<rect_core::GridRect>(),
    );
    owned_allocation_estimates.insert(
        "prepared_grid_storage_estimate".to_owned(),
        geometry.prepared.occupancy.len()
            + geometry.prepared.occupancy_prefix_sums.len() * size_of::<usize>()
            + geometry
                .prepared
                .horizontal_interior_runs
                .iter()
                .map(Vec::len)
                .sum::<usize>()
                * size_of::<(usize, usize)>()
            + geometry
                .prepared
                .vertical_interior_runs
                .iter()
                .map(Vec::len)
                .sum::<usize>()
                * size_of::<(usize, usize)>(),
    );
    if completion_backend == CompletionBackendKind::IndexedFrontier
        && let Some((x0, y0, x1, y1)) = component.bounds()
    {
        let width = x1 - x0;
        let height = y1 - y0;
        owned_allocation_estimates.insert(
            "completion_dense_arrays_estimate".to_owned(),
            width * (height + 1)
                + (width + 1) * height
                + (width + 1) * (height + 1) * size_of::<u64>(),
        );
        owned_allocation_estimates.insert(
            "rectangle_recovery_dense_storage_estimate".to_owned(),
            width * height + component.cell_count() * size_of::<usize>(),
        );
    }
    let result = DissectionResult {
        optimum_rectangle_count,
        rectangles: completion.rectangles,
        diagnostics: Diagnostics {
            cell_count: component.cell_count(),
            boundary_complexity: geometry.boundary.boundary_complexity(),
            outer_loop_count: geometry.boundary.outer_loop_count(),
            hole_count: geometry.boundary.hole_count(),
            reflex_vertex_count: geometry.boundary.reflex_vertices.len(),
            horizontal_chord_count: horizontal_count,
            vertical_chord_count: vertical_count,
            total_chord_count,
            explicit_conflict_edge_count: None,
            conflict_edge_density: None,
            biclique_count: partition.blocks.len(),
            biclique_total_vertex_occurrences: biclique_total_size,
            biclique_size_per_chord: ExactRatio::new(
                biclique_total_size as u128,
                total_chord_count as u128,
            ),
            biclique_size_per_explicit_edge: None,
            c0_network_vertex_count: 0,
            c0_network_arc_count: 0,
            compressed_network_vertex_count: flow_solution.network_vertex_count,
            compressed_network_arc_count: flow_solution.network_arc_count,
            maximum_matching_size: flow_value,
            minimum_vertex_cover_size: flow_solution.vertex_cover.size,
            output_rectangle_count: optimum_rectangle_count,
            phase_microseconds: [
                (
                    "boundary_effective_chords".to_owned(),
                    geometry_at.duration_since(started).as_micros(),
                ),
                (
                    "component_preparation".to_owned(),
                    geometry.prepared_component_build_microseconds,
                ),
                (
                    "boundary_extraction".to_owned(),
                    geometry.boundary_extraction_microseconds,
                ),
                (
                    "reflex_grouping".to_owned(),
                    geometry.reflex_grouping_microseconds,
                ),
                (
                    "effective_chord_enumeration".to_owned(),
                    geometry.effective_chord_enumeration_microseconds,
                ),
                (
                    "dominance_embedding".to_owned(),
                    embedding_at.duration_since(geometry_at).as_micros(),
                ),
                (
                    "biclique_partition".to_owned(),
                    bicliques_at.duration_since(embedding_at).as_micros(),
                ),
                (
                    "compressed_flow".to_owned(),
                    flow_at.duration_since(bicliques_at).as_micros(),
                ),
                (
                    "geometric_completion".to_owned(),
                    completed_at.duration_since(flow_at).as_micros(),
                ),
                (
                    "selected_chord_cut_materialization".to_owned(),
                    completion
                        .metrics
                        .selected_chord_cut_materialization_microseconds,
                ),
                (
                    "horizontal_simple_chord_completion".to_owned(),
                    completion
                        .metrics
                        .horizontal_simple_chord_completion_microseconds,
                ),
                (
                    "vertical_simple_chord_completion".to_owned(),
                    completion
                        .metrics
                        .vertical_simple_chord_completion_microseconds,
                ),
                (
                    "rectangle_recovery".to_owned(),
                    completion.metrics.rectangle_recovery_microseconds,
                ),
                (
                    "final_output_validation".to_owned(),
                    completion.metrics.final_output_validation_microseconds,
                ),
            ]
            .into_iter()
            .collect(),
            peak_memory_bytes: None,
            execution_trace: ExecutionTrace {
                compact_structure_check_called: true,
                ..ExecutionTrace::default()
            },
            effective_chord_enumerator: Some(enumerator.name().to_owned()),
            effective_chord_enumeration_microseconds: Some(
                geometry.effective_chord_enumeration_microseconds,
            ),
            conflict_representation: Some(
                ConflictRepresentationBackend::GeneralDominance4D
                    .name()
                    .to_owned(),
            ),
            prepared_component_build_count: Some(1),
            prepared_component_build_microseconds: Some(
                geometry.prepared_component_build_microseconds,
            ),
            boundary_index_build_count: Some(1),
            boundary_index_build_microseconds: Some(geometry.boundary_index_build_microseconds),
            boundary_index_entries: Some(geometry.boundary_index.entry_count()),
            boundary_index_owned_bytes: Some(geometry.boundary_index.owned_bytes_estimate()),
            linear_boundary_vertex_lookup_count: Some(0),
            gap_interval_membership_tests: Some(0),
            gap_event_push_count: Some(0),
            gap_event_pop_count: Some(0),
            clean_endpoint_pair_comparisons: Some(0),
            boundary_extraction_microseconds: Some(geometry.boundary_extraction_microseconds),
            reflex_grouping_microseconds: Some(geometry.reflex_grouping_microseconds),
            occupancy_bytes: Some(
                geometry.prepared.occupancy.len()
                    + geometry.prepared.occupancy_prefix_sums.len() * size_of::<usize>(),
            ),
            emitted_chord_count: Some(total_chord_count),
            owned_allocation_estimates,
            horizontal_interior_run_count: geometry.horizontal_interior_run_count,
            vertical_interior_run_count: geometry.vertical_interior_run_count,
            candidate_reflex_pair_count: geometry.candidate_reflex_pair_count,
            completion_backend: Some(completion_backend.name().to_owned()),
            ..completion_diagnostics(&completion.metrics)
        },
        certificate: Some(Certificate {
            kind: "dominance-compact-only".to_owned(),
            payload: json!({
                "verification_mode": VerificationMode::CompactOnly,
                "embedding": embedding,
                "biclique_partition": partition,
                "flow_value": flow_value,
                "compressed_network_vertex_count": flow_solution.network_vertex_count,
                "compressed_network_arc_count": flow_solution.network_arc_count,
                "internal_capacity": flow_solution.internal_capacity,
                "internal_cut_arc_count": flow_solution.internal_cut_arc_count,
                "min_cut_source_side": flow_solution.flow.source_side,
                "cover_left": flow_solution.vertex_cover.left,
                "cover_right": flow_solution.vertex_cover.right,
                "selected_horizontal": selected_horizontal_indices,
                "selected_vertical": selected_vertical_indices,
            }),
        }),
    };
    validate_dissection_prepared(&geometry.prepared, &result)?;
    Ok(result)
}

fn solve_path_tree_dispatch<C>(
    component: &GridComponent<C>,
    mode: VerificationMode,
    enumerator: ChordEnumerator,
    completion_backend: CompletionBackendKind,
    region_dual: RegionBackend,
    orientation_policy: PathTreeOrientationPolicy,
    gap_backend: GapBackend,
) -> Result<DissectionResult, DominanceError> {
    let geometry = match enumerator {
        ChordEnumerator::ReferencePairwise => {
            rect_oracle_sg::analyze_geometry_with(component, &ReferencePairwiseEnumerator)?
        }
        ChordEnumerator::GridInteriorRuns => {
            rect_oracle_sg::analyze_geometry_with(component, &GridInteriorRunEnumerator)?
        }
    };
    let certificate = rect_oracle_sg::classify_clean_hole_free_with_endpoint_index(
        component,
        &geometry.boundary,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        &geometry.endpoint_index,
    );
    if !certificate.eligible {
        return Err(DominanceError::PathTreeIneligible(certificate));
    }
    solve_path_tree_with_geometry(
        component,
        &geometry,
        &certificate,
        PathTreeSolveOptions {
            mode,
            completion_backend,
            region_dual,
            orientation_policy,
            gap_backend,
        },
    )
}

#[allow(clippy::too_many_lines)]
fn solve_path_tree_with_geometry<C>(
    component: &GridComponent<C>,
    geometry: &rect_oracle_sg::SgGeometry,
    certificate: &rect_oracle_sg::CleanHoleFreeCertificate,
    options: PathTreeSolveOptions,
) -> Result<DissectionResult, DominanceError> {
    let PathTreeSolveOptions {
        mode,
        completion_backend,
        region_dual,
        orientation_policy,
        gap_backend,
    } = options;
    let started = Instant::now();
    let path_tree = build_path_tree_partition_with_orientation_policy_and_options(
        &geometry.prepared,
        &geometry.boundary,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        certificate.clone(),
        mode == VerificationMode::FullyAudited,
        region_dual,
        orientation_policy,
        Some(&geometry.endpoint_index),
        gap_backend,
    )?;
    let path_tree_at = Instant::now();
    let mut four_d_sigma = None;
    let mut audited_matching_size = None;
    let mut audited_edge_count = None;
    if mode == VerificationMode::FullyAudited {
        let graph = rect_oracle_sg::build_conflict_graph(
            &geometry.horizontal_chords,
            &geometry.vertical_chords,
        )?;
        // The boundary dual is independently built and checked against the
        // area oracle in FullyAudited.  CompactOnly skips this comparison.
        let boundary_partition = build_best_path_tree_partition_with_backend(
            &geometry.prepared,
            &geometry.boundary,
            &geometry.horizontal_chords,
            &geometry.vertical_chords,
            certificate.clone(),
            false,
            RegionBackend::Experiment,
        )?;
        boundary_partition
            .biclique_partition
            .verify_exact_partition(&graph)?;
        boundary_partition
            .path_tree
            .verify_paths(&geometry.horizontal_chords, &geometry.vertical_chords)?;
        path_tree
            .biclique_partition
            .verify_exact_partition(&graph)?;
        audited_edge_count = Some(graph.edge_count());
        let embedding =
            DominanceEmbedding::new(&geometry.horizontal_chords, &geometry.vertical_chords)?;
        embedding
            .assert_pairwise_equivalence(&geometry.horizontal_chords, &geometry.vertical_chords)?;
        let four_d = Partition::comparability_theorem_8_audited(&embedding)?.partition;
        four_d.verify_exact_partition(&graph)?;
        four_d_sigma = Some(four_d.total_vertex_occurrences());
        audited_matching_size = Some(hopcroft_karp(&graph).size);
        for &horizontal in &geometry.horizontal_chords {
            let horizontal_endpoints =
                rect_oracle_sg::horizontal_chord_endpoints(&geometry.boundary, horizontal)?;
            for &vertical in &geometry.vertical_chords {
                let vertical_endpoints =
                    rect_oracle_sg::vertical_chord_endpoints(&geometry.boundary, vertical)?;
                let loop_len = geometry
                    .boundary
                    .loop_len(horizontal_endpoints.first.loop_id)
                    .ok_or(DominanceError::PathTreeAlternationMismatch)?;
                if rect_core::closed_chords_intersect(horizontal, vertical)
                    != rect_oracle_sg::endpoints_alternate(
                        horizontal_endpoints,
                        vertical_endpoints,
                        loop_len,
                    )
                {
                    return Err(DominanceError::PathTreeAlternationMismatch);
                }
            }
        }
    }
    let flow_solution = solve_biclique_flow(
        geometry.horizontal_chords.len(),
        geometry.vertical_chords.len(),
        &path_tree.biclique_partition,
        &DinicBackend,
    )?;
    let flow_value = usize::try_from(flow_solution.flow.value)
        .map_err(|_| DominanceError::FlowValueConversion)?;
    if let Some(matching) = audited_matching_size
        && matching != flow_value
    {
        return Err(DominanceError::MatchingFlowMismatch {
            matching,
            flow: flow_value,
        });
    }
    let selected_horizontal = flow_solution
        .vertex_cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = flow_solution
        .vertex_cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let flow_at = Instant::now();
    let total_chord_count = geometry
        .horizontal_chords
        .len()
        .checked_add(geometry.vertical_chords.len())
        .ok_or(DominanceError::MetricOverflow)?;
    let independent_count = total_chord_count
        .checked_sub(flow_value)
        .ok_or(DominanceError::FormulaUnderflow)?;
    let formula_base = geometry
        .boundary
        .reflex_vertices
        .len()
        .checked_add(1)
        .ok_or(DominanceError::FormulaUnderflow)?;
    let optimum_rectangle_count = formula_base
        .checked_sub(independent_count)
        .ok_or(DominanceError::FormulaUnderflow)?;
    let completion = complete_selected(
        component,
        &geometry.prepared,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        &selected_horizontal,
        &selected_vertical,
        completion_backend,
    )?;
    if completion.rectangles.len() != optimum_rectangle_count {
        return Err(DominanceError::CompletionCount {
            expected: optimum_rectangle_count,
            actual: completion.rectangles.len(),
        });
    }
    let completed_at = Instant::now();
    let selected_horizontal_indices = selected_horizontal
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let selected_vertical_indices = selected_vertical
        .iter()
        .enumerate()
        .filter_map(|(index, &selected)| selected.then_some(index))
        .collect::<Vec<_>>();
    let sigma = path_tree.biclique_partition.total_vertex_occurrences();
    let q = total_chord_count;
    let log_q = ceil_log2(q.saturating_add(1));
    let mut heavy_chain_interval_count = 0usize;
    for compact_path in &path_tree.path_tree.compact_paths {
        heavy_chain_interval_count = heavy_chain_interval_count
            .checked_add(
                path_tree
                    .path_tree
                    .hld
                    .decompose_path_endpoints(compact_path.start_region, compact_path.end_region)?
                    .len(),
            )
            .ok_or(DominanceError::MetricOverflow)?;
    }
    let tree_edge_occurrences =
        if path_tree.orientation == path_tree::PathTreeOrientation::VerticalTreeHorizontalPaths {
            path_tree
                .biclique_partition
                .blocks
                .iter()
                .map(|biclique| biclique.right.len())
                .sum()
        } else {
            path_tree
                .biclique_partition
                .blocks
                .iter()
                .map(|biclique| biclique.left.len())
                .sum()
        };
    let path_occurrence_bound = path_tree.path_count.saturating_mul(log_q).saturating_mul(4);
    let tree_edge_occurrence_bound = path_tree
        .path_tree
        .tree
        .edges
        .len()
        .saturating_mul(log_q)
        .saturating_mul(4);
    let boundary_vertex_count = geometry.boundary.boundary_complexity();
    let mut owned_allocation_estimates = BTreeMap::new();
    owned_allocation_estimates.insert(
        "boundary_index".to_owned(),
        geometry.boundary_index.owned_bytes_estimate(),
    );
    owned_allocation_estimates.insert(
        "endpoint_tables".to_owned(),
        geometry.endpoint_index.owned_bytes_estimate(),
    );
    owned_allocation_estimates.insert(
        "boundary_intervals_and_events".to_owned(),
        path_tree
            .path_tree
            .tree
            .edges
            .len()
            .saturating_mul(size_of::<(usize, usize, rect_core::VerticalChordId)>() * 2)
            .saturating_add(boundary_vertex_count.saturating_mul(size_of::<Vec<usize>>())),
    );
    owned_allocation_estimates.insert(
        "gap_region_labels".to_owned(),
        boundary_vertex_count * size_of::<path_tree::DualRegionId>(),
    );
    owned_allocation_estimates.insert(
        "dual_edges_and_adjacency".to_owned(),
        path_tree.path_tree.tree.edges.len() * size_of::<path_tree::DualTreeEdge>()
            + path_tree.path_tree.tree.adjacency.len()
                * size_of::<Vec<(path_tree::DualRegionId, rect_core::VerticalChordId)>>()
            + path_tree
                .path_tree
                .tree
                .adjacency
                .iter()
                .map(Vec::len)
                .sum::<usize>()
                * size_of::<(path_tree::DualRegionId, rect_core::VerticalChordId)>(),
    );
    owned_allocation_estimates.insert(
        "compact_path_records".to_owned(),
        path_tree.path_tree.compact_paths.len() * size_of::<path_tree::CompactTreePath>(),
    );
    owned_allocation_estimates.insert(
        "hld_arrays".to_owned(),
        path_tree.path_tree.hld.parent.len()
            * (size_of::<Option<path_tree::DualRegionId>>() * 2 + size_of::<usize>() * 6),
    );
    owned_allocation_estimates.insert(
        "chain_edge_vectors".to_owned(),
        path_tree
            .path_tree
            .hld
            .chain_edges
            .iter()
            .map(|edges| edges.len() * size_of::<rect_core::VerticalChordId>())
            .sum(),
    );
    owned_allocation_estimates.insert(
        "canonical_segment_nodes".to_owned(),
        path_tree.canonical_segment_node_count * size_of::<biclique::Block>(),
    );
    owned_allocation_estimates.insert(
        "biclique_vectors".to_owned(),
        path_tree.biclique_partition.blocks.len() * size_of::<biclique::Block>()
            + sigma * size_of::<usize>(),
    );
    owned_allocation_estimates.insert(
        "compressed_flow_graph".to_owned(),
        (flow_solution.network_vertex_count + flow_solution.network_arc_count) * size_of::<usize>(),
    );
    owned_allocation_estimates.insert(
        "path_tree_certificate_payload".to_owned(),
        path_tree.path_tree.compact_paths.len() * size_of::<path_tree::CompactTreePath>()
            + path_tree.path_tree.tree.edges.len() * size_of::<path_tree::DualTreeEdge>(),
    );
    let result = DissectionResult {
        optimum_rectangle_count,
        rectangles: completion.rectangles,
        diagnostics: Diagnostics {
            cell_count: component.cell_count(),
            boundary_complexity: geometry.boundary.boundary_complexity(),
            outer_loop_count: geometry.boundary.outer_loop_count(),
            hole_count: geometry.boundary.hole_count(),
            reflex_vertex_count: geometry.boundary.reflex_vertices.len(),
            horizontal_chord_count: geometry.horizontal_chords.len(),
            vertical_chord_count: geometry.vertical_chords.len(),
            total_chord_count,
            explicit_conflict_edge_count: audited_edge_count,
            biclique_count: path_tree.biclique_partition.blocks.len(),
            biclique_total_vertex_occurrences: sigma,
            biclique_size_per_chord: ExactRatio::new(sigma as u128, total_chord_count as u128),
            compressed_network_vertex_count: flow_solution.network_vertex_count,
            compressed_network_arc_count: flow_solution.network_arc_count,
            maximum_matching_size: flow_value,
            minimum_vertex_cover_size: flow_solution.vertex_cover.size,
            output_rectangle_count: optimum_rectangle_count,
            phase_microseconds: [
                (
                    "path_tree_construction".to_owned(),
                    path_tree_at.duration_since(started).as_micros(),
                ),
                (
                    "compressed_flow".to_owned(),
                    flow_at.duration_since(path_tree_at).as_micros(),
                ),
                (
                    "geometric_completion".to_owned(),
                    completed_at.duration_since(flow_at).as_micros(),
                ),
            ]
            .into_iter()
            .collect(),
            execution_trace: ExecutionTrace {
                pairwise_embedding_audit_called: mode == VerificationMode::FullyAudited,
                explicit_conflict_graph_built: mode == VerificationMode::FullyAudited,
                hopcroft_karp_called: mode == VerificationMode::FullyAudited,
                full_edge_partition_audit_called: mode == VerificationMode::FullyAudited,
                compact_structure_check_called: true,
                full_tree_path_edge_lists_materialized: mode == VerificationMode::FullyAudited,
                per_path_bfs_called: mode == VerificationMode::FullyAudited,
                area_flood_fill_dual_built: region_dual == RegionBackend::Oracle,
                unit_chord_cuts_materialized: region_dual == RegionBackend::Oracle,
                prepared_occupancy_transposed: mode == VerificationMode::FullyAudited
                    && region_dual == RegionBackend::Oracle,
                ..ExecutionTrace::default()
            },
            effective_chord_enumerator: Some("prepared-path-tree-input".to_owned()),
            effective_chord_enumeration_microseconds: Some(
                geometry.effective_chord_enumeration_microseconds,
            ),
            prepared_component_build_count: Some(1),
            prepared_component_build_microseconds: Some(
                geometry.prepared_component_build_microseconds,
            ),
            boundary_index_build_count: Some(1),
            boundary_index_build_microseconds: Some(geometry.boundary_index_build_microseconds),
            boundary_index_entries: Some(geometry.boundary_index.entry_count()),
            boundary_index_owned_bytes: Some(geometry.boundary_index.owned_bytes_estimate()),
            linear_boundary_vertex_lookup_count: Some(0),
            gap_interval_membership_tests: Some(
                path_tree.path_tree.tree.boundary_gap_membership_tests,
            ),
            gap_event_push_count: Some(path_tree.path_tree.tree.boundary_gap_event_push_count),
            gap_event_pop_count: Some(path_tree.path_tree.tree.boundary_gap_event_pop_count),
            boundary_gap_label_backend: Some(gap_backend.name().to_owned()),
            clean_endpoint_pair_comparisons: Some(0),
            boundary_extraction_microseconds: Some(geometry.boundary_extraction_microseconds),
            reflex_grouping_microseconds: Some(geometry.reflex_grouping_microseconds),
            occupancy_bytes: Some(geometry.prepared.occupancy.len()),
            conflict_representation: Some(
                ConflictRepresentationBackend::CleanHoleFreePathTree
                    .name()
                    .to_owned(),
            ),
            clean_hole_free_eligible: Some(true),
            path_tree_orientation: Some(path_tree.orientation.name().to_owned()),
            path_tree_orientation_policy: Some(orientation_policy.name().to_owned()),
            dual_region_count: Some(path_tree.dual_region_count),
            dual_tree_vertex_count: Some(path_tree.path_tree.tree.region_count),
            path_count: Some(path_tree.path_count),
            path_edge_incidence_count: Some(path_tree.total_path_edge_incidences),
            total_path_length_metric: Some(path_tree.total_path_edge_incidences),
            dual_tree_max_depth: path_tree.path_tree.hld.depth.iter().copied().max(),
            dual_tree_max_branching_degree: path_tree
                .path_tree
                .tree
                .adjacency
                .iter()
                .map(Vec::len)
                .max(),
            heavy_chain_count: Some(path_tree.path_tree.hld.chain_edges.len()),
            heavy_chain_interval_count: Some(heavy_chain_interval_count),
            tree_edge_occurrences: Some(tree_edge_occurrences),
            theoretical_path_occurrence_bound: Some(path_occurrence_bound),
            theoretical_tree_edge_occurrence_bound: Some(tree_edge_occurrence_bound),
            canonical_segment_node_count: Some(path_tree.canonical_segment_node_count),
            path_tree_sigma: Some(sigma),
            four_d_sigma,
            owned_allocation_estimates,
            region_dual_backend: Some(region_dual.name().to_owned()),
            region_dual_construction_microseconds: Some(
                path_tree_at.duration_since(started).as_micros(),
            ),
            dual_tree_edge_count: Some(path_tree.path_tree.tree.edges.len()),
            dual_allocated_bytes: Some(
                path_tree.path_tree.tree.edges.len()
                    * std::mem::size_of::<rect_core::VerticalChordId>()
                    + path_tree.path_tree.tree.adjacency.len()
                        * std::mem::size_of::<
                            Vec<(path_tree::DualRegionId, rect_core::VerticalChordId)>,
                        >(),
            ),
            dual_unit_cut_count: Some(if region_dual == RegionBackend::Oracle {
                geometry
                    .vertical_chords
                    .iter()
                    .filter_map(|chord| usize::try_from(chord.top() - chord.bottom()).ok())
                    .sum()
            } else {
                0
            }),
            dual_area_cell_visits: Some(if region_dual == RegionBackend::Oracle {
                geometry
                    .prepared
                    .occupancy
                    .iter()
                    .filter(|&&occupied| occupied)
                    .count()
            } else {
                0
            }),
            dual_interval_count: Some(if region_dual == RegionBackend::Experiment {
                path_tree.path_tree.tree.edges.len()
            } else {
                0
            }),
            dual_maximum_nesting_depth: Some(0),
            hld_interval_count: Some(path_tree.canonical_segment_node_count),
            explicit_path_records_materialized: Some(path_tree.path_tree.paths.len()),
            ..completion_diagnostics(&completion.metrics)
        },
        certificate: Some(Certificate {
            kind: "clean-hole-free-path-tree".to_owned(),
            payload: json!({
                "verification_mode": mode,
                "clean_certificate": path_tree.path_tree.certificate,
                "region_dual_tree": path_tree.path_tree.tree,
                "chord_tree_paths": path_tree.path_tree.paths,
                "compact_tree_paths": path_tree.path_tree.compact_paths,
                "orientation": path_tree.orientation,
                "biclique_partition": path_tree.biclique_partition,
                "flow_value": flow_value,
                "cover_left": flow_solution.vertex_cover.left,
                "cover_right": flow_solution.vertex_cover.right,
                "selected_horizontal": selected_horizontal_indices,
                "selected_vertical": selected_vertical_indices,
            }),
        }),
    };
    validate_dissection_prepared(&geometry.prepared, &result)?;
    Ok(result)
}

fn complete_selected<C>(
    component: &GridComponent<C>,
    prepared: &PreparedGridComponent,
    horizontal_chords: &[rect_core::HorizontalChord],
    vertical_chords: &[rect_core::VerticalChord],
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
    backend: CompletionBackendKind,
) -> Result<rect_oracle_sg::CompletionResult, DominanceError> {
    match backend {
        CompletionBackendKind::ReferenceRescan => Ok(complete_with_prepared_backend(
            component,
            prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
            &ReferenceRescanCompletion,
        )?),
        CompletionBackendKind::IndexedFrontier => Ok(complete_with_prepared_backend(
            component,
            prepared,
            horizontal_chords,
            vertical_chords,
            selected_horizontal,
            selected_vertical,
            &IndexedFrontierCompletion,
        )?),
    }
}

#[derive(Debug, Error)]
pub enum DominanceError {
    #[error(transparent)]
    Sg(#[from] SgError),
    #[error(transparent)]
    PolygonSg(#[from] PolygonSgError),
    #[error(transparent)]
    PreparedPolygon(#[from] PreparedPolygonError),
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    #[error(transparent)]
    Block(#[from] Error),
    #[error(transparent)]
    CompressedFlow(#[from] CompressedFlowError),
    #[error(transparent)]
    PathTree(#[from] PathTreeError),
    #[error("solver produced an invalid dissection: {0}")]
    InvalidOutput(#[from] ValidationError),
    #[error("geometric conflict graph differs from the explicit dominance graph")]
    ExplicitGraphMismatch,
    #[error("selected effective-chord enumerator differs from the pairwise reference families")]
    ChordFamilyMismatch,
    #[error("indexed polygon completion differs from the coordinate reference output")]
    PolygonCompletionMismatch,
    #[error("polygon geometry and chord backends select incompatible query implementations")]
    PolygonGeometryChordMismatch,
    #[error("flow value cannot be represented as usize")]
    FlowValueConversion,
    #[error("diagnostic network metric overflowed usize")]
    MetricOverflow,
    #[error("Hopcroft--Karp matching value {matching} differs from flow value {flow}")]
    MatchingFlowMismatch { matching: usize, flow: usize },
    #[error("flow-recovered vertex cover misses explicit edge ({left}, {right})")]
    UncoveredConflictEdge { left: usize, right: usize },
    #[error("completion produced {actual} rectangles, formula requires {expected}")]
    CompletionCount { expected: usize, actual: usize },
    #[error("rectangular-dissection formula underflowed")]
    FormulaUnderflow,
    #[error(
        "clean hole-free path-tree representation was requested for an ineligible component: {0:?}"
    )]
    PathTreeIneligible(rect_oracle_sg::CleanHoleFreeCertificate),
    #[error("circle alternation does not match closed chord intersection")]
    PathTreeAlternationMismatch,
    #[error("polygon path-tree flow {path_tree} differs from 4D flow {four_d}")]
    PathTreeMatchingMismatch { path_tree: u64, four_d: u64 },
}

#[cfg(test)]
mod polygon_tests {
    use rect_core::{
        Boundary, ColorGrid, CoordinateRect, OrthogonalLoop, Point, PreparedPolygonContext,
        RectilinearPolygon,
    };
    use rect_oracle_sg::{
        CoordinateCompressedCompletion, GridInteriorRunEnumerator, HorizontalCutSegment,
        HorizontalUnitCut, IndexedFrontierCompletion, VerticalCutSegment, VerticalUnitCut,
        analyze_geometry_with, complete_with_prepared_backend,
    };

    use super::{
        ChordEnumerator, CompletionBackendKind, ConflictRepresentationBackend, DominanceMode,
        FlowBackendKind, VerificationMode, solve_polygon, solve_polygon_with_representation,
        solve_with_flow_backend,
        solve_with_verification_mode_and_chord_enumerator_and_completion_backend,
    };

    fn loop_from(points: &[(i64, i64)]) -> OrthogonalLoop {
        OrthogonalLoop::new(points.iter().map(|&(x, y)| Point::new(x, y)).collect())
    }

    #[test]
    fn solves_native_large_gap_and_auto_path_tree_polygon() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[
                (0, 0),
                (1_000_000_000, 0),
                (1_000_000_000, 1),
                (1, 1),
                (1, 4),
                (0, 4),
            ]),
            vec![],
        )
        .unwrap();
        let four_d = solve_polygon(&polygon).unwrap();
        let auto = solve_polygon_with_representation(&polygon, ConflictRepresentationBackend::Auto)
            .unwrap();
        assert_eq!(four_d.optimum_rectangle_count, 2);
        assert_eq!(four_d.rectangles, auto.rectangles);
        assert_eq!(auto.diagnostics.raster_oracle_used, Some(false));
        assert_eq!(
            auto.diagnostics.conflict_representation.as_deref(),
            Some("path-tree")
        );
    }

    #[test]
    fn polygon_with_hole_falls_back_to_four_dimensions() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[(0, 0), (12, 0), (12, 10), (0, 10)]),
            vec![loop_from(&[(4, 3), (4, 7), (8, 7), (8, 3)])],
        )
        .unwrap();
        let result =
            solve_polygon_with_representation(&polygon, ConflictRepresentationBackend::Auto)
                .unwrap();
        assert_eq!(result.optimum_rectangle_count, 4);
        assert_eq!(
            result.diagnostics.conflict_representation.as_deref(),
            Some("dominance-4d")
        );
        assert_eq!(result.diagnostics.clean_hole_free_eligible, Some(false));
    }

    #[test]
    fn polygon_solver_is_invariant_under_integer_affine_symmetries() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[(0, 0), (7, 0), (7, 2), (2, 2), (2, 9), (0, 9)]),
            vec![],
        )
        .unwrap();
        let expected = solve_polygon(&polygon).unwrap().optimum_rectangle_count;
        let transforms = [
            transform_polygon(&polygon, &|point| Point::new(point.x + 13, point.y - 17)),
            transform_polygon(&polygon, &|point| Point::new(-point.x, point.y)),
            transform_polygon(&polygon, &|point| Point::new(-point.y, point.x)),
            transform_polygon(&polygon, &|point| Point::new(-point.x, -point.y)),
            transform_polygon(&polygon, &|point| Point::new(point.y, -point.x)),
            transform_polygon(&polygon, &|point| Point::new(point.x * 5, point.y * 3)),
            transform_polygon(&polygon, &|point| {
                let x = match point.x {
                    0 => -11,
                    2 => 3,
                    7 => 29,
                    _ => unreachable!("fixture has only three x coordinates"),
                };
                let y = match point.y {
                    0 => 5,
                    2 => 12,
                    9 => 40,
                    _ => unreachable!("fixture has only three y coordinates"),
                };
                Point::new(x, y)
            }),
        ];
        for transformed in transforms {
            assert_eq!(
                solve_polygon(&transformed).unwrap().optimum_rectangle_count,
                expected
            );
        }
    }

    #[test]
    fn grid_polygon_end_to_end_matches_on_all_supported_3x3_components() {
        assert_eq!(verify_grid_polygon_masks(3, 3), 893);
    }

    #[test]
    fn selected_flow_backends_agree_on_a_grid_dissection() {
        let grid = ColorGrid::new(
            3,
            3,
            vec![true, true, false, true, true, true, false, true, true],
        )
        .unwrap();
        let component = grid.four_connected_components().remove(0);
        let dinic =
            solve_with_flow_backend(&component, DominanceMode::Compact, FlowBackendKind::Dinic)
                .unwrap();
        let push_relabel = solve_with_flow_backend(
            &component,
            DominanceMode::Compact,
            FlowBackendKind::PushRelabel,
        )
        .unwrap();
        assert_eq!(
            push_relabel.optimum_rectangle_count,
            dinic.optimum_rectangle_count
        );
    }

    #[test]
    #[ignore = "release-mode exhaustive 4x4 polygon differential"]
    fn grid_polygon_end_to_end_matches_on_all_supported_4x4_components() {
        assert_eq!(verify_grid_polygon_masks(4, 4), 166_189);
    }

    #[allow(clippy::too_many_lines)]
    fn verify_grid_polygon_masks(width: usize, height: usize) -> usize {
        let mut compared = 0;
        let bit_count = width * height;
        let mask_limit = 1_u32
            .checked_shl(u32::try_from(bit_count).unwrap())
            .unwrap();
        for mask in 1_u32..mask_limit {
            let grid = ColorGrid::new(
                width,
                height,
                (0..bit_count).map(|bit| mask & (1 << bit) != 0).collect(),
            )
            .unwrap();
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                let boundary = Boundary::from_component(&component).unwrap();
                let Ok(polygon) = boundary.to_polygon() else {
                    continue;
                };
                let geometry =
                    analyze_geometry_with(&component, &GridInteriorRunEnumerator).unwrap();
                let polygon_families = rect_oracle_sg::GeneralPolygonPairwiseEnumerator
                    .enumerate(&polygon)
                    .unwrap();
                let polygon_prepared = PreparedPolygonContext::new(&polygon).unwrap();
                let indexed_families = rect_oracle_sg::IndexedPolygonPairwiseEnumerator
                    .enumerate_prepared(&polygon_prepared)
                    .unwrap();
                let sweep_families = rect_oracle_sg::SoltanGorpinevichSweepEnumerator
                    .enumerate_prepared(&polygon_prepared)
                    .unwrap();
                assert_eq!(geometry.horizontal_chords, polygon_families.horizontal);
                assert_eq!(geometry.vertical_chords, polygon_families.vertical);
                assert_eq!(
                    polygon_families.horizontal,
                    indexed_families.families.horizontal
                );
                assert_eq!(
                    polygon_families.vertical,
                    indexed_families.families.vertical
                );
                assert_eq!(
                    polygon_families.horizontal,
                    sweep_families.families.horizontal
                );
                assert_eq!(polygon_families.vertical, sweep_families.families.vertical);
                assert_eq!(sweep_families.metrics.sweep_aligned_pair_iterations, 0);
                assert_eq!(sweep_families.metrics.sweep_all_pair_iterations, 0);
                assert_eq!(sweep_families.metrics.sweep_definition7_fallback_checks, 0);
                assert_eq!(sweep_families.metrics.sweep_full_boundary_scans, 0);

                let grid_result =
                    solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                        &component,
                        VerificationMode::CompactOnly,
                        ChordEnumerator::GridInteriorRuns,
                        CompletionBackendKind::IndexedFrontier,
                    )
                    .unwrap();
                let selected_horizontal = selected_flags(
                    &grid_result,
                    "selected_horizontal",
                    geometry.horizontal_chords.len(),
                );
                let selected_vertical = selected_flags(
                    &grid_result,
                    "selected_vertical",
                    geometry.vertical_chords.len(),
                );
                let polygon_result = solve_polygon(&polygon).unwrap();
                let polygon_selected_horizontal = polygon_selected_flags(
                    &polygon_result,
                    "selected_horizontal",
                    geometry.horizontal_chords.len(),
                );
                let polygon_selected_vertical = polygon_selected_flags(
                    &polygon_result,
                    "selected_vertical",
                    geometry.vertical_chords.len(),
                );
                assert_eq!(selected_horizontal, polygon_selected_horizontal);
                assert_eq!(selected_vertical, polygon_selected_vertical);
                assert_eq!(
                    grid_result.optimum_rectangle_count,
                    polygon_result.optimum_rectangle_count
                );
                let grid_completion = complete_with_prepared_backend(
                    &component,
                    &geometry.prepared,
                    &geometry.horizontal_chords,
                    &geometry.vertical_chords,
                    &selected_horizontal,
                    &selected_vertical,
                    &IndexedFrontierCompletion,
                )
                .unwrap();
                let polygon_completion = CoordinateCompressedCompletion
                    .complete(
                        &polygon,
                        &polygon_families.horizontal,
                        &polygon_families.vertical,
                        &polygon_selected_horizontal,
                        &polygon_selected_vertical,
                    )
                    .unwrap();
                assert_eq!(
                    merge_horizontal(&grid_completion.selected_horizontal_unit_cuts),
                    polygon_completion.selected_horizontal_cuts
                );
                assert_eq!(
                    merge_vertical(&grid_completion.selected_vertical_unit_cuts),
                    polygon_completion.selected_vertical_cuts
                );
                assert_eq!(
                    merge_horizontal(&grid_completion.added_horizontal_unit_cuts),
                    polygon_completion.added_horizontal_cuts
                );
                assert_eq!(
                    merge_vertical(&grid_completion.added_vertical_unit_cuts),
                    polygon_completion.added_vertical_cuts
                );
                let rectangles = grid_result
                    .rectangles
                    .iter()
                    .map(|rectangle| {
                        CoordinateRect::new(
                            i64::try_from(rectangle.x0).unwrap(),
                            i64::try_from(rectangle.y0).unwrap(),
                            i64::try_from(rectangle.x1).unwrap(),
                            i64::try_from(rectangle.y1).unwrap(),
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>();
                assert_eq!(rectangles, polygon_completion.rectangles);
                assert_eq!(rectangles, polygon_result.rectangles);
                compared += 1;
            }
        }
        compared
    }

    fn selected_flags(result: &rect_core::DissectionResult, key: &str, len: usize) -> Vec<bool> {
        let mut flags = vec![false; len];
        for index in result.certificate.as_ref().unwrap().payload[key]
            .as_array()
            .unwrap()
        {
            flags[usize::try_from(index.as_u64().unwrap()).unwrap()] = true;
        }
        flags
    }

    fn polygon_selected_flags(
        result: &rect_core::PolygonDissectionResult,
        key: &str,
        len: usize,
    ) -> Vec<bool> {
        let flags = result.certificate.as_ref().unwrap().payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(flags.len(), len);
        flags
    }

    fn merge_horizontal(cuts: &[HorizontalUnitCut]) -> Vec<HorizontalCutSegment> {
        let mut cuts = cuts.to_vec();
        cuts.sort_unstable_by_key(|cut| (cut.y, cut.x));
        let mut result = Vec::<HorizontalCutSegment>::new();
        for cut in cuts {
            let x = i64::try_from(cut.x).unwrap();
            let y = i64::try_from(cut.y).unwrap();
            if let Some(last) = result.last_mut()
                && last.y == y
                && last.right == x
            {
                last.right += 1;
            } else {
                result.push(HorizontalCutSegment {
                    left: x,
                    right: x + 1,
                    y,
                });
            }
        }
        result.sort_unstable();
        result
    }

    fn merge_vertical(cuts: &[VerticalUnitCut]) -> Vec<VerticalCutSegment> {
        let mut cuts = cuts.to_vec();
        cuts.sort_unstable_by_key(|cut| (cut.x, cut.y));
        let mut result = Vec::<VerticalCutSegment>::new();
        for cut in cuts {
            let x = i64::try_from(cut.x).unwrap();
            let y = i64::try_from(cut.y).unwrap();
            if let Some(last) = result.last_mut()
                && last.x == x
                && last.top == y
            {
                last.top += 1;
            } else {
                result.push(VerticalCutSegment {
                    x,
                    bottom: y,
                    top: y + 1,
                });
            }
        }
        result.sort_unstable();
        result
    }

    fn transform_polygon(
        polygon: &RectilinearPolygon,
        transform: &impl Fn(Point) -> Point,
    ) -> RectilinearPolygon {
        let transform_loop = |boundary_loop: &OrthogonalLoop| {
            OrthogonalLoop::new(
                boundary_loop
                    .vertices
                    .iter()
                    .copied()
                    .map(transform)
                    .collect(),
            )
        };
        RectilinearPolygon::new(
            transform_loop(&polygon.outer),
            polygon.holes.iter().map(transform_loop).collect(),
        )
        .unwrap()
    }
}
