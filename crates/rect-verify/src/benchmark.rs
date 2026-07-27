use std::collections::BTreeMap;
use std::fmt::Write;
use std::time::Instant;

use rect_core::{ColorGrid, Diagnostics, ExactRatio, GridComponent};
use rect_dominance::{
    ChordEnumerator, ConflictRepresentationBackend, VerificationMode,
    biclique::{BicliqueConstructionMetrics, BicliquePartition},
    embedding::DominanceEmbedding,
    solve_with_representation_and_region_dual,
    solve_with_representation_and_region_dual_and_orientation_policy,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OrientationAuditRow {
    pub population: String,
    pub instance_name: String,
    pub q: usize,
    pub horizontal_chords: usize,
    pub vertical_chords: usize,
    pub chosen_orientation: String,
    pub best_orientation: String,
    pub selected_sigma: usize,
    pub best_sigma: usize,
    pub absolute_regret: usize,
    pub regret_ratio: Option<ExactRatio>,
    pub bound_construction_microseconds: u128,
    pub build_both_construction_microseconds: u128,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OrientationAuditReport {
    pub metadata: BenchmarkMetadata,
    pub rows: Vec<OrientationAuditRow>,
    pub exact_matches: usize,
    pub mismatches: usize,
    pub tie_orientation_differences: usize,
    pub maximum_absolute_regret: usize,
    pub maximum_regret_ratio: Option<ExactRatio>,
}

impl OrientationAuditReport {
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "population,instance_name,q,horizontal_chords,vertical_chords,chosen_orientation,best_orientation,selected_sigma,best_sigma,absolute_regret,regret_ratio,bound_construction_microseconds,build_both_construction_microseconds,status\n",
        );
        for row in &self.rows {
            let ratio = row.regret_ratio.map_or_else(String::new, |ratio| {
                format!("{}/{}", ratio.numerator, ratio.denominator)
            });
            let fields = [
                row.population.clone(),
                row.instance_name.clone(),
                row.q.to_string(),
                row.horizontal_chords.to_string(),
                row.vertical_chords.to_string(),
                row.chosen_orientation.clone(),
                row.best_orientation.clone(),
                row.selected_sigma.to_string(),
                row.best_sigma.to_string(),
                row.absolute_regret.to_string(),
                ratio,
                row.bound_construction_microseconds.to_string(),
                row.build_both_construction_microseconds.to_string(),
                row.status.clone(),
            ];
            let _ = writeln!(
                csv,
                "{}",
                fields
                    .iter()
                    .map(|field| escape_csv(field))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        csv
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!(
            "# v0.7 Path-tree orientation audit\n\n- Rows: {}\n- Exact sigma matches: {}\n- Regret mismatches: {}\n- Tie orientation differences: {}\n- Maximum absolute regret: {}\n- Maximum regret ratio: {:?}\n",
            self.rows.len(),
            self.exact_matches,
            self.mismatches,
            self.tie_orientation_differences,
            self.maximum_absolute_regret,
            self.maximum_regret_ratio,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreeDualDifferentialRow {
    pub population: String,
    pub instance_name: String,
    pub q: usize,
    pub boundary_sigma: Option<usize>,
    pub area_sigma: Option<usize>,
    pub rectangles_equal: bool,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreeDualDifferentialReport {
    pub metadata: BenchmarkMetadata,
    pub rows: Vec<PathTreeDualDifferentialRow>,
    pub verified: usize,
    pub counterexamples: usize,
    pub solver_errors: usize,
}

impl PathTreeDualDifferentialReport {
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "population,instance_name,q,boundary_sigma,area_sigma,rectangles_equal,status\n",
        );
        for row in &self.rows {
            let fields = [
                row.population.clone(),
                row.instance_name.clone(),
                row.q.to_string(),
                optional_number(row.boundary_sigma),
                optional_number(row.area_sigma),
                row.rectangles_equal.to_string(),
                row.status.clone(),
            ];
            let _ = writeln!(
                csv,
                "{}",
                fields
                    .iter()
                    .map(|field| escape_csv(field))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        csv
    }
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
#[allow(clippy::too_many_lines)]
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

fn append_orientation_audit_component<C>(
    rows: &mut Vec<OrientationAuditRow>,
    population: &str,
    instance_name: &str,
    component: &rect_core::GridComponent<C>,
) where
    C: Clone + Eq,
{
    let Ok(geometry) = rect_oracle_sg::analyze_geometry_with(
        component,
        &rect_oracle_sg::GridInteriorRunEnumerator,
    ) else {
        return;
    };
    let certificate = rect_oracle_sg::classify_clean_hole_free(
        component,
        &geometry.boundary,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
    );
    if !certificate.eligible {
        return;
    }
    let q = geometry
        .horizontal_chords
        .len()
        .saturating_add(geometry.vertical_chords.len());
    let build_both_started = Instant::now();
    let exact = rect_dominance::path_tree::build_path_tree_partition_with_orientation_policy(
        &geometry.prepared,
        &geometry.boundary,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        certificate.clone(),
        false,
        rect_dominance::RegionDualBackend::BoundaryLaminar,
        rect_dominance::PathTreeOrientationPolicy::BuildBothExact,
    );
    let build_both_construction_microseconds = build_both_started.elapsed().as_micros();
    let bound_started = Instant::now();
    let estimated = rect_dominance::path_tree::build_path_tree_partition_with_orientation_policy(
        &geometry.prepared,
        &geometry.boundary,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        certificate,
        false,
        rect_dominance::RegionDualBackend::BoundaryLaminar,
        rect_dominance::PathTreeOrientationPolicy::BoundEstimate,
    );
    let bound_construction_microseconds = bound_started.elapsed().as_micros();
    let (Ok(exact), Ok(estimated)) = (exact, estimated) else {
        rows.push(OrientationAuditRow {
            population: population.to_owned(),
            instance_name: instance_name.to_owned(),
            q,
            horizontal_chords: geometry.horizontal_chords.len(),
            vertical_chords: geometry.vertical_chords.len(),
            chosen_orientation: String::new(),
            best_orientation: String::new(),
            selected_sigma: 0,
            best_sigma: 0,
            absolute_regret: 0,
            regret_ratio: None,
            bound_construction_microseconds,
            build_both_construction_microseconds,
            status: "solver-error".to_owned(),
        });
        return;
    };
    let best_sigma = exact.biclique_partition.total_vertex_occurrences();
    let selected_sigma = estimated.biclique_partition.total_vertex_occurrences();
    let absolute_regret = selected_sigma.saturating_sub(best_sigma);
    let regret_ratio = ExactRatio::new(absolute_regret as u128, best_sigma as u128);
    let status = if absolute_regret != 0 {
        "mismatch"
    } else if estimated.orientation == exact.orientation {
        "verified"
    } else {
        "tie-different-orientation"
    };
    rows.push(OrientationAuditRow {
        population: population.to_owned(),
        instance_name: instance_name.to_owned(),
        q,
        horizontal_chords: geometry.horizontal_chords.len(),
        vertical_chords: geometry.vertical_chords.len(),
        chosen_orientation: estimated.orientation.name().to_owned(),
        best_orientation: exact.orientation.name().to_owned(),
        selected_sigma,
        best_sigma,
        absolute_regret,
        regret_ratio,
        bound_construction_microseconds,
        build_both_construction_microseconds,
        status: status.to_owned(),
    });
}

fn append_orientation_audit_instance(
    rows: &mut Vec<OrientationAuditRow>,
    population: &str,
    instance: &AdversarialInstance,
) {
    if let Ok(components) = instance.foreground_components() {
        for component in components {
            append_orientation_audit_component(rows, population, &instance.name, &component);
        }
    }
}

/// Compares the paper-shaped orientation bound with the exact two-orientation
/// selector on reproducible clean finite-grid populations.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn benchmark_path_tree_orientation_audit(
    context: BenchmarkContext,
    sizes: &[usize],
) -> OrientationAuditReport {
    let mut rows = Vec::new();
    for side in [3usize, 4] {
        let bit_count = side * side;
        for mask in 1_u32..(1_u32 << bit_count) {
            let cells = (0..bit_count)
                .map(|index| mask & (1_u32 << index) != 0)
                .collect::<Vec<_>>();
            if let Ok(grid) = ColorGrid::new(side, side, cells) {
                for component in grid
                    .four_connected_components()
                    .into_iter()
                    .filter(|component| component.color)
                {
                    append_orientation_audit_component(
                        &mut rows,
                        &format!("clean-{side}x{side}"),
                        &format!("binary-{side}x{side}-{mask:x}"),
                        &component,
                    );
                }
            }
        }
    }
    let scale = sizes.iter().copied().max().unwrap_or(7).max(3);
    for instance in crate::adversarial::path_tree_geometry_families(scale) {
        append_orientation_audit_instance(&mut rows, "structural-family", &instance);
    }
    for instance in crate::witness::stored_mixed_branching_witnesses() {
        append_orientation_audit_instance(&mut rows, "stored-mixed-witness", &instance);
    }
    for &size in sizes {
        if let Ok(instance) = clean_complete_bipartite_grid(size) {
            append_orientation_audit_instance(&mut rows, "complete-bipartite", &instance);
        }
    }
    for level in enumerate_free_polyominoes(10) {
        for polyomino in level {
            let instance = polyomino.to_instance(
                format!("free-polyomino-{}", polyomino.canonical_key()),
                "free-polyomino",
            );
            append_orientation_audit_instance(&mut rows, "free-polyomino", &instance);
        }
    }
    let mut state = 0x5eed_u64;
    for case in 0..256usize {
        let mut cells = Vec::with_capacity(64);
        for _ in 0..64 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            cells.push((state >> 63) != 0);
        }
        if let Ok(grid) = ColorGrid::new(8, 8, cells) {
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                append_orientation_audit_component(
                    &mut rows,
                    "random-clean-candidate",
                    &format!("random-{case}"),
                    &component,
                );
            }
        }
    }
    let exact_matches = rows.iter().filter(|row| row.absolute_regret == 0).count();
    let mismatches = rows.iter().filter(|row| row.status == "mismatch").count();
    let tie_orientation_differences = rows
        .iter()
        .filter(|row| row.status == "tie-different-orientation")
        .count();
    let maximum_absolute_regret = rows
        .iter()
        .map(|row| row.absolute_regret)
        .max()
        .unwrap_or(0);
    let maximum_regret_ratio =
        rows.iter()
            .filter_map(|row| row.regret_ratio)
            .max_by(|left, right| {
                (left.numerator.saturating_mul(right.denominator))
                    .cmp(&right.numerator.saturating_mul(left.denominator))
            });
    OrientationAuditReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: rows.len(),
            component_count: rows.len(),
            input_model: "finite-grid-path-tree-orientation-audit".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        rows,
        exact_matches,
        mismatches,
        tie_orientation_differences,
        maximum_absolute_regret,
        maximum_regret_ratio,
    }
}

fn collect_dual_differential_instances(sizes: &[usize]) -> Vec<(String, AdversarialInstance)> {
    let mut instances = Vec::new();
    for side in [3usize, 4] {
        let bit_count = side * side;
        for mask in 1_u32..(1_u32 << bit_count) {
            let cells = (0..bit_count)
                .map(|index| mask & (1_u32 << index) != 0)
                .collect::<Vec<_>>();
            instances.push((
                format!("clean-{side}x{side}"),
                AdversarialInstance {
                    name: format!("binary-{side}x{side}-{mask:x}"),
                    family: "binary-clean".to_owned(),
                    width: side,
                    height: side,
                    cells,
                    parameters: [("side".to_owned(), side)].into_iter().collect(),
                },
            ));
        }
    }
    let scale = sizes.iter().copied().max().unwrap_or(5).max(3);
    instances.extend(
        crate::adversarial::path_tree_geometry_families(scale)
            .into_iter()
            .map(|instance| ("structural-family".to_owned(), instance)),
    );
    for &size in sizes.iter().filter(|&&size| size <= 4) {
        if let Ok(instance) = clean_complete_bipartite_grid(size) {
            instances.push(("complete-bipartite".to_owned(), instance));
        }
    }
    instances
}

/// Compares the compact boundary dual with the independent area-flood-fill
/// dual on a bounded audited population.
#[must_use]
pub fn benchmark_path_tree_dual_differential(
    context: BenchmarkContext,
    sizes: &[usize],
) -> PathTreeDualDifferentialReport {
    let mut rows = Vec::new();
    for (population, instance) in collect_dual_differential_instances(sizes) {
        let Ok(components) = instance.foreground_components() else {
            continue;
        };
        for component in components {
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
            if !certificate.eligible {
                continue;
            }
            let boundary = solve_with_representation_and_region_dual_and_orientation_policy(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::CleanHoleFreePathTree,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
                rect_dominance::PathTreeOrientationPolicy::BuildBothExact,
            );
            let area = solve_with_representation_and_region_dual_and_orientation_policy(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::CleanHoleFreePathTree,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::ReferenceAreaFloodFill,
                rect_dominance::PathTreeOrientationPolicy::BuildBothExact,
            );
            let q = geometry.horizontal_chords.len() + geometry.vertical_chords.len();
            match (boundary, area) {
                (Ok(boundary), Ok(area)) => {
                    let rectangles_equal = boundary.rectangles == area.rectangles
                        && boundary.optimum_rectangle_count == area.optimum_rectangle_count;
                    rows.push(PathTreeDualDifferentialRow {
                        population: population.clone(),
                        instance_name: instance.name.clone(),
                        q,
                        boundary_sigma: boundary.diagnostics.path_tree_sigma,
                        area_sigma: area.diagnostics.path_tree_sigma,
                        rectangles_equal,
                        status: if rectangles_equal {
                            "verified".to_owned()
                        } else {
                            "counterexample".to_owned()
                        },
                    });
                }
                (Err(error), _) | (_, Err(error)) => rows.push(PathTreeDualDifferentialRow {
                    population: population.clone(),
                    instance_name: instance.name.clone(),
                    q,
                    boundary_sigma: None,
                    area_sigma: None,
                    rectangles_equal: false,
                    status: format!("solver-error: {error}"),
                }),
            }
        }
    }
    PathTreeDualDifferentialReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: rows.len(),
            component_count: rows.len(),
            input_model: "finite-grid-boundary-laminar-vs-area-dual".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        verified: rows.iter().filter(|row| row.status == "verified").count(),
        counterexamples: rows
            .iter()
            .filter(|row| row.status == "counterexample")
            .count(),
        solver_errors: rows
            .iter()
            .filter(|row| row.status.starts_with("solver-error"))
            .count(),
        rows,
    }
}

/// Exercises the Auto representation on clean and non-clean finite-grid
/// fixtures, recording the path-tree selection or compact 4D fallback.
#[must_use]
pub fn benchmark_auto_fallback(context: BenchmarkContext) -> BenchmarkReport {
    let instances = endpoint_contact_instances()
        .into_iter()
        .chain([crate::adversarial::one_hole_ring(5, 5)])
        .chain(crate::adversarial::path_tree_geometry_families(7))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for instance in &instances {
        let Ok(components) = instance.foreground_components() else {
            continue;
        };
        for component in components {
            let result = solve_with_representation_and_region_dual_and_orientation_policy(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::Auto,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
                rect_dominance::PathTreeOrientationPolicy::BuildBothExact,
            );
            rows.push(match result {
                Ok(result) => BenchmarkRow {
                    instance_name: instance.name.clone(),
                    family: "auto-fallback".to_owned(),
                    parameters: instance.parameters.clone(),
                    component_id: component.id.0,
                    status: "verified".to_owned(),
                    message: result.diagnostics.conflict_representation.clone(),
                    exact_cover_compared: false,
                    diagnostics: result.diagnostics,
                    c0_phase_microseconds: BTreeMap::new(),
                    compressed_phase_microseconds: BTreeMap::new(),
                    compact_only_phase_microseconds: BTreeMap::new(),
                },
                Err(error) => BenchmarkRow {
                    instance_name: instance.name.clone(),
                    family: "auto-fallback".to_owned(),
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
    BenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: instances.len(),
            component_count: rows.len(),
            input_model: "finite-grid-auto-representation-fallback".to_owned(),
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliqueConstructionBenchmarkRow {
    pub instance_name: String,
    pub family: String,
    pub parameters: BTreeMap<String, usize>,
    pub component_id: usize,
    pub horizontal_chords: usize,
    pub vertical_chords: usize,
    pub block_count: Option<usize>,
    pub total_vertex_occurrences: Option<usize>,
    pub reference_microseconds: Option<u128>,
    pub presorted_microseconds: Option<u128>,
    pub reference_metrics: Option<BicliqueConstructionMetrics>,
    pub presorted_metrics: Option<BicliqueConstructionMetrics>,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliqueConstructionBenchmarkReport {
    pub metadata: BenchmarkMetadata,
    pub verified_count: usize,
    pub solver_error_count: usize,
    pub counterexample_count: usize,
    pub rows: Vec<BicliqueConstructionBenchmarkRow>,
}

impl BicliqueConstructionBenchmarkReport {
    #[must_use]
    pub fn verified(&self) -> bool {
        self.solver_error_count == 0 && self.counterexample_count == 0
    }

    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "instance_name,family,parameters,component_id,horizontal_chords,vertical_chords,block_count,total_vertex_occurrences,reference_microseconds,presorted_microseconds,reference_initial_sorts,reference_recursive_sorts,presorted_initial_sorts,presorted_recursive_sorts,presorted_stable_partition_visits,presorted_scratch_buffer_acquisitions,presorted_scratch_growth_count,presorted_scratch_point_capacity,presorted_recursive_nodes,presorted_emitted_vertex_occurrences,status,message\n",
        );
        for row in &self.rows {
            let reference = row.reference_metrics.as_ref();
            let presorted = row.presorted_metrics.as_ref();
            let fields = [
                row.instance_name.clone(),
                row.family.clone(),
                serde_json::to_string(&row.parameters).unwrap_or_default(),
                row.component_id.to_string(),
                row.horizontal_chords.to_string(),
                row.vertical_chords.to_string(),
                optional_number(row.block_count),
                optional_number(row.total_vertex_occurrences),
                optional_number(row.reference_microseconds),
                optional_number(row.presorted_microseconds),
                reference.map_or_else(String::new, |value| value.initial_sort_count.to_string()),
                reference.map_or_else(String::new, |value| value.recursive_sort_count.to_string()),
                presorted.map_or_else(String::new, |value| value.initial_sort_count.to_string()),
                presorted.map_or_else(String::new, |value| value.recursive_sort_count.to_string()),
                presorted.map_or_else(String::new, |value| {
                    value.stable_partition_visits.to_string()
                }),
                presorted.map_or_else(String::new, |value| {
                    value.scratch_buffer_acquisitions.to_string()
                }),
                presorted.map_or_else(String::new, |value| value.scratch_growth_count.to_string()),
                presorted.map_or_else(String::new, |value| {
                    value.scratch_point_capacity.to_string()
                }),
                presorted.map_or_else(String::new, |value| value.recursive_node_count.to_string()),
                presorted.map_or_else(String::new, |value| {
                    value.emitted_vertex_occurrences.to_string()
                }),
                row.status.clone(),
                row.message.clone().unwrap_or_default(),
            ];
            let _ = writeln!(
                csv,
                "{}",
                fields
                    .iter()
                    .map(|field| escape_csv(field))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        csv
    }
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
            "git_commit,rustc_version,command,seed,timestamp,input_count,component_count,input_model,unsupported_input_features,instance_name,family,parameters,component_id,status,message,exact_cover_compared,cell_count,boundary_complexity,hole_count,reflex_vertex_count,horizontal_chord_count,vertical_chord_count,total_chord_count,effective_chord_enumerator,effective_chord_enumeration_microseconds,prepared_component_build_count,prepared_component_build_microseconds,boundary_index_build_count,boundary_index_build_microseconds,boundary_index_entries,boundary_index_owned_bytes,linear_boundary_vertex_lookup_count,gap_interval_membership_tests,gap_event_push_count,gap_event_pop_count,boundary_gap_label_backend,clean_endpoint_pair_comparisons,boundary_extraction_microseconds,reflex_grouping_microseconds,occupancy_bytes,horizontal_interior_run_count,vertical_interior_run_count,candidate_reflex_pair_count,emitted_chord_count,explicit_conflict_edge_count,edge_density_numerator,edge_density_denominator,biclique_count,biclique_total_vertex_occurrences,biclique_size_per_chord_numerator,biclique_size_per_chord_denominator,biclique_size_per_edge_numerator,biclique_size_per_edge_denominator,c0_network_vertex_count,c0_network_arc_count,compressed_network_vertex_count,compressed_network_arc_count,maximum_matching_size,minimum_vertex_cover_size,output_rectangle_count,completion_backend,conflict_representation,path_tree_orientation,path_tree_orientation_policy,dual_region_count,dual_tree_vertex_count,path_count,path_edge_incidence_count,total_path_length_metric,dual_tree_max_depth,dual_tree_max_branching_degree,heavy_chain_count,heavy_chain_interval_count,tree_edge_occurrences,theoretical_path_occurrence_bound,theoretical_tree_edge_occurrence_bound,canonical_segment_node_count,path_tree_sigma,four_d_sigma,selected_chord_cut_materialization_microseconds,horizontal_simple_chord_completion_microseconds,vertical_simple_chord_completion_microseconds,rectangle_recovery_microseconds,final_output_validation_microseconds,initial_horizontal_unit_cut_count,initial_vertical_unit_cut_count,completion_added_horizontal_unit_cuts,completion_added_vertical_unit_cuts,horizontal_simple_chord_count,vertical_simple_chord_count,completion_candidate_queries,completion_full_grid_scans,completion_candidate_revalidations,completion_stale_candidates,completion_ray_extension_unit_steps,rectangle_recovery_component_visits,rectangle_recovery_queue_pushes,rectangle_recovery_region_count,rectangle_recovery_allocations,c0_phase_microseconds,compressed_phase_microseconds,compact_only_phase_microseconds,peak_memory_bytes,owned_allocation_estimates,region_dual_backend,region_dual_construction_microseconds,dual_tree_edge_count,dual_allocated_bytes,dual_unit_cut_count,dual_area_cell_visits,dual_interval_count,dual_maximum_nesting_depth,hld_interval_count,explicit_path_records_materialized,subdivision_builder_backend,subdivision_input_segment_count,subdivision_sweep_event_count,subdivision_candidate_pair_tests,subdivision_reported_intersections,subdivision_atomic_segment_count,validator_backend,validator_x_event_count,validator_y_coordinate_count,validator_range_add_count,validator_parity_toggle_count,validator_segment_tree_node_visits,validator_root_checks,validator_boundary_edge_scans,validator_active_rectangle_resorts,cut_index_logical_tree_node_count,cut_index_materialized_tree_node_count,cut_index_ordered_set_entry_count,polygon_selected_recovery_backend,dense_recovery_retained_byte_estimate,sparse_recovery_retained_upper_estimate"
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
                optional_number(row.diagnostics.boundary_index_build_count),
                optional_number(row.diagnostics.boundary_index_build_microseconds),
                optional_number(row.diagnostics.boundary_index_entries),
                optional_number(row.diagnostics.boundary_index_owned_bytes),
                optional_number(row.diagnostics.linear_boundary_vertex_lookup_count),
                optional_number(row.diagnostics.gap_interval_membership_tests),
                optional_number(row.diagnostics.gap_event_push_count),
                optional_number(row.diagnostics.gap_event_pop_count),
                row.diagnostics
                    .boundary_gap_label_backend
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.clean_endpoint_pair_comparisons),
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
                row.diagnostics
                    .path_tree_orientation_policy
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.dual_region_count),
                optional_number(row.diagnostics.dual_tree_vertex_count),
                optional_number(row.diagnostics.path_count),
                optional_number(row.diagnostics.path_edge_incidence_count),
                optional_number(row.diagnostics.total_path_length_metric),
                optional_number(row.diagnostics.dual_tree_max_depth),
                optional_number(row.diagnostics.dual_tree_max_branching_degree),
                optional_number(row.diagnostics.heavy_chain_count),
                optional_number(row.diagnostics.heavy_chain_interval_count),
                optional_number(row.diagnostics.tree_edge_occurrences),
                optional_number(row.diagnostics.theoretical_path_occurrence_bound),
                optional_number(row.diagnostics.theoretical_tree_edge_occurrence_bound),
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
                optional_number(row.diagnostics.initial_horizontal_unit_cut_count),
                optional_number(row.diagnostics.initial_vertical_unit_cut_count),
                optional_number(row.diagnostics.added_horizontal_unit_cut_count),
                optional_number(row.diagnostics.added_vertical_unit_cut_count),
                optional_number(row.diagnostics.horizontal_simple_chord_count),
                optional_number(row.diagnostics.vertical_simple_chord_count),
                optional_number(row.diagnostics.completion_candidate_queries),
                optional_number(row.diagnostics.completion_full_grid_scans),
                optional_number(row.diagnostics.completion_candidate_revalidations),
                optional_number(row.diagnostics.completion_stale_candidates),
                optional_number(row.diagnostics.completion_ray_extension_unit_steps),
                optional_number(row.diagnostics.rectangle_recovery_component_visits),
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
                row.diagnostics
                    .subdivision_builder_backend
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.subdivision_input_segment_count),
                optional_number(row.diagnostics.subdivision_sweep_event_count),
                optional_number(row.diagnostics.subdivision_candidate_pair_tests),
                optional_number(row.diagnostics.subdivision_reported_intersections),
                optional_number(row.diagnostics.subdivision_atomic_segment_count),
                row.diagnostics
                    .sparse_validator_backend
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.validator_x_event_count),
                optional_number(row.diagnostics.validator_y_coordinate_count),
                optional_number(row.diagnostics.validator_range_add_count),
                optional_number(row.diagnostics.validator_parity_toggle_count),
                optional_number(row.diagnostics.validator_segment_tree_node_visits),
                optional_number(row.diagnostics.validator_root_checks),
                optional_number(row.diagnostics.validator_boundary_edge_scans),
                optional_number(row.diagnostics.validator_active_rectangle_resorts),
                optional_number(row.diagnostics.cut_index_logical_tree_node_count),
                optional_number(row.diagnostics.cut_index_materialized_tree_node_count),
                optional_number(row.diagnostics.cut_index_ordered_set_entry_count),
                row.diagnostics
                    .polygon_selected_recovery_backend
                    .clone()
                    .unwrap_or_default(),
                optional_number(row.diagnostics.dense_recovery_retained_byte_estimate),
                optional_number(row.diagnostics.sparse_recovery_retained_upper_estimate),
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
pub fn benchmark_biclique_construction(
    context: BenchmarkContext,
    sizes: &[usize],
) -> BicliqueConstructionBenchmarkReport {
    let command = context.command.split_once(" benchmark").map_or_else(
        || context.command.clone(),
        |(_, suffix)| format!("rect-cli benchmark{suffix}"),
    );
    let instances = sizes
        .iter()
        .map(|&size| dense_conflict_grid(size, size))
        .collect::<Vec<_>>();
    let input_count = instances.len();
    let mut rows = Vec::new();
    for instance in instances {
        let components = match instance.foreground_components() {
            Ok(components) => components,
            Err(error) => {
                rows.push(BicliqueConstructionBenchmarkRow {
                    instance_name: instance.name,
                    family: instance.family,
                    parameters: instance.parameters,
                    component_id: 0,
                    horizontal_chords: 0,
                    vertical_chords: 0,
                    block_count: None,
                    total_vertex_occurrences: None,
                    reference_microseconds: None,
                    presorted_microseconds: None,
                    reference_metrics: None,
                    presorted_metrics: None,
                    status: "solver-error".to_owned(),
                    message: Some(error.to_string()),
                });
                continue;
            }
        };
        for component in components {
            let row = benchmark_biclique_component(
                &instance.name,
                &instance.family,
                &instance.parameters,
                &component,
            );
            rows.push(row);
        }
    }
    BicliqueConstructionBenchmarkReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count,
            component_count: rows.len(),
            input_model: "finite-colored-unit-grid-biclique-construction".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        verified_count: rows.iter().filter(|row| row.status == "verified").count(),
        solver_error_count: rows
            .iter()
            .filter(|row| row.status == "solver-error")
            .count(),
        counterexample_count: rows
            .iter()
            .filter(|row| row.status == "counterexample")
            .count(),
        rows,
    }
}

fn benchmark_biclique_component<C>(
    instance_name: &str,
    family: &str,
    parameters: &BTreeMap<String, usize>,
    component: &GridComponent<C>,
) -> BicliqueConstructionBenchmarkRow {
    let mut row = BicliqueConstructionBenchmarkRow {
        instance_name: instance_name.to_owned(),
        family: family.to_owned(),
        parameters: parameters.clone(),
        component_id: component.id.0,
        horizontal_chords: 0,
        vertical_chords: 0,
        block_count: None,
        total_vertex_occurrences: None,
        reference_microseconds: None,
        presorted_microseconds: None,
        reference_metrics: None,
        presorted_metrics: None,
        status: "solver-error".to_owned(),
        message: None,
    };
    let geometry = match rect_oracle_sg::analyze_geometry(component) {
        Ok(geometry) => geometry,
        Err(error) => {
            row.message = Some(error.to_string());
            return row;
        }
    };
    row.horizontal_chords = geometry.horizontal_chords.len();
    row.vertical_chords = geometry.vertical_chords.len();
    let embedding =
        match DominanceEmbedding::new(&geometry.horizontal_chords, &geometry.vertical_chords) {
            Ok(embedding) => embedding,
            Err(error) => {
                row.message = Some(error.to_string());
                return row;
            }
        };
    let reference_started = Instant::now();
    let reference = match BicliquePartition::comparability_theorem_8_reference(&embedding) {
        Ok(construction) => construction,
        Err(error) => {
            row.message = Some(error.to_string());
            return row;
        }
    };
    row.reference_microseconds = Some(reference_started.elapsed().as_micros());
    let presorted_started = Instant::now();
    let presorted = match BicliquePartition::comparability_theorem_8_presorted(&embedding) {
        Ok(construction) => construction,
        Err(error) => {
            row.message = Some(error.to_string());
            return row;
        }
    };
    row.presorted_microseconds = Some(presorted_started.elapsed().as_micros());
    row.block_count = Some(presorted.partition.bicliques.len());
    row.total_vertex_occurrences = Some(presorted.partition.total_vertex_occurrences());
    row.reference_metrics = Some(reference.metrics.clone());
    row.presorted_metrics = Some(presorted.metrics.clone());
    let counters_valid = presorted.metrics.initial_sort_count == 4
        && presorted.metrics.recursive_sort_count == 0
        && presorted.metrics.emitted_vertex_occurrences
            == presorted.partition.total_vertex_occurrences();
    if reference.partition == presorted.partition && counters_valid {
        "verified".clone_into(&mut row.status);
    } else {
        "counterexample".clone_into(&mut row.status);
        row.message = Some(if reference.partition == presorted.partition {
            "presorted structural counters violate the production contract".to_owned()
        } else {
            "reference and presorted partitions differ".to_owned()
        });
    }
    row
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
pub fn benchmark_path_tree_geometry_families(
    context: BenchmarkContext,
    scale: usize,
) -> BenchmarkReport {
    let instances = crate::adversarial::path_tree_geometry_families(scale);
    let input_count = instances.len();
    let mut rows = Vec::new();
    for instance in instances {
        let components = match instance.foreground_components() {
            Ok(components) => components,
            Err(error) => {
                rows.push(BenchmarkRow {
                    instance_name: instance.name,
                    family: instance.family,
                    parameters: instance.parameters,
                    component_id: 0,
                    status: "unsupported".to_owned(),
                    message: Some(error.to_string()),
                    exact_cover_compared: false,
                    diagnostics: Diagnostics::default(),
                    c0_phase_microseconds: BTreeMap::new(),
                    compressed_phase_microseconds: BTreeMap::new(),
                    compact_only_phase_microseconds: BTreeMap::new(),
                });
                continue;
            }
        };
        for component in components {
            let geometry = match rect_oracle_sg::analyze_geometry_with(
                &component,
                &rect_oracle_sg::GridInteriorRunEnumerator,
            ) {
                Ok(geometry) => geometry,
                Err(error) => {
                    rows.push(BenchmarkRow {
                        instance_name: instance.name.clone(),
                        family: instance.family.clone(),
                        parameters: instance.parameters.clone(),
                        component_id: component.id.0,
                        status: "solver-error".to_owned(),
                        message: Some(error.to_string()),
                        exact_cover_compared: false,
                        diagnostics: Diagnostics::default(),
                        c0_phase_microseconds: BTreeMap::new(),
                        compressed_phase_microseconds: BTreeMap::new(),
                        compact_only_phase_microseconds: BTreeMap::new(),
                    });
                    continue;
                }
            };
            let certificate = rect_oracle_sg::classify_clean_hole_free(
                &component,
                &geometry.boundary,
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
            );
            if !certificate.eligible {
                rows.push(BenchmarkRow {
                    instance_name: instance.name.clone(),
                    family: instance.family.clone(),
                    parameters: instance.parameters.clone(),
                    component_id: component.id.0,
                    status: "unsupported".to_owned(),
                    message: Some(format!(
                        "clean certificate rejected: {:?}",
                        certificate.rejection_reasons
                    )),
                    exact_cover_compared: false,
                    diagnostics: Diagnostics {
                        cell_count: component.cell_count(),
                        ..Diagnostics::default()
                    },
                    c0_phase_microseconds: BTreeMap::new(),
                    compressed_phase_microseconds: BTreeMap::new(),
                    compact_only_phase_microseconds: BTreeMap::new(),
                });
                continue;
            }
            let build_both = solve_with_representation_and_region_dual_and_orientation_policy(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::CleanHoleFreePathTree,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
                rect_dominance::PathTreeOrientationPolicy::BuildBothExact,
            );
            let bound = solve_with_representation_and_region_dual_and_orientation_policy(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::CleanHoleFreePathTree,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
                rect_dominance::PathTreeOrientationPolicy::BoundEstimate,
            );
            let (status, message, diagnostics) = match (build_both, bound) {
                (Ok(exact), Ok(estimated))
                    if exact.optimum_rectangle_count == estimated.optimum_rectangle_count
                        && exact.rectangles == estimated.rectangles
                        && exact.diagnostics.path_tree_sigma
                            == estimated.diagnostics.path_tree_sigma
                        && path_tree_growth_guards(&estimated.diagnostics)
                        && estimated.diagnostics.execution_trace
                            == rect_core::ExecutionTrace {
                                compact_structure_check_called: true,
                                ..rect_core::ExecutionTrace::default()
                            } =>
                {
                    ("verified".to_owned(), None, estimated.diagnostics)
                }
                (Ok(exact), Ok(estimated)) => (
                    "counterexample".to_owned(),
                    Some(format!(
                        "bound estimate differs: exact sigma {:?}, selected sigma {:?}",
                        exact.diagnostics.path_tree_sigma, estimated.diagnostics.path_tree_sigma
                    )),
                    estimated.diagnostics,
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
                family: instance.family.clone(),
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
            input_count,
            component_count: rows.len(),
            input_model: "finite-colored-unit-grid-path-tree-families".to_owned(),
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

/// Runs the geometry-backed family suite at every requested scale. Keeping
/// each scale as a separate row prevents a large instance from hiding whether
/// dual and HLD structural quantities actually grow with the parameter.
#[must_use]
pub fn benchmark_path_tree_geometry_scaling(
    context: BenchmarkContext,
    sizes: &[usize],
) -> BenchmarkReport {
    let mut rows = Vec::new();
    for &scale in sizes {
        rows.extend(benchmark_path_tree_geometry_families(context.clone(), scale).rows);
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
            input_model: "finite-grid-scaled-path-tree-geometry-families".to_owned(),
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreeVs4dRow {
    pub family: String,
    pub instance_name: String,
    /// Dihedral-canonical foreground-cell key for replayable fixture identity.
    pub canonical_key: String,
    pub q: usize,
    pub q_bucket: String,
    pub horizontal_chords: usize,
    pub vertical_chords: usize,
    pub boundary_complexity: usize,
    pub clean_eligible: bool,
    pub orientation_policy: String,
    pub path_tree_orientation: Option<String>,
    pub sigma_path_tree: Option<usize>,
    pub sigma_4d: Option<usize>,
    pub biclique_count_path_tree: Option<usize>,
    pub biclique_count_4d: Option<usize>,
    pub network_vertices_path_tree: Option<usize>,
    pub network_arcs_path_tree: Option<usize>,
    pub network_vertices_4d: Option<usize>,
    pub network_arcs_4d: Option<usize>,
    pub path_tree_construction_microseconds: Option<u128>,
    pub four_d_representation_microseconds: Option<u128>,
    pub path_tree_flow_microseconds: Option<u128>,
    pub four_d_flow_microseconds: Option<u128>,
    pub path_tree_completion_microseconds: Option<u128>,
    pub four_d_completion_microseconds: Option<u128>,
    pub path_tree_total_microseconds: Option<u128>,
    pub four_d_total_microseconds: Option<u128>,
    pub owned_path_tree: String,
    pub owned_4d: String,
    pub optimum_equal: bool,
    pub rectangles_equal: bool,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PathTreeVs4dReport {
    pub metadata: BenchmarkMetadata,
    pub rows: Vec<PathTreeVs4dRow>,
    pub counterexamples: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepresentationAdvantageRow {
    pub family: String,
    pub instance_name: String,
    pub canonical_key: String,
    pub q: usize,
    pub horizontal_chords: usize,
    pub vertical_chords: usize,
    pub sigma_path_tree: usize,
    pub sigma_4d: usize,
    pub sigma_4d_over_path_tree: ExactRatio,
    pub sigma_path_tree_over_4d: ExactRatio,
    pub network_arcs_path_tree: usize,
    pub network_arcs_4d: usize,
    pub owned_path_tree_bytes: usize,
    pub owned_4d_bytes: usize,
    pub optimum_equal: bool,
    pub rectangles_equal: bool,
    pub status: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepresentationAdvantageReport {
    pub metadata: BenchmarkMetadata,
    pub eligible_rows: usize,
    pub strict_path_tree_advantages: usize,
    pub strict_four_d_advantages: usize,
    pub top_path_tree_advantages: Vec<RepresentationAdvantageRow>,
    pub top_four_d_advantages: Vec<RepresentationAdvantageRow>,
}

impl RepresentationAdvantageReport {
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "rank,direction,family,instance_name,canonical_key,q,horizontal_chords,vertical_chords,sigma_path_tree,sigma_4d,sigma_4d_over_path_tree,sigma_path_tree_over_4d,network_arcs_path_tree,network_arcs_4d,owned_path_tree_bytes,owned_4d_bytes,optimum_equal,rectangles_equal,status\n",
        );
        for (direction, rows) in [
            ("path-tree-advantage", &self.top_path_tree_advantages),
            ("four-d-advantage", &self.top_four_d_advantages),
        ] {
            for (rank, row) in rows.iter().enumerate() {
                let fields = [
                    (rank + 1).to_string(),
                    direction.to_owned(),
                    row.family.clone(),
                    row.instance_name.clone(),
                    row.canonical_key.clone(),
                    row.q.to_string(),
                    row.horizontal_chords.to_string(),
                    row.vertical_chords.to_string(),
                    row.sigma_path_tree.to_string(),
                    row.sigma_4d.to_string(),
                    format!(
                        "{}/{}",
                        row.sigma_4d_over_path_tree.numerator,
                        row.sigma_4d_over_path_tree.denominator
                    ),
                    format!(
                        "{}/{}",
                        row.sigma_path_tree_over_4d.numerator,
                        row.sigma_path_tree_over_4d.denominator
                    ),
                    row.network_arcs_path_tree.to_string(),
                    row.network_arcs_4d.to_string(),
                    row.owned_path_tree_bytes.to_string(),
                    row.owned_4d_bytes.to_string(),
                    row.optimum_equal.to_string(),
                    row.rectangles_equal.to_string(),
                    row.status.clone(),
                ];
                let _ = writeln!(
                    csv,
                    "{}",
                    fields
                        .iter()
                        .map(|field| escape_csv(field))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            }
        }
        csv
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        format!(
            "# v0.8 path-tree versus 4D advantage search\n\n- Eligible mixed-orientation rows: {}\n- Strict path-tree advantages: {}\n- Strict 4D advantages: {}\n- Top path-tree advantage rows: {}\n- Top 4D advantage rows: {}\n",
            self.eligible_rows,
            self.strict_path_tree_advantages,
            self.strict_four_d_advantages,
            self.top_path_tree_advantages.len(),
            self.top_four_d_advantages.len(),
        )
    }
}

impl PathTreeVs4dReport {
    #[must_use]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "family,instance_name,canonical_key,q,q_bucket,horizontal_chords,vertical_chords,boundary_complexity,clean_eligible,orientation_policy,path_tree_orientation,sigma_path_tree,sigma_4d,biclique_count_path_tree,biclique_count_4d,network_vertices_path_tree,network_arcs_path_tree,network_vertices_4d,network_arcs_4d,path_tree_construction_microseconds,four_d_representation_microseconds,path_tree_flow_microseconds,four_d_flow_microseconds,path_tree_completion_microseconds,four_d_completion_microseconds,path_tree_total_microseconds,four_d_total_microseconds,owned_path_tree,owned_4d,optimum_equal,rectangles_equal,status\n",
        );
        for row in &self.rows {
            let fields = [
                row.family.clone(),
                row.instance_name.clone(),
                row.canonical_key.clone(),
                row.q.to_string(),
                row.q_bucket.clone(),
                row.horizontal_chords.to_string(),
                row.vertical_chords.to_string(),
                row.boundary_complexity.to_string(),
                row.clean_eligible.to_string(),
                row.orientation_policy.clone(),
                row.path_tree_orientation.clone().unwrap_or_default(),
                optional_number(row.sigma_path_tree),
                optional_number(row.sigma_4d),
                optional_number(row.biclique_count_path_tree),
                optional_number(row.biclique_count_4d),
                optional_number(row.network_vertices_path_tree),
                optional_number(row.network_arcs_path_tree),
                optional_number(row.network_vertices_4d),
                optional_number(row.network_arcs_4d),
                optional_number(row.path_tree_construction_microseconds),
                optional_number(row.four_d_representation_microseconds),
                optional_number(row.path_tree_flow_microseconds),
                optional_number(row.four_d_flow_microseconds),
                optional_number(row.path_tree_completion_microseconds),
                optional_number(row.four_d_completion_microseconds),
                optional_number(row.path_tree_total_microseconds),
                optional_number(row.four_d_total_microseconds),
                row.owned_path_tree.clone(),
                row.owned_4d.clone(),
                row.optimum_equal.to_string(),
                row.rectangles_equal.to_string(),
                row.status.clone(),
            ];
            let _ = writeln!(
                csv,
                "{}",
                fields
                    .iter()
                    .map(|field| escape_csv(field))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        csv
    }
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn benchmark_path_tree_vs_4d(context: BenchmarkContext, sizes: &[usize]) -> PathTreeVs4dReport {
    let scale = sizes.iter().copied().max().unwrap_or(5).max(3);
    let mut instances = crate::adversarial::path_tree_geometry_families(scale);
    instances.extend(crate::witness::stored_mixed_branching_witnesses());
    for &size in sizes {
        if let Ok(instance) = clean_complete_bipartite_grid(size) {
            instances.push(instance);
        }
    }
    let mut rows = Vec::new();
    for instance in instances {
        let components = match instance.foreground_components() {
            Ok(components) => components,
            Err(error) => {
                rows.push(PathTreeVs4dRow {
                    family: instance.family,
                    instance_name: instance.name,
                    canonical_key: String::new(),
                    q: 0,
                    q_bucket: q_bucket(0).to_owned(),
                    horizontal_chords: 0,
                    vertical_chords: 0,
                    boundary_complexity: 0,
                    clean_eligible: false,
                    orientation_policy: "bound-estimate".to_owned(),
                    path_tree_orientation: None,
                    sigma_path_tree: None,
                    sigma_4d: None,
                    biclique_count_path_tree: None,
                    biclique_count_4d: None,
                    network_vertices_path_tree: None,
                    network_arcs_path_tree: None,
                    network_vertices_4d: None,
                    network_arcs_4d: None,
                    path_tree_construction_microseconds: None,
                    four_d_representation_microseconds: None,
                    path_tree_flow_microseconds: None,
                    four_d_flow_microseconds: None,
                    path_tree_completion_microseconds: None,
                    four_d_completion_microseconds: None,
                    path_tree_total_microseconds: None,
                    four_d_total_microseconds: None,
                    owned_path_tree: String::new(),
                    owned_4d: String::new(),
                    optimum_equal: false,
                    rectangles_equal: false,
                    status: format!("unsupported: {error}"),
                });
                continue;
            }
        };
        for component in components {
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
            let q = geometry
                .horizontal_chords
                .len()
                .saturating_add(geometry.vertical_chords.len());
            let canonical_key = canonical_component_key(&component);
            if !certificate.eligible {
                rows.push(PathTreeVs4dRow {
                    family: instance.family.clone(),
                    instance_name: instance.name.clone(),
                    canonical_key: canonical_key.clone(),
                    q,
                    q_bucket: q_bucket(q).to_owned(),
                    horizontal_chords: geometry.horizontal_chords.len(),
                    vertical_chords: geometry.vertical_chords.len(),
                    boundary_complexity: geometry.boundary.boundary_complexity(),
                    clean_eligible: false,
                    orientation_policy: "bound-estimate".to_owned(),
                    path_tree_orientation: None,
                    sigma_path_tree: None,
                    sigma_4d: None,
                    biclique_count_path_tree: None,
                    biclique_count_4d: None,
                    network_vertices_path_tree: None,
                    network_arcs_path_tree: None,
                    network_vertices_4d: None,
                    network_arcs_4d: None,
                    path_tree_construction_microseconds: None,
                    four_d_representation_microseconds: None,
                    path_tree_flow_microseconds: None,
                    four_d_flow_microseconds: None,
                    path_tree_completion_microseconds: None,
                    four_d_completion_microseconds: None,
                    path_tree_total_microseconds: None,
                    four_d_total_microseconds: None,
                    owned_path_tree: String::new(),
                    owned_4d: String::new(),
                    optimum_equal: false,
                    rectangles_equal: false,
                    status: "clean-ineligible".to_owned(),
                });
                continue;
            }
            let path = solve_with_representation_and_region_dual_and_orientation_policy(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::CleanHoleFreePathTree,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
                rect_dominance::PathTreeOrientationPolicy::BoundEstimate,
            );
            let general = solve_with_representation_and_region_dual(
                &component,
                VerificationMode::CompactOnly,
                ConflictRepresentationBackend::GeneralDominance4D,
                ChordEnumerator::GridInteriorRuns,
                CompletionBackendKind::IndexedFrontier,
                rect_dominance::RegionDualBackend::BoundaryLaminar,
            );
            let (path, general) = match (path, general) {
                (Ok(path), Ok(general)) => (path, general),
                (Err(error), _) | (_, Err(error)) => {
                    rows.push(PathTreeVs4dRow {
                        family: instance.family.clone(),
                        instance_name: instance.name.clone(),
                        canonical_key: canonical_key.clone(),
                        q,
                        q_bucket: q_bucket(q).to_owned(),
                        horizontal_chords: geometry.horizontal_chords.len(),
                        vertical_chords: geometry.vertical_chords.len(),
                        boundary_complexity: geometry.boundary.boundary_complexity(),
                        clean_eligible: true,
                        orientation_policy: "bound-estimate".to_owned(),
                        path_tree_orientation: None,
                        sigma_path_tree: None,
                        sigma_4d: None,
                        biclique_count_path_tree: None,
                        biclique_count_4d: None,
                        network_vertices_path_tree: None,
                        network_arcs_path_tree: None,
                        network_vertices_4d: None,
                        network_arcs_4d: None,
                        path_tree_construction_microseconds: None,
                        four_d_representation_microseconds: None,
                        path_tree_flow_microseconds: None,
                        four_d_flow_microseconds: None,
                        path_tree_completion_microseconds: None,
                        four_d_completion_microseconds: None,
                        path_tree_total_microseconds: None,
                        four_d_total_microseconds: None,
                        owned_path_tree: String::new(),
                        owned_4d: String::new(),
                        optimum_equal: false,
                        rectangles_equal: false,
                        status: format!("solver-error: {error}"),
                    });
                    continue;
                }
            };
            let phase = |result: &rect_core::DissectionResult, key: &str| {
                result.diagnostics.phase_microseconds.get(key).copied()
            };
            let total = |result: &rect_core::DissectionResult| {
                Some(result.diagnostics.phase_microseconds.values().sum())
            };
            let optimum_equal = path.optimum_rectangle_count == general.optimum_rectangle_count;
            let rectangles_equal = path.rectangles == general.rectangles;
            rows.push(PathTreeVs4dRow {
                family: instance.family.clone(),
                instance_name: instance.name.clone(),
                canonical_key,
                q,
                q_bucket: q_bucket(q).to_owned(),
                horizontal_chords: geometry.horizontal_chords.len(),
                vertical_chords: geometry.vertical_chords.len(),
                boundary_complexity: geometry.boundary.boundary_complexity(),
                clean_eligible: true,
                orientation_policy: "bound-estimate".to_owned(),
                path_tree_orientation: path.diagnostics.path_tree_orientation.clone(),
                sigma_path_tree: path.diagnostics.path_tree_sigma,
                sigma_4d: general.diagnostics.biclique_total_vertex_occurrences.into(),
                biclique_count_path_tree: Some(path.diagnostics.biclique_count),
                biclique_count_4d: Some(general.diagnostics.biclique_count),
                network_vertices_path_tree: Some(path.diagnostics.compressed_network_vertex_count),
                network_arcs_path_tree: Some(path.diagnostics.compressed_network_arc_count),
                network_vertices_4d: Some(general.diagnostics.compressed_network_vertex_count),
                network_arcs_4d: Some(general.diagnostics.compressed_network_arc_count),
                path_tree_construction_microseconds: phase(&path, "path_tree_construction"),
                four_d_representation_microseconds: phase(&general, "biclique_partition"),
                path_tree_flow_microseconds: phase(&path, "compressed_flow"),
                four_d_flow_microseconds: phase(&general, "compressed_flow"),
                path_tree_completion_microseconds: phase(&path, "geometric_completion"),
                four_d_completion_microseconds: phase(&general, "geometric_completion"),
                path_tree_total_microseconds: total(&path),
                four_d_total_microseconds: total(&general),
                owned_path_tree: serde_json::to_string(
                    &path.diagnostics.owned_allocation_estimates,
                )
                .unwrap_or_default(),
                owned_4d: serde_json::to_string(&general.diagnostics.owned_allocation_estimates)
                    .unwrap_or_default(),
                optimum_equal,
                rectangles_equal,
                status: if optimum_equal && rectangles_equal {
                    "verified".to_owned()
                } else {
                    "counterexample".to_owned()
                },
            });
        }
    }
    let counterexamples = rows
        .iter()
        .filter(|row| row.status == "counterexample")
        .count();
    PathTreeVs4dReport {
        metadata: BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed: context.seed,
            timestamp: context.timestamp,
            input_count: rows.len(),
            component_count: rows.len(),
            input_model: "finite-colored-unit-grid-path-tree-vs-4d".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        rows,
        counterexamples,
    }
}

/// Searches the committed geometry corpus for structural representation
/// advantages. The objective is sigma ratio, never the final optimum count.
#[must_use]
pub fn benchmark_path_tree_advantage(
    context: &BenchmarkContext,
    sizes: &[usize],
    top_k: usize,
) -> RepresentationAdvantageReport {
    let comparison = benchmark_path_tree_vs_4d((*context).clone(), sizes);
    let mut rows = comparison
        .rows
        .iter()
        .filter_map(|row| {
            if !row.clean_eligible || row.horizontal_chords == 0 || row.vertical_chords == 0 {
                return None;
            }
            let sigma_path_tree = row.sigma_path_tree?;
            let sigma_4d = row.sigma_4d?;
            let owned_path_tree_bytes = owned_bytes_from_json(&row.owned_path_tree);
            let owned_4d_bytes = owned_bytes_from_json(&row.owned_4d);
            Some(RepresentationAdvantageRow {
                family: row.family.clone(),
                instance_name: row.instance_name.clone(),
                canonical_key: row.canonical_key.clone(),
                q: row.q,
                horizontal_chords: row.horizontal_chords,
                vertical_chords: row.vertical_chords,
                sigma_path_tree,
                sigma_4d,
                sigma_4d_over_path_tree: ExactRatio {
                    numerator: sigma_4d as u128,
                    denominator: sigma_path_tree.max(1) as u128,
                },
                sigma_path_tree_over_4d: ExactRatio {
                    numerator: sigma_path_tree as u128,
                    denominator: sigma_4d.max(1) as u128,
                },
                network_arcs_path_tree: row.network_arcs_path_tree.unwrap_or(0),
                network_arcs_4d: row.network_arcs_4d.unwrap_or(0),
                owned_path_tree_bytes,
                owned_4d_bytes,
                optimum_equal: row.optimum_equal,
                rectangles_equal: row.rectangles_equal,
                status: row.status.clone(),
            })
        })
        .collect::<Vec<_>>();
    let strict_path_tree_advantages = rows
        .iter()
        .filter(|row| row.sigma_4d > row.sigma_path_tree)
        .count();
    let strict_four_d_advantages = rows
        .iter()
        .filter(|row| row.sigma_path_tree > row.sigma_4d)
        .count();
    let eligible_rows = rows.len();
    let mut path_tree_rows = rows.clone();
    path_tree_rows.sort_by(|left, right| {
        compare_ratio_desc(left.sigma_4d_over_path_tree, right.sigma_4d_over_path_tree)
            .then_with(|| left.canonical_key.cmp(&right.canonical_key))
    });
    let mut four_d_rows = std::mem::take(&mut rows);
    four_d_rows.sort_by(|left, right| {
        compare_ratio_desc(left.sigma_path_tree_over_4d, right.sigma_path_tree_over_4d)
            .then_with(|| left.canonical_key.cmp(&right.canonical_key))
    });
    let limit = top_k.max(1);
    path_tree_rows.truncate(limit);
    four_d_rows.truncate(limit);
    RepresentationAdvantageReport {
        metadata: BenchmarkMetadata {
            git_commit: comparison.metadata.git_commit,
            rustc_version: comparison.metadata.rustc_version,
            command: comparison.metadata.command,
            seed: comparison.metadata.seed,
            timestamp: comparison.metadata.timestamp,
            input_count: comparison.metadata.input_count,
            component_count: comparison.metadata.component_count,
            input_model: "finite-grid-path-tree-vs-4d-sigma-advantage-search".to_owned(),
            unsupported_input_features: unsupported_input_features(),
        },
        eligible_rows,
        strict_path_tree_advantages,
        strict_four_d_advantages,
        top_path_tree_advantages: path_tree_rows,
        top_four_d_advantages: four_d_rows,
    }
}

fn compare_ratio_desc(left: ExactRatio, right: ExactRatio) -> std::cmp::Ordering {
    let lhs = left.numerator.saturating_mul(right.denominator);
    let rhs = right.numerator.saturating_mul(left.denominator);
    rhs.cmp(&lhs)
}

fn owned_bytes_from_json(value: &str) -> usize {
    serde_json::from_str::<BTreeMap<String, usize>>(value)
        .map(|entries| entries.values().copied().sum())
        .unwrap_or(0)
}

fn path_tree_growth_guards(diagnostics: &Diagnostics) -> bool {
    let q = diagnostics.total_chord_count;
    let l = if q == 0 {
        0
    } else {
        usize::BITS as usize - q.leading_zeros() as usize
    };
    let interval_bound = diagnostics
        .path_count
        .unwrap_or(0)
        .saturating_mul(l)
        .saturating_mul(4);
    let canonical_bound = diagnostics
        .path_count
        .unwrap_or(0)
        .saturating_mul(l)
        .saturating_mul(l)
        .saturating_mul(4);
    let tree_edge_bound = diagnostics
        .dual_tree_edge_count
        .unwrap_or(0)
        .saturating_mul(l)
        .saturating_mul(4);
    diagnostics.heavy_chain_interval_count.unwrap_or(0) <= interval_bound
        && diagnostics.canonical_segment_node_count.unwrap_or(0) <= canonical_bound
        && diagnostics.tree_edge_occurrences.unwrap_or(0) <= tree_edge_bound
}

fn q_bucket(q: usize) -> &'static str {
    match q {
        0..=8 => "0-8",
        9..=32 => "9-32",
        33..=128 => "33-128",
        129..=512 => "129-512",
        513..=2048 => "513-2048",
        _ => "2049+",
    }
}

/// Returns a stable dihedral-canonical key for a finite component.
///
/// The key is intentionally independent of the input grid's padding and
/// translation so advantage reports can retain replayable fixture identities.
fn canonical_component_key<C>(component: &rect_core::GridComponent<C>) -> String
where
    C: Clone + Eq,
{
    let source_width = component.grid_width as i128;
    let source_height = component.grid_height as i128;
    let mut variants = Vec::with_capacity(8);
    for symmetry in 0..8 {
        let swap = symmetry >= 4;
        let width = if swap { source_height } else { source_width };
        let mut points = component
            .cells
            .iter()
            .map(|cell| {
                let (mut x, mut y) = (cell.x as i128, cell.y as i128);
                match symmetry % 4 {
                    1 => (x, y) = (source_height - 1 - y, x),
                    2 => (x, y) = (source_width - 1 - x, source_height - 1 - y),
                    3 => (x, y) = (y, source_width - 1 - x),
                    _ => {}
                }
                if swap {
                    x = width - 1 - x;
                }
                (x, y)
            })
            .collect::<Vec<_>>();
        let min_x = points.iter().map(|(x, _)| *x).min().unwrap_or(0);
        let min_y = points.iter().map(|(_, y)| *y).min().unwrap_or(0);
        for (x, y) in &mut points {
            *x -= min_x;
            *y -= min_y;
        }
        points.sort_unstable();
        let key = points
            .iter()
            .map(|(x, y)| format!("{x}:{y}"))
            .collect::<Vec<_>>()
            .join(";");
        variants.push(key);
    }
    variants.sort_unstable();
    let canonical = variants.into_iter().next().unwrap_or_default();
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in canonical.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{}-{hash:016x}", component.cells.len())
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
