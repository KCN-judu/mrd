pub mod biclique;
pub mod compressed_flow;
pub mod embedding;

use std::time::Instant;

use biclique::{BicliqueError, BicliquePartition};
use compressed_flow::{CompressedFlowError, solve_biclique_flow};
use embedding::{DominanceEmbedding, EmbeddingError};
use rect_core::{
    Certificate, Diagnostics, DissectionResult, ExactRatio, GridComponent, ValidationError,
    validate_dissection,
};
use rect_graph::DinicBackend;
use rect_oracle_sg::{SgError, complete_with_chord_families, complete_with_selected_chords};
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
    match mode {
        VerificationMode::FullyAudited => solve(component, DominanceMode::Compact),
        VerificationMode::CompactOnly => solve_compact_only(component),
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
    let started = Instant::now();
    let sg_analysis = rect_oracle_sg::analyze(component)?;
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
    let rectangles = complete_with_selected_chords(
        component,
        &sg_analysis,
        &selected_horizontal,
        &selected_vertical,
    )?;
    if rectangles.len() != sg_analysis.optimum_rectangle_count {
        return Err(DominanceError::CompletionCount {
            expected: sg_analysis.optimum_rectangle_count,
            actual: rectangles.len(),
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
        rectangles,
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
            ]
            .into_iter()
            .collect(),
            peak_memory_bytes: None,
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
fn solve_compact_only<C>(component: &GridComponent<C>) -> Result<DissectionResult, DominanceError> {
    let started = Instant::now();
    let geometry = rect_oracle_sg::analyze_geometry(component)?;
    let geometry_at = Instant::now();
    let embedding =
        DominanceEmbedding::new(&geometry.horizontal_chords, &geometry.vertical_chords)?;
    let embedding_at = Instant::now();
    let partition = BicliquePartition::comparability_theorem_8(&embedding)?;
    partition.verify_structure(embedding.horizontal.len(), embedding.vertical.len())?;
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
    let rectangles = complete_with_chord_families(
        component,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        &selected_horizontal,
        &selected_vertical,
    )?;
    if rectangles.len() != optimum_rectangle_count {
        return Err(DominanceError::CompletionCount {
            expected: optimum_rectangle_count,
            actual: rectangles.len(),
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
    let result = DissectionResult {
        optimum_rectangle_count,
        rectangles,
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
            ]
            .into_iter()
            .collect(),
            peak_memory_bytes: None,
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
