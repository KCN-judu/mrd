pub mod biclique;
pub mod compressed_flow;
pub mod embedding;

use std::collections::BTreeMap;
use std::mem::size_of;
use std::time::Instant;

use biclique::{BicliqueError, BicliquePartition};
use compressed_flow::{CompressedFlowError, solve_biclique_flow};
use embedding::{DominanceEmbedding, EmbeddingError};
use rect_core::{
    Certificate, Diagnostics, DissectionResult, ExactRatio, ExecutionTrace, GridComponent,
    ValidationError, validate_dissection,
};
use rect_graph::DinicBackend;
use rect_oracle_sg::{
    CompletionMetrics, EffectiveChordEnumerator, GeometricCompletionBackend,
    GridInteriorRunEnumerator, ReferencePairwiseEnumerator, ReferenceRescanCompletion, SgError,
    complete_with_backend,
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
        ..Diagnostics::default()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChordEnumerator {
    ReferencePairwise,
    GridInteriorRuns,
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
    solve_with_verification_mode_and_chord_enumerator(
        component,
        mode,
        ChordEnumerator::GridInteriorRuns,
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
    match mode {
        VerificationMode::FullyAudited => match enumerator {
            ChordEnumerator::ReferencePairwise => solve_fully_audited_with(
                component,
                DominanceMode::Compact,
                &ReferencePairwiseEnumerator,
            ),
            ChordEnumerator::GridInteriorRuns => solve_fully_audited_with(
                component,
                DominanceMode::Compact,
                &GridInteriorRunEnumerator,
            ),
        },
        VerificationMode::CompactOnly => match enumerator {
            ChordEnumerator::ReferencePairwise => {
                solve_compact_only_with(component, &ReferencePairwiseEnumerator)
            }
            ChordEnumerator::GridInteriorRuns => {
                solve_compact_only_with(component, &GridInteriorRunEnumerator)
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
    solve_fully_audited_with(component, mode, &ReferencePairwiseEnumerator)
}

#[allow(clippy::too_many_lines)]
fn solve_fully_audited_with<C, E: EffectiveChordEnumerator>(
    component: &GridComponent<C>,
    mode: DominanceMode,
    enumerator: &E,
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
        DominanceMode::ExplicitEdges => BicliquePartition::from_explicit_edges(&dominance_graph),
        DominanceMode::Compact => BicliquePartition::comparability_theorem_8(&embedding)?,
    };
    partition.verify_exact_partition(&dominance_graph)?;
    let biclique_certificate = partition.certificate(&dominance_graph);
    let bicliques_at = Instant::now();
    let flow_solution = solve_biclique_flow(
        embedding.horizontal.len(),
        embedding.vertical.len(),
        &partition,
        &DinicBackend,
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
    let completion = complete_with_backend(
        component,
        &sg_analysis.horizontal_chords,
        &sg_analysis.vertical_chords,
        &selected_horizontal,
        &selected_vertical,
        &ReferenceRescanCompletion,
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
            biclique_count: partition.bicliques.len(),
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
            },
            effective_chord_enumerator: Some(enumerator.name().to_owned()),
            effective_chord_enumeration_microseconds: Some(
                geometry_at.duration_since(started).as_micros(),
            ),
            emitted_chord_count: Some(total_chord_count),
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: None,
            owned_allocation_estimates: BTreeMap::new(),
            completion_backend: Some(ReferenceRescanCompletion.name().to_owned()),
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
) -> Result<DissectionResult, DominanceError> {
    let started = Instant::now();
    let geometry = rect_oracle_sg::analyze_geometry_with(component, enumerator)?;
    let geometry_at = Instant::now();
    let embedding =
        DominanceEmbedding::new(&geometry.horizontal_chords, &geometry.vertical_chords)?;
    let embedding_at = Instant::now();
    let partition = BicliquePartition::comparability_theorem_8(&embedding)?;
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
    let completion = complete_with_backend(
        component,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        &selected_horizontal,
        &selected_vertical,
        &ReferenceRescanCompletion,
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
        "embedding_point_arrays".to_owned(),
        total_chord_count * size_of::<embedding::DominancePoint>(),
    );
    owned_allocation_estimates.insert(
        "biclique_vectors".to_owned(),
        partition.bicliques.len() * size_of::<biclique::Biclique>()
            + biclique_total_size * size_of::<usize>(),
    );
    owned_allocation_estimates.insert(
        "flow_graph_storage".to_owned(),
        (flow_solution.network_vertex_count + flow_solution.network_arc_count) * size_of::<usize>(),
    );
    owned_allocation_estimates.insert(
        "certificate_payload".to_owned(),
        total_chord_count * size_of::<embedding::DominancePoint>()
            + partition.bicliques.len() * size_of::<biclique::Biclique>()
            + biclique_total_size * size_of::<usize>()
            + (flow_solution.flow.source_side.len()
                + flow_solution.vertex_cover.left.len()
                + flow_solution.vertex_cover.right.len()
                + selected_horizontal_indices.len()
                + selected_vertical_indices.len())
                * size_of::<usize>(),
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
            horizontal_chord_count: horizontal_count,
            vertical_chord_count: vertical_count,
            total_chord_count,
            explicit_conflict_edge_count: None,
            conflict_edge_density: None,
            biclique_count: partition.bicliques.len(),
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
                geometry_at.duration_since(started).as_micros(),
            ),
            emitted_chord_count: Some(total_chord_count),
            owned_allocation_estimates,
            horizontal_interior_run_count: geometry.horizontal_interior_run_count,
            vertical_interior_run_count: geometry.vertical_interior_run_count,
            candidate_reflex_pair_count: geometry.candidate_reflex_pair_count,
            completion_backend: Some(ReferenceRescanCompletion.name().to_owned()),
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
    validate_dissection(component, &result)?;
    Ok(result)
}

#[derive(Debug, Error)]
pub enum DominanceError {
    #[error(transparent)]
    Sg(#[from] SgError),
    #[error(transparent)]
    Embedding(#[from] EmbeddingError),
    #[error(transparent)]
    Biclique(#[from] BicliqueError),
    #[error(transparent)]
    CompressedFlow(#[from] CompressedFlowError),
    #[error("solver produced an invalid dissection: {0}")]
    InvalidOutput(#[from] ValidationError),
    #[error("geometric conflict graph differs from the explicit dominance graph")]
    ExplicitGraphMismatch,
    #[error("selected effective-chord enumerator differs from the pairwise reference families")]
    ChordFamilyMismatch,
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
}
