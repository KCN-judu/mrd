use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CoordinateRect, GridRect};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExactRatio {
    pub numerator: u128,
    pub denominator: u128,
}

impl ExactRatio {
    #[must_use]
    pub const fn new(numerator: u128, denominator: u128) -> Option<Self> {
        if denominator == 0 {
            None
        } else {
            Some(Self {
                numerator,
                denominator,
            })
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct ExecutionTrace {
    pub pairwise_embedding_audit_called: bool,
    pub explicit_conflict_graph_built: bool,
    pub hopcroft_karp_called: bool,
    pub c0_partition_built: bool,
    pub full_edge_partition_audit_called: bool,
    pub compact_structure_check_called: bool,
    /// True only when the audited path-tree oracle materializes every tree
    /// edge traversed by every path chord.
    pub full_tree_path_edge_lists_materialized: bool,
    /// True only for the independent reference path reconstruction oracle.
    pub per_path_bfs_called: bool,
    /// True only for the area-sensitive reference region dual.
    pub area_flood_fill_dual_built: bool,
    /// True only when geometric chords are expanded into unit cuts by the
    /// path-tree dual builder.
    pub unit_chord_cuts_materialized: bool,
    /// True when a prepared occupancy context is copied/transposed.
    pub prepared_occupancy_transposed: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostics {
    #[serde(default)]
    pub input_model: Option<String>,
    #[serde(default)]
    pub polygon_outer_vertices: Option<usize>,
    #[serde(default)]
    pub polygon_hole_count: Option<usize>,
    #[serde(default)]
    pub polygon_hole_vertices: Option<usize>,
    #[serde(default)]
    pub polygon_validation_backend: Option<String>,
    #[serde(default)]
    pub polygon_chord_enumerator: Option<String>,
    #[serde(default)]
    pub coordinate_compression_x_count: Option<usize>,
    #[serde(default)]
    pub coordinate_compression_y_count: Option<usize>,
    #[serde(default)]
    pub atomic_cell_count: Option<usize>,
    #[serde(default)]
    pub polygon_completion_backend: Option<String>,
    #[serde(default)]
    pub polygon_validator_backend: Option<String>,
    #[serde(default)]
    pub polygon_geometry_backend: Option<String>,
    #[serde(default)]
    pub polygon_arrangement_backend: Option<String>,
    #[serde(default)]
    pub polygon_prepare_build_count: Option<usize>,
    #[serde(default)]
    pub polygon_normalization_count: Option<usize>,
    #[serde(default)]
    pub polygon_validation_count: Option<usize>,
    #[serde(default)]
    pub polygon_boundary_build_count: Option<usize>,
    #[serde(default)]
    pub polygon_boundary_index_build_count: Option<usize>,
    #[serde(default)]
    pub polygon_edge_index_build_count: Option<usize>,
    #[serde(default)]
    pub polygon_prepare_microseconds: Option<u128>,
    #[serde(default)]
    pub polygon_prepare_owned_bytes: Option<usize>,
    #[serde(default)]
    pub polygon_boundary_edge_visits: Option<usize>,
    #[serde(default)]
    pub polygon_point_location_queries: Option<usize>,
    #[serde(default)]
    pub polygon_segment_reporting_queries: Option<usize>,
    #[serde(default)]
    pub polygon_reported_boundary_intersections: Option<usize>,
    #[serde(default)]
    pub polygon_aligned_reflex_candidate_pairs: Option<usize>,
    #[serde(default)]
    pub polygon_unaligned_reflex_pair_checks: Option<usize>,
    #[serde(default)]
    pub polygon_definition7_full_boundary_scans: Option<usize>,
    #[serde(default)]
    pub polygon_completion_candidate_rebuilds: Option<usize>,
    #[serde(default)]
    pub polygon_completion_cut_pair_tests: Option<usize>,
    #[serde(default)]
    pub polygon_completion_intersections_reported: Option<usize>,
    #[serde(default)]
    pub polygon_completion_candidate_insertions: Option<usize>,
    #[serde(default)]
    pub polygon_completion_candidate_revalidations: Option<usize>,
    #[serde(default)]
    pub polygon_completion_stale_candidates: Option<usize>,
    #[serde(default)]
    pub polygon_completion_boundary_ray_queries: Option<usize>,
    #[serde(default)]
    pub polygon_completion_cut_ray_queries: Option<usize>,
    #[serde(default)]
    pub polygon_completion_full_boundary_scans: Option<usize>,
    #[serde(default)]
    pub polygon_completion_full_cut_scans: Option<usize>,
    #[serde(default)]
    pub polygon_arrangement_point_location_queries: Option<usize>,
    #[serde(default)]
    pub polygon_arrangement_boundary_edge_visits: Option<usize>,
    #[serde(default)]
    pub polygon_arrangement_span_writes: Option<usize>,
    #[serde(default)]
    pub polygon_validator_rectangle_cell_tests: Option<usize>,
    #[serde(default)]
    pub raster_oracle_used: Option<bool>,
    pub cell_count: usize,
    pub boundary_complexity: usize,
    pub outer_loop_count: usize,
    pub hole_count: usize,
    pub reflex_vertex_count: usize,
    pub horizontal_chord_count: usize,
    pub vertical_chord_count: usize,
    pub total_chord_count: usize,
    pub explicit_conflict_edge_count: Option<usize>,
    pub conflict_edge_density: Option<ExactRatio>,
    pub biclique_count: usize,
    pub biclique_total_vertex_occurrences: usize,
    pub biclique_size_per_chord: Option<ExactRatio>,
    pub biclique_size_per_explicit_edge: Option<ExactRatio>,
    pub c0_network_vertex_count: usize,
    pub c0_network_arc_count: usize,
    pub compressed_network_vertex_count: usize,
    pub compressed_network_arc_count: usize,
    pub maximum_matching_size: usize,
    pub minimum_vertex_cover_size: usize,
    pub output_rectangle_count: usize,
    pub phase_microseconds: BTreeMap<String, u128>,
    pub peak_memory_bytes: Option<usize>,
    pub execution_trace: ExecutionTrace,
    pub effective_chord_enumerator: Option<String>,
    pub effective_chord_enumeration_microseconds: Option<u128>,
    pub horizontal_interior_run_count: Option<usize>,
    pub vertical_interior_run_count: Option<usize>,
    pub candidate_reflex_pair_count: Option<usize>,
    pub emitted_chord_count: Option<usize>,
    pub completion_backend: Option<String>,
    pub selected_chord_cut_materialization_microseconds: Option<u128>,
    pub horizontal_simple_chord_completion_microseconds: Option<u128>,
    pub vertical_simple_chord_completion_microseconds: Option<u128>,
    pub rectangle_recovery_microseconds: Option<u128>,
    pub final_output_validation_microseconds: Option<u128>,
    pub initial_horizontal_unit_cut_count: Option<usize>,
    pub initial_vertical_unit_cut_count: Option<usize>,
    pub added_horizontal_unit_cut_count: Option<usize>,
    pub added_vertical_unit_cut_count: Option<usize>,
    pub horizontal_simple_chord_count: Option<usize>,
    pub vertical_simple_chord_count: Option<usize>,
    pub completion_candidate_queries: Option<usize>,
    pub completion_full_grid_scans: Option<usize>,
    pub completion_candidate_revalidations: Option<usize>,
    pub completion_stale_candidates: Option<usize>,
    pub completion_ray_extension_unit_steps: Option<usize>,
    pub rectangle_recovery_component_visits: Option<usize>,
    pub rectangle_recovery_queue_pushes: Option<usize>,
    pub rectangle_recovery_region_count: Option<usize>,
    pub rectangle_recovery_allocations: Option<usize>,
    pub prepared_component_build_count: Option<usize>,
    pub prepared_component_build_microseconds: Option<u128>,
    pub boundary_index_build_count: Option<usize>,
    pub boundary_index_build_microseconds: Option<u128>,
    pub boundary_index_entries: Option<usize>,
    pub boundary_index_owned_bytes: Option<usize>,
    pub linear_boundary_vertex_lookup_count: Option<usize>,
    pub gap_interval_membership_tests: Option<usize>,
    pub gap_event_push_count: Option<usize>,
    pub gap_event_pop_count: Option<usize>,
    pub boundary_gap_label_backend: Option<String>,
    pub clean_endpoint_pair_comparisons: Option<usize>,
    pub boundary_extraction_microseconds: Option<u128>,
    pub reflex_grouping_microseconds: Option<u128>,
    pub occupancy_bytes: Option<usize>,
    pub conflict_representation: Option<String>,
    pub clean_hole_free_eligible: Option<bool>,
    pub path_tree_orientation: Option<String>,
    pub path_tree_orientation_policy: Option<String>,
    pub dual_region_count: Option<usize>,
    pub dual_tree_vertex_count: Option<usize>,
    pub path_count: Option<usize>,
    pub path_edge_incidence_count: Option<usize>,
    pub total_path_length_metric: Option<usize>,
    pub dual_tree_max_depth: Option<usize>,
    pub dual_tree_max_branching_degree: Option<usize>,
    pub heavy_chain_count: Option<usize>,
    pub heavy_chain_interval_count: Option<usize>,
    pub tree_edge_occurrences: Option<usize>,
    pub theoretical_path_occurrence_bound: Option<usize>,
    pub theoretical_tree_edge_occurrence_bound: Option<usize>,
    pub canonical_segment_node_count: Option<usize>,
    pub path_tree_sigma: Option<usize>,
    pub four_d_sigma: Option<usize>,
    pub owned_allocation_estimates: BTreeMap<String, usize>,
    pub region_dual_backend: Option<String>,
    pub region_dual_construction_microseconds: Option<u128>,
    pub dual_tree_edge_count: Option<usize>,
    pub dual_allocated_bytes: Option<usize>,
    pub dual_unit_cut_count: Option<usize>,
    pub dual_area_cell_visits: Option<usize>,
    pub dual_interval_count: Option<usize>,
    pub dual_maximum_nesting_depth: Option<usize>,
    pub hld_interval_count: Option<usize>,
    pub explicit_path_records_materialized: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Certificate {
    pub kind: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DissectionResult {
    pub optimum_rectangle_count: usize,
    pub rectangles: Vec<GridRect>,
    pub diagnostics: Diagnostics,
    pub certificate: Option<Certificate>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolygonDissectionResult {
    pub optimum_rectangle_count: usize,
    pub rectangles: Vec<CoordinateRect>,
    pub diagnostics: Diagnostics,
    pub certificate: Option<Certificate>,
}
