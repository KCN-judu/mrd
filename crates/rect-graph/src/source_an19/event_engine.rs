use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    An19PetalError, An19PetalMetrics, ExactHeapEntry, FigureSixSelection, MembershipThresholds,
    RecoveredPath, ShortestPaths, fast_shortest_paths, hierarchy_or_oracle_paths,
    portal_is_interior, ratio, ratio_less, recover_path, select_weighted_figure_six_fast,
    select_weighted_figure_six_oracle, shortest_paths, validate_weighted_domain,
    weighted_membership_thresholds_oracle,
};
use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct An19ExactRatioRecord {
    pub numerator: i128,
    pub denominator: i128,
}

impl From<ExactRatio> for An19ExactRatioRecord {
    fn from(value: ExactRatio) -> Self {
        Self {
            numerator: value.numerator(),
            denominator: value.denominator(),
        }
    }
}

impl TryFrom<An19ExactRatioRecord> for ExactRatio {
    type Error = An19PetalError;

    fn try_from(value: An19ExactRatioRecord) -> Result<Self, Self::Error> {
        ExactRatio::new(value.numerator, value.denominator).map_err(|_| An19PetalError::Overflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum An19EventEngineKind {
    ExactOracle,
    ReducedExact,
    ProvedUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum An19EventType {
    VertexEntry,
    OutsideToBoundaryEdgeTransition,
    BoundaryToInternalEdgeTransition,
    HighwayEndpoint,
    PortalSplit,
    VirtualSegmentEvent,
    ContractionRelatedEvent,
    QueueInsertion,
    StaleQueueEvent,
    StoppingConditionCheck,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum An19EventOrientation {
    FirstToSecond,
    SecondToFirst,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum An19StaleReason {
    SupersededDistance,
    SettledVertex,
    AfterStoppingRadius,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19EventContext {
    pub cluster_id: u64,
    pub projection_snapshot_id: u64,
    pub logical_partition_depth: u64,
    pub recursion_parent_id: Option<u64>,
    pub portal_split_generation: u64,
    pub contraction_generation: u64,
    pub projection_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19EventSegmentMetadata {
    pub source_edge_id: Option<usize>,
    pub active_segment_id: usize,
    pub segment_lineage_root_id: usize,
    pub symbolic_unsplit_rounded_length: An19ExactRatioRecord,
    pub highway_halved: bool,
    pub portal_split_generation: u64,
    pub contraction_generation: u64,
    pub projection_generation: u64,
}

impl An19EventSegmentMetadata {
    fn from_graph(graph: &SourceDynamicGraph) -> Result<Vec<Self>, An19PetalError> {
        (0..graph.edge_count())
            .map(|index| {
                let edge = graph
                    .edge(SourceEdgeId(index))
                    .ok_or(An19PetalError::InvalidDomain)?;
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

pub struct An19EventProblem<'a> {
    pub graph: &'a SourceDynamicGraph,
    pub cluster: &'a BTreeSet<FlowNodeId>,
    pub remaining: &'a BTreeSet<FlowNodeId>,
    pub center: FlowNodeId,
    pub target: FlowNodeId,
    pub budget: ExactRatio,
    pub context: An19EventContext,
    pub segments: &'a [An19EventSegmentMetadata],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19EventState {
    pub active_vertices: usize,
    pub internal_edges: usize,
    pub boundary_edges: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19EventTraceRecord {
    pub cluster_id: u64,
    pub projection_snapshot_id: u64,
    pub logical_partition_depth: u64,
    pub recursion_parent_id: Option<u64>,
    pub event_sequence_number: u64,
    pub event_type: An19EventType,
    pub source_edge_id: Option<usize>,
    pub active_segment_id: Option<usize>,
    pub segment_lineage_root_id: Option<usize>,
    pub orientation: Option<An19EventOrientation>,
    pub exact_materialized_segment_length: Option<An19ExactRatioRecord>,
    pub symbolic_unsplit_rounded_length: Option<An19ExactRatioRecord>,
    pub highway_halved: Option<bool>,
    pub exact_reduced_cost: Option<An19ExactRatioRecord>,
    pub exact_event_radius: An19ExactRatioRecord,
    pub queue_insertion_sequence: Option<u64>,
    pub queue_pop_sequence: Option<u64>,
    pub stale: bool,
    pub stale_reason: Option<An19StaleReason>,
    pub state_before: An19EventState,
    pub state_after: An19EventState,
    pub endpoint_ids: Option<[usize; 2]>,
    pub affected_vertex_id: Option<usize>,
    pub affected_directed_incidence_id: Option<usize>,
    pub portal_split_generation: u64,
    pub contraction_generation: u64,
    pub projection_generation: u64,
    pub tie_break_fields: Vec<u64>,
    pub charge_source_depth: Option<[u64; 2]>,
    pub charge_lineage_event: Option<[u64; 2]>,
    pub charge_source_depth_event: Option<[u64; 3]>,
    pub charge_incidence_transition: Option<[u64; 2]>,
    pub charge_portal_descendant: Option<[u64; 2]>,
    pub charge_snapshot_segment_event: Option<[u64; 3]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19StoppingCertificate {
    pub window_index: usize,
    pub window_start: An19ExactRatioRecord,
    pub window_end: An19ExactRatioRecord,
    pub selected_radius: An19ExactRatioRecord,
    pub internal_edges: usize,
    pub boundary_edges: usize,
    pub cluster_edges: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19CountByKey {
    pub key: String,
    pub count: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19SnapshotMetrics {
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
    pub events_per_source_edge: Vec<An19CountByKey>,
    pub events_per_segment_lineage: Vec<An19CountByKey>,
    pub events_per_logical_partition_depth: Vec<An19CountByKey>,
    pub events_per_symbolic_label: Vec<An19CountByKey>,
    pub events_created_by_portal_split: Vec<An19CountByKey>,
    pub events_created_by_contraction: Vec<An19CountByKey>,
    pub events_created_by_projection_rebuild: Vec<An19CountByKey>,
    pub events_preserved_by_incremental_projection_updates: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum An19ChargeMapKind {
    SourceDepth,
    LineageEvent,
    SourceDepthEvent,
    DirectedIncidenceTransition,
    PortalSplitDescendant,
    SnapshotSegmentEvent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19ChargeAnalysis {
    pub map: An19ChargeMapKind,
    pub charge_targets: u64,
    pub maximum_fiber_size: u64,
    pub histogram: Vec<An19CountByKey>,
    pub worst_witness_event_sequence_numbers: Vec<u64>,
    pub observed_growth_with_input_size: Option<bool>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19HierarchyEventMetrics {
    pub total_events_across_logical_calls: u64,
    pub maximum_events_for_one_source_edge_at_one_depth: u64,
    pub maximum_events_for_one_source_edge_across_all_depths: u64,
    pub maximum_events_for_one_segment_lineage: u64,
    pub maximum_reduced_classes_in_one_snapshot: u64,
    pub total_reduced_classes_across_snapshots: u64,
    pub total_exact_comparisons: u64,
    pub total_stale_events: u64,
    pub total_event_work_grouped_by_logical_depth: Vec<An19CountByKey>,
    pub total_event_work_grouped_by_top_level_source_edge: Vec<An19CountByKey>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19LocalEventBoundCertificate {
    pub schema_version: u32,
    pub vertex_count: u64,
    pub edge_count: u64,
    pub semantic_event_bound: u64,
    pub queue_item_bound: u64,
    pub semantic_event_count: u64,
    pub candidate_vertex_event_count: u64,
    pub vertex_entry_count: u64,
    pub highway_endpoint_count: u64,
    pub stopping_check_count: u64,
    pub directed_transition_count: u64,
    pub virtual_segment_event_count: u64,
    pub structural_event_count: u64,
    pub queue_insertion_count: u64,
    pub queue_pop_count: u64,
    pub stale_queue_item_count: u64,
    pub priority_queue_comparison_bound_included: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct An19EventRuntimeStatus {
    pub semantics_implemented: bool,
    pub exact_oracle_verified: bool,
    pub differential_verified: bool,
    pub trace_complete: bool,
    pub local_event_bound_proved: bool,
    pub global_amortization_proved: bool,
    pub priority_queue_bound_proved: bool,
    pub an19_runtime_verified: bool,
}

impl An19EventRuntimeStatus {
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
pub struct An19EventRun {
    pub engine: An19EventEngineKind,
    pub selected_radius: An19ExactRatioRecord,
    pub selected_vertices: Vec<usize>,
    pub internal_edge_ids: Vec<usize>,
    pub boundary_edge_ids: Vec<usize>,
    pub path_edge_ids: Vec<usize>,
    pub stopping_certificate: An19StoppingCertificate,
    pub semantic_trace: Vec<An19EventTraceRecord>,
    pub queue_trace: Vec<An19EventTraceRecord>,
    pub metrics: An19SnapshotMetrics,
    pub local_event_bound: An19LocalEventBoundCertificate,
    pub charge_analyses: Vec<An19ChargeAnalysis>,
    pub runtime_status: An19EventRuntimeStatus,
}

pub trait An19EventEngine {
    fn kind(&self) -> An19EventEngineKind;

    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, trace-consistency, or unsupported
    /// backend error.
    fn run(&self, problem: &An19EventProblem<'_>) -> Result<An19EventRun, An19PetalError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ExactEventOracle;

#[derive(Clone, Copy, Debug, Default)]
pub struct An19ReducedEventEngine;

#[derive(Clone, Copy, Debug, Default)]
pub struct ProvedEventEngine;

impl An19EventEngine for ProvedEventEngine {
    fn kind(&self) -> An19EventEngineKind {
        An19EventEngineKind::ProvedUnavailable
    }

    fn run(&self, _problem: &An19EventProblem<'_>) -> Result<An19EventRun, An19PetalError> {
        Err(An19PetalError::UnprovedEventEngine)
    }
}

#[derive(Clone, Copy)]
struct ArcWitness {
    edge: SourceEdgeId,
    to: FlowNodeId,
    reduced_cost: ExactRatio,
    orientation: An19EventOrientation,
    directed_incidence: usize,
}

#[derive(Clone)]
struct TraceQueueItem {
    distance: ExactRatio,
    vertex: FlowNodeId,
    insertion_sequence: u64,
    predecessor: Option<ArcWitness>,
}

#[derive(Clone)]
struct QueueObservation {
    item: TraceQueueItem,
    pop_sequence: Option<u64>,
    stale_reason: Option<An19StaleReason>,
    insertion: bool,
}

#[derive(Default)]
struct QueueStatistics {
    inserted: u64,
    popped: u64,
    stale: u64,
    comparisons: u64,
    replacements: u64,
    equal_key_ties: u64,
    maximum_size: u64,
}

struct EnginePreparation {
    path: RecoveredPath,
    center_distances: Vec<Option<ExactRatio>>,
    thresholds: MembershipThresholds,
    selection: FigureSixSelection,
    witnesses: Vec<Option<ArcWitness>>,
    queue_observations: Vec<QueueObservation>,
    queue_statistics: QueueStatistics,
    distinct_reduced_costs: BTreeSet<(i128, i128)>,
}

impl An19EventEngine for ExactEventOracle {
    fn kind(&self) -> An19EventEngineKind {
        An19EventEngineKind::ExactOracle
    }

    fn run(&self, problem: &An19EventProblem<'_>) -> Result<An19EventRun, An19PetalError> {
        validate_event_problem(problem)?;
        let mut metrics = An19PetalMetrics::default();
        let paths = shortest_paths(problem.graph, problem.cluster, problem.center, &mut metrics)?;
        let path = recover_path(problem.center, problem.target, &paths)?;
        validate_recovered_path(problem, &path, &paths, false, &mut metrics)?;
        let thresholds = weighted_membership_thresholds_oracle(
            problem.graph,
            problem.remaining,
            problem.target,
            &path,
            &paths.distances,
            problem.budget,
            &mut metrics,
        )?;
        let selection = select_weighted_figure_six_oracle(
            problem.graph,
            problem.cluster,
            problem.remaining,
            &thresholds,
            problem.budget,
            false,
            problem.graph.node_count(),
            &mut metrics,
        )?;
        let (observations, queue_statistics) = oracle_queue_observations(problem, &thresholds)?;
        build_run(
            problem,
            self.kind(),
            &EnginePreparation {
                path,
                center_distances: paths.distances.clone(),
                thresholds,
                selection,
                witnesses: vec![None; problem.graph.node_count()],
                queue_observations: observations,
                queue_statistics,
                distinct_reduced_costs: reduced_cost_set(problem.graph, problem.remaining, &paths)?,
            },
        )
    }
}

impl An19EventEngine for An19ReducedEventEngine {
    fn kind(&self) -> An19EventEngineKind {
        An19EventEngineKind::ReducedExact
    }

    fn run(&self, problem: &An19EventProblem<'_>) -> Result<An19EventRun, An19PetalError> {
        validate_event_problem(problem)?;
        let mut metrics = An19PetalMetrics::default();
        let paths =
            fast_shortest_paths(problem.graph, problem.cluster, problem.center, &mut metrics)?;
        let path = recover_path(problem.center, problem.target, &paths)?;
        validate_recovered_path(problem, &path, &paths, true, &mut metrics)?;
        let traced = traced_reduced_thresholds(problem, &path, &paths)?;

        // Cross-check the traced implementation against the existing source-shaped
        // monotone-queue path. This is not an Oracle fallback: disagreement is an
        // explicit error and the traced output is never replaced.
        let fast = super::fast_weighted_membership_thresholds(
            problem.graph,
            problem.remaining,
            problem.target,
            &path,
            &paths.distances,
            problem.budget,
            &mut metrics,
        )?;
        if traced.thresholds.by_vertex != fast.by_vertex
            || traced.thresholds.path_distance_from_target != fast.path_distance_from_target
        {
            return Err(An19PetalError::InvalidEventTrace);
        }
        let selection = select_weighted_figure_six_fast(
            problem.graph,
            problem.cluster,
            problem.remaining,
            &traced.thresholds,
            problem.budget,
            false,
            problem.graph.node_count(),
            &mut metrics,
        )?;
        build_run(
            problem,
            self.kind(),
            &EnginePreparation {
                path,
                center_distances: paths.distances,
                thresholds: traced.thresholds,
                selection,
                witnesses: traced.witnesses,
                queue_observations: traced.queue_observations,
                queue_statistics: traced.queue_statistics,
                distinct_reduced_costs: traced.distinct_reduced_costs,
            },
        )
    }
}

fn validate_event_problem(problem: &An19EventProblem<'_>) -> Result<(), An19PetalError> {
    validate_weighted_domain(
        problem.graph,
        problem.cluster,
        problem.remaining,
        problem.center,
        problem.target,
        problem.budget,
    )?;
    if !problem.budget.is_positive()
        || problem.segments.len() != problem.graph.edge_count()
        || problem
            .segments
            .iter()
            .enumerate()
            .any(|(index, segment)| segment.active_segment_id != index)
    {
        return Err(An19PetalError::InvalidEventTrace);
    }
    for (index, metadata) in problem.segments.iter().enumerate() {
        let edge = problem
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(An19PetalError::InvalidDomain)?;
        let symbolic = ExactRatio::try_from(metadata.symbolic_unsplit_rounded_length)?;
        if !symbolic.is_positive()
            || metadata.portal_split_generation > problem.context.portal_split_generation
            || metadata.contraction_generation > problem.context.contraction_generation
            || metadata.projection_generation > problem.context.projection_generation
            || !edge.length.is_positive()
        {
            return Err(An19PetalError::InvalidEventTrace);
        }
    }
    Ok(())
}

fn validate_recovered_path(
    problem: &An19EventProblem<'_>,
    path: &RecoveredPath,
    cluster_paths: &ShortestPaths,
    fast: bool,
    metrics: &mut An19PetalMetrics,
) -> Result<(), An19PetalError> {
    if path
        .vertices
        .iter()
        .any(|vertex| !problem.remaining.contains(vertex))
    {
        return Err(An19PetalError::InvalidDomain);
    }
    let remaining_paths = hierarchy_or_oracle_paths(
        problem.graph,
        problem.remaining,
        problem.center,
        fast,
        metrics,
    )?;
    for vertex in &path.vertices {
        if cluster_paths.distances[vertex.0] != remaining_paths.distances[vertex.0] {
            return Err(An19PetalError::InvalidDomain);
        }
    }
    let target_distance =
        cluster_paths.distances[problem.target.0].ok_or(An19PetalError::Disconnected)?;
    if ratio_less(target_distance, problem.budget)? {
        return Err(An19PetalError::InvalidRadius);
    }
    Ok(())
}

struct TracedThresholds {
    thresholds: MembershipThresholds,
    witnesses: Vec<Option<ArcWitness>>,
    queue_observations: Vec<QueueObservation>,
    queue_statistics: QueueStatistics,
    distinct_reduced_costs: BTreeSet<(i128, i128)>,
}

#[allow(clippy::too_many_lines)]
fn traced_reduced_thresholds(
    problem: &An19EventProblem<'_>,
    path: &RecoveredPath,
    paths: &ShortestPaths,
) -> Result<TracedThresholds, An19PetalError> {
    let node_count = problem.graph.node_count();
    let target_distance = paths.distances[problem.target.0].ok_or(An19PetalError::Disconnected)?;
    let mut labels = vec![None; node_count];
    let mut path_distance_from_target = vec![None; node_count];
    let mut seeds = Vec::new();
    let mut insertion_sequence = 0_u64;
    add_trace_seed(
        problem.target,
        ratio(0, 1)?,
        &mut labels,
        &mut seeds,
        &mut insertion_sequence,
    )?;
    path_distance_from_target[problem.target.0] = Some(ratio(0, 1)?);
    let mut distance_from_target = ratio(0, 1)?;
    for path_index in (0..path.edges.len()).rev() {
        let edge = problem
            .graph
            .edge(path.edges[path_index])
            .ok_or(An19PetalError::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let next_distance = distance_from_target
            .checked_add(edge.length)
            .map_err(|_| An19PetalError::Overflow)?;
        path_distance_from_target[toward_center.0] = Some(next_distance);
        if ratio_less(problem.budget, next_distance)? {
            if ratio_less(distance_from_target, problem.budget)? {
                add_trace_interior_seeds(
                    from,
                    toward_center,
                    edge.length,
                    problem
                        .budget
                        .checked_sub(distance_from_target)
                        .map_err(|_| An19PetalError::Overflow)?,
                    target_distance,
                    problem.budget,
                    &paths.distances,
                    &mut labels,
                    &mut seeds,
                    &mut insertion_sequence,
                )?;
            }
            break;
        }
        add_trace_seed(
            toward_center,
            next_distance,
            &mut labels,
            &mut seeds,
            &mut insertion_sequence,
        )?;
        distance_from_target = next_distance;
        if distance_from_target == problem.budget {
            break;
        }
    }
    let (adjacency, distinct_reduced_costs) =
        traced_reduced_adjacency(problem.graph, problem.remaining, &paths.distances)?;
    let mut queue = seeds;
    let mut queue_observations = queue
        .iter()
        .cloned()
        .map(|item| QueueObservation {
            item,
            pop_sequence: None,
            stale_reason: None,
            insertion: true,
        })
        .collect::<Vec<_>>();
    let mut statistics = QueueStatistics {
        inserted: u64::try_from(queue.len()).map_err(|_| An19PetalError::Overflow)?,
        maximum_size: u64::try_from(queue.len()).map_err(|_| An19PetalError::Overflow)?,
        ..QueueStatistics::default()
    };
    let mut settled = vec![false; node_count];
    let mut witnesses = vec![None; node_count];
    let mut ordered = Vec::new();
    let mut pop_sequence = 0_u64;
    while !queue.is_empty() {
        let minimum = trace_queue_minimum(&queue, &mut statistics)?;
        let item = queue.remove(minimum);
        pop_sequence = pop_sequence
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        statistics.popped = statistics
            .popped
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        let stale_reason = if settled[item.vertex.0] {
            Some(An19StaleReason::SettledVertex)
        } else if labels[item.vertex.0] != Some(item.distance) {
            Some(An19StaleReason::SupersededDistance)
        } else {
            None
        };
        queue_observations.push(QueueObservation {
            item: item.clone(),
            pop_sequence: Some(pop_sequence),
            stale_reason,
            insertion: false,
        });
        if stale_reason.is_some() {
            statistics.stale = statistics
                .stale
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            continue;
        }
        settled[item.vertex.0] = true;
        witnesses[item.vertex.0] = item.predecessor;
        ordered.push(ExactHeapEntry {
            distance: item.distance,
            vertex: item.vertex,
        });
        for arc in &adjacency[item.vertex.0] {
            if settled[arc.to.0] {
                continue;
            }
            let candidate = item
                .distance
                .checked_add(arc.reduced_cost)
                .map_err(|_| An19PetalError::Overflow)?;
            let improves = match labels[arc.to.0] {
                Some(old) => {
                    statistics.comparisons = statistics
                        .comparisons
                        .checked_add(1)
                        .ok_or(An19PetalError::Overflow)?;
                    if candidate == old {
                        statistics.equal_key_ties = statistics
                            .equal_key_ties
                            .checked_add(1)
                            .ok_or(An19PetalError::Overflow)?;
                    }
                    ratio_less(candidate, old)?
                }
                None => true,
            };
            if improves {
                if labels[arc.to.0].is_some() {
                    statistics.replacements = statistics
                        .replacements
                        .checked_add(1)
                        .ok_or(An19PetalError::Overflow)?;
                }
                labels[arc.to.0] = Some(candidate);
                insertion_sequence = insertion_sequence
                    .checked_add(1)
                    .ok_or(An19PetalError::Overflow)?;
                let queued = TraceQueueItem {
                    distance: candidate,
                    vertex: arc.to,
                    insertion_sequence,
                    predecessor: Some(*arc),
                };
                queue.push(queued.clone());
                queue_observations.push(QueueObservation {
                    item: queued,
                    pop_sequence: None,
                    stale_reason: None,
                    insertion: true,
                });
                statistics.inserted = statistics
                    .inserted
                    .checked_add(1)
                    .ok_or(An19PetalError::Overflow)?;
                statistics.maximum_size = statistics
                    .maximum_size
                    .max(u64::try_from(queue.len()).map_err(|_| An19PetalError::Overflow)?);
            }
        }
    }
    let mut by_vertex = vec![None; node_count];
    for vertex in problem.remaining {
        let threshold = labels[vertex.0].ok_or(An19PetalError::Disconnected)?;
        if threshold.is_negative() {
            return Err(An19PetalError::InvalidRadius);
        }
        if !ratio_less(problem.budget, threshold)? {
            by_vertex[vertex.0] = Some(threshold);
        }
    }
    ordered.retain(|entry| by_vertex[entry.vertex.0] == Some(entry.distance));
    Ok(TracedThresholds {
        thresholds: MembershipThresholds {
            by_vertex,
            path_distance_from_target,
            ordered_events: Some(ordered),
        },
        witnesses,
        queue_observations,
        queue_statistics: statistics,
        distinct_reduced_costs,
    })
}

#[allow(clippy::too_many_arguments)]
fn add_trace_interior_seeds(
    from: FlowNodeId,
    toward_center: FlowNodeId,
    edge_length: ExactRatio,
    offset_from: ExactRatio,
    target_distance: ExactRatio,
    radius: ExactRatio,
    center_distances: &[Option<ExactRatio>],
    labels: &mut [Option<ExactRatio>],
    seeds: &mut Vec<TraceQueueItem>,
    insertion_sequence: &mut u64,
) -> Result<(), An19PetalError> {
    let two = ratio(2, 1)?;
    let from_center = center_distances[from.0].ok_or(An19PetalError::Disconnected)?;
    let toward_center_distance =
        center_distances[toward_center.0].ok_or(An19PetalError::Disconnected)?;
    let potential = target_distance
        .checked_mul(two)
        .and_then(|value| value.checked_sub(radius))
        .map_err(|_| An19PetalError::Overflow)?;
    let from_threshold = potential
        .checked_add(
            offset_from
                .checked_mul(two)
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .and_then(|value| value.checked_sub(from_center.checked_mul(two)?))
        .map_err(|_| An19PetalError::Overflow)?;
    let toward_threshold = potential
        .checked_add(
            edge_length
                .checked_sub(offset_from)
                .and_then(|value| value.checked_mul(two))
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .and_then(|value| value.checked_sub(toward_center_distance.checked_mul(two)?))
        .map_err(|_| An19PetalError::Overflow)?;
    add_trace_seed(from, from_threshold, labels, seeds, insertion_sequence)?;
    add_trace_seed(
        toward_center,
        toward_threshold,
        labels,
        seeds,
        insertion_sequence,
    )
}

fn add_trace_seed(
    vertex: FlowNodeId,
    distance: ExactRatio,
    labels: &mut [Option<ExactRatio>],
    seeds: &mut Vec<TraceQueueItem>,
    insertion_sequence: &mut u64,
) -> Result<(), An19PetalError> {
    let improves = match labels[vertex.0] {
        Some(old) => ratio_less(distance, old)?,
        None => true,
    };
    if improves {
        labels[vertex.0] = Some(distance);
        *insertion_sequence = insertion_sequence
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        seeds.push(TraceQueueItem {
            distance,
            vertex,
            insertion_sequence: *insertion_sequence,
            predecessor: None,
        });
    }
    Ok(())
}

type ReducedAdjacency = (Vec<Vec<ArcWitness>>, BTreeSet<(i128, i128)>);

fn traced_reduced_adjacency(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    center_distances: &[Option<ExactRatio>],
) -> Result<ReducedAdjacency, An19PetalError> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    let mut distinct = BTreeSet::new();
    for index in 0..graph.edge_count() {
        let edge_id = SourceEdgeId(index);
        let edge = graph.edge(edge_id).ok_or(An19PetalError::InvalidDomain)?;
        if !allowed.contains(&edge.first) || !allowed.contains(&edge.second) {
            continue;
        }
        let first_distance = center_distances[edge.first.0].ok_or(An19PetalError::Disconnected)?;
        let second_distance =
            center_distances[edge.second.0].ok_or(An19PetalError::Disconnected)?;
        let forward = edge
            .length
            .checked_add(first_distance)
            .and_then(|value| value.checked_sub(second_distance))
            .and_then(|value| value.checked_mul_integer(2))
            .map_err(|_| An19PetalError::Overflow)?;
        let reverse = edge
            .length
            .checked_add(second_distance)
            .and_then(|value| value.checked_sub(first_distance))
            .and_then(|value| value.checked_mul_integer(2))
            .map_err(|_| An19PetalError::Overflow)?;
        if forward.is_negative() || reverse.is_negative() {
            return Err(An19PetalError::InvalidHighway);
        }
        distinct.insert((forward.numerator(), forward.denominator()));
        distinct.insert((reverse.numerator(), reverse.denominator()));
        adjacency[edge.first.0].push(ArcWitness {
            edge: edge_id,
            to: edge.second,
            reduced_cost: forward,
            orientation: An19EventOrientation::FirstToSecond,
            directed_incidence: index.checked_mul(2).ok_or(An19PetalError::Overflow)?,
        });
        adjacency[edge.second.0].push(ArcWitness {
            edge: edge_id,
            to: edge.first,
            reduced_cost: reverse,
            orientation: An19EventOrientation::SecondToFirst,
            directed_incidence: index
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .ok_or(An19PetalError::Overflow)?,
        });
    }
    Ok((adjacency, distinct))
}

fn trace_queue_minimum(
    queue: &[TraceQueueItem],
    statistics: &mut QueueStatistics,
) -> Result<usize, An19PetalError> {
    let mut minimum = 0;
    for index in 1..queue.len() {
        statistics.comparisons = statistics
            .comparisons
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        if trace_item_less(&queue[index], &queue[minimum], statistics)? {
            minimum = index;
        }
    }
    Ok(minimum)
}

fn trace_item_less(
    first: &TraceQueueItem,
    second: &TraceQueueItem,
    statistics: &mut QueueStatistics,
) -> Result<bool, An19PetalError> {
    if first.distance == second.distance {
        statistics.equal_key_ties = statistics
            .equal_key_ties
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        return Ok(
            (first.vertex, first.insertion_sequence) < (second.vertex, second.insertion_sequence)
        );
    }
    ratio_less(first.distance, second.distance)
}

fn reduced_cost_set(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    paths: &ShortestPaths,
) -> Result<BTreeSet<(i128, i128)>, An19PetalError> {
    Ok(traced_reduced_adjacency(graph, allowed, &paths.distances)?.1)
}

fn oracle_queue_observations(
    problem: &An19EventProblem<'_>,
    thresholds: &MembershipThresholds,
) -> Result<(Vec<QueueObservation>, QueueStatistics), An19PetalError> {
    let mut items = problem
        .remaining
        .iter()
        .filter_map(|vertex| {
            thresholds.by_vertex[vertex.0].map(|distance| TraceQueueItem {
                distance,
                vertex: *vertex,
                insertion_sequence: u64::try_from(vertex.0).unwrap_or(u64::MAX),
                predecessor: None,
            })
        })
        .collect::<Vec<_>>();
    let mut statistics = QueueStatistics {
        inserted: u64::try_from(items.len()).map_err(|_| An19PetalError::Overflow)?,
        maximum_size: u64::try_from(items.len()).map_err(|_| An19PetalError::Overflow)?,
        ..QueueStatistics::default()
    };
    let insertions = items
        .iter()
        .cloned()
        .map(|item| QueueObservation {
            item,
            pop_sequence: None,
            stale_reason: None,
            insertion: true,
        })
        .collect::<Vec<_>>();
    for index in 1..items.len() {
        let mut cursor = index;
        while cursor > 0 {
            statistics.comparisons = statistics
                .comparisons
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            if !trace_item_less(&items[cursor], &items[cursor - 1], &mut statistics)? {
                break;
            }
            items.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
    let mut observations = insertions;
    for (index, item) in items.into_iter().enumerate() {
        let pop_sequence = u64::try_from(index + 1).map_err(|_| An19PetalError::Overflow)?;
        observations.push(QueueObservation {
            item,
            pop_sequence: Some(pop_sequence),
            stale_reason: None,
            insertion: false,
        });
        statistics.popped = statistics
            .popped
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
    }
    Ok((observations, statistics))
}

impl An19ReducedEventEngine {
    /// Runs both independent exact paths and returns their fully traced outputs
    /// only when their normalized Figure 6 semantics agree.
    ///
    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, or semantic disagreement error.
    pub fn run_differential(
        problem: &An19EventProblem<'_>,
    ) -> Result<(An19EventRun, An19EventRun), An19PetalError> {
        let mut oracle = ExactEventOracle.run(problem)?;
        let mut reduced = Self.run(problem)?;
        if !oracle.semantically_agrees(&reduced) {
            return Err(An19PetalError::InvalidEventTrace);
        }
        oracle.runtime_status.differential_verified = true;
        reduced.runtime_status.exact_oracle_verified = true;
        reduced.runtime_status.differential_verified = true;
        Ok((oracle, reduced))
    }
}

impl An19EventRun {
    #[must_use]
    pub fn semantically_agrees(&self, other: &Self) -> bool {
        self.selected_radius == other.selected_radius
            && self.selected_vertices == other.selected_vertices
            && self.internal_edge_ids == other.internal_edge_ids
            && self.boundary_edge_ids == other.boundary_edge_ids
            && self.path_edge_ids == other.path_edge_ids
            && self.stopping_certificate == other.stopping_certificate
            && normalized_semantic_trace(&self.semantic_trace)
                == normalized_semantic_trace(&other.semantic_trace)
    }

    /// Rechecks canonical sequencing, exact ratios, state transitions, charge
    /// fields, and the conservative runtime-status boundary.
    ///
    /// # Errors
    ///
    /// Returns [`An19PetalError::InvalidEventTrace`] when any trace invariant
    /// fails, or an exact-arithmetic error for a malformed ratio.
    pub fn verify_trace(&self) -> Result<(), An19PetalError> {
        if !self.runtime_status.local_event_bound_proved
            || self.runtime_status.global_amortization_proved
            || self.runtime_status.priority_queue_bound_proved
            || self.runtime_status.an19_runtime_verified
            || self
                .semantic_trace
                .iter()
                .enumerate()
                .any(|(index, event)| {
                    event.event_sequence_number != u64::try_from(index).unwrap_or(u64::MAX)
                })
            || self.queue_trace.iter().enumerate().any(|(index, event)| {
                event.event_sequence_number != u64::try_from(index).unwrap_or(u64::MAX)
            })
        {
            return Err(An19PetalError::InvalidEventTrace);
        }
        let mut previous = None;
        let mut previous_state = An19EventState::default();
        let mut semantic_keys = BTreeSet::new();
        for event in &self.semantic_trace {
            let radius = ExactRatio::try_from(event.exact_event_radius)?;
            if let Some(old) = previous
                && ratio_less(radius, old)?
            {
                return Err(An19PetalError::InvalidEventTrace);
            }
            previous = Some(radius);
            if event.state_before != previous_state
                || event.stale != event.stale_reason.is_some()
                || event.state_after.active_vertices < event.state_before.active_vertices
                || event.state_after.internal_edges < event.state_before.internal_edges
                || (event.event_type == An19EventType::BoundaryToInternalEdgeTransition
                    && event.state_after.boundary_edges >= event.state_before.boundary_edges)
                || event.charge_source_depth.is_some() != event.source_edge_id.is_some()
                || event.charge_lineage_event.is_some() != event.segment_lineage_root_id.is_some()
                || event.charge_snapshot_segment_event.is_some()
                    != event.active_segment_id.is_some()
            {
                return Err(An19PetalError::InvalidEventTrace);
            }
            previous_state = event.state_after;
            let key = (
                event.event_type,
                event.exact_event_radius,
                event.affected_vertex_id,
                event.affected_directed_incidence_id,
            );
            if !event.stale && !semantic_keys.insert(key) {
                return Err(An19PetalError::InvalidEventTrace);
            }
        }
        if self.metrics.vertex_entry_count
            != u64::try_from(
                self.semantic_trace
                    .iter()
                    .filter(|event| event.event_type == An19EventType::VertexEntry && !event.stale)
                    .count(),
            )
            .map_err(|_| An19PetalError::Overflow)?
            || self.metrics.stale_queue_item_count
                != u64::try_from(
                    self.queue_trace
                        .iter()
                        .filter(|event| event.event_type == An19EventType::StaleQueueEvent)
                        .count(),
                )
                .map_err(|_| An19PetalError::Overflow)?
        {
            return Err(An19PetalError::InvalidEventTrace);
        }
        verify_local_event_bound(self)?;
        Ok(())
    }

    /// Reruns the selected exact backend and rejects any trace mutation.
    ///
    /// # Errors
    ///
    /// Returns an exact engine error or
    /// [`An19PetalError::InvalidEventTrace`] when the rerun differs.
    pub fn verify_against(&self, problem: &An19EventProblem<'_>) -> Result<(), An19PetalError> {
        let expected = match self.engine {
            An19EventEngineKind::ExactOracle => ExactEventOracle.run(problem)?,
            An19EventEngineKind::ReducedExact => An19ReducedEventEngine.run(problem)?,
            An19EventEngineKind::ProvedUnavailable => {
                return Err(An19PetalError::UnprovedEventEngine);
            }
        };
        let mut actual = self.clone();
        let mut rebuilt = expected;
        actual.runtime_status.differential_verified = false;
        rebuilt.runtime_status.differential_verified = false;
        actual.runtime_status.exact_oracle_verified = rebuilt.runtime_status.exact_oracle_verified;
        if actual != rebuilt {
            return Err(An19PetalError::InvalidEventTrace);
        }
        self.verify_trace()
    }
}

#[allow(clippy::too_many_lines)]
fn verify_local_event_bound(run: &An19EventRun) -> Result<(), An19PetalError> {
    let certificate = run.local_event_bound;
    let semantic_count =
        u64::try_from(run.semantic_trace.len()).map_err(|_| An19PetalError::Overflow)?;
    let count_semantic = |event_type| {
        u64::try_from(
            run.semantic_trace
                .iter()
                .filter(|event| event.event_type == event_type)
                .count(),
        )
        .map_err(|_| An19PetalError::Overflow)
    };
    let vertex_entries = count_semantic(An19EventType::VertexEntry)?;
    let highway_endpoints = count_semantic(An19EventType::HighwayEndpoint)?;
    let stopping_checks = count_semantic(An19EventType::StoppingConditionCheck)?;
    let outside_boundary = count_semantic(An19EventType::OutsideToBoundaryEdgeTransition)?;
    let boundary_internal = count_semantic(An19EventType::BoundaryToInternalEdgeTransition)?;
    let transitions = outside_boundary
        .checked_add(boundary_internal)
        .ok_or(An19PetalError::Overflow)?;
    let virtual_events = count_semantic(An19EventType::VirtualSegmentEvent)?;
    let structural_events = count_semantic(An19EventType::PortalSplit)?
        .checked_add(count_semantic(An19EventType::ContractionRelatedEvent)?)
        .ok_or(An19PetalError::Overflow)?;
    let categorized = vertex_entries
        .checked_add(highway_endpoints)
        .and_then(|value| value.checked_add(stopping_checks))
        .and_then(|value| value.checked_add(transitions))
        .and_then(|value| value.checked_add(virtual_events))
        .and_then(|value| value.checked_add(structural_events))
        .ok_or(An19PetalError::Overflow)?;
    let queue_insertions = u64::try_from(
        run.queue_trace
            .iter()
            .filter(|event| event.event_type == An19EventType::QueueInsertion)
            .count(),
    )
    .map_err(|_| An19PetalError::Overflow)?;
    let queue_pops = u64::try_from(
        run.queue_trace
            .iter()
            .filter(|event| event.queue_pop_sequence.is_some())
            .count(),
    )
    .map_err(|_| An19PetalError::Overflow)?;
    let stale_queue_items = u64::try_from(
        run.queue_trace
            .iter()
            .filter(|event| event.event_type == An19EventType::StaleQueueEvent)
            .count(),
    )
    .map_err(|_| An19PetalError::Overflow)?;
    let semantic_bound = certificate
        .vertex_count
        .checked_mul(3)
        .and_then(|value| {
            certificate
                .edge_count
                .checked_mul(4)
                .and_then(|edges| value.checked_add(edges))
        })
        .and_then(|value| value.checked_add(2))
        .ok_or(An19PetalError::Overflow)?;
    let queue_bound = certificate
        .edge_count
        .checked_mul(2)
        .and_then(|edges| certificate.vertex_count.checked_add(edges))
        .and_then(|value| value.checked_add(2))
        .ok_or(An19PetalError::Overflow)?;
    let twice_edges = certificate
        .edge_count
        .checked_mul(2)
        .ok_or(An19PetalError::Overflow)?;
    if certificate.schema_version != 1
        || certificate.priority_queue_comparison_bound_included
        || certificate.semantic_event_bound != semantic_bound
        || certificate.queue_item_bound != queue_bound
        || certificate.semantic_event_count != semantic_count
        || certificate.candidate_vertex_event_count != run.metrics.candidate_event_count
        || certificate.vertex_entry_count != vertex_entries
        || certificate.highway_endpoint_count != highway_endpoints
        || certificate.stopping_check_count != stopping_checks
        || certificate.directed_transition_count != transitions
        || certificate.virtual_segment_event_count != virtual_events
        || certificate.structural_event_count != structural_events
        || certificate.queue_insertion_count != queue_insertions
        || certificate.queue_pop_count != queue_pops
        || certificate.stale_queue_item_count != stale_queue_items
        || categorized != semantic_count
        || certificate.candidate_vertex_event_count > certificate.vertex_count
        || vertex_entries > certificate.vertex_count
        || highway_endpoints > certificate.vertex_count
        || stopping_checks > certificate.vertex_count
        || transitions > twice_edges
        || virtual_events > transitions
        || structural_events > 2
        || semantic_count > semantic_bound
        || queue_insertions > queue_bound
        || queue_pops != queue_insertions
        || stale_queue_items > queue_pops
        || run.metrics.inserted_queue_item_count != queue_insertions
        || run.metrics.popped_queue_item_count != queue_pops
        || run.metrics.stale_queue_item_count != stale_queue_items
        || run.metrics.directed_incidence_transition_count != transitions
    {
        return Err(An19PetalError::InvalidEventTrace);
    }
    Ok(())
}

type NormalizedEvent = (
    An19EventType,
    An19ExactRatioRecord,
    Option<usize>,
    Option<usize>,
    bool,
    An19EventState,
    An19EventState,
);

fn normalized_semantic_trace(trace: &[An19EventTraceRecord]) -> Vec<NormalizedEvent> {
    trace
        .iter()
        .map(|event| {
            (
                event.event_type,
                event.exact_event_radius,
                event.affected_vertex_id,
                event.affected_directed_incidence_id,
                event.stale,
                event.state_before,
                event.state_after,
            )
        })
        .collect()
}

fn build_run(
    problem: &An19EventProblem<'_>,
    engine: An19EventEngineKind,
    preparation: &EnginePreparation,
) -> Result<An19EventRun, An19PetalError> {
    let selected_vertices = vertices_at_selected_radius(
        problem.remaining,
        &preparation.thresholds,
        preparation.selection.radius,
    )?;
    let (internal_edge_ids, boundary_edge_ids) =
        edge_partitions(problem.graph, problem.cluster, &selected_vertices)?;
    if internal_edge_ids.len() != preparation.selection.internal_edges
        || boundary_edge_ids.len() != preparation.selection.boundary_edges
    {
        return Err(An19PetalError::InvalidEventTrace);
    }
    let semantic_trace = build_semantic_trace(problem, preparation)?;
    let queue_trace = build_queue_trace(problem, &preparation.queue_observations)?;
    let metrics = build_snapshot_metrics(problem, preparation, &semantic_trace, &queue_trace)?;
    let local_event_bound =
        build_local_event_bound(problem, &semantic_trace, &queue_trace, &metrics)?;
    let charge_analyses = analyze_all_charge_maps(&semantic_trace)?;
    let runtime_status = match engine {
        An19EventEngineKind::ExactOracle => An19EventRuntimeStatus {
            semantics_implemented: true,
            exact_oracle_verified: true,
            differential_verified: false,
            trace_complete: true,
            local_event_bound_proved: true,
            global_amortization_proved: false,
            priority_queue_bound_proved: false,
            an19_runtime_verified: false,
        },
        An19EventEngineKind::ReducedExact => An19EventRuntimeStatus {
            semantics_implemented: true,
            exact_oracle_verified: false,
            differential_verified: false,
            trace_complete: true,
            local_event_bound_proved: true,
            global_amortization_proved: false,
            priority_queue_bound_proved: false,
            an19_runtime_verified: false,
        },
        An19EventEngineKind::ProvedUnavailable => {
            return Err(An19PetalError::UnprovedEventEngine);
        }
    };
    let run = An19EventRun {
        engine,
        selected_radius: preparation.selection.radius.into(),
        selected_vertices: selected_vertices.iter().map(|vertex| vertex.0).collect(),
        internal_edge_ids,
        boundary_edge_ids,
        path_edge_ids: preparation.path.edges.iter().map(|edge| edge.0).collect(),
        stopping_certificate: An19StoppingCertificate {
            window_index: preparation.selection.window_index,
            window_start: preparation.selection.window_start.into(),
            window_end: preparation.selection.window_end.into(),
            selected_radius: preparation.selection.radius.into(),
            internal_edges: preparation.selection.internal_edges,
            boundary_edges: preparation.selection.boundary_edges,
            cluster_edges: preparation.selection.cluster_edges,
        },
        semantic_trace,
        queue_trace,
        metrics,
        local_event_bound,
        charge_analyses,
        runtime_status,
    };
    run.verify_trace()?;
    Ok(run)
}

fn vertices_at_selected_radius(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    radius: ExactRatio,
) -> Result<BTreeSet<FlowNodeId>, An19PetalError> {
    let mut vertices = BTreeSet::new();
    for vertex in remaining {
        let Some(threshold) = thresholds.by_vertex[vertex.0] else {
            continue;
        };
        if !ratio_less(radius, threshold)? {
            vertices.insert(*vertex);
        }
    }
    Ok(vertices)
}

fn edge_partitions(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    vertices: &BTreeSet<FlowNodeId>,
) -> Result<(Vec<usize>, Vec<usize>), An19PetalError> {
    let mut internal = Vec::new();
    let mut boundary = Vec::new();
    for index in 0..graph.edge_count() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(An19PetalError::InvalidDomain)?;
        if !cluster.contains(&edge.first) || !cluster.contains(&edge.second) {
            continue;
        }
        let first = vertices.contains(&edge.first);
        let second = vertices.contains(&edge.second);
        if first && second {
            internal.push(index);
        } else if first || second {
            boundary.push(index);
        }
    }
    Ok((internal, boundary))
}

fn sorted_threshold_entries(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
) -> Result<Vec<ExactHeapEntry>, An19PetalError> {
    let mut entries = remaining
        .iter()
        .filter_map(|vertex| {
            thresholds.by_vertex[vertex.0].map(|distance| ExactHeapEntry {
                distance,
                vertex: *vertex,
            })
        })
        .collect::<Vec<_>>();
    for index in 1..entries.len() {
        let mut cursor = index;
        while cursor > 0 {
            let first = &entries[cursor];
            let second = &entries[cursor - 1];
            let less = ratio_less(first.distance, second.distance)?
                || (first.distance == second.distance && first.vertex < second.vertex);
            if !less {
                break;
            }
            entries.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
    Ok(entries)
}

#[allow(clippy::too_many_lines)]
fn build_semantic_trace(
    problem: &An19EventProblem<'_>,
    preparation: &EnginePreparation,
) -> Result<Vec<An19EventTraceRecord>, An19PetalError> {
    let entries = sorted_threshold_entries(problem.remaining, &preparation.thresholds)?;
    let mut incident = vec![Vec::new(); problem.graph.node_count()];
    for index in 0..problem.graph.edge_count() {
        let edge = problem
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(An19PetalError::InvalidDomain)?;
        if problem.cluster.contains(&edge.first) && problem.cluster.contains(&edge.second) {
            incident[edge.first.0].push(index);
            incident[edge.second.0].push(index);
        }
    }
    let mut trace = Vec::new();
    let mut state = An19EventState::default();
    let mut active = vec![false; problem.graph.node_count()];
    let mut edge_state = vec![0_u8; problem.graph.edge_count()];
    let mut structural_events_emitted = false;
    let mut cursor = 0;
    while cursor < entries.len() {
        let radius = entries[cursor].distance;
        if !structural_events_emitted && ratio_less(preparation.selection.radius, radius)? {
            append_structural_events(problem, preparation, state, &mut trace)?;
            structural_events_emitted = true;
        }
        let group_start = cursor;
        while cursor < entries.len() && entries[cursor].distance == radius {
            cursor += 1;
        }
        for entry in &entries[group_start..cursor] {
            let after_stop = ratio_less(preparation.selection.radius, radius)?;
            let before = state;
            if !after_stop && !active[entry.vertex.0] {
                active[entry.vertex.0] = true;
                state.active_vertices = state
                    .active_vertices
                    .checked_add(1)
                    .ok_or(An19PetalError::Overflow)?;
            }
            trace.push(make_trace_record(
                problem,
                An19EventType::VertexEntry,
                radius,
                preparation.witnesses[entry.vertex.0],
                None,
                Some(entry.vertex),
                before,
                state,
                after_stop,
                after_stop.then_some(An19StaleReason::AfterStoppingRadius),
                None,
                None,
            )?);
            if after_stop {
                continue;
            }
            if preparation.thresholds.path_distance_from_target[entry.vertex.0].is_some() {
                trace.push(make_trace_record(
                    problem,
                    An19EventType::HighwayEndpoint,
                    radius,
                    preparation.witnesses[entry.vertex.0],
                    None,
                    Some(entry.vertex),
                    state,
                    state,
                    false,
                    None,
                    None,
                    None,
                )?);
            }
            for edge_index in &incident[entry.vertex.0] {
                let edge = problem
                    .graph
                    .edge(SourceEdgeId(*edge_index))
                    .ok_or(An19PetalError::InvalidDomain)?;
                let other = if edge.first == entry.vertex {
                    edge.second
                } else {
                    edge.first
                };
                let transition = if active[other.0] {
                    if edge_state[*edge_index] == 1 {
                        Some(An19EventType::BoundaryToInternalEdgeTransition)
                    } else {
                        None
                    }
                } else if edge_state[*edge_index] == 0 {
                    Some(An19EventType::OutsideToBoundaryEdgeTransition)
                } else {
                    None
                };
                let Some(event_type) = transition else {
                    continue;
                };
                let transition_before = state;
                match event_type {
                    An19EventType::OutsideToBoundaryEdgeTransition => {
                        edge_state[*edge_index] = 1;
                        state.boundary_edges = state
                            .boundary_edges
                            .checked_add(1)
                            .ok_or(An19PetalError::Overflow)?;
                    }
                    An19EventType::BoundaryToInternalEdgeTransition => {
                        edge_state[*edge_index] = 2;
                        state.boundary_edges = state
                            .boundary_edges
                            .checked_sub(1)
                            .ok_or(An19PetalError::InvalidEventTrace)?;
                        state.internal_edges = state
                            .internal_edges
                            .checked_add(1)
                            .ok_or(An19PetalError::Overflow)?;
                    }
                    _ => return Err(An19PetalError::InvalidEventTrace),
                }
                let orientation = if edge.first == entry.vertex {
                    An19EventOrientation::FirstToSecond
                } else {
                    An19EventOrientation::SecondToFirst
                };
                let incidence = edge_index
                    .checked_mul(2)
                    .and_then(|value| {
                        value.checked_add(usize::from(
                            orientation == An19EventOrientation::SecondToFirst,
                        ))
                    })
                    .ok_or(An19PetalError::Overflow)?;
                let from_distance = preparation.center_distances[entry.vertex.0]
                    .ok_or(An19PetalError::Disconnected)?;
                let to_distance =
                    preparation.center_distances[other.0].ok_or(An19PetalError::Disconnected)?;
                let reduced_cost = edge
                    .length
                    .checked_add(from_distance)
                    .and_then(|value| value.checked_sub(to_distance))
                    .and_then(|value| value.checked_mul_integer(2))
                    .map_err(|_| An19PetalError::Overflow)?;
                if reduced_cost.is_negative() {
                    return Err(An19PetalError::InvalidHighway);
                }
                trace.push(make_trace_record(
                    problem,
                    event_type,
                    radius,
                    Some(ArcWitness {
                        edge: SourceEdgeId(*edge_index),
                        to: other,
                        reduced_cost,
                        orientation,
                        directed_incidence: incidence,
                    }),
                    Some(*edge_index),
                    Some(entry.vertex),
                    transition_before,
                    state,
                    false,
                    None,
                    None,
                    None,
                )?);
                if problem.segments[*edge_index].source_edge_id.is_none() {
                    trace.push(make_trace_record(
                        problem,
                        An19EventType::VirtualSegmentEvent,
                        radius,
                        None,
                        Some(*edge_index),
                        Some(entry.vertex),
                        state,
                        state,
                        false,
                        None,
                        None,
                        None,
                    )?);
                }
            }
        }
        if !ratio_less(radius, preparation.selection.window_start)?
            && !ratio_less(preparation.selection.radius, radius)?
        {
            trace.push(make_trace_record(
                problem,
                An19EventType::StoppingConditionCheck,
                radius,
                None,
                None,
                None,
                state,
                state,
                false,
                None,
                None,
                None,
            )?);
        }
    }
    if !structural_events_emitted {
        append_structural_events(problem, preparation, state, &mut trace)?;
    }
    for (index, event) in trace.iter_mut().enumerate() {
        event.event_sequence_number = u64::try_from(index).map_err(|_| An19PetalError::Overflow)?;
    }
    Ok(trace)
}

fn append_structural_events(
    problem: &An19EventProblem<'_>,
    preparation: &EnginePreparation,
    state: An19EventState,
    trace: &mut Vec<An19EventTraceRecord>,
) -> Result<(), An19PetalError> {
    if portal_is_interior(&preparation.thresholds, preparation.selection.radius) {
        trace.push(make_trace_record(
            problem,
            An19EventType::PortalSplit,
            preparation.selection.radius,
            None,
            None,
            None,
            state,
            state,
            false,
            None,
            None,
            None,
        )?);
    }
    if problem.context.contraction_generation > 0 {
        trace.push(make_trace_record(
            problem,
            An19EventType::ContractionRelatedEvent,
            preparation.selection.radius,
            None,
            None,
            None,
            state,
            state,
            false,
            None,
            None,
            None,
        )?);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn make_trace_record(
    problem: &An19EventProblem<'_>,
    event_type: An19EventType,
    radius: ExactRatio,
    witness: Option<ArcWitness>,
    explicit_segment: Option<usize>,
    vertex: Option<FlowNodeId>,
    state_before: An19EventState,
    state_after: An19EventState,
    stale: bool,
    stale_reason: Option<An19StaleReason>,
    insertion_sequence: Option<u64>,
    pop_sequence: Option<u64>,
) -> Result<An19EventTraceRecord, An19PetalError> {
    let segment_index = explicit_segment.or_else(|| witness.map(|value| value.edge.0));
    let metadata = segment_index.and_then(|index| problem.segments.get(index));
    let edge = segment_index
        .map(|index| {
            problem
                .graph
                .edge(SourceEdgeId(index))
                .ok_or(An19PetalError::InvalidDomain)
        })
        .transpose()?;
    let type_code = event_type_code(event_type);
    let source = metadata.and_then(|value| value.source_edge_id);
    let depth = problem.context.logical_partition_depth;
    let transition_code = match event_type {
        An19EventType::OutsideToBoundaryEdgeTransition => 1,
        An19EventType::BoundaryToInternalEdgeTransition => 2,
        _ => 0,
    };
    Ok(An19EventTraceRecord {
        cluster_id: problem.context.cluster_id,
        projection_snapshot_id: problem.context.projection_snapshot_id,
        logical_partition_depth: depth,
        recursion_parent_id: problem.context.recursion_parent_id,
        event_sequence_number: 0,
        event_type,
        source_edge_id: source,
        active_segment_id: metadata.map(|value| value.active_segment_id),
        segment_lineage_root_id: metadata.map(|value| value.segment_lineage_root_id),
        orientation: witness.map(|value| value.orientation),
        exact_materialized_segment_length: edge.map(|value| value.length.into()),
        symbolic_unsplit_rounded_length: metadata
            .map(|value| value.symbolic_unsplit_rounded_length),
        highway_halved: metadata.map(|value| value.highway_halved),
        exact_reduced_cost: witness.map(|value| value.reduced_cost.into()),
        exact_event_radius: radius.into(),
        queue_insertion_sequence: insertion_sequence,
        queue_pop_sequence: pop_sequence,
        stale,
        stale_reason,
        state_before,
        state_after,
        endpoint_ids: edge.map(|value| [value.first.0, value.second.0]),
        affected_vertex_id: vertex.map(|value| value.0),
        affected_directed_incidence_id: witness.map(|value| value.directed_incidence),
        portal_split_generation: metadata
            .map_or(problem.context.portal_split_generation, |value| {
                value.portal_split_generation
            }),
        contraction_generation: metadata.map_or(problem.context.contraction_generation, |value| {
            value.contraction_generation
        }),
        projection_generation: metadata.map_or(problem.context.projection_generation, |value| {
            value.projection_generation
        }),
        tie_break_fields: vec![
            vertex.map_or(u64::MAX, |value| u64::try_from(value.0).unwrap_or(u64::MAX)),
            source.map_or(u64::MAX, |value| u64::try_from(value).unwrap_or(u64::MAX)),
            metadata.map_or(u64::MAX, |value| {
                u64::try_from(value.active_segment_id).unwrap_or(u64::MAX)
            }),
            insertion_sequence.unwrap_or(u64::MAX),
        ],
        charge_source_depth: source.map(|value| {
            [
                u64::try_from(value).unwrap_or(u64::MAX),
                problem.context.logical_partition_depth,
            ]
        }),
        charge_lineage_event: metadata.map(|value| {
            [
                u64::try_from(value.segment_lineage_root_id).unwrap_or(u64::MAX),
                type_code,
            ]
        }),
        charge_source_depth_event: source.map(|value| {
            [
                u64::try_from(value).unwrap_or(u64::MAX),
                problem.context.logical_partition_depth,
                type_code,
            ]
        }),
        charge_incidence_transition: witness.map(|value| {
            [
                u64::try_from(value.directed_incidence).unwrap_or(u64::MAX),
                transition_code,
            ]
        }),
        charge_portal_descendant: (problem.context.portal_split_generation > 0)
            .then_some([problem.context.portal_split_generation, type_code]),
        charge_snapshot_segment_event: metadata.map(|value| {
            [
                problem.context.projection_snapshot_id,
                u64::try_from(value.active_segment_id).unwrap_or(u64::MAX),
                type_code,
            ]
        }),
    })
}

fn build_queue_trace(
    problem: &An19EventProblem<'_>,
    observations: &[QueueObservation],
) -> Result<Vec<An19EventTraceRecord>, An19PetalError> {
    let mut trace = Vec::with_capacity(observations.len());
    for observation in observations {
        let event_type = if observation.insertion {
            An19EventType::QueueInsertion
        } else if observation.stale_reason.is_some() {
            An19EventType::StaleQueueEvent
        } else {
            An19EventType::VertexEntry
        };
        trace.push(make_trace_record(
            problem,
            event_type,
            observation.item.distance,
            observation.item.predecessor,
            None,
            Some(observation.item.vertex),
            An19EventState::default(),
            An19EventState::default(),
            observation.stale_reason.is_some(),
            observation.stale_reason,
            Some(observation.item.insertion_sequence),
            observation.pop_sequence,
        )?);
    }
    for (index, event) in trace.iter_mut().enumerate() {
        event.event_sequence_number = u64::try_from(index).map_err(|_| An19PetalError::Overflow)?;
    }
    Ok(trace)
}

const fn event_type_code(event_type: An19EventType) -> u64 {
    match event_type {
        An19EventType::VertexEntry => 0,
        An19EventType::OutsideToBoundaryEdgeTransition => 1,
        An19EventType::BoundaryToInternalEdgeTransition => 2,
        An19EventType::HighwayEndpoint => 3,
        An19EventType::PortalSplit => 4,
        An19EventType::VirtualSegmentEvent => 5,
        An19EventType::ContractionRelatedEvent => 6,
        An19EventType::QueueInsertion => 7,
        An19EventType::StaleQueueEvent => 8,
        An19EventType::StoppingConditionCheck => 9,
    }
}

fn build_local_event_bound(
    problem: &An19EventProblem<'_>,
    semantic_trace: &[An19EventTraceRecord],
    queue_trace: &[An19EventTraceRecord],
    metrics: &An19SnapshotMetrics,
) -> Result<An19LocalEventBoundCertificate, An19PetalError> {
    let vertex_count =
        u64::try_from(problem.remaining.len()).map_err(|_| An19PetalError::Overflow)?;
    let edge_count =
        u64::try_from(problem.graph.edge_count()).map_err(|_| An19PetalError::Overflow)?;
    let semantic_event_bound = vertex_count
        .checked_mul(3)
        .and_then(|value| {
            edge_count
                .checked_mul(4)
                .and_then(|edges| value.checked_add(edges))
        })
        .and_then(|value| value.checked_add(2))
        .ok_or(An19PetalError::Overflow)?;
    let queue_item_bound = edge_count
        .checked_mul(2)
        .and_then(|edges| vertex_count.checked_add(edges))
        .and_then(|value| value.checked_add(2))
        .ok_or(An19PetalError::Overflow)?;
    let count_semantic = |event_type| {
        u64::try_from(
            semantic_trace
                .iter()
                .filter(|event| event.event_type == event_type)
                .count(),
        )
        .map_err(|_| An19PetalError::Overflow)
    };
    let directed_transition_count = count_semantic(An19EventType::OutsideToBoundaryEdgeTransition)?
        .checked_add(count_semantic(
            An19EventType::BoundaryToInternalEdgeTransition,
        )?)
        .ok_or(An19PetalError::Overflow)?;
    let structural_event_count = count_semantic(An19EventType::PortalSplit)?
        .checked_add(count_semantic(An19EventType::ContractionRelatedEvent)?)
        .ok_or(An19PetalError::Overflow)?;
    Ok(An19LocalEventBoundCertificate {
        schema_version: 1,
        vertex_count,
        edge_count,
        semantic_event_bound,
        queue_item_bound,
        semantic_event_count: u64::try_from(semantic_trace.len())
            .map_err(|_| An19PetalError::Overflow)?,
        candidate_vertex_event_count: metrics.candidate_event_count,
        vertex_entry_count: count_semantic(An19EventType::VertexEntry)?,
        highway_endpoint_count: count_semantic(An19EventType::HighwayEndpoint)?,
        stopping_check_count: count_semantic(An19EventType::StoppingConditionCheck)?,
        directed_transition_count,
        virtual_segment_event_count: count_semantic(An19EventType::VirtualSegmentEvent)?,
        structural_event_count,
        queue_insertion_count: u64::try_from(
            queue_trace
                .iter()
                .filter(|event| event.event_type == An19EventType::QueueInsertion)
                .count(),
        )
        .map_err(|_| An19PetalError::Overflow)?,
        queue_pop_count: u64::try_from(
            queue_trace
                .iter()
                .filter(|event| event.queue_pop_sequence.is_some())
                .count(),
        )
        .map_err(|_| An19PetalError::Overflow)?,
        stale_queue_item_count: u64::try_from(
            queue_trace
                .iter()
                .filter(|event| event.event_type == An19EventType::StaleQueueEvent)
                .count(),
        )
        .map_err(|_| An19PetalError::Overflow)?,
        priority_queue_comparison_bound_included: false,
    })
}

#[allow(clippy::too_many_lines)]
fn build_snapshot_metrics(
    problem: &An19EventProblem<'_>,
    preparation: &EnginePreparation,
    semantic_trace: &[An19EventTraceRecord],
    _queue_trace: &[An19EventTraceRecord],
) -> Result<An19SnapshotMetrics, An19PetalError> {
    let mut original_classes = BTreeSet::new();
    let mut materialized_classes = BTreeSet::new();
    let mut symbolic_source_classes = BTreeSet::new();
    let mut symbolic_virtual_classes = BTreeSet::new();
    let mut active_segments = 0_u64;
    for index in 0..problem.graph.edge_count() {
        let edge = problem
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(An19PetalError::InvalidDomain)?;
        if !problem.remaining.contains(&edge.first) || !problem.remaining.contains(&edge.second) {
            continue;
        }
        active_segments = active_segments
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        let materialized = (edge.length.numerator(), edge.length.denominator());
        original_classes.insert(materialized);
        materialized_classes.insert(materialized);
        let metadata = &problem.segments[index];
        let mut symbolic = ExactRatio::try_from(metadata.symbolic_unsplit_rounded_length)?;
        if metadata.highway_halved {
            symbolic = symbolic
                .checked_mul(ratio(1, 2)?)
                .map_err(|_| An19PetalError::Overflow)?;
        }
        let class = (symbolic.numerator(), symbolic.denominator());
        if metadata.source_edge_id.is_some() {
            symbolic_source_classes.insert(class);
        } else {
            symbolic_virtual_classes.insert(class);
        }
    }
    let event_radii = preparation
        .thresholds
        .by_vertex
        .iter()
        .flatten()
        .map(|value| (value.numerator(), value.denominator()))
        .collect::<BTreeSet<_>>();
    let counted_semantic = semantic_trace
        .iter()
        .filter(|event| !event.stale)
        .collect::<Vec<_>>();
    let vertex_entries = counted_semantic
        .iter()
        .filter(|event| event.event_type == An19EventType::VertexEntry)
        .count();
    let incidence_transitions = counted_semantic
        .iter()
        .filter(|event| {
            matches!(
                event.event_type,
                An19EventType::OutsideToBoundaryEdgeTransition
                    | An19EventType::BoundaryToInternalEdgeTransition
            )
        })
        .count();
    let events_per_source_edge = count_trace_keys(&counted_semantic, |event| {
        event.source_edge_id.map(|value| value.to_string())
    })?;
    let events_per_segment_lineage = count_trace_keys(&counted_semantic, |event| {
        event.segment_lineage_root_id.map(|value| value.to_string())
    })?;
    let events_per_logical_partition_depth = count_trace_keys(&counted_semantic, |event| {
        Some(event.logical_partition_depth.to_string())
    })?;
    let events_per_symbolic_label = count_trace_keys(&counted_semantic, |event| {
        event
            .symbolic_unsplit_rounded_length
            .map(|value| format!("{}/{}", value.numerator, value.denominator))
    })?;
    let events_created_by_portal_split = count_trace_keys(&counted_semantic, |event| {
        (event.portal_split_generation > 0).then(|| event.portal_split_generation.to_string())
    })?;
    let events_created_by_contraction = count_trace_keys(&counted_semantic, |event| {
        (event.contraction_generation > 0).then(|| event.contraction_generation.to_string())
    })?;
    let events_created_by_projection_rebuild = count_trace_keys(&counted_semantic, |event| {
        Some(event.projection_generation.to_string())
    })?;
    let preserved = counted_semantic
        .iter()
        .filter(|event| event.projection_generation < problem.context.projection_generation)
        .count();
    Ok(An19SnapshotMetrics {
        active_vertex_count: u64::try_from(problem.remaining.len())
            .map_err(|_| An19PetalError::Overflow)?,
        active_directed_arc_count: active_segments
            .checked_mul(2)
            .ok_or(An19PetalError::Overflow)?,
        active_undirected_segment_count: active_segments,
        original_length_class_count: u64::try_from(original_classes.len())
            .map_err(|_| An19PetalError::Overflow)?,
        symbolic_source_label_class_count: u64::try_from(symbolic_source_classes.len())
            .map_err(|_| An19PetalError::Overflow)?,
        symbolic_virtual_label_class_count: u64::try_from(symbolic_virtual_classes.len())
            .map_err(|_| An19PetalError::Overflow)?,
        materialized_exact_length_class_count: u64::try_from(materialized_classes.len())
            .map_err(|_| An19PetalError::Overflow)?,
        distinct_reduced_cost_count: u64::try_from(preparation.distinct_reduced_costs.len())
            .map_err(|_| An19PetalError::Overflow)?,
        distinct_event_radius_count: u64::try_from(event_radii.len())
            .map_err(|_| An19PetalError::Overflow)?,
        candidate_event_count: u64::try_from(
            preparation
                .thresholds
                .by_vertex
                .iter()
                .filter(|value| value.is_some())
                .count(),
        )
        .map_err(|_| An19PetalError::Overflow)?,
        inserted_queue_item_count: preparation.queue_statistics.inserted,
        popped_queue_item_count: preparation.queue_statistics.popped,
        stale_queue_item_count: preparation.queue_statistics.stale,
        exact_comparison_count: preparation.queue_statistics.comparisons,
        decrease_key_or_replacement_count: preparation.queue_statistics.replacements,
        equal_key_tie_count: preparation.queue_statistics.equal_key_ties,
        maximum_queue_size: preparation.queue_statistics.maximum_size,
        vertex_entry_count: u64::try_from(vertex_entries).map_err(|_| An19PetalError::Overflow)?,
        directed_incidence_transition_count: u64::try_from(incidence_transitions)
            .map_err(|_| An19PetalError::Overflow)?,
        events_per_source_edge,
        events_per_segment_lineage,
        events_per_logical_partition_depth,
        events_per_symbolic_label,
        events_created_by_portal_split,
        events_created_by_contraction,
        events_created_by_projection_rebuild,
        events_preserved_by_incremental_projection_updates: u64::try_from(preserved)
            .map_err(|_| An19PetalError::Overflow)?,
    })
}

fn count_trace_keys<F>(
    trace: &[&An19EventTraceRecord],
    key: F,
) -> Result<Vec<An19CountByKey>, An19PetalError>
where
    F: Fn(&An19EventTraceRecord) -> Option<String>,
{
    let mut counts = BTreeMap::<String, u64>::new();
    for event in trace {
        let Some(value) = key(event) else {
            continue;
        };
        let count = counts.entry(value).or_default();
        *count = count.checked_add(1).ok_or(An19PetalError::Overflow)?;
    }
    Ok(counts
        .into_iter()
        .map(|(key, count)| An19CountByKey { key, count })
        .collect())
}

fn analyze_all_charge_maps(
    trace: &[An19EventTraceRecord],
) -> Result<Vec<An19ChargeAnalysis>, An19PetalError> {
    [
        An19ChargeMapKind::SourceDepth,
        An19ChargeMapKind::LineageEvent,
        An19ChargeMapKind::SourceDepthEvent,
        An19ChargeMapKind::DirectedIncidenceTransition,
        An19ChargeMapKind::PortalSplitDescendant,
        An19ChargeMapKind::SnapshotSegmentEvent,
    ]
    .into_iter()
    .map(|map| analyze_charge_map(trace, map))
    .collect()
}

fn analyze_charge_map(
    trace: &[An19EventTraceRecord],
    map: An19ChargeMapKind,
) -> Result<An19ChargeAnalysis, An19PetalError> {
    let mut fibers = BTreeMap::<String, Vec<u64>>::new();
    for event in trace.iter().filter(|event| !event.stale) {
        let key = match map {
            An19ChargeMapKind::SourceDepth => event
                .charge_source_depth
                .map(|value| format!("{}:{}", value[0], value[1])),
            An19ChargeMapKind::LineageEvent => event
                .charge_lineage_event
                .map(|value| format!("{}:{}", value[0], value[1])),
            An19ChargeMapKind::SourceDepthEvent => event
                .charge_source_depth_event
                .map(|value| format!("{}:{}:{}", value[0], value[1], value[2])),
            An19ChargeMapKind::DirectedIncidenceTransition => event
                .charge_incidence_transition
                .map(|value| format!("{}:{}", value[0], value[1])),
            An19ChargeMapKind::PortalSplitDescendant => event
                .charge_portal_descendant
                .map(|value| format!("{}:{}", value[0], value[1])),
            An19ChargeMapKind::SnapshotSegmentEvent => event
                .charge_snapshot_segment_event
                .map(|value| format!("{}:{}:{}", value[0], value[1], value[2])),
        };
        if let Some(key) = key {
            fibers
                .entry(key)
                .or_default()
                .push(event.event_sequence_number);
        }
    }
    let maximum = fibers.values().map(Vec::len).max().unwrap_or(0);
    let worst = fibers
        .values()
        .filter(|fiber| fiber.len() == maximum)
        .min()
        .cloned()
        .unwrap_or_default();
    let mut histogram = BTreeMap::<usize, u64>::new();
    for fiber in fibers.values() {
        let count = histogram.entry(fiber.len()).or_default();
        *count = count.checked_add(1).ok_or(An19PetalError::Overflow)?;
    }
    Ok(An19ChargeAnalysis {
        map,
        charge_targets: u64::try_from(fibers.len()).map_err(|_| An19PetalError::Overflow)?,
        maximum_fiber_size: u64::try_from(maximum).map_err(|_| An19PetalError::Overflow)?,
        histogram: histogram
            .into_iter()
            .map(|(size, count)| An19CountByKey {
                key: size.to_string(),
                count,
            })
            .collect(),
        worst_witness_event_sequence_numbers: worst,
        observed_growth_with_input_size: None,
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum An19AdversarialFamily {
    ManyReducedCostsFewSourceLengths,
    RepeatedPortalSplitting,
    FullDepthPersistence,
    AllEqualReducedKeys,
    AllDistinctReducedKeys,
    AlternatingPartitionContraction,
    HighwayHalvingReorder,
    VirtualRealMixedSegments,
}

impl An19AdversarialFamily {
    pub const ALL: [Self; 8] = [
        Self::ManyReducedCostsFewSourceLengths,
        Self::RepeatedPortalSplitting,
        Self::FullDepthPersistence,
        Self::AllEqualReducedKeys,
        Self::AllDistinctReducedKeys,
        Self::AlternatingPartitionContraction,
        Self::HighwayHalvingReorder,
        Self::VirtualRealMixedSegments,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ManyReducedCostsFewSourceLengths => "many_reduced_costs_few_source_lengths",
            Self::RepeatedPortalSplitting => "repeated_portal_splitting",
            Self::FullDepthPersistence => "full_depth_persistence",
            Self::AllEqualReducedKeys => "all_equal_reduced_keys",
            Self::AllDistinctReducedKeys => "all_distinct_reduced_keys",
            Self::AlternatingPartitionContraction => "alternating_partition_contraction",
            Self::HighwayHalvingReorder => "highway_halving_reorder",
            Self::VirtualRealMixedSegments => "virtual_real_mixed_segments",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19AdversarialCaseResult {
    pub input_family: An19AdversarialFamily,
    pub size_parameter: usize,
    pub logical_call_index: usize,
    pub graph_nodes: usize,
    pub graph_edges: usize,
    pub original_length_classes: u64,
    pub symbolic_source_label_classes: u64,
    pub symbolic_virtual_label_classes: u64,
    pub materialized_length_classes: u64,
    pub distinct_reduced_costs: u64,
    pub distinct_event_radii: u64,
    pub total_events: u64,
    pub events_per_source_depth_maximum: u64,
    pub events_per_lineage_maximum: u64,
    pub queue_insertions: u64,
    pub queue_pops: u64,
    pub exact_comparisons: u64,
    pub stale_events: u64,
    pub oracle_agreement: bool,
    pub selected_radius: An19ExactRatioRecord,
    pub charge_analyses: Vec<An19ChargeAnalysis>,
    pub oracle_run: An19EventRun,
    pub reduced_run: An19EventRun,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct An19AdversarialCampaign {
    pub schema_version: u32,
    pub commit_sha: String,
    pub command_line: String,
    pub seed: Option<u64>,
    pub cases: Vec<An19AdversarialCaseResult>,
    pub aggregate: An19HierarchyEventMetrics,
    pub naive_reduced_class_conversion_survived: bool,
    pub runtime_status: An19EventRuntimeStatus,
}

impl An19AdversarialCampaign {
    /// Runs deterministic fixed-snapshot campaigns. These measurements are
    /// counterexample/proof-discovery evidence only and never set a proof flag.
    ///
    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, trace, or differential error.
    pub fn run(
        families: &[An19AdversarialFamily],
        sizes: &[usize],
        commit_sha: String,
        command_line: String,
    ) -> Result<Self, An19PetalError> {
        if families.is_empty() || sizes.is_empty() {
            return Err(An19PetalError::InvalidDomain);
        }
        let mut cases = Vec::new();
        for family in families {
            for size in sizes {
                for (logical_call_index, owned) in adversarial_problems(*family, *size)?
                    .into_iter()
                    .enumerate()
                {
                    let problem = owned.as_problem();
                    let (oracle, reduced) = An19ReducedEventEngine::run_differential(&problem)?;
                    if !oracle.semantically_agrees(&reduced) {
                        return Err(An19PetalError::InvalidEventTrace);
                    }
                    let source_depth_maximum =
                        maximum_count(&reduced.metrics.events_per_source_edge);
                    let lineage_maximum =
                        maximum_count(&reduced.metrics.events_per_segment_lineage);
                    cases.push(An19AdversarialCaseResult {
                        input_family: *family,
                        size_parameter: *size,
                        logical_call_index,
                        graph_nodes: owned.graph.node_count(),
                        graph_edges: owned.graph.edge_count(),
                        original_length_classes: reduced.metrics.original_length_class_count,
                        symbolic_source_label_classes: reduced
                            .metrics
                            .symbolic_source_label_class_count,
                        symbolic_virtual_label_classes: reduced
                            .metrics
                            .symbolic_virtual_label_class_count,
                        materialized_length_classes: reduced
                            .metrics
                            .materialized_exact_length_class_count,
                        distinct_reduced_costs: reduced.metrics.distinct_reduced_cost_count,
                        distinct_event_radii: reduced.metrics.distinct_event_radius_count,
                        total_events: u64::try_from(reduced.semantic_trace.len())
                            .map_err(|_| An19PetalError::Overflow)?,
                        events_per_source_depth_maximum: source_depth_maximum,
                        events_per_lineage_maximum: lineage_maximum,
                        queue_insertions: reduced.metrics.inserted_queue_item_count,
                        queue_pops: reduced.metrics.popped_queue_item_count,
                        exact_comparisons: reduced.metrics.exact_comparison_count,
                        stale_events: reduced.metrics.stale_queue_item_count,
                        oracle_agreement: true,
                        selected_radius: reduced.selected_radius,
                        charge_analyses: reduced.charge_analyses.clone(),
                        oracle_run: oracle,
                        reduced_run: reduced,
                    });
                }
            }
        }
        let aggregate = aggregate_campaign(&cases)?;
        set_growth_observations(&mut cases);
        let naive_reduced_class_conversion_survived = !cases.iter().any(|case| {
            case.input_family == An19AdversarialFamily::ManyReducedCostsFewSourceLengths
                && case.distinct_reduced_costs > case.original_length_classes.saturating_mul(4)
        });
        Ok(Self {
            schema_version: 1,
            commit_sha,
            command_line,
            seed: None,
            cases,
            aggregate,
            naive_reduced_class_conversion_survived,
            runtime_status: An19EventRuntimeStatus::exact_traced(true),
        })
    }

    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut output = vec![
            "# AN19 exact event adversarial campaign".to_owned(),
            String::new(),
            format!("- Commit: `{}`", self.commit_sha),
            format!("- Cases: {}", self.cases.len()),
            format!(
                "- Naive reduced-class conversion survived: {}",
                self.naive_reduced_class_conversion_survived
            ),
            "- AN19 runtime verified: false".to_owned(),
            String::new(),
            "| family | size | call | nodes | edges | original classes | reduced costs | event radii | events | comparisons | stale | Oracle |".to_owned(),
            "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |".to_owned(),
        ];
        for case in &self.cases {
            output.push(format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                case.input_family.name(),
                case.size_parameter,
                case.logical_call_index,
                case.graph_nodes,
                case.graph_edges,
                case.original_length_classes,
                case.distinct_reduced_costs,
                case.distinct_event_radii,
                case.total_events,
                case.exact_comparisons,
                case.stale_events,
                case.oracle_agreement,
            ));
        }
        output.extend([
            String::new(),
            "The campaign establishes exact differential semantics on these finite fixtures. It does not prove a fixed-snapshot event bound, hierarchy-wide amortization, priority-queue bound, or the AN19 runtime.".to_owned(),
        ]);
        output.join("\n") + "\n"
    }
}

fn maximum_count(counts: &[An19CountByKey]) -> u64 {
    counts.iter().map(|entry| entry.count).max().unwrap_or(0)
}

fn set_growth_observations(cases: &mut [An19AdversarialCaseResult]) {
    for family in An19AdversarialFamily::ALL {
        let family_cases = cases
            .iter()
            .filter(|case| case.input_family == family)
            .collect::<Vec<_>>();
        let grows = family_cases.windows(2).any(|pair| {
            pair[1].size_parameter > pair[0].size_parameter
                && pair[1].events_per_source_depth_maximum > pair[0].events_per_source_depth_maximum
        });
        for case in cases.iter_mut().filter(|case| case.input_family == family) {
            for analysis in &mut case.charge_analyses {
                analysis.observed_growth_with_input_size = Some(grows);
            }
            case.reduced_run.charge_analyses = case.charge_analyses.clone();
        }
    }
}

fn aggregate_campaign(
    cases: &[An19AdversarialCaseResult],
) -> Result<An19HierarchyEventMetrics, An19PetalError> {
    let mut by_depth = BTreeMap::<String, u64>::new();
    let mut by_source = BTreeMap::<String, u64>::new();
    let mut source_depth = BTreeMap::<(String, String), u64>::new();
    let mut lineage = BTreeMap::<String, u64>::new();
    let mut total_events = 0_u64;
    let mut total_reduced = 0_u64;
    let mut total_comparisons = 0_u64;
    let mut total_stale = 0_u64;
    let mut maximum_reduced = 0_u64;
    for case in cases {
        total_events = total_events
            .checked_add(case.total_events)
            .ok_or(An19PetalError::Overflow)?;
        total_reduced = total_reduced
            .checked_add(case.distinct_reduced_costs)
            .ok_or(An19PetalError::Overflow)?;
        total_comparisons = total_comparisons
            .checked_add(case.exact_comparisons)
            .ok_or(An19PetalError::Overflow)?;
        total_stale = total_stale
            .checked_add(case.stale_events)
            .ok_or(An19PetalError::Overflow)?;
        maximum_reduced = maximum_reduced.max(case.distinct_reduced_costs);
        for event in case
            .reduced_run
            .semantic_trace
            .iter()
            .filter(|event| !event.stale)
        {
            increment_string_count(&mut by_depth, event.logical_partition_depth.to_string())?;
            if let Some(source) = event.source_edge_id {
                let source = source.to_string();
                increment_string_count(&mut by_source, source.clone())?;
                let key = (source, event.logical_partition_depth.to_string());
                let count = source_depth.entry(key).or_default();
                *count = count.checked_add(1).ok_or(An19PetalError::Overflow)?;
            }
            if let Some(lineage_id) = event.segment_lineage_root_id {
                increment_string_count(&mut lineage, lineage_id.to_string())?;
            }
        }
    }
    Ok(An19HierarchyEventMetrics {
        total_events_across_logical_calls: total_events,
        maximum_events_for_one_source_edge_at_one_depth: source_depth
            .values()
            .copied()
            .max()
            .unwrap_or(0),
        maximum_events_for_one_source_edge_across_all_depths: by_source
            .values()
            .copied()
            .max()
            .unwrap_or(0),
        maximum_events_for_one_segment_lineage: lineage.values().copied().max().unwrap_or(0),
        maximum_reduced_classes_in_one_snapshot: maximum_reduced,
        total_reduced_classes_across_snapshots: total_reduced,
        total_exact_comparisons: total_comparisons,
        total_stale_events: total_stale,
        total_event_work_grouped_by_logical_depth: count_map_to_vec(by_depth),
        total_event_work_grouped_by_top_level_source_edge: count_map_to_vec(by_source),
    })
}

fn increment_string_count(
    counts: &mut BTreeMap<String, u64>,
    key: String,
) -> Result<(), An19PetalError> {
    let count = counts.entry(key).or_default();
    *count = count.checked_add(1).ok_or(An19PetalError::Overflow)?;
    Ok(())
}

fn count_map_to_vec(counts: BTreeMap<String, u64>) -> Vec<An19CountByKey> {
    counts
        .into_iter()
        .map(|(key, count)| An19CountByKey { key, count })
        .collect()
}

struct OwnedEventProblem {
    graph: SourceDynamicGraph,
    cluster: BTreeSet<FlowNodeId>,
    remaining: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
    context: An19EventContext,
    segments: Vec<An19EventSegmentMetadata>,
}

impl OwnedEventProblem {
    fn as_problem(&self) -> An19EventProblem<'_> {
        An19EventProblem {
            graph: &self.graph,
            cluster: &self.cluster,
            remaining: &self.remaining,
            center: self.center,
            target: self.target,
            budget: self.budget,
            context: self.context,
            segments: &self.segments,
        }
    }
}

fn adversarial_problems(
    family: An19AdversarialFamily,
    requested_size: usize,
) -> Result<Vec<OwnedEventProblem>, An19PetalError> {
    let size = requested_size.max(10);
    match family {
        An19AdversarialFamily::ManyReducedCostsFewSourceLengths
        | An19AdversarialFamily::AllDistinctReducedKeys => Ok(vec![power_of_two_chord_problem(
            size.next_power_of_two().max(16),
            family,
        )?]),
        An19AdversarialFamily::FullDepthPersistence => {
            let depth = usize::try_from(size.ilog2()).map_err(|_| An19PetalError::Overflow)?;
            (0..=depth)
                .map(|logical_depth| {
                    path_problem(
                        size,
                        family,
                        u64::try_from(logical_depth).map_err(|_| An19PetalError::Overflow)?,
                    )
                })
                .collect()
        }
        An19AdversarialFamily::AlternatingPartitionContraction => (0..=2)
            .map(|generation| {
                let mut problem = path_problem(
                    size,
                    family,
                    u64::try_from(generation / 2).map_err(|_| An19PetalError::Overflow)?,
                )?;
                problem.context.contraction_generation =
                    u64::try_from(generation).map_err(|_| An19PetalError::Overflow)?;
                for segment in &mut problem.segments {
                    segment.contraction_generation = problem.context.contraction_generation;
                }
                Ok(problem)
            })
            .collect(),
        An19AdversarialFamily::HighwayHalvingReorder => (0..=1)
            .map(|projection_generation| path_problem(size, family, projection_generation))
            .collect(),
        An19AdversarialFamily::RepeatedPortalSplitting
        | An19AdversarialFamily::AllEqualReducedKeys
        | An19AdversarialFamily::VirtualRealMixedSegments => {
            Ok(vec![path_problem(size, family, 0)?])
        }
    }
}

fn path_problem(
    nodes: usize,
    family: An19AdversarialFamily,
    logical_depth: u64,
) -> Result<OwnedEventProblem, An19PetalError> {
    let mut edges = Vec::new();
    let mut total_length = 0_i128;
    for index in 0..nodes - 1 {
        let length = if family == An19AdversarialFamily::HighwayHalvingReorder {
            if index % 2 == 0 {
                if logical_depth == 0 { 4 } else { 2 }
            } else {
                3
            }
        } else {
            1
        };
        total_length = total_length
            .checked_add(length)
            .ok_or(An19PetalError::Overflow)?;
        edges.push(SourceWeightedEdge {
            first: FlowNodeId(index),
            second: FlowNodeId(index + 1),
            length: ExactRatio::new(length, 1).map_err(|_| An19PetalError::Overflow)?,
            weight: ratio(1, 1)?,
        });
    }
    let maximum_coordinate =
        i128::try_from(nodes.saturating_mul(64)).map_err(|_| An19PetalError::Overflow)?;
    let graph = SourceDynamicGraph::new(nodes, edges, maximum_coordinate)
        .map_err(|_| An19PetalError::InvalidDomain)?;
    let mut segments = An19EventSegmentMetadata::from_graph(&graph)?;
    let portal_generation = if family == An19AdversarialFamily::RepeatedPortalSplitting {
        u64::try_from(nodes - 1).map_err(|_| An19PetalError::Overflow)?
    } else {
        0
    };
    for (index, segment) in segments.iter_mut().enumerate() {
        if family == An19AdversarialFamily::RepeatedPortalSplitting {
            segment.source_edge_id = Some(0);
            segment.segment_lineage_root_id = 0;
            segment.portal_split_generation =
                u64::try_from(index).map_err(|_| An19PetalError::Overflow)?;
        }
        if family == An19AdversarialFamily::VirtualRealMixedSegments && index % 3 == 1 {
            segment.source_edge_id = None;
        }
        if family == An19AdversarialFamily::HighwayHalvingReorder
            && logical_depth > 0
            && index % 2 == 0
        {
            segment.highway_halved = true;
            segment.symbolic_unsplit_rounded_length = ratio(4, 1)?.into();
        }
        segment.projection_generation = logical_depth;
    }
    let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
    Ok(OwnedEventProblem {
        graph,
        cluster: cluster.clone(),
        remaining: cluster,
        center: FlowNodeId(0),
        target: FlowNodeId(nodes - 1),
        budget: ExactRatio::new(total_length.max(3) / 3, 1)
            .map_err(|_| An19PetalError::Overflow)?,
        context: An19EventContext {
            cluster_id: logical_depth,
            projection_snapshot_id: logical_depth,
            logical_partition_depth: logical_depth,
            recursion_parent_id: logical_depth.checked_sub(1),
            portal_split_generation: portal_generation,
            contraction_generation: 0,
            projection_generation: logical_depth,
        },
        segments,
    })
}

fn power_of_two_chord_problem(
    nodes: usize,
    _family: An19AdversarialFamily,
) -> Result<OwnedEventProblem, An19PetalError> {
    let mut edges = (0..nodes - 1)
        .map(|index| SourceWeightedEdge {
            first: FlowNodeId(index),
            second: FlowNodeId(index + 1),
            length: ratio(1, 1).expect("constant ratio"),
            weight: ratio(1, 1).expect("constant ratio"),
        })
        .collect::<Vec<_>>();
    for index in 0..nodes - 2 {
        let distance = nodes - 1 - index;
        edges.push(SourceWeightedEdge {
            first: FlowNodeId(index),
            second: FlowNodeId(nodes - 1),
            length: ExactRatio::new(
                i128::try_from(distance.next_power_of_two())
                    .map_err(|_| An19PetalError::Overflow)?,
                1,
            )
            .map_err(|_| An19PetalError::Overflow)?,
            weight: ratio(1, 1)?,
        });
    }
    let maximum_coordinate =
        i128::try_from(nodes.saturating_mul(128)).map_err(|_| An19PetalError::Overflow)?;
    let graph = SourceDynamicGraph::new(nodes, edges, maximum_coordinate)
        .map_err(|_| An19PetalError::InvalidDomain)?;
    let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
    let segments = An19EventSegmentMetadata::from_graph(&graph)?;
    Ok(OwnedEventProblem {
        graph,
        cluster: cluster.clone(),
        remaining: cluster,
        center: FlowNodeId(0),
        target: FlowNodeId(nodes - 1),
        budget: ExactRatio::new(
            i128::try_from(nodes / 4).map_err(|_| An19PetalError::Overflow)?,
            1,
        )
        .map_err(|_| An19PetalError::Overflow)?,
        context: An19EventContext {
            cluster_id: u64::try_from(nodes).map_err(|_| An19PetalError::Overflow)?,
            projection_snapshot_id: u64::try_from(nodes).map_err(|_| An19PetalError::Overflow)?,
            logical_partition_depth: 0,
            recursion_parent_id: None,
            portal_split_generation: 0,
            contraction_generation: 0,
            projection_generation: 0,
        },
        segments,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_rejected(run: &An19EventRun, problem: &An19EventProblem<'_>) {
        assert_eq!(
            run.verify_against(problem),
            Err(An19PetalError::InvalidEventTrace)
        );
    }

    #[test]
    fn event_engine_path_snapshot_matches_exact_oracle() {
        let owned = path_problem(16, An19AdversarialFamily::AllEqualReducedKeys, 0).unwrap();
        let problem = owned.as_problem();
        let (oracle, reduced) = An19ReducedEventEngine::run_differential(&problem).unwrap();
        assert!(oracle.semantically_agrees(&reduced));
        assert!(oracle.runtime_status.differential_verified);
        assert!(reduced.runtime_status.differential_verified);
        assert!(reduced.runtime_status.local_event_bound_proved);
        assert!(
            reduced.local_event_bound.semantic_event_count
                <= reduced.local_event_bound.semantic_event_bound
        );
        assert!(
            reduced.local_event_bound.queue_insertion_count
                <= reduced.local_event_bound.queue_item_bound
        );
        assert!(
            !reduced
                .local_event_bound
                .priority_queue_comparison_bound_included
        );
        assert!(!reduced.runtime_status.an19_runtime_verified);
        oracle.verify_trace().unwrap();
        reduced.verify_trace().unwrap();
        let mut metrics = An19PetalMetrics::default();
        let paths =
            shortest_paths(problem.graph, problem.cluster, problem.center, &mut metrics).unwrap();
        for event in reduced.semantic_trace.iter().filter(|event| {
            matches!(
                event.event_type,
                An19EventType::OutsideToBoundaryEdgeTransition
                    | An19EventType::BoundaryToInternalEdgeTransition
            )
        }) {
            let edge = problem
                .graph
                .edge(SourceEdgeId(event.active_segment_id.unwrap()))
                .unwrap();
            let (from, to) = match event.orientation.unwrap() {
                An19EventOrientation::FirstToSecond => (edge.first, edge.second),
                An19EventOrientation::SecondToFirst => (edge.second, edge.first),
            };
            let expected = edge
                .length
                .checked_add(paths.distances[from.0].unwrap())
                .and_then(|value| value.checked_sub(paths.distances[to.0].unwrap()))
                .and_then(|value| value.checked_mul_integer(2))
                .unwrap();
            assert_eq!(
                ExactRatio::try_from(event.exact_reduced_cost.unwrap()).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn event_engine_bounded_adversarial_campaign_covers_all_families() {
        let campaign = An19AdversarialCampaign::run(
            &An19AdversarialFamily::ALL,
            &[16, 32],
            "test-sha".to_owned(),
            "bounded-test".to_owned(),
        )
        .unwrap();
        assert!(campaign.cases.iter().all(|case| case.oracle_agreement));
        assert!(An19AdversarialFamily::ALL.iter().all(|family| {
            campaign
                .cases
                .iter()
                .any(|case| case.input_family == *family)
        }));
        assert!(
            campaign
                .cases
                .iter()
                .all(|case| case.charge_analyses.len() == 6)
        );
        assert!(!campaign.naive_reduced_class_conversion_survived);
        assert!(campaign.runtime_status.local_event_bound_proved);
        assert!(!campaign.runtime_status.priority_queue_bound_proved);
        assert!(!campaign.runtime_status.an19_runtime_verified);
    }

    #[test]
    fn event_engine_highway_halving_fixture_reorders_reverse_keys() {
        let snapshots =
            adversarial_problems(An19AdversarialFamily::HighwayHalvingReorder, 16).unwrap();
        assert_eq!(snapshots.len(), 2);
        let reverse_costs = snapshots
            .iter()
            .map(|snapshot| {
                let problem = snapshot.as_problem();
                let mut metrics = An19PetalMetrics::default();
                let paths = fast_shortest_paths(
                    problem.graph,
                    problem.cluster,
                    problem.center,
                    &mut metrics,
                )
                .unwrap();
                let (adjacency, _) =
                    traced_reduced_adjacency(problem.graph, problem.remaining, &paths.distances)
                        .unwrap();
                [0, 1].map(|edge_id| {
                    adjacency
                        .iter()
                        .flatten()
                        .find(|arc| {
                            arc.edge == SourceEdgeId(edge_id)
                                && arc.orientation == An19EventOrientation::SecondToFirst
                        })
                        .unwrap()
                        .reduced_cost
                })
            })
            .collect::<Vec<_>>();
        assert!(ratio_less(reverse_costs[0][1], reverse_costs[0][0]).unwrap());
        assert!(ratio_less(reverse_costs[1][0], reverse_costs[1][1]).unwrap());
    }

    #[test]
    fn event_engine_proved_placeholder_is_explicitly_unavailable() {
        let owned = path_problem(12, An19AdversarialFamily::AllEqualReducedKeys, 0).unwrap();
        assert_eq!(
            ProvedEventEngine.run(&owned.as_problem()),
            Err(An19PetalError::UnprovedEventEngine)
        );
    }

    #[test]
    fn event_engine_exact_ratio_record_serializes_without_floating_point() {
        let record = An19ExactRatioRecord {
            numerator: -7,
            denominator: 13,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(json, r#"{"numerator":-7,"denominator":13}"#);
        assert_eq!(
            serde_json::from_str::<An19ExactRatioRecord>(&json).unwrap(),
            record
        );
        assert_eq!(
            ExactRatio::try_from(record).unwrap(),
            ExactRatio::new(-7, 13).unwrap()
        );
    }

    #[test]
    fn event_engine_trace_mutations_are_rejected() {
        let owned = path_problem(20, An19AdversarialFamily::RepeatedPortalSplitting, 2).unwrap();
        let problem = owned.as_problem();
        let (_, original) = An19ReducedEventEngine::run_differential(&problem).unwrap();

        let semantic_with_segment = original
            .semantic_trace
            .iter()
            .position(|event| event.active_segment_id.is_some())
            .unwrap();
        let semantic_with_reduced = original
            .semantic_trace
            .iter()
            .position(|event| event.exact_reduced_cost.is_some())
            .unwrap();
        let stale = original
            .semantic_trace
            .iter()
            .position(|event| event.stale)
            .unwrap();

        let mut changed = original.clone();
        changed.semantic_trace[semantic_with_reduced]
            .exact_reduced_cost
            .as_mut()
            .unwrap()
            .numerator += 1;
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[0].exact_event_radius.numerator += 1;
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[semantic_with_segment].source_edge_id = Some(usize::MAX);
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[0].logical_partition_depth += 1;
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[semantic_with_segment].segment_lineage_root_id = Some(usize::MAX);
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[semantic_with_segment].highway_halved = Some(true);
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[0].tie_break_fields[0] ^= 1;
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[stale].stale = false;
        changed.semantic_trace[stale].stale_reason = None;
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        let duplicate = changed.semantic_trace[0].clone();
        changed.semantic_trace.insert(1, duplicate);
        for (index, event) in changed.semantic_trace.iter_mut().enumerate() {
            event.event_sequence_number = u64::try_from(index).unwrap();
        }
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.semantic_trace[0].state_after.active_vertices += 1;
        assert_rejected(&changed, &problem);

        let mut changed = original.clone();
        changed.local_event_bound.semantic_event_bound += 1;
        assert_eq!(
            changed.verify_trace(),
            Err(An19PetalError::InvalidEventTrace)
        );

        let mut changed = original.clone();
        changed
            .local_event_bound
            .priority_queue_comparison_bound_included = true;
        assert_eq!(
            changed.verify_trace(),
            Err(An19PetalError::InvalidEventTrace)
        );
    }
}
