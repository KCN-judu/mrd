use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{certificate, engine, trace};
use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, source_an19::petal::Error};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Ratio {
    pub numerator: i128,
    pub denominator: i128,
}

impl From<ExactRatio> for Ratio {
    fn from(value: ExactRatio) -> Self {
        Self {
            numerator: value.numerator(),
            denominator: value.denominator(),
        }
    }
}

impl TryFrom<Ratio> for ExactRatio {
    type Error = Error;

    fn try_from(value: Ratio) -> Result<Self, Self::Error> {
        ExactRatio::new(value.numerator, value.denominator).map_err(|_| Error::Overflow)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Context {
    pub cluster_id: u64,
    pub projection_snapshot_id: u64,
    pub logical_partition_depth: u64,
    pub recursion_parent_id: Option<u64>,
    pub portal_split_generation: u64,
    pub contraction_generation: u64,
    pub projection_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub source_edge_id: Option<usize>,
    pub active_segment_id: usize,
    pub segment_lineage_root_id: usize,
    pub symbolic_unsplit_rounded_length: Ratio,
    pub highway_halved: bool,
    pub portal_split_generation: u64,
    pub contraction_generation: u64,
    pub projection_generation: u64,
}

impl Segment {
    pub(super) fn from_graph(graph: &SourceDynamicGraph) -> Result<Vec<Self>, Error> {
        (0..graph.edge_count())
            .map(|index| {
                let edge = graph
                    .edge(SourceEdgeId(index))
                    .ok_or(Error::InvalidDomain)?;
                Ok(Self {
                    source_edge_id: Some(index),
                    active_segment_id: index,
                    segment_lineage_root_id: index,
                    symbolic_unsplit_rounded_length: edge.length.into(),
                    highway_halved: false,
                    portal_split_generation: 0,
                    contraction_generation: 0,
                    projection_generation: 0,
                })
            })
            .collect()
    }
}

pub struct Problem<'a> {
    pub graph: &'a SourceDynamicGraph,
    pub cluster: &'a BTreeSet<FlowNodeId>,
    pub remaining: &'a BTreeSet<FlowNodeId>,
    pub center: FlowNodeId,
    pub target: FlowNodeId,
    pub budget: ExactRatio,
    pub context: Context,
    pub segments: &'a [Segment],
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StoppingCertificate {
    pub window_index: usize,
    pub window_start: Ratio,
    pub window_end: Ratio,
    pub selected_radius: Ratio,
    pub internal_edges: usize,
    pub boundary_edges: usize,
    pub cluster_edges: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Count {
    pub key: String,
    pub count: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SnapshotMetrics {
    pub active_vertex_count: u64,
    pub active_directed_arc_count: u64,
    pub active_undirected_segment_count: u64,
    pub original_length_class_count: u64,
    pub symbolic_source_label_class_count: u64,
    pub symbolic_virtual_label_class_count: u64,
    pub materialized_exact_length_class_count: u64,
    pub distinct_reduced_cost_count: u64,
    pub distinct_event_radius_count: u64,
    pub candidate_event_count: u64,
    pub inserted_queue_item_count: u64,
    pub popped_queue_item_count: u64,
    pub stale_queue_item_count: u64,
    pub exact_comparison_count: u64,
    pub decrease_key_or_replacement_count: u64,
    pub equal_key_tie_count: u64,
    pub maximum_queue_size: u64,
    pub vertex_entry_count: u64,
    pub directed_incidence_transition_count: u64,
    pub events_per_source_edge: Vec<Count>,
    pub events_per_segment_lineage: Vec<Count>,
    pub events_per_logical_partition_depth: Vec<Count>,
    pub events_per_symbolic_label: Vec<Count>,
    pub events_created_by_portal_split: Vec<Count>,
    pub events_created_by_contraction: Vec<Count>,
    pub events_created_by_projection_rebuild: Vec<Count>,
    pub events_preserved_by_incremental_projection_updates: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChargeKind {
    SourceDepth,
    LineageEvent,
    SourceDepthEvent,
    DirectedIncidenceTransition,
    PortalSplitDescendant,
    SnapshotSegmentEvent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ChargeAnalysis {
    pub map: ChargeKind,
    pub charge_targets: u64,
    pub maximum_fiber_size: u64,
    pub histogram: Vec<Count>,
    pub worst_witness_event_sequence_numbers: Vec<u64>,
    pub observed_growth_with_input_size: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct HierarchyMetrics {
    pub total_events_across_logical_calls: u64,
    pub maximum_events_for_one_source_edge_at_one_depth: u64,
    pub maximum_events_for_one_source_edge_across_all_depths: u64,
    pub maximum_events_for_one_segment_lineage: u64,
    pub maximum_reduced_classes_in_one_snapshot: u64,
    pub total_reduced_classes_across_snapshots: u64,
    pub total_exact_comparisons: u64,
    pub total_stale_events: u64,
    pub total_event_work_grouped_by_logical_depth: Vec<Count>,
    pub total_event_work_grouped_by_top_level_source_edge: Vec<Count>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct RuntimeStatus {
    pub semantics_implemented: bool,
    pub exact_oracle_verified: bool,
    pub differential_verified: bool,
    pub trace_complete: bool,
    pub local_event_bound_proved: bool,
    pub global_amortization_proved: bool,
    pub priority_queue_bound_proved: bool,
    pub an19_runtime_verified: bool,
}

impl RuntimeStatus {
    #[must_use]
    pub const fn exact_traced(differential_verified: bool) -> Self {
        Self {
            semantics_implemented: true,
            exact_oracle_verified: true,
            differential_verified,
            trace_complete: true,
            local_event_bound_proved: true,
            global_amortization_proved: false,
            priority_queue_bound_proved: false,
            an19_runtime_verified: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub engine: engine::Kind,
    pub selected_radius: Ratio,
    pub selected_vertices: Vec<usize>,
    pub internal_edge_ids: Vec<usize>,
    pub boundary_edge_ids: Vec<usize>,
    pub path_edge_ids: Vec<usize>,
    pub stopping_certificate: StoppingCertificate,
    pub semantic_trace: Vec<trace::Record>,
    pub queue_trace: Vec<trace::Record>,
    pub metrics: SnapshotMetrics,
    pub local_event_bound: certificate::LocalBound,
    pub practical_queue_bound: Option<certificate::QueueBound>,
    pub charge_analyses: Vec<ChargeAnalysis>,
    pub runtime_status: RuntimeStatus,
}
