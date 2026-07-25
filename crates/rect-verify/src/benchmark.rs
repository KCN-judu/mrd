use std::collections::BTreeMap;
use std::fmt::Write;

use rect_core::{ColorGrid, Diagnostics, ExactRatio, GridComponent};
use rect_dominance::{
    ChordEnumerator, ConflictRepresentationBackend, VerificationMode,
    solve_with_representation_and_region_dual,
};
use rect_oracle_sg::CompletionBackendKind;
use serde::{Deserialize, Serialize};

use crate::adversarial::{
    AdversarialInstance, alternating_notch_corridor, clean_complete_bipartite_grid, comb,
    dense_conflict_grid, double_comb, endpoint_contact_instances, orthogonal_spiral, staircase,
    topological_stress_instances,
};
use crate::polyomino::{enumerate_free_polyominoes, explicit_hole_polyominoes};
use crate::{GridFixture, VerificationError, verify_component};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CleanCensusReport {
    pub metadata: BenchmarkMetadata,
    pub total_components: usize,
    pub hole_free_components: usize,
    pub eligible_components: usize,
    pub total_chord_count: usize,
    pub eligible_chord_count: usize,
    pub rejection_counts: BTreeMap<String, usize>,
    pub eligible_q_histogram: BTreeMap<usize, usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CleanBoundaryDifferentialReport {
    pub metadata: BenchmarkMetadata,
    pub masks: usize,
    pub components: usize,
    pub eligible_components: usize,
    pub verified: usize,
    pub unsupported: usize,
    pub solver_errors: usize,
    pub counterexamples: usize,
    pub execution_trace_violations: usize,
    pub orientation_counts: BTreeMap<String, usize>,
    pub q_min: Option<usize>,
    pub q_max: Option<usize>,
    pub sigma_min: Option<usize>,
    pub sigma_max: Option<usize>,
}

impl CleanBoundaryDifferentialReport {
    #[must_use]
    pub fn to_csv(&self) -> String {
        format!(
            "git_commit,rustc_version,command,seed,timestamp,masks,components,eligible_components,verified,unsupported,solver_errors,counterexamples,execution_trace_violations,orientation_counts,q_min,q_max,sigma_min,sigma_max\n{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            self.metadata.git_commit,
            self.metadata.rustc_version,
            escape_csv(&self.metadata.command),
            self.metadata
                .seed
                .map_or_else(String::new, |seed| seed.to_string()),
            self.metadata.timestamp,
            self.masks,
            self.components,
            self.eligible_components,
            self.verified,
            self.unsupported,
            self.solver_errors,
            self.counterexamples,
            self.execution_trace_violations,
            escape_csv(&serde_json::to_string(&self.orientation_counts).unwrap_or_default()),
            optional_number(self.q_min),
            optional_number(self.q_max),
            optional_number(self.sigma_min),
            optional_number(self.sigma_max),
        )
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!(
            "# v0.6 BoundaryLaminar Differential\n\n- Masks: {}\n- Components: {}\n- Eligible components: {}\n- Verified: {}\n- Unsupported: {}\n- Solver errors: {}\n- Counterexamples: {}\n- Execution-trace violations: {}\n- Orientation counts: `{}`\n- q range: {:?}\n- sigma range: {:?}\n",
            self.masks,
            self.components,
            self.eligible_components,
            self.verified,
            self.unsupported,
            self.solver_errors,
            self.counterexamples,
            self.execution_trace_violations,
            serde_json::to_string(&self.orientation_counts).unwrap_or_default(),
            self.q_min.zip(self.q_max),
            self.sigma_min.zip(self.sigma_max),
        )
    }
}

#[must_use]
pub fn clean_boundary_differential_4x4(
    context: BenchmarkContext,
) -> CleanBoundaryDifferentialReport {
    let mut components = 0;
    let mut eligible_components = 0;
    let mut verified = 0;
    let mut unsupported = 0;
    let mut solver_errors = 0;
    let mut counterexamples = 0;
    let mut execution_trace_violations = 0;
    let mut orientation_counts = BTreeMap::new();
    let mut q_min = None;
    let mut q_max = None;
    let mut sigma_min = None;
    let mut sigma_max = None;
    for mask in 1_u32..(1_u32 << 16) {
        let cells = (0..16)
            .map(|index| mask & (1_u32 << index) != 0)
            .collect::<Vec<_>>();
        let Ok(grid) = ColorGrid::new(4, 4, cells) else {
            continue;
        };
        for component in grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
        {
            components += 1;
            let Ok(geometry) = rect_oracle_sg::analyze_geometry_with(
                &component,
                &rect_oracle_sg::GridInteriorRunEnumerator,
            ) else {
                unsupported += 1;
                continue;
            };
            let certificate = rect_oracle_sg::classify_clean_hole_free(
                &component,
                &geometry.boundary,
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
            );
            if !certificate.eligible {
                continue;
            }
            eligible_components += 1;
            let q = geometry
                .horizontal_chords
                .len()
                .saturating_add(geometry.vertical_chords.len());
            q_min = Some(q_min.map_or(q, |value: usize| value.min(q)));
            q_max = Some(q_max.map_or(q, |value: usize| value.max(q)));
            let path = solve_with_representation_and_region_dual(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::CleanHoleFreePathTree,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
            );
            let general = solve_with_representation_and_region_dual(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::GeneralDominance4D,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
            );
            match (path, general) {
                (Ok(path), Ok(general)) => {
                    let trace_ok = !path
                        .diagnostics
                        .execution_trace
                        .pairwise_embedding_audit_called
                        && !path
                            .diagnostics
                            .execution_trace
                            .explicit_conflict_graph_built
                        && !path.diagnostics.execution_trace.hopcroft_karp_called
                        && !path.diagnostics.execution_trace.c0_partition_built
                        && !path
                            .diagnostics
                            .execution_trace
                            .full_edge_partition_audit_called
                        && !path
                            .diagnostics
                            .execution_trace
                            .full_tree_path_edge_lists_materialized
                        && !path.diagnostics.execution_trace.per_path_bfs_called
                        && !path.diagnostics.execution_trace.area_flood_fill_dual_built
                        && !path
                            .diagnostics
                            .execution_trace
                            .unit_chord_cuts_materialized
                        && !path
                            .diagnostics
                            .execution_trace
                            .prepared_occupancy_transposed
                        && path.diagnostics.explicit_conflict_edge_count.is_none();
                    if !trace_ok {
                        execution_trace_violations += 1;
                    }
                    let sigma = path.diagnostics.path_tree_sigma.unwrap_or(0);
                    sigma_min = Some(sigma_min.map_or(sigma, |value: usize| value.min(sigma)));
                    sigma_max = Some(sigma_max.map_or(sigma, |value: usize| value.max(sigma)));
                    if let Some(orientation) = path.diagnostics.path_tree_orientation.clone() {
                        *orientation_counts.entry(orientation).or_default() += 1;
                    }
                    if trace_ok
                        && path.optimum_rectangle_count == general.optimum_rectangle_count
                        && path.rectangles == general.rectangles
                        && path.diagnostics.clean_hole_free_eligible == Some(true)
                        && general.diagnostics.clean_hole_free_eligible == Some(true)
                    {
                        verified += 1;
                    } else {
                        counterexamples += 1;
                    }
                }
                (Err(_), _) | (_, Err(_)) => solver_errors += 1,
            }
        }
    }
    CleanBoundaryDifferentialReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: (1_u32 << 16) as usize - 1,
            component_count: components,
            input_model: "finite-colored-unit-grid-binary-4x4-boundary-differential".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        masks: (1_u32 << 16) as usize - 1,
        components,
        eligible_components,
        verified,
        unsupported,
        solver_errors,
        counterexamples,
        execution_trace_violations,
        orientation_counts,
        q_min,
        q_max,
        sigma_min,
        sigma_max,
    }
}

impl CleanCensusReport {
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "git_commit,rustc_version,command,seed,timestamp,total_components,hole_free_components,eligible_components,total_chord_count,eligible_chord_count,rejection_reason,count,eligible_q,eligible_q_count\n",
        );
        let mut reasons = self.rejection_counts.iter();
        let mut histogram = self.eligible_q_histogram.iter();
        loop {
            let reason = reasons.next();
            let q = histogram.next();
            if reason.is_none() && q.is_none() {
                break;
            }
            let (reason_name, reason_count) = reason
                .map_or((String::new(), String::new()), |(name, count)| {
                    (name.clone(), count.to_string())
                });
            let (q_value, q_count) = q.map_or((String::new(), String::new()), |(value, count)| {
                (value.to_string(), count.to_string())
            });
            let _ = writeln!(
                csv,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                self.metadata.git_commit,
                self.metadata.rustc_version,
                escape_csv(&self.metadata.command),
                self.metadata
                    .seed
                    .map_or_else(String::new, |seed| seed.to_string()),
                self.metadata.timestamp,
                self.total_components,
                self.hole_free_components,
                self.eligible_components,
                self.total_chord_count,
                self.eligible_chord_count,
                escape_csv(&reason_name),
                reason_count,
                q_value,
                q_count,
            );
        }
        csv
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!(
            "# v0.5 Clean Census\n\n- Components: {}\n- Hole-free: {}\n- Clean eligible: {}\n- Total chord mass: {}\n- Eligible chord mass: {}\n- Component fraction: {}/{}\n- Chord-mass fraction: {}/{}\n\nRejection counts and eligible-q histogram are generated in the companion CSV/JSON files.\n",
            self.total_components,
            self.hole_free_components,
            self.eligible_components,
            self.total_chord_count,
            self.eligible_chord_count,
            self.eligible_components,
            self.total_components.max(1),
            self.eligible_chord_count,
            self.total_chord_count.max(1),
        )
    }
}

#[must_use]
pub fn clean_census_4x4(context: BenchmarkContext) -> CleanCensusReport {
    let mut total_components = 0;
    let mut hole_free_components = 0;
    let mut eligible_components = 0;
    let mut total_chord_count = 0;
    let mut eligible_chord_count = 0;
    let mut rejection_counts = BTreeMap::new();
    let mut eligible_q_histogram = BTreeMap::new();
    for mask in 1_u32..(1_u32 << 16) {
        let cells = (0..16)
            .map(|index| mask & (1_u32 << index) != 0)
            .collect::<Vec<_>>();
        let Ok(grid) = ColorGrid::new(4, 4, cells) else {
            continue;
        };
        for component in grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
        {
            let Ok(geometry) = rect_oracle_sg::analyze_geometry_with(
                &component,
                &rect_oracle_sg::GridInteriorRunEnumerator,
            ) else {
                continue;
            };
            let certificate = rect_oracle_sg::classify_clean_hole_free(
                &component,
                &geometry.boundary,
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
            );
            let q = geometry.horizontal_chords.len() + geometry.vertical_chords.len();
            total_components += 1;
            total_chord_count += q;
            if certificate.hole_count == 0 {
                hole_free_components += 1;
            }
            if certificate.eligible {
                eligible_components += 1;
                eligible_chord_count += q;
                *eligible_q_histogram.entry(q).or_default() += 1;
            } else {
                for reason in certificate.rejection_reasons {
                    *rejection_counts
                        .entry(clean_rejection_name(&reason))
                        .or_default() += 1;
                }
            }
        }
    }
    CleanCensusReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: (1_u32 << 16) as usize - 1,
            component_count: total_components,
            input_model: "finite-colored-unit-grid-binary-4x4".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        total_components,
        hole_free_components,
        eligible_components,
        total_chord_count,
        eligible_chord_count,
        rejection_counts,
        eligible_q_histogram,
    }
}

fn clean_rejection_name(reason: &rect_oracle_sg::CleanRejectionReason) -> String {
    match reason {
        rect_oracle_sg::CleanRejectionReason::MultipleOuterLoops { .. } => {
            "multiple-outer-loops".to_owned()
        }
        rect_oracle_sg::CleanRejectionReason::HasHole { .. } => "has-hole".to_owned(),
        rect_oracle_sg::CleanRejectionReason::UnsupportedOrnamentModel => {
            "unsupported-ornament-model".to_owned()
        }
        rect_oracle_sg::CleanRejectionReason::NonProperHorizontalChord(_) => {
            "non-proper-horizontal-chord".to_owned()
        }
        rect_oracle_sg::CleanRejectionReason::NonProperVerticalChord(_) => {
            "non-proper-vertical-chord".to_owned()
        }
        rect_oracle_sg::CleanRejectionReason::EndpointNotOnBoundary => {
            "endpoint-not-on-boundary".to_owned()
        }
        rect_oracle_sg::CleanRejectionReason::SharedBoundaryEndpoint { .. } => {
            "shared-boundary-endpoint".to_owned()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkContext {
    pub git_commit: String,
    pub rustc_version: String,
    pub command: String,
    pub seed: Option<u64>,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkMetadata {
    pub git_commit: String,
    pub rustc_version: String,
    pub command: String,
    pub seed: Option<u64>,
    pub timestamp: u64,
    pub input_count: usize,
    pub component_count: usize,
    pub input_model: String,
    pub unsupported_input_features: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkRow {
    pub instance_name: String,
    pub family: String,
    pub parameters: BTreeMap<String, usize>,
    pub component_id: usize,
    pub status: String,
    pub message: Option<String>,
    pub exact_cover_compared: bool,
    pub diagnostics: Diagnostics,
    pub c0_phase_microseconds: BTreeMap<String, u128>,
    pub compressed_phase_microseconds: BTreeMap<String, u128>,
    pub compact_only_phase_microseconds: BTreeMap<String, u128>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub metadata: BenchmarkMetadata,
    pub verified_count: usize,
    pub unsupported_count: usize,
    pub solver_error_count: usize,
    pub counterexample_count: usize,
    pub failure_fixtures: Vec<GridFixture>,
    pub rows: Vec<BenchmarkRow>,
}

impl BenchmarkReport {
    /// Serializes the report to a stable, machine-readable CSV schema.
    ///
    /// # Errors
    ///
    /// Returns `fmt::Error` if writing to the in-memory string fails.
    #[allow(clippy::too_many_lines)]
    pub fn to_csv(&self) -> Result<String, std::fmt::Error> {
        let mut csv = String::new();
        writeln!(
            csv,
            "git_commit,rustc_version,command,seed,timestamp,input_count,component_count,input_model,unsupported_input_features,instance_name,family,parameters,component_id,status,message,exact_cover_compared,cell_count,boundary_complexity,hole_count,reflex_vertex_count,horizontal_chord_count,vertical_chord_count,total_chord_count,effective_chord_enumerator,effective_chord_enumeration_microseconds,prepared_component_build_count,prepared_component_build_microseconds,boundary_extraction_microseconds,reflex_grouping_microseconds,occupancy_bytes,horizontal_interior_run_count,vertical_interior_run_count,candidate_reflex_pair_count,emitted_chord_count,explicit_conflict_edge_count,edge_density_numerator,edge_density_denominator,biclique_count,biclique_total_vertex_occurrences,biclique_size_per_chord_numerator,biclique_size_per_chord_denominator,biclique_size_per_edge_numerator,biclique_size_per_edge_denominator,c0_network_vertex_count,c0_network_arc_count,compressed_network_vertex_count,compressed_network_arc_count,maximum_matching_size,minimum_vertex_cover_size,output_rectangle_count,completion_backend,conflict_representation,path_tree_orientation,dual_region_count,path_count,path_edge_incidence_count,canonical_segment_node_count,path_tree_sigma,four_d_sigma,selected_chord_cut_materialization_microseconds,horizontal_simple_chord_completion_microseconds,vertical_simple_chord_completion_microseconds,rectangle_recovery_microseconds,final_output_validation_microseconds,completion_candidate_queries,completion_full_grid_scans,completion_added_horizontal_unit_cuts,completion_added_vertical_unit_cuts,completion_stale_candidates,rectangle_recovery_queue_pushes,rectangle_recovery_region_count,rectangle_recovery_allocations,c0_phase_microseconds,compressed_phase_microseconds,compact_only_phase_microseconds,peak_memory_bytes,owned_allocation_estimates,region_dual_backend,region_dual_construction_microseconds,dual_tree_edge_count,dual_allocated_bytes,dual_unit_cut_count,dual_area_cell_visits,dual_interval_count,dual_maximum_nesting_depth,hld_interval_count,explicit_path_records_materialized"
        )?;
        for row in &self.rows {
            let density = ratio_columns(row.diagnostics.conflict_edge_density);
            let sigma_per_chord = ratio_columns(row.diagnostics.biclique_size_per_chord);
            let sigma_per_edge = ratio_columns(row.diagnostics.biclique_size_per_explicit_edge);
            let c0_phases = serde_json::to_string(&row.c0_phase_microseconds)
                .unwrap_or_else(|_| "{}".to_owned());
            let compressed_phases = serde_json::to_string(&row.compressed_phase_microseconds)
                .unwrap_or_else(|_| "{}".to_owned());
            let compact_only_phases = serde_json::to_string(&row.compact_only_phase_microseconds)
                .unwrap_or_else(|_| "{}".to_owned());
            let parameters =
                serde_json::to_string(&row.parameters).unwrap_or_else(|_| "{}".to_owned());
            let owned_allocation_estimates =
                serde_json::to_string(&row.diagnostics.owned_allocation_estimates)
                    .unwrap_or_else(|_| "{}".to_owned());
            let fields = [
                self.metadata.git_commit.clone(),
                self.metadata.rustc_version.clone(),
                self.metadata.command.clone(),
                self.metadata
                    .seed
                    .map(|seed| seed.to_string())
                    .unwrap_or_default(),
                self.metadata.timestamp.to_string(),
                self.metadata.input_count.to_string(),
                self.metadata.component_count.to_string(),
                self.metadata.input_model.clone(),
                self.metadata.unsupported_input_features.join(";"),
                row.instance_name.clone(),
                row.family.clone(),
                parameters,
                row.component_id.to_string(),
                row.status.clone(),
                row.message.clone().unwrap_or_default(),
                row.exact_cover_compared.to_string(),
                row.diagnostics.cell_count.to_string(),
                row.diagnostics.boundary_complexity.to_string(),
                row.diagnostics.hole_count.to_string(),
                row.diagnostics.reflex_vertex_count.to_string(),
                row.diagnostics.horizontal_chord_count.to_string(),
                row.diagnostics.vertical_chord_count.to_string(),
                row.diagnostics.total_chord_count.to_string(),
                row.diagnostics
                    .effective_chord_enumerator
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.effective_chord_enumeration_microseconds),
                optional_number(row.diagnostics.prepared_component_build_count),
                optional_number(row.diagnostics.prepared_component_build_microseconds),
                optional_number(row.diagnostics.boundary_extraction_microseconds),
                optional_number(row.diagnostics.reflex_grouping_microseconds),
                optional_number(row.diagnostics.occupancy_bytes),
                optional_number(row.diagnostics.horizontal_interior_run_count),
                optional_number(row.diagnostics.vertical_interior_run_count),
                optional_number(row.diagnostics.candidate_reflex_pair_count),
                optional_number(row.diagnostics.emitted_chord_count),
                row.diagnostics
                    .explicit_conflict_edge_count
                    .map_or_else(String::new, |count| count.to_string()),
                density.0,
                density.1,
                row.diagnostics.biclique_count.to_string(),
                row.diagnostics
                    .biclique_total_vertex_occurrences
                    .to_string(),
                sigma_per_chord.0,
                sigma_per_chord.1,
                sigma_per_edge.0,
                sigma_per_edge.1,
                row.diagnostics.c0_network_vertex_count.to_string(),
                row.diagnostics.c0_network_arc_count.to_string(),
                row.diagnostics.compressed_network_vertex_count.to_string(),
                row.diagnostics.compressed_network_arc_count.to_string(),
                row.diagnostics.maximum_matching_size.to_string(),
                row.diagnostics.minimum_vertex_cover_size.to_string(),
                row.diagnostics.output_rectangle_count.to_string(),
                row.diagnostics
                    .completion_backend
                    .clone()
                    .unwrap_or_default(),
                row.diagnostics
                    .conflict_representation
                    .clone()
                    .unwrap_or_default(),
                row.diagnostics
                    .path_tree_orientation
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.dual_region_count),
                optional_number(row.diagnostics.path_count),
                optional_number(row.diagnostics.path_edge_incidence_count),
                optional_number(row.diagnostics.canonical_segment_node_count),
                optional_number(row.diagnostics.path_tree_sigma),
                optional_number(row.diagnostics.four_d_sigma),
                optional_number(
                    row.diagnostics
                        .selected_chord_cut_materialization_microseconds,
                ),
                optional_number(
                    row.diagnostics
                        .horizontal_simple_chord_completion_microseconds,
                ),
                optional_number(
                    row.diagnostics
                        .vertical_simple_chord_completion_microseconds,
                ),
                optional_number(row.diagnostics.rectangle_recovery_microseconds),
                optional_number(row.diagnostics.final_output_validation_microseconds),
                optional_number(row.diagnostics.completion_candidate_queries),
                optional_number(row.diagnostics.completion_full_grid_scans),
                optional_number(row.diagnostics.added_horizontal_unit_cut_count),
                optional_number(row.diagnostics.added_vertical_unit_cut_count),
                optional_number(row.diagnostics.completion_stale_candidates),
                optional_number(row.diagnostics.rectangle_recovery_queue_pushes),
                optional_number(row.diagnostics.rectangle_recovery_region_count),
                optional_number(row.diagnostics.rectangle_recovery_allocations),
                c0_phases,
                compressed_phases,
                compact_only_phases,
                row.diagnostics
                    .peak_memory_bytes
                    .map(|bytes| bytes.to_string())
                    .unwrap_or_default(),
                owned_allocation_estimates,
                row.diagnostics
                    .region_dual_backend
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.region_dual_construction_microseconds),
                optional_number(row.diagnostics.dual_tree_edge_count),
                optional_number(row.diagnostics.dual_allocated_bytes),
                optional_number(row.diagnostics.dual_unit_cut_count),
                optional_number(row.diagnostics.dual_area_cell_visits),
                optional_number(row.diagnostics.dual_interval_count),
                optional_number(row.diagnostics.dual_maximum_nesting_depth),
                optional_number(row.diagnostics.hld_interval_count),
                optional_number(row.diagnostics.explicit_path_records_materialized),
            ];
            writeln!(
                csv,
                "{}",
                fields
                    .iter()
                    .map(|field| escape_csv(field))
                    .collect::<Vec<_>>()
                    .join(",")
            )?;
        }
        Ok(csv)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExperimentManifest {
    pub schema_version: usize,
    pub runs: Vec<BenchmarkMetadata>,
}

impl Default for ExperimentManifest {
    fn default() -> Self {
        Self {
            schema_version: 1,
            runs: Vec::new(),
        }
    }
}

#[must_use]
pub fn benchmark_adversarial(context: BenchmarkContext) -> BenchmarkReport {
    let instances = endpoint_contact_instances()
        .into_iter()
        .chain(topological_stress_instances())
        .chain([dense_conflict_grid(4, 5), dense_conflict_grid(8, 8)])
        .collect::<Vec<_>>();
    benchmark_instances(context, &instances, 40)
}

#[must_use]
pub fn benchmark_dense_compact_only(context: BenchmarkContext, sizes: &[usize]) -> BenchmarkReport {
    let instances = sizes
        .iter()
        .map(|&size| dense_conflict_grid(size, size))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for instance in &instances {
        match instance.foreground_components() {
            Ok(components) => {
                for component in components {
                    let result = rect_dominance::solve_with_verification_mode(
                        &component,
                        VerificationMode::CompactOnly,
                    );
                    rows.push(match result {
                        Ok(result) => BenchmarkRow {
                            instance_name: instance.name.clone(),
                            family: "dense-compact-only".to_owned(),
                            parameters: instance.parameters.clone(),
                            component_id: component.id.0,
                            status: "verified".to_owned(),
                            message: None,
                            exact_cover_compared: false,
                            compact_only_phase_microseconds: result
                                .diagnostics
                                .phase_microseconds
                                .clone(),
                            diagnostics: result.diagnostics,
                            c0_phase_microseconds: BTreeMap::new(),
                            compressed_phase_microseconds: BTreeMap::new(),
                        },
                        Err(error) => BenchmarkRow {
                            instance_name: instance.name.clone(),
                            family: "dense-compact-only".to_owned(),
                            parameters: instance.parameters.clone(),
                            component_id: component.id.0,
                            status: "solver-error".to_owned(),
                            message: Some(error.to_string()),
                            exact_cover_compared: false,
                            diagnostics: Diagnostics {
                                cell_count: component.cell_count(),
                                ..Diagnostics::default()
                            },
                            c0_phase_microseconds: BTreeMap::new(),
                            compressed_phase_microseconds: BTreeMap::new(),
                            compact_only_phase_microseconds: BTreeMap::new(),
                        },
                    });
                }
            }
            Err(error) => rows.push(BenchmarkRow {
                instance_name: instance.name.clone(),
                family: "dense-compact-only".to_owned(),
                parameters: instance.parameters.clone(),
                component_id: 0,
                status: "unsupported".to_owned(),
                message: Some(error.to_string()),
                exact_cover_compared: false,
                diagnostics: Diagnostics::default(),
                c0_phase_microseconds: BTreeMap::new(),
                compressed_phase_microseconds: BTreeMap::new(),
                compact_only_phase_microseconds: BTreeMap::new(),
            }),
        }
    }
    let verified_count = count_status(&rows, "verified");
    let unsupported_count = count_status(&rows, "unsupported");
    let solver_error_count = count_status(&rows, "solver-error");
    BenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: instances.len(),
            component_count: rows.len(),
            input_model: "finite-colored-unit-cell-grid".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        verified_count,
        unsupported_count,
        solver_error_count,
        counterexample_count: 0,
        failure_fixtures: Vec::new(),
        rows,
    }
}

#[must_use]
pub fn benchmark_dense_completion(context: BenchmarkContext, sizes: &[usize]) -> BenchmarkReport {
    let backends = [
        (CompletionBackendKind::ReferenceRescan, "reference-rescan"),
        (CompletionBackendKind::IndexedFrontier, "indexed-frontier"),
    ];
    let instances = sizes
        .iter()
        .map(|&size| dense_conflict_grid(size, size))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for (backend, backend_name) in backends {
        for instance in &instances {
            if let Ok(components) = instance.foreground_components() {
                for component in components {
                    let result = rect_dominance::solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                        &component,
                        VerificationMode::CompactOnly,
                        rect_dominance::ChordEnumerator::GridInteriorRuns,
                        backend,
                    );
                    rows.push(match result {
                        Ok(result) => BenchmarkRow {
                            instance_name: format!("{}-{backend_name}", instance.name),
                            family: "dense-completion".to_owned(),
                            parameters: instance.parameters.clone(),
                            component_id: component.id.0,
                            status: "verified".to_owned(),
                            message: None,
                            exact_cover_compared: false,
                            compact_only_phase_microseconds: result
                                .diagnostics
                                .phase_microseconds
                                .clone(),
                            diagnostics: result.diagnostics,
                            c0_phase_microseconds: BTreeMap::new(),
                            compressed_phase_microseconds: BTreeMap::new(),
                        },
                        Err(error) => BenchmarkRow {
                            instance_name: format!("{}-{backend_name}", instance.name),
                            family: "dense-completion".to_owned(),
                            parameters: instance.parameters.clone(),
                            component_id: component.id.0,
                            status: "solver-error".to_owned(),
                            message: Some(error.to_string()),
                            exact_cover_compared: false,
                            diagnostics: Diagnostics {
                                cell_count: component.cell_count(),
                                ..Diagnostics::default()
                            },
                            c0_phase_microseconds: BTreeMap::new(),
                            compressed_phase_microseconds: BTreeMap::new(),
                            compact_only_phase_microseconds: BTreeMap::new(),
                        },
                    });
                }
            }
        }
    }
    BenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: instances.len() * backends.len(),
            component_count: rows.len(),
            input_model: "finite-colored-unit-cell-grid".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        verified_count: count_status(&rows, "verified"),
        unsupported_count: count_status(&rows, "unsupported"),
        solver_error_count: count_status(&rows, "solver-error"),
        counterexample_count: 0,
        failure_fixtures: Vec::new(),
        rows,
    }
}

#[must_use]
pub fn benchmark_completion_heavy(
    context: BenchmarkContext,
    sizes: &[usize],
    families: &[String],
) -> BenchmarkReport {
    let mut instances = Vec::new();
    for &size in sizes {
        for family in families {
            let mut instance = match family.as_str() {
                "staircase" => staircase(size.max(2)),
                "alternating-notch-corridor" | "notch-corridor" => {
                    alternating_notch_corridor(size.max(2))
                }
                "comb" => comb(size.max(2), size.div_ceil(2).max(3)),
                "double-comb" => double_comb(size.max(2), size.div_ceil(2).max(5)),
                "orthogonal-spiral" | "spiral" => {
                    let odd_size = size.max(5) | 1;
                    orthogonal_spiral(odd_size)
                }
                _ => continue,
            };
            "completion-heavy".clone_into(&mut instance.family);
            instance
                .parameters
                .insert("requested_size".to_owned(), size);
            instances.push(instance);
        }
    }
    benchmark_completion_instances(context, &instances, "completion-heavy")
}

#[must_use]
pub fn benchmark_area_heavy(context: BenchmarkContext, sizes: &[usize]) -> BenchmarkReport {
    let instances = sizes
        .iter()
        .map(|&size| AdversarialInstance {
            name: format!("solid-area-{size}x{size}"),
            family: "area-heavy".to_owned(),
            width: size,
            height: size,
            cells: vec![true; size * size],
            parameters: [("side".to_owned(), size)].into_iter().collect(),
        })
        .collect::<Vec<_>>();
    benchmark_completion_instances(context, &instances, "area-heavy")
}

fn benchmark_completion_instances(
    context: BenchmarkContext,
    instances: &[AdversarialInstance],
    family: &str,
) -> BenchmarkReport {
    let backends = [
        (CompletionBackendKind::ReferenceRescan, "reference-rescan"),
        (CompletionBackendKind::IndexedFrontier, "indexed-frontier"),
    ];
    let mut rows = Vec::new();
    for (backend, backend_name) in backends {
        for instance in instances {
            if let Ok(components) = instance.foreground_components() {
                for component in components {
                    let result = rect_dominance::solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
                        &component,
                        VerificationMode::CompactOnly,
                        rect_dominance::ChordEnumerator::GridInteriorRuns,
                        backend,
                    );
                    rows.push(match result {
                        Ok(result) => BenchmarkRow {
                            instance_name: format!("{}-{backend_name}", instance.name),
                            family: family.to_owned(),
                            parameters: instance.parameters.clone(),
                            component_id: component.id.0,
                            status: "verified".to_owned(),
                            message: None,
                            exact_cover_compared: false,
                            compact_only_phase_microseconds: result
                                .diagnostics
                                .phase_microseconds
                                .clone(),
                            diagnostics: result.diagnostics,
                            c0_phase_microseconds: BTreeMap::new(),
                            compressed_phase_microseconds: BTreeMap::new(),
                        },
                        Err(error) => BenchmarkRow {
                            instance_name: format!("{}-{backend_name}", instance.name),
                            family: family.to_owned(),
                            parameters: instance.parameters.clone(),
                            component_id: component.id.0,
                            status: "solver-error".to_owned(),
                            message: Some(error.to_string()),
                            exact_cover_compared: false,
                            diagnostics: Diagnostics {
                                cell_count: component.cell_count(),
                                ..Diagnostics::default()
                            },
                            c0_phase_microseconds: BTreeMap::new(),
                            compressed_phase_microseconds: BTreeMap::new(),
                            compact_only_phase_microseconds: BTreeMap::new(),
                        },
                    });
                }
            }
        }
    }
    BenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: instances.len() * backends.len(),
            component_count: rows.len(),
            input_model: "finite-colored-unit-cell-grid".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        verified_count: count_status(&rows, "verified"),
        unsupported_count: count_status(&rows, "unsupported"),
        solver_error_count: count_status(&rows, "solver-error"),
        counterexample_count: 0,
        failure_fixtures: Vec::new(),
        rows,
    }
}

#[must_use]
pub fn benchmark_polyomino(
    context: BenchmarkContext,
    max_cells: usize,
    oracle_cell_limit: usize,
) -> BenchmarkReport {
    let free = enumerate_free_polyominoes(max_cells)
        .into_iter()
        .enumerate()
        .flat_map(|(size_index, level)| {
            level.into_iter().enumerate().map(move |(index, shape)| {
                shape.to_instance(
                    format!("free-polyomino-{}-{}", size_index + 1, index + 1),
                    "free-polyomino",
                )
            })
        });
    let instances = free
        .chain(explicit_hole_polyominoes(max_cells))
        .collect::<Vec<_>>();
    benchmark_instances(context, &instances, oracle_cell_limit)
}

#[must_use]
pub fn benchmark_dense_conflict(context: BenchmarkContext, sizes: &[usize]) -> BenchmarkReport {
    let instances = sizes
        .iter()
        .map(|&size| dense_conflict_grid(size, size))
        .collect::<Vec<_>>();
    benchmark_instances(context, &instances, 0)
}

#[must_use]
pub fn benchmark_clean_complete_bipartite(
    context: BenchmarkContext,
    sizes: &[usize],
) -> BenchmarkReport {
    let instances = sizes
        .iter()
        .filter_map(|&size| clean_complete_bipartite_grid(size).ok())
        .collect::<Vec<_>>();
    benchmark_instances(context, &instances, 40)
}

/// Measures the true compact path-tree pipeline without invoking the
/// area-sensitive SG/matching or explicit 4D audit paths.  The general 4D
/// `CompactOnly` result is retained as an independent output oracle.
#[must_use]
pub fn benchmark_clean_complete_bipartite_compact(
    context: BenchmarkContext,
    sizes: &[usize],
) -> BenchmarkReport {
    let mut rows = Vec::new();
    for &size in sizes {
        let Ok(instance) = clean_complete_bipartite_grid(size) else {
            continue;
        };
        let Ok(components) = instance.foreground_components() else {
            continue;
        };
        for component in components {
            let path = rect_dominance::solve_with_representation(
                &component,
                VerificationMode::CompactOnly,
                rect_dominance::ConflictRepresentationBackend::CleanHoleFreePathTree,
                rect_dominance::ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
            );
            let general = rect_dominance::solve_with_representation(
                &component,
                VerificationMode::CompactOnly,
                rect_dominance::ConflictRepresentationBackend::GeneralDominance4D,
                rect_dominance::ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
            );
            let (status, message, diagnostics) = match (path, general) {
                (Ok(path), Ok(general))
                    if path.optimum_rectangle_count == general.optimum_rectangle_count
                        && path.rectangles == general.rectangles
                        && path.diagnostics.explicit_conflict_edge_count.is_none()
                        && !path
                            .diagnostics
                            .execution_trace
                            .full_tree_path_edge_lists_materialized =>
                {
                    ("verified".to_owned(), None, path.diagnostics)
                }
                (Ok(path), Ok(_)) => (
                    "counterexample".to_owned(),
                    Some("compact path-tree and compact 4D outputs differ".to_owned()),
                    path.diagnostics,
                ),
                (Err(error), _) | (_, Err(error)) => (
                    "solver-error".to_owned(),
                    Some(error.to_string()),
                    Diagnostics {
                        cell_count: component.cell_count(),
                        ..Diagnostics::default()
                    },
                ),
            };
            rows.push(BenchmarkRow {
                instance_name: instance.name.clone(),
                family: "clean-complete-bipartite-compact".to_owned(),
                parameters: instance.parameters.clone(),
                component_id: component.id.0,
                status,
                message,
                exact_cover_compared: false,
                diagnostics,
                c0_phase_microseconds: BTreeMap::new(),
                compressed_phase_microseconds: BTreeMap::new(),
                compact_only_phase_microseconds: BTreeMap::new(),
            });
        }
    }
    BenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: rows.len(),
            component_count: rows.len(),
            input_model: "finite-colored-unit-grid-clean-complete-bipartite-compact".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        verified_count: count_status(&rows, "verified"),
        unsupported_count: count_status(&rows, "unsupported"),
        solver_error_count: count_status(&rows, "solver-error"),
        counterexample_count: count_status(&rows, "counterexample"),
        failure_fixtures: Vec::new(),
        rows,
    }
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn benchmark_path_tree_comparison(
    context: BenchmarkContext,
    sizes: &[usize],
) -> BenchmarkReport {
    let requested = sizes.iter().copied().max().unwrap_or(3).min(4);
    let side = requested.max(3);
    let bit_count = side * side;
    let mut rows = Vec::new();
    for mask in 1_u32..(1_u32 << bit_count) {
        let cells = (0..bit_count)
            .map(|index| mask & (1_u32 << index) != 0)
            .collect::<Vec<_>>();
        let Ok(grid) = ColorGrid::new(side, side, cells) else {
            continue;
        };
        for component in grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
        {
            let Ok(geometry) = rect_oracle_sg::analyze_geometry(&component) else {
                continue;
            };
            let certificate = rect_oracle_sg::classify_clean_hole_free(
                &component,
                &geometry.boundary,
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
            );
            if !certificate.eligible {
                continue;
            }
            let path = rect_dominance::solve_with_representation(
                &component,
                rect_dominance::VerificationMode::FullyAudited,
                rect_dominance::ConflictRepresentationBackend::CleanHoleFreePathTree,
                rect_dominance::ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::ReferenceRescan,
            );
            let general = rect_dominance::solve_with_representation(
                &component,
                rect_dominance::VerificationMode::FullyAudited,
                rect_dominance::ConflictRepresentationBackend::GeneralDominance4D,
                rect_dominance::ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::ReferenceRescan,
            );
            let (status, message, diagnostics) = match (path, general) {
                (Ok(path), Ok(general))
                    if path.optimum_rectangle_count == general.optimum_rectangle_count
                        && path.rectangles == general.rectangles =>
                {
                    ("verified".to_owned(), None, path.diagnostics)
                }
                (Ok(path), Ok(_general)) => (
                    "counterexample".to_owned(),
                    Some("path-tree and 4D outputs differ".to_owned()),
                    path.diagnostics,
                ),
                (Err(error), _) | (_, Err(error)) => (
                    "solver-error".to_owned(),
                    Some(error.to_string()),
                    Diagnostics {
                        cell_count: component.cell_count(),
                        ..Diagnostics::default()
                    },
                ),
            };
            rows.push(BenchmarkRow {
                instance_name: format!("binary-{side}x{side}-{mask:x}"),
                family: "path-tree-comparison".to_owned(),
                parameters: [("side".to_owned(), side)].into_iter().collect(),
                component_id: component.id.0,
                status,
                message,
                exact_cover_compared: false,
                diagnostics,
                c0_phase_microseconds: BTreeMap::new(),
                compressed_phase_microseconds: BTreeMap::new(),
                compact_only_phase_microseconds: BTreeMap::new(),
            });
        }
    }
    let verified_count = count_status(&rows, "verified");
    let unsupported_count = count_status(&rows, "unsupported");
    let solver_error_count = count_status(&rows, "solver-error");
    let counterexample_count = count_status(&rows, "counterexample");
    BenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: usize::try_from(
                1_u32
                    .checked_shl(u32::try_from(bit_count).unwrap_or(32))
                    .unwrap_or(0),
            )
            .unwrap_or(0),
            component_count: rows.len(),
            input_model: "finite-colored-unit-grid-path-tree-comparison".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        verified_count,
        unsupported_count,
        solver_error_count,
        counterexample_count,
        failure_fixtures: Vec::new(),
        rows,
    }
}

fn benchmark_instances(
    context: BenchmarkContext,
    instances: &[AdversarialInstance],
    oracle_cell_limit: usize,
) -> BenchmarkReport {
    let mut rows = Vec::new();
    let mut failure_fixtures = Vec::new();
    for instance in instances {
        match instance.foreground_components() {
            Ok(components) => {
                for component in components {
                    let row = benchmark_component(instance, &component, oracle_cell_limit);
                    if matches!(row.status.as_str(), "counterexample" | "solver-error") {
                        failure_fixtures.push(GridFixture {
                            width: instance.width,
                            height: instance.height,
                            cells: instance.cells.clone(),
                            reason: row.message.clone().unwrap_or_else(|| row.status.clone()),
                        });
                    }
                    rows.push(row);
                }
            }
            Err(error) => rows.push(BenchmarkRow {
                instance_name: instance.name.clone(),
                family: instance.family.clone(),
                parameters: instance.parameters.clone(),
                component_id: 0,
                status: "unsupported".to_owned(),
                message: Some(error.to_string()),
                exact_cover_compared: false,
                diagnostics: Diagnostics::default(),
                c0_phase_microseconds: BTreeMap::new(),
                compressed_phase_microseconds: BTreeMap::new(),
                compact_only_phase_microseconds: BTreeMap::new(),
            }),
        }
    }
    let verified_count = count_status(&rows, "verified");
    let unsupported_count = count_status(&rows, "unsupported");
    let solver_error_count = count_status(&rows, "solver-error");
    let counterexample_count = count_status(&rows, "counterexample");
    BenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: instances.len(),
            component_count: rows.len(),
            input_model: "finite-colored-unit-cell-grid".to_owned(),
            unsupported_input_features: vec![
                "ornaments".to_owned(),
                "isolated-formal-boundary-points".to_owned(),
                "line-segment-holes".to_owned(),
                "point-holes".to_owned(),
                "degenerate-formal-holes".to_owned(),
                "general-polygon-input".to_owned(),
            ],
        },
        verified_count,
        unsupported_count,
        solver_error_count,
        counterexample_count,
        failure_fixtures,
        rows,
    }
}

fn benchmark_component<C>(
    instance: &AdversarialInstance,
    component: &GridComponent<C>,
    oracle_cell_limit: usize,
) -> BenchmarkRow {
    match verify_component(component, oracle_cell_limit) {
        Ok(verification) => {
            let c0_phase_microseconds = verification
                .dominance_c0
                .diagnostics
                .phase_microseconds
                .clone();
            let compressed_phase_microseconds = verification
                .dominance_compact
                .diagnostics
                .phase_microseconds
                .clone();
            let compact_only_phase_microseconds = verification
                .dominance_compact_only
                .diagnostics
                .phase_microseconds
                .clone();
            BenchmarkRow {
                instance_name: instance.name.clone(),
                family: instance.family.clone(),
                parameters: instance.parameters.clone(),
                component_id: component.id.0,
                status: "verified".to_owned(),
                message: None,
                exact_cover_compared: verification.exact_cover.is_some(),
                diagnostics: verification.dominance_compact.diagnostics,
                c0_phase_microseconds,
                compressed_phase_microseconds,
                compact_only_phase_microseconds,
            }
        }
        Err(error) => {
            let status = match error {
                VerificationError::OptimumMismatch { .. } => "counterexample",
                VerificationError::Solver { .. } | VerificationError::Fixture { .. } => {
                    "solver-error"
                }
                VerificationError::EnumerationTooLarge => "unsupported",
            };
            BenchmarkRow {
                instance_name: instance.name.clone(),
                family: instance.family.clone(),
                parameters: instance.parameters.clone(),
                component_id: component.id.0,
                status: status.to_owned(),
                message: Some(error.to_string()),
                exact_cover_compared: component.cell_count() <= oracle_cell_limit,
                diagnostics: Diagnostics {
                    cell_count: component.cell_count(),
                    ..Diagnostics::default()
                },
                c0_phase_microseconds: BTreeMap::new(),
                compressed_phase_microseconds: BTreeMap::new(),
                compact_only_phase_microseconds: BTreeMap::new(),
            }
        }
    }
}

fn count_status(rows: &[BenchmarkRow], status: &str) -> usize {
    rows.iter().filter(|row| row.status == status).count()
}

fn optional_number<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

fn unsupported_input_features() -> Vec<String> {
    [
        "ornaments",
        "isolated-formal-boundary-points",
        "line-segment-holes",
        "point-holes",
        "degenerate-formal-holes",
        "general-polygon-input",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn ratio_columns(ratio: Option<ExactRatio>) -> (String, String) {
    ratio.map_or_else(
        || (String::new(), String::new()),
        |ratio| (ratio.numerator.to_string(), ratio.denominator.to_string()),
    )
}

fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[must_use]
pub fn summarize_compression(report: &BenchmarkReport) -> BTreeMap<String, u128> {
    let verified = report.rows.iter().filter(|row| row.status == "verified");
    let mut summary = BTreeMap::new();
    for row in verified {
        *summary.entry("explicit_edges".to_owned()).or_default() += row
            .diagnostics
            .explicit_conflict_edge_count
            .map_or(0, |count| count as u128);
        *summary.entry("biclique_count".to_owned()).or_default() +=
            row.diagnostics.biclique_count as u128;
        *summary.entry("biclique_total_size".to_owned()).or_default() +=
            row.diagnostics.biclique_total_vertex_occurrences as u128;
        *summary.entry("network_vertices".to_owned()).or_default() +=
            row.diagnostics.compressed_network_vertex_count as u128;
        *summary.entry("network_arcs".to_owned()).or_default() +=
            row.diagnostics.compressed_network_arc_count as u128;
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::{BenchmarkContext, benchmark_adversarial};

    #[test]
    fn adversarial_benchmark_is_machine_readable_and_clean() {
        let report = benchmark_adversarial(BenchmarkContext {
            git_commit: "test".to_owned(),
            rustc_version: "test".to_owned(),
            command: "test".to_owned(),
            seed: None,
            timestamp: 0,
        });
        assert_eq!(report.counterexample_count, 0);
        assert_eq!(report.solver_error_count, 0);
        assert_eq!(report.verified_count, report.metadata.component_count);
        let csv = report.to_csv().unwrap();
        assert_eq!(csv.lines().count(), report.rows.len() + 1);
    }
}
