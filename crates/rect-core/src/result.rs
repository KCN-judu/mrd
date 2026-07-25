use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::GridRect;

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
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostics {
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
