use std::collections::BTreeMap;
use std::fmt::Write;

use rect_core::{Diagnostics, ExactRatio, GridComponent};
use serde::{Deserialize, Serialize};

use crate::adversarial::{
    AdversarialInstance, dense_conflict_grid, endpoint_contact_instances,
    topological_stress_instances,
};
use crate::polyomino::{enumerate_free_polyominoes, explicit_hole_polyominoes};
use crate::{VerificationError, verify_component};

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
    pub diagnostics: Diagnostics,
    pub c0_phase_microseconds: BTreeMap<String, u128>,
    pub compressed_phase_microseconds: BTreeMap<String, u128>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub metadata: BenchmarkMetadata,
    pub verified_count: usize,
    pub unsupported_count: usize,
    pub solver_error_count: usize,
    pub counterexample_count: usize,
    pub rows: Vec<BenchmarkRow>,
}

impl BenchmarkReport {
    /// Serializes the report to a stable, machine-readable CSV schema.
    ///
    /// # Errors
    ///
    /// Returns `fmt::Error` if writing to the in-memory string fails.
    pub fn to_csv(&self) -> Result<String, std::fmt::Error> {
        let mut csv = String::new();
        writeln!(
            csv,
            "git_commit,rustc_version,command,seed,timestamp,input_count,component_count,input_model,unsupported_input_features,instance_name,family,parameters,component_id,status,message,cell_count,boundary_complexity,hole_count,reflex_vertex_count,horizontal_chord_count,vertical_chord_count,total_chord_count,explicit_conflict_edge_count,edge_density_numerator,edge_density_denominator,biclique_count,biclique_total_vertex_occurrences,biclique_size_per_chord_numerator,biclique_size_per_chord_denominator,biclique_size_per_edge_numerator,biclique_size_per_edge_denominator,c0_network_vertex_count,c0_network_arc_count,compressed_network_vertex_count,compressed_network_arc_count,maximum_matching_size,minimum_vertex_cover_size,output_rectangle_count,c0_phase_microseconds,compressed_phase_microseconds,peak_memory_bytes"
        )?;
        for row in &self.rows {
            let density = ratio_columns(row.diagnostics.conflict_edge_density);
            let sigma_per_chord = ratio_columns(row.diagnostics.biclique_size_per_chord);
            let sigma_per_edge = ratio_columns(row.diagnostics.biclique_size_per_explicit_edge);
            let c0_phases = serde_json::to_string(&row.c0_phase_microseconds)
                .unwrap_or_else(|_| "{}".to_owned());
            let compressed_phases = serde_json::to_string(&row.compressed_phase_microseconds)
                .unwrap_or_else(|_| "{}".to_owned());
            let parameters =
                serde_json::to_string(&row.parameters).unwrap_or_else(|_| "{}".to_owned());
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
                row.diagnostics.cell_count.to_string(),
                row.diagnostics.boundary_complexity.to_string(),
                row.diagnostics.hole_count.to_string(),
                row.diagnostics.reflex_vertex_count.to_string(),
                row.diagnostics.horizontal_chord_count.to_string(),
                row.diagnostics.vertical_chord_count.to_string(),
                row.diagnostics.total_chord_count.to_string(),
                row.diagnostics.explicit_conflict_edge_count.to_string(),
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
                c0_phases,
                compressed_phases,
                row.diagnostics
                    .peak_memory_bytes
                    .map(|bytes| bytes.to_string())
                    .unwrap_or_default(),
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

fn benchmark_instances(
    context: BenchmarkContext,
    instances: &[AdversarialInstance],
    oracle_cell_limit: usize,
) -> BenchmarkReport {
    let mut rows = Vec::new();
    for instance in instances {
        match instance.foreground_components() {
            Ok(components) => {
                for component in components {
                    rows.push(benchmark_component(instance, &component, oracle_cell_limit));
                }
            }
            Err(error) => rows.push(BenchmarkRow {
                instance_name: instance.name.clone(),
                family: instance.family.clone(),
                parameters: instance.parameters.clone(),
                component_id: 0,
                status: "unsupported".to_owned(),
                message: Some(error.to_string()),
                diagnostics: Diagnostics::default(),
                c0_phase_microseconds: BTreeMap::new(),
                compressed_phase_microseconds: BTreeMap::new(),
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
            BenchmarkRow {
                instance_name: instance.name.clone(),
                family: instance.family.clone(),
                parameters: instance.parameters.clone(),
                component_id: component.id.0,
                status: "verified".to_owned(),
                message: None,
                diagnostics: verification.dominance_compact.diagnostics,
                c0_phase_microseconds,
                compressed_phase_microseconds,
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
                diagnostics: Diagnostics {
                    cell_count: component.cell_count(),
                    ..Diagnostics::default()
                },
                c0_phase_microseconds: BTreeMap::new(),
                compressed_phase_microseconds: BTreeMap::new(),
            }
        }
    }
}

fn count_status(rows: &[BenchmarkRow], status: &str) -> usize {
    rows.iter().filter(|row| row.status == status).count()
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
        *summary.entry("explicit_edges".to_owned()).or_default() +=
            row.diagnostics.explicit_conflict_edge_count as u128;
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
