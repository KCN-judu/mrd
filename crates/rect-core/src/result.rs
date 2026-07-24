use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::GridRect;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Diagnostics {
    pub cell_count: usize,
    pub boundary_complexity: usize,
    pub outer_loop_count: usize,
    pub hole_count: usize,
    pub reflex_vertex_count: usize,
    pub horizontal_chord_count: usize,
    pub vertical_chord_count: usize,
    pub explicit_conflict_edge_count: usize,
    pub biclique_count: usize,
    pub biclique_representation_size: usize,
    pub matching_or_flow_value: usize,
    pub minimum_vertex_cover_size: usize,
    pub final_rectangle_count: usize,
    pub phase_microseconds: BTreeMap<String, u128>,
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
