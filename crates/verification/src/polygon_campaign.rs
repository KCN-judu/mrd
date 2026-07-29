//! Reproducible reference-versus-indexed polygon release campaigns.

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use dominance::experiment::{
    PolygonArrangementBackend, PolygonChordBackend, PolygonCompletionBackend, PolygonSolveOptions,
    Representation, Verification, solve_polygon_with_options,
};
use mrd_domain::{
    Boundary, ColorGrid, CoordinateRect, Diagnostics, OrthogonalLoop, Point,
    PolygonDissectionResult, PolygonErrorCategory, PreparedPolygonContext, RectilinearPolygon,
    polygon,
};
use serde::{Deserialize, Serialize};
use sg_oracle::grid::{EndpointIndex, classify_clean_polygon};
use sg_oracle::polygon::{
    self as sg_polygon, HorizontalCutSegment, PolygonValidationError, VerticalCutSegment,
    validate_polygon_dissection,
};
use sg_oracle::{polygon_arrangement, polygon_cut_index, polygon_sparse};

use crate::adversarial::{
    AdversarialInstance, clean_complete_bipartite_grid, dense_conflict_grid,
    endpoint_contact_instances, external_oracle_adversarial_instances, path_tree_geometry_families,
    topological_stress_instances,
};
use crate::benchmark::{BenchmarkContext, BenchmarkMetadata};
use crate::polygon::{RasterLimits, verify_polygon};
use crate::polyomino::{enumerate_free_polyominoes, explicit_hole_polyominoes};
use crate::witness::{mixed_branching_connected_sum_family, stored_mixed_branching_witnesses};

const UNSUPPORTED_FEATURES: [&str; 5] = [
    "ornaments",
    "point-holes",
    "segment-holes",
    "degenerate-formal-holes",
    "disconnected-outer-components",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonCounterexample {
    pub name: String,
    pub original: RectilinearPolygon,
    pub minimized: RectilinearPolygon,
    pub reason: String,
    pub reference: Option<PolygonDissectionResult>,
    pub indexed: Option<PolygonDissectionResult>,
    pub sweep: Option<PolygonDissectionResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonCampaignReport {
    pub metadata: BenchmarkMetadata,
    pub population: String,
    pub input_count: usize,
    pub component_count: usize,
    pub supported_components: usize,
    pub model_rejections: usize,
    pub verified_components: usize,
    pub solver_errors: usize,
    pub timeouts: usize,
    pub disagreements: usize,
    pub raster_oracle_comparisons: usize,
    pub path_tree_comparisons: usize,
    pub minimized_counterexamples: Vec<PolygonCounterexample>,
}

impl PolygonCampaignReport {
    #[must_use]
    pub const fn verified(&self) -> bool {
        self.solver_errors == 0 && self.timeouts == 0 && self.disagreements == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonNegativeRecord {
    pub name: String,
    pub reference_category: String,
    pub indexed_category: String,
    pub sparse_category: String,
    pub deterministic_match: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonNegativeReport {
    pub metadata: BenchmarkMetadata,
    pub input_population: String,
    pub records: Vec<PolygonNegativeRecord>,
    pub disagreements: usize,
    pub solver_errors: usize,
    pub minimized_counterexamples: Vec<PolygonCounterexample>,
}

impl PolygonNegativeReport {
    #[must_use]
    pub const fn verified(&self) -> bool {
        self.disagreements == 0 && self.solver_errors == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct PolygonScalingRow {
    pub family: String,
    pub family_name: String,
    pub size: usize,
    pub boundary_complexity: usize,
    pub hole_count: usize,
    pub reflex_count: usize,
    pub aligned_candidate_count: usize,
    pub chord_count: usize,
    pub selected_horizontal_cut_count: usize,
    pub selected_vertical_cut_count: usize,
    pub added_horizontal_cut_count: usize,
    pub added_vertical_cut_count: usize,
    pub coordinate_x_count: usize,
    pub coordinate_y_count: usize,
    pub coordinate_cartesian_product: usize,
    pub sparse_subdivision_vertices: usize,
    pub sparse_subdivision_half_edges: usize,
    pub sparse_subdivision_junctions: usize,
    pub sparse_subdivision_interior_faces: usize,
    pub dense_owned_bytes_estimate: usize,
    pub sparse_owned_bytes_estimate: usize,
    pub cut_index_owned_bytes_estimate: usize,
    pub completion_microseconds: u128,
    pub recovery_microseconds: u128,
    pub validation_microseconds: u128,
    pub reference_microseconds: u128,
    pub indexed_microseconds: u128,
    pub sweep_microseconds: u128,
    pub subdivision_input_segment_count: usize,
    pub subdivision_reported_intersections: usize,
    pub reference_subdivision_candidate_pair_tests: usize,
    pub sweep_subdivision_candidate_pair_tests: usize,
    pub reference_subdivision_recovery_microseconds: u128,
    pub sweep_subdivision_recovery_microseconds: u128,
    pub dense_recovery_microseconds: u128,
    pub reference_validator_microseconds: u128,
    pub event_validator_microseconds: u128,
    pub dense_validator_microseconds: u128,
    pub reference_validator_boundary_edge_scans: usize,
    pub reference_validator_active_rectangle_resorts: usize,
    pub event_validator_boundary_edge_scans: usize,
    pub event_validator_active_rectangle_resorts: usize,
    pub sparse_materialized_tree_nodes: usize,
    pub sparse_logical_tree_nodes: usize,
    pub geometry_backends_equal: bool,
    pub auto_selected_backend: String,
    pub actual_fastest_recovery_backend: String,
    pub auto_time_regret_microseconds: u128,
    pub auto_memory_regret_bytes: usize,
    /// `C / max(1, q)` is represented exactly by these two fields.
    pub candidate_output_ratio_numerator: usize,
    pub candidate_output_ratio_denominator: usize,
    pub reference_pair_iterations: usize,
    pub indexed_pair_iterations: usize,
    pub sweep_event_count: usize,
    pub sweep_status_operations: usize,
    pub sweep_output_record_count: usize,
    pub chord_families_equal: bool,
    pub optimum_equal: bool,
    pub cuts_equal: bool,
    pub rectangles_equal: bool,
    pub three_backend_equal: bool,
    pub reference_diagnostics: Diagnostics,
    pub indexed_diagnostics: Diagnostics,
    pub sweep_diagnostics: Diagnostics,
    pub status: String,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonScalingReport {
    pub metadata: BenchmarkMetadata,
    pub rows: Vec<PolygonScalingRow>,
    pub verified_rows: usize,
    pub solver_errors: usize,
    pub disagreements: usize,
}

impl PolygonScalingReport {
    #[must_use]
    pub const fn verified(&self) -> bool {
        self.solver_errors == 0 && self.disagreements == 0
    }

    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn to_csv(&self) -> String {
        let mut csv = String::from(
            "family,family_name,size,boundary_complexity,hole_count,reflex_count,aligned_candidate_count,chord_count,candidate_output_ratio_numerator,candidate_output_ratio_denominator,selected_horizontal_cut_count,selected_vertical_cut_count,added_horizontal_cut_count,added_vertical_cut_count,coordinate_x_count,coordinate_y_count,coordinate_cartesian_product,sparse_subdivision_vertices,sparse_subdivision_half_edges,sparse_subdivision_junctions,sparse_subdivision_interior_faces,dense_owned_bytes_estimate,sparse_owned_bytes_estimate,cut_index_owned_bytes_estimate,completion_microseconds,recovery_microseconds,validation_microseconds,reference_microseconds,indexed_microseconds,sweep_microseconds,subdivision_input_segment_count,subdivision_reported_intersections,reference_subdivision_candidate_pair_tests,sweep_subdivision_candidate_pair_tests,reference_subdivision_recovery_microseconds,sweep_subdivision_recovery_microseconds,dense_recovery_microseconds,reference_validator_microseconds,event_validator_microseconds,dense_validator_microseconds,reference_validator_boundary_edge_scans,reference_validator_active_rectangle_resorts,event_validator_boundary_edge_scans,event_validator_active_rectangle_resorts,sparse_materialized_tree_nodes,sparse_logical_tree_nodes,geometry_backends_equal,auto_selected_backend,actual_fastest_recovery_backend,auto_time_regret_microseconds,auto_memory_regret_bytes,reference_pair_iterations,indexed_pair_iterations,sweep_event_count,sweep_status_operations,sweep_output_record_count,chord_families_equal,optimum_equal,cuts_equal,rectangles_equal,three_backend_equal,reference_boundary_edge_visits,indexed_boundary_edge_visits,reference_definition7_full_boundary_scans,indexed_definition7_full_boundary_scans,sweep_aligned_pair_iterations,sweep_all_pair_iterations,sweep_definition7_fallback_checks,sweep_full_boundary_scans,sweep_duplicate_output_count,reference_completion_candidate_rebuilds,indexed_completion_candidate_rebuilds,reference_completion_cut_pair_tests,indexed_completion_cut_pair_tests,reference_completion_full_boundary_scans,indexed_completion_full_boundary_scans,reference_completion_full_cut_scans,indexed_completion_full_cut_scans,reference_arrangement_boundary_edge_visits,indexed_arrangement_boundary_edge_visits,reference_validator_rectangle_cell_tests,indexed_validator_rectangle_cell_tests,reference_prepare_owned_bytes,indexed_prepare_owned_bytes,sweep_prepare_owned_bytes,reference_owned_allocations,indexed_owned_allocations,sweep_owned_allocations,status,message\n",
        );
        for row in &self.rows {
            let reference_owned =
                serde_json::to_string(&row.reference_diagnostics.owned_allocation_estimates)
                    .unwrap_or_else(|_| "{}".to_owned());
            let indexed_owned =
                serde_json::to_string(&row.indexed_diagnostics.owned_allocation_estimates)
                    .unwrap_or_else(|_| "{}".to_owned());
            let sweep_owned =
                serde_json::to_string(&row.sweep_diagnostics.owned_allocation_estimates)
                    .unwrap_or_else(|_| "{}".to_owned());
            let fields = [
                row.family.clone(),
                row.family_name.clone(),
                row.size.to_string(),
                row.boundary_complexity.to_string(),
                row.hole_count.to_string(),
                row.reflex_count.to_string(),
                row.aligned_candidate_count.to_string(),
                row.chord_count.to_string(),
                row.candidate_output_ratio_numerator.to_string(),
                row.candidate_output_ratio_denominator.to_string(),
                row.selected_horizontal_cut_count.to_string(),
                row.selected_vertical_cut_count.to_string(),
                row.added_horizontal_cut_count.to_string(),
                row.added_vertical_cut_count.to_string(),
                row.coordinate_x_count.to_string(),
                row.coordinate_y_count.to_string(),
                row.coordinate_cartesian_product.to_string(),
                row.sparse_subdivision_vertices.to_string(),
                row.sparse_subdivision_half_edges.to_string(),
                row.sparse_subdivision_junctions.to_string(),
                row.sparse_subdivision_interior_faces.to_string(),
                row.dense_owned_bytes_estimate.to_string(),
                row.sparse_owned_bytes_estimate.to_string(),
                row.cut_index_owned_bytes_estimate.to_string(),
                row.completion_microseconds.to_string(),
                row.recovery_microseconds.to_string(),
                row.validation_microseconds.to_string(),
                row.reference_microseconds.to_string(),
                row.indexed_microseconds.to_string(),
                row.sweep_microseconds.to_string(),
                row.subdivision_input_segment_count.to_string(),
                row.subdivision_reported_intersections.to_string(),
                row.reference_subdivision_candidate_pair_tests.to_string(),
                row.sweep_subdivision_candidate_pair_tests.to_string(),
                row.reference_subdivision_recovery_microseconds.to_string(),
                row.sweep_subdivision_recovery_microseconds.to_string(),
                row.dense_recovery_microseconds.to_string(),
                row.reference_validator_microseconds.to_string(),
                row.event_validator_microseconds.to_string(),
                row.dense_validator_microseconds.to_string(),
                row.reference_validator_boundary_edge_scans.to_string(),
                row.reference_validator_active_rectangle_resorts.to_string(),
                row.event_validator_boundary_edge_scans.to_string(),
                row.event_validator_active_rectangle_resorts.to_string(),
                row.sparse_materialized_tree_nodes.to_string(),
                row.sparse_logical_tree_nodes.to_string(),
                row.geometry_backends_equal.to_string(),
                row.auto_selected_backend.clone(),
                row.actual_fastest_recovery_backend.clone(),
                row.auto_time_regret_microseconds.to_string(),
                row.auto_memory_regret_bytes.to_string(),
                row.reference_pair_iterations.to_string(),
                row.indexed_pair_iterations.to_string(),
                row.sweep_event_count.to_string(),
                row.sweep_status_operations.to_string(),
                row.sweep_output_record_count.to_string(),
                row.chord_families_equal.to_string(),
                row.optimum_equal.to_string(),
                row.cuts_equal.to_string(),
                row.rectangles_equal.to_string(),
                row.three_backend_equal.to_string(),
                optional_usize(row.reference_diagnostics.polygon_boundary_edge_visits),
                optional_usize(row.indexed_diagnostics.polygon_boundary_edge_visits),
                optional_usize(
                    row.reference_diagnostics
                        .polygon_definition7_full_boundary_scans,
                ),
                optional_usize(
                    row.indexed_diagnostics
                        .polygon_definition7_full_boundary_scans,
                ),
                optional_usize(row.sweep_diagnostics.sweep_aligned_pair_iterations),
                optional_usize(row.sweep_diagnostics.sweep_all_pair_iterations),
                optional_usize(row.sweep_diagnostics.sweep_definition7_fallback_checks),
                optional_usize(row.sweep_diagnostics.sweep_full_boundary_scans),
                optional_usize(row.sweep_diagnostics.sweep_duplicate_output_count),
                optional_usize(
                    row.reference_diagnostics
                        .polygon_completion_candidate_rebuilds,
                ),
                optional_usize(
                    row.indexed_diagnostics
                        .polygon_completion_candidate_rebuilds,
                ),
                optional_usize(row.reference_diagnostics.polygon_completion_cut_pair_tests),
                optional_usize(row.indexed_diagnostics.polygon_completion_cut_pair_tests),
                optional_usize(
                    row.reference_diagnostics
                        .polygon_completion_full_boundary_scans,
                ),
                optional_usize(
                    row.indexed_diagnostics
                        .polygon_completion_full_boundary_scans,
                ),
                optional_usize(row.reference_diagnostics.polygon_completion_full_cut_scans),
                optional_usize(row.indexed_diagnostics.polygon_completion_full_cut_scans),
                optional_usize(
                    row.reference_diagnostics
                        .polygon_arrangement_boundary_edge_visits,
                ),
                optional_usize(
                    row.indexed_diagnostics
                        .polygon_arrangement_boundary_edge_visits,
                ),
                optional_usize(
                    row.reference_diagnostics
                        .polygon_validator_rectangle_cell_tests,
                ),
                optional_usize(
                    row.indexed_diagnostics
                        .polygon_validator_rectangle_cell_tests,
                ),
                optional_usize(row.reference_diagnostics.polygon_prepare_owned_bytes),
                optional_usize(row.indexed_diagnostics.polygon_prepare_owned_bytes),
                optional_usize(row.sweep_diagnostics.polygon_prepare_owned_bytes),
                reference_owned,
                indexed_owned,
                sweep_owned,
                row.status.clone(),
                row.message.clone().unwrap_or_default(),
            ];
            csv.push_str(
                &fields
                    .iter()
                    .map(|field| csv_field(field))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            csv.push('\n');
        }
        csv
    }
}

#[derive(Default)]
struct CampaignCounts {
    inputs: usize,
    components: usize,
    supported: usize,
    rejected: usize,
    verified: usize,
    solver_errors: usize,
    disagreements: usize,
    raster: usize,
    path_tree: usize,
    counterexamples: Vec<PolygonCounterexample>,
}

/// Runs the complete grid-derived polygon differential for each requested
/// square dimension.
#[must_use]
pub fn exhaustive_grid_polygon_campaign(
    context: BenchmarkContext,
    dimensions: &[usize],
) -> PolygonCampaignReport {
    let mut counts = CampaignCounts::default();
    for &dimension in dimensions {
        if dimension == 0 || dimension.saturating_mul(dimension) > 20 {
            counts.solver_errors += 1;
            continue;
        }
        let bits = dimension * dimension;
        let limit = 1_u64 << bits;
        for mask in 1..limit {
            counts.inputs += 1;
            let cells = (0..bits).map(|bit| mask & (1 << bit) != 0).collect();
            let Ok(grid) = ColorGrid::new(dimension, dimension, cells) else {
                counts.solver_errors += 1;
                continue;
            };
            for component in grid
                .four_connected_components()
                .into_iter()
                .filter(|component| component.color)
            {
                counts.components += 1;
                let Ok(boundary) = Boundary::from_component(&component) else {
                    counts.solver_errors += 1;
                    continue;
                };
                let Ok(polygon) = boundary.to_polygon() else {
                    counts.rejected += 1;
                    continue;
                };
                counts.supported += 1;
                compare_and_record(
                    format!(
                        "{dimension}x{dimension}-mask-{mask}-component-{}",
                        component.id.0
                    ),
                    &polygon,
                    dimension <= 3,
                    &mut counts,
                );
            }
        }
    }
    campaign_report(
        context,
        format!("grid-derived-{}", join_sizes(dimensions)),
        counts,
    )
}

/// Runs native, polyomino, random, adversarial, witness, and metamorphic
/// reference-versus-indexed populations.
#[must_use]
pub fn extended_polygon_backend_campaign(
    context: BenchmarkContext,
    max_cells: usize,
    random_cases: usize,
    family_sizes: &[usize],
) -> PolygonCampaignReport {
    let mut counts = CampaignCounts::default();
    for level in enumerate_free_polyominoes(max_cells) {
        for polyomino in level {
            compare_instance(
                &polyomino.to_instance(
                    format!("polyomino-{}", polyomino.canonical_key()),
                    "free-polyomino",
                ),
                false,
                &mut counts,
            );
        }
    }
    for instance in explicit_hole_polyominoes(max_cells)
        .into_iter()
        .chain(endpoint_contact_instances())
        .chain(topological_stress_instances())
        .chain(external_oracle_adversarial_instances())
        .chain(path_tree_geometry_families(12))
        .chain(stored_mixed_branching_witnesses())
        .chain(mixed_branching_connected_sum_family(6))
        .chain([dense_conflict_grid(4, 5), dense_conflict_grid(8, 8)])
    {
        compare_instance(&instance, false, &mut counts);
    }
    for t in 1..=4 {
        match clean_complete_bipartite_grid(t) {
            Ok(instance) => compare_instance(&instance, false, &mut counts),
            Err(_) => counts.solver_errors += 1,
        }
    }
    for case in 0..random_cases {
        compare_instance(&random_connected_instance(case), case < 16, &mut counts);
    }
    for &size in family_sizes {
        match native_polygon_families(size) {
            Ok(families) => {
                for (code, name, polygon) in families {
                    counts.inputs += 1;
                    counts.components += 1;
                    counts.supported += 1;
                    compare_and_record(
                        format!("{code}-{name}-{size}"),
                        &polygon,
                        true,
                        &mut counts,
                    );
                    for (variant, transformed) in metamorphic_polygons(&polygon) {
                        counts.inputs += 1;
                        counts.components += 1;
                        counts.supported += 1;
                        compare_and_record(
                            format!("{code}-{name}-{size}-{variant}"),
                            &transformed,
                            false,
                            &mut counts,
                        );
                    }
                }
            }
            Err(_) => counts.solver_errors += 1,
        }
    }
    campaign_report(context, "extended-polygon-backends".to_owned(), counts)
}

/// Runs only the deterministic boundary-native A-H fixture population.
#[must_use]
pub fn native_polygon_fixture_campaign(
    context: BenchmarkContext,
    sizes: &[usize],
) -> PolygonCampaignReport {
    let mut counts = CampaignCounts::default();
    for &size in sizes {
        match native_polygon_families(size) {
            Ok(families) => {
                for (code, name, polygon) in families {
                    counts.inputs += 1;
                    counts.components += 1;
                    counts.supported += 1;
                    compare_and_record(
                        format!("{code}-{name}-{size}"),
                        &polygon,
                        true,
                        &mut counts,
                    );
                }
            }
            Err(_) => counts.solver_errors += 1,
        }
    }
    campaign_report(
        context,
        "polygon-native-fixtures-a-through-h".to_owned(),
        counts,
    )
}

/// Differentially checks the structural validators on deterministic invalid
/// polygons, including every broad v1.0 error category represented locally.
///
/// # Panics
///
/// Panics only if the hard-coded valid rectangle fixture is changed into an
/// invalid polygon or its prepared arrangement cannot be constructed.
#[must_use]
pub fn polygon_negative_campaign(context: BenchmarkContext) -> PolygonNegativeReport {
    let cases = negative_polygons();
    let mut records = Vec::with_capacity(cases.len());
    let mut disagreements = 0;
    for (name, polygon) in cases {
        let reference =
            PreparedPolygonContext::new_with_validator(&polygon, polygon::Backend::Oracle)
                .err()
                .map(|error| error.to_string());
        let indexed =
            PreparedPolygonContext::new_with_validator(&polygon, polygon::Backend::Experiment)
                .err()
                .map(|error| error.to_string());
        let reference_category = validator_category(&polygon, polygon::Backend::Oracle);
        let indexed_category = validator_category(&polygon, polygon::Backend::Experiment);
        let sparse_category = indexed_category.clone();
        let deterministic_match = reference_category == indexed_category && reference == indexed;
        disagreements += usize::from(!deterministic_match);
        records.push(PolygonNegativeRecord {
            name: name.to_owned(),
            reference_category,
            indexed_category,
            sparse_category,
            deterministic_match,
        });
    }
    let rectangle_polygon = RectilinearPolygon::new(rectangle_loop(0, 0, 4, 4, false), vec![])
        .expect("rectangle fixture is valid");
    let prepared = PreparedPolygonContext::new_with_validator(
        &rectangle_polygon,
        polygon::Backend::Experiment,
    )
    .expect("rectangle fixture prepares");
    let vertical = [1, 2, 3]
        .into_iter()
        .map(|x| VerticalCutSegment {
            x,
            bottom: 0,
            top: 4,
        })
        .collect::<BTreeSet<_>>();
    let arrangement = polygon_arrangement::Arrangement::new(&prepared, &BTreeSet::new(), &vertical)
        .expect("rectangle fixture arrangement builds");
    for (name, rectangles) in invalid_rectangle_sets() {
        let reference = validate_polygon_dissection(&rectangle_polygon, &rectangles)
            .expect_err("fixture must be invalid");
        let indexed = polygon_arrangement::experiment::Validator
            .validate(&arrangement, &rectangle_polygon, &rectangles)
            .expect_err("fixture must be invalid");
        let reference_category = dissection_error_category(&reference);
        let indexed_category = dissection_error_category(&indexed);
        let sparse = sg_oracle::polygon_sparse::validator::Validator
            .validate_with_backend(
                &rectangle_polygon,
                &rectangles,
                polygon_sparse::validator::Backend::Experiment,
            )
            .expect_err("fixture must be invalid");
        let sparse_category = dissection_error_category(&sparse);
        let sparse_reference = sg_oracle::polygon_sparse::validator::Validator
            .validate_with_backend(
                &rectangle_polygon,
                &rectangles,
                polygon_sparse::validator::Backend::Oracle,
            )
            .expect_err("fixture must be invalid");
        let deterministic_match = reference_category == indexed_category
            && reference_category == sparse_category
            && sparse_category == dissection_error_category(&sparse_reference);
        disagreements += usize::from(!deterministic_match);
        records.push(PolygonNegativeRecord {
            name: name.to_owned(),
            reference_category,
            indexed_category,
            sparse_category,
            deterministic_match,
        });
    }
    PolygonNegativeReport {
        metadata: metadata(
            context,
            records.len(),
            records.len(),
            "invalid-rectilinear-polygons",
        ),
        input_population: "deterministic-invalid-polygon-categories".to_owned(),
        records,
        disagreements,
        solver_errors: 0,
        minimized_counterexamples: Vec::new(),
    }
}

/// Benchmarks all three exact polygon chord pipelines on the native scaling
/// families while retaining complete diagnostics for every row.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn polygon_scaling_campaign(
    context: BenchmarkContext,
    sizes: &[usize],
) -> PolygonScalingReport {
    let mut rows = Vec::new();
    let mut solver_errors = 0;
    let mut disagreements = 0;
    for &size in sizes {
        let families = match native_polygon_families(size) {
            Ok(families) => families,
            Err(message) => {
                solver_errors += 1;
                rows.push(error_scaling_row(size, message));
                continue;
            }
        };
        for (family, family_name, polygon) in families {
            match solve_triple(&polygon) {
                Ok((reference, indexed, sweep, reference_micros, indexed_micros, sweep_micros)) => {
                    let (dense_geometry, reference_sparse, auto) =
                        match solve_geometry_variants(&polygon) {
                            Ok(variants) => variants,
                            Err(message) => {
                                solver_errors += 1;
                                rows.push(PolygonScalingRow {
                                    family,
                                    family_name,
                                    size,
                                    status: "solver-error".to_owned(),
                                    message: Some(message),
                                    ..empty_scaling_row()
                                });
                                continue;
                            }
                        };
                    let cuts_equal = certificate_field(&reference, "selected_horizontal_cuts")
                        == certificate_field(&indexed, "selected_horizontal_cuts")
                        && certificate_field(&reference, "selected_horizontal_cuts")
                            == certificate_field(&sweep, "selected_horizontal_cuts")
                        && certificate_field(&reference, "selected_vertical_cuts")
                            == certificate_field(&indexed, "selected_vertical_cuts")
                        && certificate_field(&reference, "selected_vertical_cuts")
                            == certificate_field(&sweep, "selected_vertical_cuts")
                        && certificate_field(&reference, "added_horizontal_cuts")
                            == certificate_field(&indexed, "added_horizontal_cuts")
                        && certificate_field(&reference, "added_horizontal_cuts")
                            == certificate_field(&sweep, "added_horizontal_cuts")
                        && certificate_field(&reference, "added_vertical_cuts")
                            == certificate_field(&indexed, "added_vertical_cuts")
                        && certificate_field(&reference, "added_vertical_cuts")
                            == certificate_field(&sweep, "added_vertical_cuts");
                    let chord_families_equal = certificate_field(&reference, "horizontal_chords")
                        == certificate_field(&indexed, "horizontal_chords")
                        && certificate_field(&reference, "horizontal_chords")
                            == certificate_field(&sweep, "horizontal_chords")
                        && certificate_field(&reference, "vertical_chords")
                            == certificate_field(&indexed, "vertical_chords")
                        && certificate_field(&reference, "vertical_chords")
                            == certificate_field(&sweep, "vertical_chords");
                    let optimum_equal = reference.optimum_rectangle_count
                        == indexed.optimum_rectangle_count
                        && reference.optimum_rectangle_count == sweep.optimum_rectangle_count;
                    let rectangles_equal = reference.rectangles == indexed.rectangles
                        && reference.rectangles == sweep.rectangles;
                    let sweep_contract_valid =
                        sweep.diagnostics.polygon_chord_enumerator.as_deref() == Some("sg-sweep")
                            && sweep.diagnostics.sweep_aligned_pair_iterations == Some(0)
                            && sweep.diagnostics.sweep_all_pair_iterations == Some(0)
                            && sweep.diagnostics.sweep_definition7_fallback_checks == Some(0)
                            && sweep.diagnostics.sweep_full_boundary_scans == Some(0)
                            && sweep.diagnostics.sweep_duplicate_output_count == Some(0);
                    let geometry_backends_equal = dense_geometry.rectangles == sweep.rectangles
                        && reference_sparse.rectangles == sweep.rectangles
                        && auto.rectangles == sweep.rectangles;
                    let sweep_recovery_microseconds =
                        phase_time(&sweep, "polygon_rectangle_recovery");
                    let reference_sparse_recovery_microseconds =
                        phase_time(&reference_sparse, "polygon_rectangle_recovery");
                    let dense_recovery_microseconds =
                        phase_time(&dense_geometry, "polygon_rectangle_recovery");
                    let auto_recovery_microseconds =
                        phase_time(&auto, "polygon_rectangle_recovery");
                    let (actual_fastest_recovery_backend, fastest_recovery_microseconds) =
                        if dense_recovery_microseconds <= sweep_recovery_microseconds {
                            ("dense-arrangement", dense_recovery_microseconds)
                        } else {
                            ("sparse-subdivision", sweep_recovery_microseconds)
                        };
                    let dense_bytes = dense_geometry
                        .diagnostics
                        .dense_recovery_retained_byte_estimate
                        .unwrap_or(0);
                    let sparse_bytes = sweep
                        .diagnostics
                        .sparse_subdivision_owned_bytes
                        .unwrap_or(0);
                    let auto_selected_backend = auto
                        .diagnostics
                        .polygon_selected_recovery_backend
                        .clone()
                        .unwrap_or_default();
                    let selected_bytes = if auto_selected_backend == "dense-arrangement" {
                        dense_bytes
                    } else {
                        sparse_bytes
                    };
                    let three_backend_equal = chord_families_equal
                        && optimum_equal
                        && cuts_equal
                        && rectangles_equal
                        && geometry_backends_equal
                        && sweep_contract_valid;
                    let verified = three_backend_equal;
                    disagreements += usize::from(!verified);
                    let aligned_candidate_count = reference
                        .diagnostics
                        .polygon_aligned_reflex_candidate_pairs
                        .unwrap_or(0);
                    let chord_count = reference.diagnostics.total_chord_count;
                    let coordinate_counts = (
                        indexed
                            .diagnostics
                            .coordinate_compression_x_count
                            .unwrap_or(0),
                        indexed
                            .diagnostics
                            .coordinate_compression_y_count
                            .unwrap_or(0),
                    );
                    rows.push(PolygonScalingRow {
                        family,
                        family_name,
                        size,
                        boundary_complexity: reference.diagnostics.boundary_complexity,
                        hole_count: reference.diagnostics.hole_count,
                        reflex_count: reference.diagnostics.reflex_vertex_count,
                        aligned_candidate_count,
                        chord_count,
                        candidate_output_ratio_numerator: aligned_candidate_count,
                        candidate_output_ratio_denominator: chord_count.max(1),
                        selected_horizontal_cut_count: certificate_array_len(
                            &reference,
                            "selected_horizontal_cuts",
                        ),
                        selected_vertical_cut_count: certificate_array_len(
                            &reference,
                            "selected_vertical_cuts",
                        ),
                        added_horizontal_cut_count: certificate_array_len(
                            &reference,
                            "added_horizontal_cuts",
                        ),
                        added_vertical_cut_count: certificate_array_len(
                            &reference,
                            "added_vertical_cuts",
                        ),
                        coordinate_x_count: coordinate_counts.0,
                        coordinate_y_count: coordinate_counts.1,
                        coordinate_cartesian_product: coordinate_counts
                            .0
                            .saturating_mul(coordinate_counts.1),
                        sparse_subdivision_vertices: indexed
                            .diagnostics
                            .sparse_subdivision_vertex_count
                            .unwrap_or(0),
                        sparse_subdivision_half_edges: indexed
                            .diagnostics
                            .sparse_subdivision_half_edge_count
                            .unwrap_or(0),
                        sparse_subdivision_junctions: indexed
                            .diagnostics
                            .sparse_subdivision_junction_count
                            .unwrap_or(0),
                        sparse_subdivision_interior_faces: indexed
                            .diagnostics
                            .output_rectangle_count,
                        dense_owned_bytes_estimate: dense_arrangement_owned_bytes_estimate(
                            coordinate_counts.0,
                            coordinate_counts.1,
                        ),
                        sparse_owned_bytes_estimate: indexed
                            .diagnostics
                            .sparse_subdivision_owned_bytes
                            .unwrap_or(0),
                        cut_index_owned_bytes_estimate: indexed
                            .diagnostics
                            .cut_index_owned_bytes
                            .unwrap_or(0),
                        completion_microseconds: indexed
                            .diagnostics
                            .phase_microseconds
                            .get("polygon_horizontal_completion")
                            .copied()
                            .unwrap_or(0)
                            + indexed
                                .diagnostics
                                .phase_microseconds
                                .get("polygon_vertical_completion")
                                .copied()
                                .unwrap_or(0),
                        recovery_microseconds: indexed
                            .diagnostics
                            .phase_microseconds
                            .get("polygon_rectangle_recovery")
                            .copied()
                            .unwrap_or(0),
                        validation_microseconds: indexed
                            .diagnostics
                            .phase_microseconds
                            .get("polygon_final_validation")
                            .copied()
                            .unwrap_or(0),
                        reference_microseconds: reference_micros,
                        indexed_microseconds: indexed_micros,
                        sweep_microseconds: sweep_micros,
                        subdivision_input_segment_count: sweep
                            .diagnostics
                            .subdivision_input_segment_count
                            .unwrap_or(0),
                        subdivision_reported_intersections: sweep
                            .diagnostics
                            .subdivision_reported_intersections
                            .unwrap_or(0),
                        reference_subdivision_candidate_pair_tests: reference_sparse
                            .diagnostics
                            .subdivision_candidate_pair_tests
                            .unwrap_or(0),
                        sweep_subdivision_candidate_pair_tests: sweep
                            .diagnostics
                            .subdivision_candidate_pair_tests
                            .unwrap_or(0),
                        reference_subdivision_recovery_microseconds:
                            reference_sparse_recovery_microseconds,
                        sweep_subdivision_recovery_microseconds: sweep_recovery_microseconds,
                        dense_recovery_microseconds,
                        reference_validator_microseconds: phase_time(
                            &reference_sparse,
                            "polygon_final_validation",
                        ),
                        event_validator_microseconds: phase_time(
                            &sweep,
                            "polygon_final_validation",
                        ),
                        dense_validator_microseconds: phase_time(
                            &dense_geometry,
                            "polygon_final_validation",
                        ),
                        reference_validator_boundary_edge_scans: reference_sparse
                            .diagnostics
                            .validator_boundary_edge_scans
                            .unwrap_or(0),
                        reference_validator_active_rectangle_resorts: reference_sparse
                            .diagnostics
                            .validator_active_rectangle_resorts
                            .unwrap_or(0),
                        event_validator_boundary_edge_scans: sweep
                            .diagnostics
                            .validator_boundary_edge_scans
                            .unwrap_or(0),
                        event_validator_active_rectangle_resorts: sweep
                            .diagnostics
                            .validator_active_rectangle_resorts
                            .unwrap_or(0),
                        sparse_materialized_tree_nodes: sweep
                            .diagnostics
                            .cut_index_materialized_tree_node_count
                            .unwrap_or(0),
                        sparse_logical_tree_nodes: sweep
                            .diagnostics
                            .cut_index_logical_tree_node_count
                            .unwrap_or(0),
                        geometry_backends_equal,
                        auto_selected_backend,
                        actual_fastest_recovery_backend: actual_fastest_recovery_backend.to_owned(),
                        auto_time_regret_microseconds: auto_recovery_microseconds
                            .saturating_sub(fastest_recovery_microseconds),
                        auto_memory_regret_bytes: selected_bytes
                            .saturating_sub(dense_bytes.min(sparse_bytes)),
                        reference_pair_iterations: reference
                            .diagnostics
                            .polygon_aligned_reflex_candidate_pairs
                            .unwrap_or(0)
                            + reference
                                .diagnostics
                                .polygon_unaligned_reflex_pair_checks
                                .unwrap_or(0),
                        indexed_pair_iterations: indexed
                            .diagnostics
                            .polygon_aligned_reflex_candidate_pairs
                            .unwrap_or(0),
                        sweep_event_count: sweep
                            .diagnostics
                            .sweep_horizontal_event_count
                            .unwrap_or(0)
                            + sweep.diagnostics.sweep_vertical_event_count.unwrap_or(0),
                        sweep_status_operations: sweep
                            .diagnostics
                            .sweep_auxiliary_tree_operations
                            .unwrap_or(0),
                        sweep_output_record_count: sweep
                            .diagnostics
                            .sweep_output_horizontal_chords
                            .unwrap_or(0)
                            + sweep.diagnostics.sweep_output_vertical_chords.unwrap_or(0),
                        chord_families_equal,
                        optimum_equal,
                        cuts_equal,
                        rectangles_equal,
                        three_backend_equal,
                        reference_diagnostics: reference.diagnostics,
                        indexed_diagnostics: indexed.diagnostics,
                        sweep_diagnostics: sweep.diagnostics,
                        status: if verified {
                            "verified"
                        } else {
                            "counterexample"
                        }
                        .to_owned(),
                        message: None,
                    });
                }
                Err(message) => {
                    solver_errors += 1;
                    rows.push(PolygonScalingRow {
                        family,
                        family_name,
                        size,
                        status: "solver-error".to_owned(),
                        message: Some(message),
                        ..empty_scaling_row()
                    });
                }
            }
        }
    }
    let verified_rows = rows.iter().filter(|row| row.status == "verified").count();
    PolygonScalingReport {
        metadata: metadata(
            context,
            rows.len(),
            rows.len(),
            "boundary-native-polygon-scaling",
        ),
        rows,
        verified_rows,
        solver_errors,
        disagreements,
    }
}

#[allow(clippy::manual_let_else, clippy::single_match_else)]
fn compare_instance(instance: &AdversarialInstance, raster: bool, counts: &mut CampaignCounts) {
    counts.inputs += 1;
    let components = match instance.foreground_components() {
        Ok(components) => components,
        Err(_) => {
            counts.solver_errors += 1;
            return;
        }
    };
    for component in components {
        counts.components += 1;
        let Ok(boundary) = Boundary::from_component(&component) else {
            counts.solver_errors += 1;
            continue;
        };
        let Ok(polygon) = boundary.to_polygon() else {
            counts.rejected += 1;
            continue;
        };
        counts.supported += 1;
        compare_and_record(
            format!("{}-component-{}", instance.name, component.id.0),
            &polygon,
            raster,
            counts,
        );
    }
}

fn compare_and_record(
    name: String,
    polygon: &RectilinearPolygon,
    raster: bool,
    counts: &mut CampaignCounts,
) {
    match compare_polygon_backends(polygon, raster) {
        Ok(path_tree_compared) => {
            counts.verified += 1;
            counts.raster += usize::from(raster);
            counts.path_tree += usize::from(path_tree_compared);
        }
        Err((reason, reference, indexed, sweep)) => {
            counts.disagreements += 1;
            counts.counterexamples.push(PolygonCounterexample {
                name,
                original: polygon.clone(),
                minimized: polygon.clone(),
                reason,
                reference,
                indexed,
                sweep,
            });
        }
    }
}

type PolygonBackendMismatch = (
    String,
    Option<PolygonDissectionResult>,
    Option<PolygonDissectionResult>,
    Option<PolygonDissectionResult>,
);

#[allow(clippy::result_large_err, clippy::too_many_lines)]
fn compare_polygon_backends(
    polygon: &RectilinearPolygon,
    raster: bool,
) -> Result<bool, PolygonBackendMismatch> {
    let reference_prepared =
        PreparedPolygonContext::new_with_validator(polygon, polygon::Backend::Oracle)
            .map_err(|error| (error.to_string(), None, None, None))?;
    let indexed_prepared =
        PreparedPolygonContext::new_with_validator(polygon, polygon::Backend::Experiment)
            .map_err(|error| (error.to_string(), None, None, None))?;
    if reference_prepared.polygon() != indexed_prepared.polygon()
        || reference_prepared.boundary().reflex_vertices
            != indexed_prepared.boundary().reflex_vertices
    {
        return Err((
            format!(
                "normalized polygon or reflex vertices differ: reference_polygon={:?}; indexed_polygon={:?}; reference_reflex={:?}; indexed_reflex={:?}",
                reference_prepared.polygon(),
                indexed_prepared.polygon(),
                reference_prepared.boundary().reflex_vertices,
                indexed_prepared.boundary().reflex_vertices,
            ),
            None,
            None,
            None,
        ));
    }
    let reference_chords = sg_polygon::chord::oracle::Pairwise
        .enumerate_prepared_with_metrics(&reference_prepared)
        .map_err(|error| (error.to_string(), None, None, None))?;
    let indexed_chords = sg_polygon::chord::oracle::Indexed
        .enumerate_prepared(&indexed_prepared)
        .map_err(|error| (error.to_string(), None, None, None))?;
    let sweep_chords = sg_polygon::chord::experiment::Sweep
        .enumerate_prepared(&indexed_prepared)
        .map_err(|error| (error.to_string(), None, None, None))?;
    if reference_chords.families.horizontal != indexed_chords.families.horizontal
        || reference_chords.families.vertical != indexed_chords.families.vertical
        || reference_chords.families.horizontal != sweep_chords.families.horizontal
        || reference_chords.families.vertical != sweep_chords.families.vertical
    {
        return Err((
            format!(
                "effective chord families differ: reference={:?}; indexed={:?}; sweep={:?}",
                reference_chords.families, indexed_chords.families, sweep_chords.families
            ),
            None,
            None,
            None,
        ));
    }
    if indexed_chords.metrics.polygon_unaligned_reflex_pair_checks != 0
        || indexed_chords
            .metrics
            .polygon_definition7_full_boundary_scans
            != 0
    {
        return Err((
            "indexed chord contract counters are nonzero".to_owned(),
            None,
            None,
            None,
        ));
    }
    if sweep_chords.metrics.sweep_aligned_pair_iterations != 0
        || sweep_chords.metrics.sweep_all_pair_iterations != 0
        || sweep_chords.metrics.sweep_definition7_fallback_checks != 0
        || sweep_chords.metrics.sweep_full_boundary_scans != 0
        || sweep_chords.metrics.sweep_duplicate_output_count != 0
        || sweep_chords.metrics.sweep_output_horizontal_chords
            != sweep_chords.families.horizontal.len()
        || sweep_chords.metrics.sweep_output_vertical_chords != sweep_chords.families.vertical.len()
        || sweep_chords.sweep_certificate.is_none()
    {
        return Err((
            "sweep chord contract counters or certificate are invalid".to_owned(),
            None,
            None,
            None,
        ));
    }
    let reference_endpoints = EndpointIndex::new(
        reference_prepared.boundary_index(),
        &reference_chords.families.horizontal,
        &reference_chords.families.vertical,
    )
    .map_err(|error| (error.to_string(), None, None, None))?;
    let indexed_endpoints = EndpointIndex::new(
        indexed_prepared.boundary_index(),
        &indexed_chords.families.horizontal,
        &indexed_chords.families.vertical,
    )
    .map_err(|error| (error.to_string(), None, None, None))?;
    let sweep_endpoints = EndpointIndex::new(
        indexed_prepared.boundary_index(),
        &sweep_chords.families.horizontal,
        &sweep_chords.families.vertical,
    )
    .map_err(|error| (error.to_string(), None, None, None))?;
    if reference_endpoints != indexed_endpoints || reference_endpoints != sweep_endpoints {
        return Err(("endpoint tables differ".to_owned(), None, None, None));
    }
    let reference_clean = classify_clean_polygon(
        reference_prepared.polygon(),
        reference_prepared.boundary(),
        &reference_chords.families.horizontal,
        &reference_chords.families.vertical,
        &reference_endpoints,
    );
    let indexed_clean = classify_clean_polygon(
        indexed_prepared.polygon(),
        indexed_prepared.boundary(),
        &indexed_chords.families.horizontal,
        &indexed_chords.families.vertical,
        &indexed_endpoints,
    );
    let sweep_clean = classify_clean_polygon(
        indexed_prepared.polygon(),
        indexed_prepared.boundary(),
        &sweep_chords.families.horizontal,
        &sweep_chords.families.vertical,
        &sweep_endpoints,
    );
    if reference_clean != indexed_clean || reference_clean != sweep_clean {
        return Err((
            format!(
                "clean certificates differ: reference={reference_clean:?}; indexed={indexed_clean:?}; sweep={sweep_clean:?}"
            ),
            None,
            None,
            None,
        ));
    }
    let (reference, indexed, _, _) =
        solve_pair(polygon).map_err(|error| (error, None, None, None))?;
    let sweep = solve_polygon_with_options(polygon, sweep_options())
        .map_err(|error| (format!("sweep solve failed: {error}"), None, None, None))?;
    let horizontal_cuts =
        certificate_segments::<HorizontalCutSegment>(&sweep, "selected_horizontal_cuts")
            .and_then(|mut selected| {
                selected.extend(certificate_segments::<HorizontalCutSegment>(
                    &sweep,
                    "added_horizontal_cuts",
                )?);
                Ok(selected.into_iter().collect::<BTreeSet<_>>())
            })
            .map_err(|error| (error, None, None, None))?;
    let vertical_cuts =
        certificate_segments::<VerticalCutSegment>(&sweep, "selected_vertical_cuts")
            .and_then(|mut selected| {
                selected.extend(certificate_segments::<VerticalCutSegment>(
                    &sweep,
                    "added_vertical_cuts",
                )?);
                Ok(selected.into_iter().collect::<BTreeSet<_>>())
            })
            .map_err(|error| (error, None, None, None))?;
    let reference_subdivision = polygon_sparse::subdivision::Graph::with_backend(
        &indexed_prepared,
        &horizontal_cuts,
        &vertical_cuts,
        polygon_sparse::subdivision::Backend::Oracle,
    )
    .map_err(|error| (error.to_string(), None, None, None))?;
    let sweep_subdivision = polygon_sparse::subdivision::Graph::with_backend(
        &indexed_prepared,
        &horizontal_cuts,
        &vertical_cuts,
        polygon_sparse::subdivision::Backend::Experiment,
    )
    .map_err(|error| (error.to_string(), None, None, None))?;
    if reference_subdivision.split_junctions != sweep_subdivision.split_junctions
        || reference_subdivision.atomic_segments != sweep_subdivision.atomic_segments
        || reference_subdivision.half_edges != sweep_subdivision.half_edges
        || reference_subdivision.faces != sweep_subdivision.faces
        || reference_subdivision
            .recover_rectangles(indexed_prepared.polygon())
            .map_err(|error| error.to_string())
            != sweep_subdivision
                .recover_rectangles(indexed_prepared.polygon())
                .map_err(|error| error.to_string())
        || sweep_subdivision.metrics.candidate_pair_tests != 0
    {
        return Err((
            "reference and output-sensitive subdivisions differ".to_owned(),
            Some(reference),
            Some(indexed),
            Some(sweep),
        ));
    }
    let reference_validation = polygon_sparse::validator::Validator.validate_with_backend(
        indexed_prepared.polygon(),
        &sweep.rectangles,
        polygon_sparse::validator::Backend::Oracle,
    );
    let event_validation = polygon_sparse::validator::Validator.validate_with_backend(
        indexed_prepared.polygon(),
        &sweep.rectangles,
        polygon_sparse::validator::Backend::Experiment,
    );
    if reference_validation.as_ref().map(|_| ()) != event_validation.as_ref().map(|_| ())
        || event_validation.as_ref().is_ok_and(|metrics| {
            metrics.boundary_edge_scans != 0 || metrics.active_rectangle_resorts != 0
        })
    {
        return Err((
            "reference and event sparse validators differ".to_owned(),
            Some(reference),
            Some(indexed),
            Some(sweep),
        ));
    }
    let comparison_fields = [
        "horizontal_chords",
        "vertical_chords",
        "selected_horizontal",
        "selected_vertical",
        "selected_horizontal_cuts",
        "selected_vertical_cuts",
        "added_horizontal_cuts",
        "added_vertical_cuts",
        "flow_value",
        "representation",
    ];
    if reference.optimum_rectangle_count != indexed.optimum_rectangle_count
        || reference.rectangles != indexed.rectangles
        || reference.optimum_rectangle_count != sweep.optimum_rectangle_count
        || reference.rectangles != sweep.rectangles
        || comparison_fields
            .iter()
            .any(|field| certificate_field(&reference, field) != certificate_field(&indexed, field))
        || comparison_fields
            .iter()
            .any(|field| certificate_field(&reference, field) != certificate_field(&sweep, field))
    {
        return Err((
            "three-backend solver certificate, optimum, cuts, or rectangles differ".to_owned(),
            Some(reference),
            Some(indexed),
            Some(sweep),
        ));
    }
    for result in [&reference, &indexed, &sweep] {
        validate_polygon_dissection(indexed_prepared.polygon(), &result.rectangles).map_err(
            |error| {
                (
                    error.to_string(),
                    Some(reference.clone()),
                    Some(indexed.clone()),
                    Some(sweep.clone()),
                )
            },
        )?;
        validate_with_indexed_arrangement(&indexed_prepared, result).map_err(|error| {
            (
                error,
                Some(reference.clone()),
                Some(indexed.clone()),
                Some(sweep.clone()),
            )
        })?;
    }
    let diagnostics = &indexed.diagnostics;
    if diagnostics.polygon_prepare_build_count != Some(1)
        || diagnostics.polygon_normalization_count != Some(1)
        || diagnostics.polygon_validation_count != Some(1)
        || diagnostics.polygon_boundary_build_count != Some(1)
        || diagnostics.polygon_boundary_index_build_count != Some(1)
        || diagnostics.polygon_edge_index_build_count != Some(1)
        || diagnostics.polygon_unaligned_reflex_pair_checks != Some(0)
        || diagnostics.polygon_definition7_full_boundary_scans != Some(0)
        || diagnostics.polygon_completion_candidate_rebuilds != Some(0)
        || diagnostics.polygon_completion_full_boundary_scans != Some(0)
        || diagnostics.polygon_completion_full_cut_scans != Some(0)
        || diagnostics.polygon_validator_rectangle_cell_tests != Some(0)
    {
        return Err((
            "indexed production counters violate the v1.0 contract".to_owned(),
            Some(reference.clone()),
            Some(indexed.clone()),
            Some(sweep.clone()),
        ));
    }
    let sweep_diagnostics = &sweep.diagnostics;
    if sweep_diagnostics.polygon_chord_enumerator.as_deref() != Some("sg-sweep")
        || sweep_diagnostics.sweep_aligned_pair_iterations != Some(0)
        || sweep_diagnostics.sweep_all_pair_iterations != Some(0)
        || sweep_diagnostics.sweep_definition7_fallback_checks != Some(0)
        || sweep_diagnostics.sweep_full_boundary_scans != Some(0)
        || sweep_diagnostics.sweep_duplicate_output_count != Some(0)
        || sweep_diagnostics.sweep_output_horizontal_chords
            != Some(sweep_diagnostics.horizontal_chord_count)
        || sweep_diagnostics.sweep_output_vertical_chords
            != Some(sweep_diagnostics.vertical_chord_count)
    {
        return Err((
            "sweep production counters violate the v1.1 contract".to_owned(),
            Some(reference.clone()),
            Some(indexed.clone()),
            Some(sweep.clone()),
        ));
    }
    if raster {
        let report = verify_polygon(polygon, Some(RasterLimits::default())).map_err(|error| {
            (
                error.to_string(),
                Some(reference.clone()),
                Some(indexed.clone()),
                Some(sweep.clone()),
            )
        })?;
        if !report.verified() {
            return Err((
                report.disagreements.join("; "),
                Some(reference),
                Some(indexed),
                Some(sweep),
            ));
        }
    }
    Ok(reference_clean.eligible)
}

fn solve_pair(
    polygon: &RectilinearPolygon,
) -> Result<(PolygonDissectionResult, PolygonDissectionResult, u128, u128), String> {
    let reference_started = Instant::now();
    let reference = solve_polygon_with_options(polygon, reference_options())
        .map_err(|error| format!("reference solve failed: {error}"))?;
    let reference_microseconds = reference_started.elapsed().as_micros();
    let indexed_started = Instant::now();
    let indexed = solve_polygon_with_options(polygon, indexed_options())
        .map_err(|error| format!("indexed solve failed: {error}"))?;
    let indexed_microseconds = indexed_started.elapsed().as_micros();
    Ok((
        reference,
        indexed,
        reference_microseconds,
        indexed_microseconds,
    ))
}

fn solve_triple(
    polygon: &RectilinearPolygon,
) -> Result<
    (
        PolygonDissectionResult,
        PolygonDissectionResult,
        PolygonDissectionResult,
        u128,
        u128,
        u128,
    ),
    String,
> {
    let (reference, indexed, reference_microseconds, indexed_microseconds) = solve_pair(polygon)?;
    let sweep_started = Instant::now();
    let sweep = solve_polygon_with_options(polygon, sweep_options())
        .map_err(|error| format!("sweep solve failed: {error}"))?;
    Ok((
        reference,
        indexed,
        sweep,
        reference_microseconds,
        indexed_microseconds,
        sweep_started.elapsed().as_micros(),
    ))
}

fn solve_geometry_variants(
    polygon: &RectilinearPolygon,
) -> Result<
    (
        PolygonDissectionResult,
        PolygonDissectionResult,
        PolygonDissectionResult,
    ),
    String,
> {
    let mut dense_options = sweep_options();
    dense_options.recovery_backend = polygon_sparse::recovery::Backend::Oracle;
    dense_options.dissection_validator_backend = polygon_sparse::validation::Backend::Oracle;
    let dense = solve_polygon_with_options(polygon, dense_options)
        .map_err(|error| format!("dense geometry solve failed: {error}"))?;

    let mut reference_sparse_options = sweep_options();
    reference_sparse_options.subdivision_builder_backend =
        polygon_sparse::subdivision::Backend::Oracle;
    reference_sparse_options.sparse_validator_backend = polygon_sparse::validator::Backend::Oracle;
    let reference_sparse = solve_polygon_with_options(polygon, reference_sparse_options)
        .map_err(|error| format!("reference sparse geometry solve failed: {error}"))?;

    let mut auto_options = sweep_options();
    auto_options.recovery_backend = polygon_sparse::recovery::Backend::Auto;
    let auto = solve_polygon_with_options(polygon, auto_options)
        .map_err(|error| format!("auto recovery solve failed: {error}"))?;
    Ok((dense, reference_sparse, auto))
}

fn phase_time(result: &PolygonDissectionResult, phase: &str) -> u128 {
    result
        .diagnostics
        .phase_microseconds
        .get(phase)
        .copied()
        .unwrap_or(0)
}

const fn reference_options() -> PolygonSolveOptions {
    PolygonSolveOptions {
        verification_mode: Verification::CompactOnly,
        geometry_backend: mrd_domain::PolygonGeometryBackend::ReferenceScan,
        validation_backend: polygon::Backend::Oracle,
        chord_backend: PolygonChordBackend::ReferencePairwise,
        completion_backend: PolygonCompletionBackend::CoordinateReference,
        cut_index_backend: polygon_cut_index::Backend::Oracle,
        recovery_backend: polygon_sparse::recovery::Backend::Oracle,
        dissection_validator_backend: polygon_sparse::validation::Backend::Oracle,
        subdivision_builder_backend: polygon_sparse::subdivision::Backend::Oracle,
        sparse_validator_backend: polygon_sparse::validator::Backend::Oracle,
        arrangement_backend: PolygonArrangementBackend::Reference,
        representation: Representation::Auto,
    }
}

const fn indexed_options() -> PolygonSolveOptions {
    PolygonSolveOptions {
        verification_mode: Verification::CompactOnly,
        geometry_backend: mrd_domain::PolygonGeometryBackend::Indexed,
        validation_backend: polygon::Backend::Experiment,
        chord_backend: PolygonChordBackend::IndexedPairwise,
        completion_backend: PolygonCompletionBackend::IndexedFrontier,
        cut_index_backend: polygon_cut_index::Backend::Experiment,
        recovery_backend: polygon_sparse::recovery::Backend::Experiment,
        dissection_validator_backend: polygon_sparse::validation::Backend::Experiment,
        subdivision_builder_backend: polygon_sparse::subdivision::Backend::Experiment,
        sparse_validator_backend: polygon_sparse::validator::Backend::Experiment,
        arrangement_backend: PolygonArrangementBackend::Indexed,
        representation: Representation::Auto,
    }
}

const fn sweep_options() -> PolygonSolveOptions {
    PolygonSolveOptions {
        chord_backend: PolygonChordBackend::SoltanGorpinevichSweep,
        ..indexed_options()
    }
}

fn validate_with_indexed_arrangement(
    prepared: &PreparedPolygonContext,
    result: &PolygonDissectionResult,
) -> Result<(), String> {
    let horizontal =
        certificate_segments::<HorizontalCutSegment>(result, "selected_horizontal_cuts")?
            .into_iter()
            .chain(certificate_segments(result, "added_horizontal_cuts")?)
            .collect::<BTreeSet<_>>();
    let vertical = certificate_segments::<VerticalCutSegment>(result, "selected_vertical_cuts")?
        .into_iter()
        .chain(certificate_segments(result, "added_vertical_cuts")?)
        .collect::<BTreeSet<_>>();
    let arrangement = polygon_arrangement::Arrangement::new(prepared, &horizontal, &vertical)
        .map_err(|error| error.to_string())?;
    polygon_arrangement::experiment::Validator
        .validate(&arrangement, prepared.polygon(), &result.rectangles)
        .map_err(|error| error.to_string())
}

fn certificate_segments<T>(result: &PolygonDissectionResult, key: &str) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(
        certificate_field(result, key)
            .cloned()
            .ok_or_else(|| format!("missing certificate field {key}"))?,
    )
    .map_err(|error| error.to_string())
}

fn certificate_field<'a>(
    result: &'a PolygonDissectionResult,
    key: &str,
) -> Option<&'a serde_json::Value> {
    result
        .certificate
        .as_ref()
        .and_then(|certificate| certificate.payload.get(key))
}

fn certificate_array_len(result: &PolygonDissectionResult, key: &str) -> usize {
    certificate_field(result, key)
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len)
}

fn campaign_report(
    context: BenchmarkContext,
    population: String,
    counts: CampaignCounts,
) -> PolygonCampaignReport {
    PolygonCampaignReport {
        metadata: metadata(
            context,
            counts.inputs,
            counts.components,
            "boundary-native-ordinary-rectilinear-polygon-differential",
        ),
        population,
        input_count: counts.inputs,
        component_count: counts.components,
        supported_components: counts.supported,
        model_rejections: counts.rejected,
        verified_components: counts.verified,
        solver_errors: counts.solver_errors,
        timeouts: 0,
        disagreements: counts.disagreements,
        raster_oracle_comparisons: counts.raster,
        path_tree_comparisons: counts.path_tree,
        minimized_counterexamples: counts.counterexamples,
    }
}

fn metadata(
    context: BenchmarkContext,
    input_count: usize,
    component_count: usize,
    input_model: &str,
) -> BenchmarkMetadata {
    BenchmarkMetadata {
        git_commit: context.git_commit,
        rustc_version: context.rustc_version,
        command: context.command,
        seed: context.seed,
        timestamp: context.timestamp,
        input_count,
        component_count,
        input_model: input_model.to_owned(),
        unsupported_input_features: UNSUPPORTED_FEATURES.map(str::to_owned).to_vec(),
    }
}

/// Returns the deterministic polygon-native scaling families A-H.
///
/// # Errors
///
/// Returns a string describing arithmetic overflow or an invalid generated
/// polygon.
pub fn native_polygon_families(
    size: usize,
) -> Result<Vec<(String, String, RectilinearPolygon)>, String> {
    let size = size.max(1);
    let boundary_heavy = varying_top_notches(size, 1)?;
    let aligned_heavy = four_sided_notches(size, false, 1)?;
    let hole_heavy = hole_row(size, false, 1)?;
    let completion_heavy = four_sided_notches(size, true, 1)?;
    let arrangement_heavy = varying_top_notches(size.saturating_mul(2), 1)?;
    let huge_coordinate = four_sided_notches(size.min(8), true, 1_000_000_000_000)?;
    let clean_path_tree = four_sided_notches(size, true, 3)?;
    let non_clean_fallback = hole_row(size, true, 2)?;
    let range_dense_intersection_sparse = four_sided_notches(size, false, 2)?;
    let intersection_dense = four_sided_notches(size, true, 2)?;
    let validator_active_heavy = hole_row(size, false, 2)?;
    let validator_boundary_heavy = varying_top_notches(size.saturating_mul(3), 2)?;
    let sparse_tree_node_heavy = hole_row(size.saturating_mul(2), true, 3)?;
    let dense_sparse_crossover = varying_top_notches(size.saturating_mul(2), 3)?;
    Ok(vec![
        (
            "A".to_owned(),
            "staircase-sparse".to_owned(),
            boundary_heavy,
        ),
        (
            "B".to_owned(),
            "many-coordinates-few-faces".to_owned(),
            arrangement_heavy,
        ),
        (
            "C".to_owned(),
            "hole-coordinate-cross-product".to_owned(),
            hole_heavy,
        ),
        (
            "D".to_owned(),
            "completion-heavy".to_owned(),
            completion_heavy,
        ),
        (
            "E".to_owned(),
            "sparse-path-tree".to_owned(),
            clean_path_tree,
        ),
        (
            "F".to_owned(),
            "4d-fallback-sparse".to_owned(),
            non_clean_fallback,
        ),
        (
            "G".to_owned(),
            "aligned-reflex-heavy".to_owned(),
            aligned_heavy,
        ),
        (
            "H".to_owned(),
            "huge-coordinate".to_owned(),
            huge_coordinate,
        ),
        (
            "v1.3-A".to_owned(),
            "range-dense-intersection-sparse".to_owned(),
            range_dense_intersection_sparse,
        ),
        (
            "v1.3-B".to_owned(),
            "intersection-dense".to_owned(),
            intersection_dense,
        ),
        (
            "v1.3-C".to_owned(),
            "validator-active-heavy".to_owned(),
            validator_active_heavy,
        ),
        (
            "v1.3-D".to_owned(),
            "validator-boundary-heavy".to_owned(),
            validator_boundary_heavy,
        ),
        (
            "v1.3-E".to_owned(),
            "sparse-tree-node-heavy".to_owned(),
            sparse_tree_node_heavy,
        ),
        (
            "v1.3-F".to_owned(),
            "dense-sparse-crossover".to_owned(),
            dense_sparse_crossover,
        ),
    ])
}

fn varying_top_notches(size: usize, scale: i64) -> Result<RectilinearPolygon, String> {
    let width = i64::try_from(size)
        .ok()
        .and_then(|value| value.checked_mul(4))
        .and_then(|value| value.checked_add(6))
        .and_then(|value| value.checked_mul(scale))
        .ok_or_else(|| "boundary-heavy coordinate overflow".to_owned())?;
    let height = i64::try_from(size)
        .ok()
        .and_then(|value| value.checked_add(4))
        .and_then(|value| value.checked_mul(scale))
        .ok_or_else(|| "boundary-heavy coordinate overflow".to_owned())?;
    let mut vertices = vec![
        Point::new(0, 0),
        Point::new(width, 0),
        Point::new(width, height),
    ];
    for index in (0..size).rev() {
        let left = i64::try_from(index * 4 + 2).map_err(|error| error.to_string())? * scale;
        let right = left + scale;
        let depth = i64::try_from(index + 1).map_err(|error| error.to_string())? * scale;
        vertices.extend([
            Point::new(right, height),
            Point::new(right, height - depth),
            Point::new(left, height - depth),
            Point::new(left, height),
        ]);
    }
    vertices.push(Point::new(0, height));
    RectilinearPolygon::new(OrthogonalLoop::new(vertices), vec![])
        .map_err(|error| error.to_string())
}

#[allow(clippy::similar_names)]
fn four_sided_notches(
    size: usize,
    varying_depth: bool,
    scale: i64,
) -> Result<RectilinearPolygon, String> {
    let margin = size + 4;
    let side = size
        .checked_mul(5)
        .and_then(|value| value.checked_add(margin * 2 + 2))
        .ok_or_else(|| "four-sided family dimension overflow".to_owned())?;
    let side = i64::try_from(side).map_err(|error| error.to_string())? * scale;
    let mut vertices = vec![Point::new(0, 0)];
    for index in 0..size {
        let start = i64::try_from(margin + index * 5).map_err(|error| error.to_string())? * scale;
        let end = start + scale;
        let depth = i64::try_from(if varying_depth { index + 2 } else { 2 })
            .map_err(|error| error.to_string())?
            * scale;
        vertices.extend([
            Point::new(start, 0),
            Point::new(start, depth),
            Point::new(end, depth),
            Point::new(end, 0),
        ]);
    }
    vertices.push(Point::new(side, 0));
    for index in 0..size {
        let start = i64::try_from(margin + index * 5).map_err(|error| error.to_string())? * scale;
        let end = start + scale;
        let depth = i64::try_from(if varying_depth { size - index + 1 } else { 2 })
            .map_err(|error| error.to_string())?
            * scale;
        vertices.extend([
            Point::new(side, start),
            Point::new(side - depth, start),
            Point::new(side - depth, end),
            Point::new(side, end),
        ]);
    }
    vertices.push(Point::new(side, side));
    for index in (0..size).rev() {
        let start = i64::try_from(margin + index * 5).map_err(|error| error.to_string())? * scale;
        let end = start + scale;
        let depth = i64::try_from(if varying_depth { index + 2 } else { 2 })
            .map_err(|error| error.to_string())?
            * scale;
        vertices.extend([
            Point::new(end, side),
            Point::new(end, side - depth),
            Point::new(start, side - depth),
            Point::new(start, side),
        ]);
    }
    vertices.push(Point::new(0, side));
    for index in (0..size).rev() {
        let start = i64::try_from(margin + index * 5).map_err(|error| error.to_string())? * scale;
        let end = start + scale;
        let depth = i64::try_from(if varying_depth { size - index + 1 } else { 2 })
            .map_err(|error| error.to_string())?
            * scale;
        vertices.extend([
            Point::new(0, end),
            Point::new(depth, end),
            Point::new(depth, start),
            Point::new(0, start),
        ]);
    }
    RectilinearPolygon::new(OrthogonalLoop::new(vertices), vec![])
        .map_err(|error| error.to_string())
}

fn hole_row(size: usize, staggered: bool, scale: i64) -> Result<RectilinearPolygon, String> {
    let width = i64::try_from(size)
        .ok()
        .and_then(|value| value.checked_mul(5))
        .and_then(|value| value.checked_add(6))
        .and_then(|value| value.checked_mul(scale))
        .ok_or_else(|| "hole family coordinate overflow".to_owned())?;
    let height = i64::try_from(size)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .and_then(|value| value.checked_add(10))
        .and_then(|value| value.checked_mul(scale))
        .ok_or_else(|| "hole family coordinate overflow".to_owned())?;
    let outer = rectangle_loop(0, 0, width, height, false);
    let mut holes = Vec::with_capacity(size);
    for index in 0..size {
        let left = i64::try_from(index * 5 + 3).map_err(|error| error.to_string())? * scale;
        let bottom = i64::try_from(if staggered { index * 2 + 3 } else { 3 })
            .map_err(|error| error.to_string())?
            * scale;
        holes.push(rectangle_loop(
            left,
            bottom,
            left + 2 * scale,
            bottom + 2 * scale,
            true,
        ));
    }
    RectilinearPolygon::new(outer, holes).map_err(|error| error.to_string())
}

fn rectangle_loop(left: i64, bottom: i64, right: i64, top: i64, clockwise: bool) -> OrthogonalLoop {
    let points = if clockwise {
        vec![
            Point::new(left, bottom),
            Point::new(left, top),
            Point::new(right, top),
            Point::new(right, bottom),
        ]
    } else {
        vec![
            Point::new(left, bottom),
            Point::new(right, bottom),
            Point::new(right, top),
            Point::new(left, top),
        ]
    };
    OrthogonalLoop::new(points)
}

fn metamorphic_polygons(polygon: &RectilinearPolygon) -> Vec<(&'static str, RectilinearPolygon)> {
    [
        ("translate", 17_i64, -23_i64, 2_i64, 3_i64),
        ("stretch", 0, 0, 3, 5),
        ("reflect-180", 0, 0, -1, -1),
    ]
    .into_iter()
    .filter_map(|(name, dx, dy, sx, sy)| {
        transform_polygon(polygon, dx, dy, sx, sy)
            .ok()
            .map(|polygon| (name, polygon))
    })
    .collect()
}

fn transform_polygon(
    polygon: &RectilinearPolygon,
    dx: i64,
    dy: i64,
    sx: i64,
    sy: i64,
) -> Result<RectilinearPolygon, String> {
    let transform_loop = |boundary_loop: &OrthogonalLoop| {
        boundary_loop
            .vertices
            .iter()
            .map(|point| {
                Some(Point::new(
                    point.x.checked_mul(sx)?.checked_add(dx)?,
                    point.y.checked_mul(sy)?.checked_add(dy)?,
                ))
            })
            .collect::<Option<Vec<_>>>()
            .map(OrthogonalLoop::new)
            .ok_or_else(|| "metamorphic coordinate overflow".to_owned())
    };
    RectilinearPolygon::new(
        transform_loop(&polygon.outer)?,
        polygon
            .holes
            .iter()
            .map(transform_loop)
            .collect::<Result<Vec<_>, _>>()?,
    )
    .map_err(|error| error.to_string())
}

fn negative_polygons() -> Vec<(&'static str, RectilinearPolygon)> {
    vec![
        (
            "non-axis-aligned",
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 1),
                    Point::new(4, 4),
                    Point::new(0, 4),
                ]),
                holes: vec![],
            },
        ),
        (
            "zero-length",
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(4, 4),
                    Point::new(0, 4),
                    Point::new(0, 4),
                ]),
                holes: vec![],
            },
        ),
        (
            "too-few-vertices",
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(4, 4),
                ]),
                holes: vec![],
            },
        ),
        (
            "self-intersection",
            RectilinearPolygon {
                outer: OrthogonalLoop::new(vec![
                    Point::new(0, 0),
                    Point::new(4, 0),
                    Point::new(4, 4),
                    Point::new(1, 4),
                    Point::new(1, -1),
                    Point::new(0, -1),
                ]),
                holes: vec![],
            },
        ),
        (
            "hole-outside",
            RectilinearPolygon {
                outer: rectangle_loop(0, 0, 10, 10, false),
                holes: vec![rectangle_loop(12, 2, 14, 4, true)],
            },
        ),
        (
            "hole-contact",
            RectilinearPolygon {
                outer: rectangle_loop(0, 0, 10, 10, false),
                holes: vec![rectangle_loop(0, 2, 4, 4, true)],
            },
        ),
        (
            "hole-intersection",
            RectilinearPolygon {
                outer: rectangle_loop(0, 0, 20, 20, false),
                holes: vec![
                    rectangle_loop(2, 2, 8, 8, true),
                    rectangle_loop(6, 6, 12, 12, true),
                ],
            },
        ),
        (
            "nested-hole",
            RectilinearPolygon {
                outer: rectangle_loop(0, 0, 20, 20, false),
                holes: vec![
                    rectangle_loop(2, 2, 12, 12, true),
                    rectangle_loop(4, 4, 6, 6, true),
                ],
            },
        ),
        (
            "wrong-orientation",
            RectilinearPolygon {
                outer: rectangle_loop(0, 0, 10, 10, true),
                holes: vec![],
            },
        ),
    ]
}

fn invalid_rectangle_sets() -> Vec<(&'static str, Vec<CoordinateRect>)> {
    vec![
        (
            "rectangles-non-positive",
            vec![CoordinateRect {
                x0: 0,
                y0: 0,
                x1: 0,
                y1: 4,
            }],
        ),
        (
            "rectangles-area-mismatch",
            vec![CoordinateRect::new(0, 0, 3, 4).expect("positive rectangle")],
        ),
        (
            "rectangles-outside",
            vec![CoordinateRect::new(-1, 0, 3, 4).expect("positive rectangle")],
        ),
        (
            "rectangles-overlap",
            vec![
                CoordinateRect::new(0, 0, 3, 4).expect("positive rectangle"),
                CoordinateRect::new(2, 0, 3, 4).expect("positive rectangle"),
            ],
        ),
    ]
}

fn dissection_error_category(error: &PolygonValidationError) -> String {
    match error {
        PolygonValidationError::Polygon(_) => "polygon".to_owned(),
        PolygonValidationError::DeclaredCount { .. } => "declared-count".to_owned(),
        PolygonValidationError::NonPositiveRectangle { .. } => "non-positive-rectangle".to_owned(),
        PolygonValidationError::OutsidePolygon { .. } => "outside-polygon".to_owned(),
        PolygonValidationError::Overlap { .. } => "overlap".to_owned(),
        PolygonValidationError::UncoveredInterior { .. } => "uncovered-interior".to_owned(),
        PolygonValidationError::AreaMismatch { .. } => "area-mismatch".to_owned(),
        PolygonValidationError::AreaOverflow => "area-overflow".to_owned(),
    }
}

fn validator_category(polygon: &RectilinearPolygon, backend: polygon::Backend) -> String {
    PreparedPolygonContext::new_with_validator(polygon, backend).map_or_else(
        |error| match error {
            mrd_domain::PreparedPolygonError::Polygon(error) => {
                format!("{:?}", PolygonErrorCategory::from_error(&error))
            }
            mrd_domain::PreparedPolygonError::BoundaryIndex(_) => "BoundaryIndex".to_owned(),
        },
        |_| "Accepted".to_owned(),
    )
}

fn random_connected_instance(case: usize) -> AdversarialInstance {
    let width = 6 + case % 7;
    let height = 6 + (case / 7) % 7;
    let target = 1 + case % (width * height / 2).max(1);
    let mut state = (case as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut occupied = BTreeSet::from([(width / 2, height / 2)]);
    while occupied.len() < target {
        state = splitmix64(state);
        let index = usize::try_from(state % occupied.len() as u64).unwrap_or(0);
        let &(x, y) = occupied.iter().nth(index).unwrap_or(&(0, 0));
        state = splitmix64(state);
        let candidate = match state & 3 {
            0 if x + 1 < width => Some((x + 1, y)),
            1 if x > 0 => Some((x - 1, y)),
            2 if y + 1 < height => Some((x, y + 1)),
            3 if y > 0 => Some((x, y - 1)),
            _ => None,
        };
        if let Some(candidate) = candidate {
            occupied.insert(candidate);
        }
    }
    let mut cells = vec![false; width * height];
    for (x, y) in occupied {
        cells[y * width + x] = true;
    }
    AdversarialInstance {
        name: format!("polygon-random-{case}"),
        family: "deterministic-random-connected".to_owned(),
        width,
        height,
        cells,
        parameters: BTreeMap::from([("seed".to_owned(), case)]),
    }
}

const fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = state;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

fn error_scaling_row(size: usize, message: String) -> PolygonScalingRow {
    PolygonScalingRow {
        size,
        status: "generator-error".to_owned(),
        message: Some(message),
        ..empty_scaling_row()
    }
}

fn empty_scaling_row() -> PolygonScalingRow {
    PolygonScalingRow {
        family: String::new(),
        family_name: String::new(),
        size: 0,
        boundary_complexity: 0,
        hole_count: 0,
        reflex_count: 0,
        aligned_candidate_count: 0,
        chord_count: 0,
        selected_horizontal_cut_count: 0,
        selected_vertical_cut_count: 0,
        added_horizontal_cut_count: 0,
        added_vertical_cut_count: 0,
        coordinate_x_count: 0,
        coordinate_y_count: 0,
        coordinate_cartesian_product: 0,
        sparse_subdivision_vertices: 0,
        sparse_subdivision_half_edges: 0,
        sparse_subdivision_junctions: 0,
        sparse_subdivision_interior_faces: 0,
        dense_owned_bytes_estimate: 0,
        sparse_owned_bytes_estimate: 0,
        cut_index_owned_bytes_estimate: 0,
        completion_microseconds: 0,
        recovery_microseconds: 0,
        validation_microseconds: 0,
        reference_microseconds: 0,
        indexed_microseconds: 0,
        sweep_microseconds: 0,
        subdivision_input_segment_count: 0,
        subdivision_reported_intersections: 0,
        reference_subdivision_candidate_pair_tests: 0,
        sweep_subdivision_candidate_pair_tests: 0,
        reference_subdivision_recovery_microseconds: 0,
        sweep_subdivision_recovery_microseconds: 0,
        dense_recovery_microseconds: 0,
        reference_validator_microseconds: 0,
        event_validator_microseconds: 0,
        dense_validator_microseconds: 0,
        reference_validator_boundary_edge_scans: 0,
        reference_validator_active_rectangle_resorts: 0,
        event_validator_boundary_edge_scans: 0,
        event_validator_active_rectangle_resorts: 0,
        sparse_materialized_tree_nodes: 0,
        sparse_logical_tree_nodes: 0,
        geometry_backends_equal: false,
        auto_selected_backend: String::new(),
        actual_fastest_recovery_backend: String::new(),
        auto_time_regret_microseconds: 0,
        auto_memory_regret_bytes: 0,
        candidate_output_ratio_numerator: 0,
        candidate_output_ratio_denominator: 0,
        reference_pair_iterations: 0,
        indexed_pair_iterations: 0,
        sweep_event_count: 0,
        sweep_status_operations: 0,
        sweep_output_record_count: 0,
        chord_families_equal: false,
        optimum_equal: false,
        cuts_equal: false,
        rectangles_equal: false,
        three_backend_equal: false,
        reference_diagnostics: Diagnostics::default(),
        indexed_diagnostics: Diagnostics::default(),
        sweep_diagnostics: Diagnostics::default(),
        status: String::new(),
        message: None,
    }
}

fn dense_arrangement_owned_bytes_estimate(x_count: usize, y_count: usize) -> usize {
    let width = x_count.saturating_sub(1);
    let height = y_count.saturating_sub(1);
    let coordinates = x_count
        .saturating_add(y_count)
        .saturating_mul(std::mem::size_of::<i64>());
    let occupancy_and_barriers = width
        .saturating_mul(height)
        .saturating_add(y_count.saturating_mul(width))
        .saturating_add(x_count.saturating_mul(height));
    let coverage_difference = x_count
        .saturating_mul(y_count)
        .saturating_mul(std::mem::size_of::<i64>());
    coordinates
        .saturating_add(occupancy_and_barriers)
        .saturating_add(coverage_difference)
}

fn optional_usize(value: Option<usize>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn join_sizes(sizes: &[usize]) -> String {
    sizes
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::{
        BenchmarkContext, native_polygon_families, polygon_negative_campaign,
        polygon_scaling_campaign,
    };

    fn context(command: &str) -> BenchmarkContext {
        BenchmarkContext {
            git_commit: "test".to_owned(),
            rustc_version: "test".to_owned(),
            command: command.to_owned(),
            seed: Some(42),
            timestamp: 0,
        }
    }

    #[test]
    fn all_native_scaling_families_are_valid_and_distinct() {
        let families = native_polygon_families(2).unwrap();
        assert_eq!(families.len(), 14);
        for (_, _, polygon) in families {
            assert!(polygon.boundary_complexity() >= 4);
        }
    }

    #[test]
    fn negative_validator_campaign_is_exact() {
        let report = polygon_negative_campaign(context("polygon-negative"));
        assert!(report.verified(), "{:#?}", report.records);
    }

    #[test]
    fn small_scaling_campaign_matches_backends() {
        let report = polygon_scaling_campaign(context("polygon-scaling"), &[1]);
        assert!(report.verified(), "{:#?}", report.rows);
        assert_eq!(report.verified_rows, 14);
    }

    #[test]
    fn candidate_gap_rows_bound_sweep_work_by_boundary_events() {
        let report = polygon_scaling_campaign(context("polygon-sweep-scaling"), &[8]);
        assert!(report.verified(), "{:#?}", report.rows);
        let bidirectional = report.rows.iter().find(|row| row.family == "G").unwrap();
        let ordinary_hole = report.rows.iter().find(|row| row.family == "C").unwrap();
        for row in [bidirectional, ordinary_hole] {
            assert!(row.aligned_candidate_count > row.chord_count);
            assert_eq!(
                row.candidate_output_ratio_numerator,
                row.aligned_candidate_count
            );
            assert_eq!(
                row.candidate_output_ratio_denominator,
                row.chord_count.max(1)
            );
            assert_eq!(row.sweep_output_record_count, row.chord_count);
            assert!(
                row.sweep_event_count <= 2 * (row.boundary_complexity + row.reflex_count),
                "row={row:#?}"
            );
            assert_eq!(row.sweep_diagnostics.sweep_aligned_pair_iterations, Some(0));
            assert_eq!(row.sweep_diagnostics.sweep_all_pair_iterations, Some(0));
            assert_eq!(
                row.sweep_diagnostics.sweep_definition7_fallback_checks,
                Some(0)
            );
            assert_eq!(row.sweep_diagnostics.sweep_full_boundary_scans, Some(0));
        }
        assert!(bidirectional.reference_diagnostics.horizontal_chord_count > 0);
        assert!(bidirectional.reference_diagnostics.vertical_chord_count > 0);
        assert!(ordinary_hole.hole_count > 0);
    }
}
