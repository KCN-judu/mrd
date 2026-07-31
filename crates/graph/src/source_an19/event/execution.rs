use std::collections::{BTreeMap, BTreeSet};

use crate::source_an19::{
    event::{
        backend::Kind,
        model::{
            ChargeAnalysis, ChargeKind, Count, Problem, Run, RuntimeStatus, SnapshotMetrics,
            StoppingCertificate,
        },
        queue, trace,
    },
    petal::{
        Error, ExactHeapEntry, FigureSixSelection, MembershipThresholds, PetalMetrics,
        RecoveredPath, ShortestPaths, hierarchy_or_oracle_paths, portal_is_interior, ratio,
        ratio_less, validate_weighted_domain,
    },
};
use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId};

#[derive(Clone)]
pub(in crate::source_an19) struct ArcWitness {
    pub(in crate::source_an19) edge: SourceEdgeId,
    pub(in crate::source_an19) to: FlowNodeId,
    pub(in crate::source_an19) reduced_cost: ExactRatio,
    pub(in crate::source_an19) orientation: trace::Orientation,
    pub(in crate::source_an19) directed_incidence: usize,
}

pub(in crate::source_an19) struct Preparation {
    pub(in crate::source_an19) path: RecoveredPath,
    pub(in crate::source_an19) center_distances: Vec<Option<ExactRatio>>,
    pub(in crate::source_an19) thresholds: MembershipThresholds,
    pub(in crate::source_an19) selection: FigureSixSelection,
    pub(in crate::source_an19) witnesses: Vec<Option<ArcWitness>>,
    pub(in crate::source_an19) queue_observations: Vec<queue::Observation>,
    pub(in crate::source_an19) queue_statistics: queue::Statistics,
    pub(in crate::source_an19) distinct_reduced_costs: BTreeSet<(String, String)>,
}

pub(in crate::source_an19) fn validate_problem(problem: &Problem<'_>) -> Result<(), Error> {
    validate_weighted_domain(
        problem.graph,
        problem.cluster,
        problem.remaining,
        problem.center,
        problem.target,
        problem.budget.clone(),
    )?;
    if !problem.budget.is_positive()
        || problem.segments.len() != problem.graph.edge_count()
        || problem
            .segments
            .iter()
            .enumerate()
            .any(|(index, segment)| segment.active_segment_id != index)
    {
        return Err(Error::InvalidEventTrace);
    }
    for (index, metadata) in problem.segments.iter().enumerate() {
        let edge = problem
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidDomain)?;
        let symbolic = ExactRatio::try_from(metadata.symbolic_unsplit_rounded_length.clone())?;
        if !symbolic.is_positive()
            || metadata.portal_split_generation > problem.context.portal_split_generation
            || metadata.contraction_generation > problem.context.contraction_generation
            || metadata.projection_generation > problem.context.projection_generation
            || !edge.length.is_positive()
        {
            return Err(Error::InvalidEventTrace);
        }
    }
    Ok(())
}

pub(in crate::source_an19) fn validate_path(
    problem: &Problem<'_>,
    path: &RecoveredPath,
    cluster_paths: &ShortestPaths,
    fast: bool,
    metrics: &mut PetalMetrics,
) -> Result<(), Error> {
    if path
        .vertices
        .iter()
        .any(|vertex| !problem.remaining.contains(vertex))
    {
        return Err(Error::InvalidDomain);
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
            return Err(Error::InvalidDomain);
        }
    }
    let target_distance = cluster_paths.distances[problem.target.0]
        .clone()
        .ok_or(Error::Disconnected)?;
    if ratio_less(target_distance, problem.budget.clone())? {
        return Err(Error::InvalidRadius);
    }
    Ok(())
}

pub(in crate::source_an19) fn build_run(
    problem: &Problem<'_>,
    engine: Kind,
    preparation: &Preparation,
) -> Result<Run, Error> {
    let selected_vertices = vertices_at_selected_radius(
        problem.remaining,
        &preparation.thresholds,
        preparation.selection.radius.clone(),
    )?;
    let (internal_edge_ids, boundary_edge_ids) =
        edge_partitions(problem.graph, problem.cluster, &selected_vertices)?;
    if internal_edge_ids.len() != preparation.selection.internal_edges
        || boundary_edge_ids.len() != preparation.selection.boundary_edges
    {
        return Err(Error::InvalidEventTrace);
    }
    let semantic_trace = build_semantic_trace(problem, preparation)?;
    let queue_trace = build_queue_trace(problem, &preparation.queue_observations)?;
    let metrics = build_snapshot_metrics(problem, preparation, &semantic_trace, &queue_trace)?;
    let local_event_bound = super::certificate::build_local_event_bound(
        problem,
        &semantic_trace,
        &queue_trace,
        &metrics,
    )?;
    let practical_queue_bound = match engine {
        Kind::Experiment => Some(super::certificate::build_practical_queue_bound(
            problem,
            &preparation.queue_statistics,
        )?),
        Kind::Oracle => None,
        Kind::ProvedUnavailable => {
            return Err(Error::UnprovedEventEngine);
        }
    };
    let charge_analyses = analyze_all_charge_maps(&semantic_trace)?;
    let runtime_status = match engine {
        Kind::Oracle => RuntimeStatus {
            semantics_implemented: true,
            exact_oracle_verified: true,
            differential_verified: false,
            trace_complete: true,
            local_event_bound_proved: true,
            global_amortization_proved: false,
            priority_queue_bound_proved: false,
            an19_runtime_verified: false,
        },
        Kind::Experiment => RuntimeStatus {
            semantics_implemented: true,
            exact_oracle_verified: false,
            differential_verified: false,
            trace_complete: true,
            local_event_bound_proved: true,
            global_amortization_proved: false,
            priority_queue_bound_proved: false,
            an19_runtime_verified: false,
        },
        Kind::ProvedUnavailable => {
            return Err(Error::UnprovedEventEngine);
        }
    };
    let run = Run {
        engine,
        selected_radius: preparation.selection.radius.clone().into(),
        selected_vertices: selected_vertices.iter().map(|vertex| vertex.0).collect(),
        internal_edge_ids,
        boundary_edge_ids,
        path_edge_ids: preparation.path.edges.iter().map(|edge| edge.0).collect(),
        stopping_certificate: StoppingCertificate {
            window_index: preparation.selection.window_index,
            window_start: preparation.selection.window_start.clone().into(),
            window_end: preparation.selection.window_end.clone().into(),
            selected_radius: preparation.selection.radius.clone().into(),
            internal_edges: preparation.selection.internal_edges,
            boundary_edges: preparation.selection.boundary_edges,
            cluster_edges: preparation.selection.cluster_edges,
        },
        semantic_trace,
        queue_trace,
        metrics,
        local_event_bound,
        practical_queue_bound,
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
) -> Result<BTreeSet<FlowNodeId>, Error> {
    let mut vertices = BTreeSet::new();
    for vertex in remaining {
        let Some(ref threshold) = thresholds.by_vertex[vertex.0] else {
            continue;
        };
        if !ratio_less(radius.clone(), threshold.clone())? {
            vertices.insert(*vertex);
        }
    }
    Ok(vertices)
}

fn edge_partitions(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    vertices: &BTreeSet<FlowNodeId>,
) -> Result<(Vec<usize>, Vec<usize>), Error> {
    let mut internal = Vec::new();
    let mut boundary = Vec::new();
    for index in 0..graph.edge_count() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidDomain)?;
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
) -> Result<Vec<ExactHeapEntry>, Error> {
    let mut entries = remaining
        .iter()
        .filter_map(|vertex| {
            thresholds.by_vertex[vertex.0]
                .clone()
                .map(|distance| ExactHeapEntry {
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
            let less = ratio_less(first.distance.clone(), second.distance.clone())?
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
    problem: &Problem<'_>,
    preparation: &Preparation,
) -> Result<Vec<trace::Record>, Error> {
    let entries = sorted_threshold_entries(problem.remaining, &preparation.thresholds)?;
    let mut incident = vec![Vec::new(); problem.graph.node_count()];
    for index in 0..problem.graph.edge_count() {
        let edge = problem
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidDomain)?;
        if problem.cluster.contains(&edge.first) && problem.cluster.contains(&edge.second) {
            incident[edge.first.0].push(index);
            incident[edge.second.0].push(index);
        }
    }
    let mut trace = Vec::new();
    let mut state = trace::State::default();
    let mut active = vec![false; problem.graph.node_count()];
    let mut edge_state = vec![0_u8; problem.graph.edge_count()];
    let mut structural_events_emitted = false;
    let mut cursor = 0;
    while cursor < entries.len() {
        let radius = entries[cursor].distance.clone();
        if !structural_events_emitted
            && ratio_less(preparation.selection.radius.clone(), radius.clone())?
        {
            append_structural_events(problem, preparation, state, &mut trace)?;
            structural_events_emitted = true;
        }
        let group_start = cursor;
        while cursor < entries.len() && entries[cursor].distance == radius {
            cursor += 1;
        }
        for entry in &entries[group_start..cursor] {
            let after_stop = ratio_less(preparation.selection.radius.clone(), radius.clone())?;
            let before = state;
            if !after_stop && !active[entry.vertex.0] {
                active[entry.vertex.0] = true;
                state.active_vertices = state
                    .active_vertices
                    .checked_add(1)
                    .ok_or(Error::Overflow)?;
            }
            trace.push(make_trace_record(
                problem,
                trace::Kind::VertexEntry,
                radius.clone(),
                preparation.witnesses[entry.vertex.0].clone(),
                None,
                Some(entry.vertex),
                before,
                state,
                after_stop,
                after_stop.then_some(trace::StaleReason::AfterStoppingRadius),
                None,
                None,
            )?);
            if after_stop {
                continue;
            }
            if preparation.thresholds.path_distance_from_target[entry.vertex.0].is_some() {
                trace.push(make_trace_record(
                    problem,
                    trace::Kind::HighwayEndpoint,
                    radius.clone(),
                    preparation.witnesses[entry.vertex.0].clone(),
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
                    .ok_or(Error::InvalidDomain)?;
                let other = if edge.first == entry.vertex {
                    edge.second
                } else {
                    edge.first
                };
                let transition = if active[other.0] {
                    if edge_state[*edge_index] == 1 {
                        Some(trace::Kind::BoundaryToInternalEdgeTransition)
                    } else {
                        None
                    }
                } else if edge_state[*edge_index] == 0 {
                    Some(trace::Kind::OutsideToBoundaryEdgeTransition)
                } else {
                    None
                };
                let Some(event_type) = transition else {
                    continue;
                };
                let transition_before = state;
                match event_type {
                    trace::Kind::OutsideToBoundaryEdgeTransition => {
                        edge_state[*edge_index] = 1;
                        state.boundary_edges =
                            state.boundary_edges.checked_add(1).ok_or(Error::Overflow)?;
                    }
                    trace::Kind::BoundaryToInternalEdgeTransition => {
                        edge_state[*edge_index] = 2;
                        state.boundary_edges = state
                            .boundary_edges
                            .checked_sub(1)
                            .ok_or(Error::InvalidEventTrace)?;
                        state.internal_edges =
                            state.internal_edges.checked_add(1).ok_or(Error::Overflow)?;
                    }
                    _ => return Err(Error::InvalidEventTrace),
                }
                let orientation = if edge.first == entry.vertex {
                    trace::Orientation::FirstToSecond
                } else {
                    trace::Orientation::SecondToFirst
                };
                let incidence = edge_index
                    .checked_mul(2)
                    .and_then(|value| {
                        value.checked_add(usize::from(
                            orientation == trace::Orientation::SecondToFirst,
                        ))
                    })
                    .ok_or(Error::Overflow)?;
                let from_distance = preparation.center_distances[entry.vertex.0]
                    .clone()
                    .ok_or(Error::Disconnected)?;
                let to_distance = preparation.center_distances[other.0]
                    .clone()
                    .ok_or(Error::Disconnected)?;
                let reduced_cost = edge
                    .length
                    .checked_add(&from_distance)
                    .and_then(|value| value.checked_sub(&to_distance))
                    .and_then(|value| value.checked_mul_integer(2))
                    .map_err(|_| Error::Overflow)?;
                if reduced_cost.is_negative() {
                    return Err(Error::InvalidHighway);
                }
                trace.push(make_trace_record(
                    problem,
                    event_type,
                    radius.clone(),
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
                        trace::Kind::VirtualSegmentEvent,
                        radius.clone(),
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
        if !ratio_less(radius.clone(), preparation.selection.window_start.clone())?
            && !ratio_less(preparation.selection.radius.clone(), radius.clone())?
        {
            trace.push(make_trace_record(
                problem,
                trace::Kind::StoppingConditionCheck,
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
        event.event_sequence_number = u64::try_from(index).map_err(|_| Error::Overflow)?;
    }
    Ok(trace)
}

fn append_structural_events(
    problem: &Problem<'_>,
    preparation: &Preparation,
    state: trace::State,
    trace: &mut Vec<trace::Record>,
) -> Result<(), Error> {
    if portal_is_interior(
        &preparation.thresholds,
        preparation.selection.radius.clone(),
    ) {
        trace.push(make_trace_record(
            problem,
            trace::Kind::PortalSplit,
            preparation.selection.radius.clone(),
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
            trace::Kind::ContractionRelatedEvent,
            preparation.selection.radius.clone(),
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
    problem: &Problem<'_>,
    event_type: trace::Kind,
    radius: ExactRatio,
    witness: Option<ArcWitness>,
    explicit_segment: Option<usize>,
    vertex: Option<FlowNodeId>,
    state_before: trace::State,
    state_after: trace::State,
    stale: bool,
    stale_reason: Option<trace::StaleReason>,
    insertion_sequence: Option<u64>,
    pop_sequence: Option<u64>,
) -> Result<trace::Record, Error> {
    let segment_index = explicit_segment.or_else(|| witness.clone().map(|value| value.edge.0));
    let metadata = segment_index.and_then(|index| problem.segments.get(index));
    let edge = segment_index
        .map(|index| {
            problem
                .graph
                .edge(SourceEdgeId(index))
                .ok_or(Error::InvalidDomain)
        })
        .transpose()?;
    let type_code = event_type_code(event_type);
    let source = metadata.and_then(|value| value.source_edge_id);
    let depth = problem.context.logical_partition_depth;
    let transition_code = match event_type {
        trace::Kind::OutsideToBoundaryEdgeTransition => 1,
        trace::Kind::BoundaryToInternalEdgeTransition => 2,
        _ => 0,
    };
    Ok(trace::Record {
        cluster_id: problem.context.cluster_id,
        projection_snapshot_id: problem.context.projection_snapshot_id,
        logical_partition_depth: depth,
        recursion_parent_id: problem.context.recursion_parent_id,
        event_sequence_number: 0,
        event_type,
        source_edge_id: source,
        active_segment_id: metadata.map(|value| value.active_segment_id),
        segment_lineage_root_id: metadata.map(|value| value.segment_lineage_root_id),
        orientation: witness.clone().map(|value| value.orientation),
        exact_materialized_segment_length: edge.map(|value| value.length.clone().into()),
        symbolic_unsplit_rounded_length: metadata
            .map(|value| value.symbolic_unsplit_rounded_length.clone()),
        highway_halved: metadata.map(|value| value.highway_halved),
        exact_reduced_cost: witness
            .clone()
            .map(|value| value.reduced_cost.clone().into()),
        exact_event_radius: radius.into(),
        queue_insertion_sequence: insertion_sequence,
        queue_pop_sequence: pop_sequence,
        stale,
        stale_reason,
        state_before,
        state_after,
        endpoint_ids: edge.map(|value| [value.first.0, value.second.0]),
        affected_vertex_id: vertex.map(|value| value.0),
        affected_directed_incidence_id: witness.clone().map(|value| value.directed_incidence),
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
    problem: &Problem<'_>,
    observations: &[queue::Observation],
) -> Result<Vec<trace::Record>, Error> {
    let mut trace = Vec::with_capacity(observations.len());
    for observation in observations {
        let event_type = if observation.insertion {
            trace::Kind::QueueInsertion
        } else if observation.stale_reason.is_some() {
            trace::Kind::StaleQueueEvent
        } else {
            trace::Kind::VertexEntry
        };
        trace.push(make_trace_record(
            problem,
            event_type,
            observation.item.distance.clone(),
            observation.item.predecessor.clone(),
            None,
            Some(observation.item.vertex),
            trace::State::default(),
            trace::State::default(),
            observation.stale_reason.is_some(),
            observation.stale_reason,
            Some(observation.item.insertion_sequence),
            observation.pop_sequence,
        )?);
    }
    for (index, event) in trace.iter_mut().enumerate() {
        event.event_sequence_number = u64::try_from(index).map_err(|_| Error::Overflow)?;
    }
    Ok(trace)
}

const fn event_type_code(event_type: trace::Kind) -> u64 {
    match event_type {
        trace::Kind::VertexEntry => 0,
        trace::Kind::OutsideToBoundaryEdgeTransition => 1,
        trace::Kind::BoundaryToInternalEdgeTransition => 2,
        trace::Kind::HighwayEndpoint => 3,
        trace::Kind::PortalSplit => 4,
        trace::Kind::VirtualSegmentEvent => 5,
        trace::Kind::ContractionRelatedEvent => 6,
        trace::Kind::QueueInsertion => 7,
        trace::Kind::StaleQueueEvent => 8,
        trace::Kind::StoppingConditionCheck => 9,
    }
}

#[allow(clippy::too_many_lines)]
fn build_snapshot_metrics(
    problem: &Problem<'_>,
    preparation: &Preparation,
    semantic_trace: &[trace::Record],
    _queue_trace: &[trace::Record],
) -> Result<SnapshotMetrics, Error> {
    let mut original_classes = BTreeSet::new();
    let mut materialized_classes = BTreeSet::new();
    let mut symbolic_source_classes = BTreeSet::new();
    let mut symbolic_virtual_classes = BTreeSet::new();
    let mut active_segments = 0_u64;
    for index in 0..problem.graph.edge_count() {
        let edge = problem
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidDomain)?;
        if !problem.remaining.contains(&edge.first) || !problem.remaining.contains(&edge.second) {
            continue;
        }
        active_segments = active_segments.checked_add(1).ok_or(Error::Overflow)?;
        let materialized = (edge.length.numerator(), edge.length.denominator());
        original_classes.insert(materialized);
        materialized_classes.insert(materialized);
        let metadata = &problem.segments[index];
        let mut symbolic = ExactRatio::try_from(metadata.symbolic_unsplit_rounded_length.clone())?;
        if metadata.highway_halved {
            symbolic = symbolic
                .checked_mul(&ratio(1, 2)?)
                .map_err(|_| Error::Overflow)?;
        }
        let class = (
            symbolic.numerator().to_string(),
            symbolic.denominator().to_string(),
        );
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
        .filter(|event| event.event_type == trace::Kind::VertexEntry)
        .count();
    let incidence_transitions = counted_semantic
        .iter()
        .filter(|event| {
            matches!(
                event.event_type,
                trace::Kind::OutsideToBoundaryEdgeTransition
                    | trace::Kind::BoundaryToInternalEdgeTransition
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
            .clone()
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
    Ok(SnapshotMetrics {
        active_vertex_count: u64::try_from(problem.remaining.len()).map_err(|_| Error::Overflow)?,
        active_directed_arc_count: active_segments.checked_mul(2).ok_or(Error::Overflow)?,
        active_undirected_segment_count: active_segments,
        original_length_class_count: u64::try_from(original_classes.len())
            .map_err(|_| Error::Overflow)?,
        symbolic_source_label_class_count: u64::try_from(symbolic_source_classes.len())
            .map_err(|_| Error::Overflow)?,
        symbolic_virtual_label_class_count: u64::try_from(symbolic_virtual_classes.len())
            .map_err(|_| Error::Overflow)?,
        materialized_exact_length_class_count: u64::try_from(materialized_classes.len())
            .map_err(|_| Error::Overflow)?,
        distinct_reduced_cost_count: u64::try_from(preparation.distinct_reduced_costs.len())
            .map_err(|_| Error::Overflow)?,
        distinct_event_radius_count: u64::try_from(event_radii.len())
            .map_err(|_| Error::Overflow)?,
        candidate_event_count: u64::try_from(
            preparation
                .thresholds
                .by_vertex
                .iter()
                .filter(|value| value.is_some())
                .count(),
        )
        .map_err(|_| Error::Overflow)?,
        inserted_queue_item_count: preparation.queue_statistics.inserted,
        popped_queue_item_count: preparation.queue_statistics.popped,
        stale_queue_item_count: preparation.queue_statistics.stale,
        exact_comparison_count: preparation.queue_statistics.comparisons,
        decrease_key_or_replacement_count: preparation.queue_statistics.replacements,
        equal_key_tie_count: preparation.queue_statistics.equal_key_ties,
        maximum_queue_size: preparation.queue_statistics.maximum_size,
        vertex_entry_count: u64::try_from(vertex_entries).map_err(|_| Error::Overflow)?,
        directed_incidence_transition_count: u64::try_from(incidence_transitions)
            .map_err(|_| Error::Overflow)?,
        events_per_source_edge,
        events_per_segment_lineage,
        events_per_logical_partition_depth,
        events_per_symbolic_label,
        events_created_by_portal_split,
        events_created_by_contraction,
        events_created_by_projection_rebuild,
        events_preserved_by_incremental_projection_updates: u64::try_from(preserved)
            .map_err(|_| Error::Overflow)?,
    })
}

fn count_trace_keys<F>(trace: &[&trace::Record], key: F) -> Result<Vec<Count>, Error>
where
    F: Fn(&trace::Record) -> Option<String>,
{
    let mut counts = BTreeMap::<String, u64>::new();
    for event in trace {
        let Some(value) = key(event) else {
            continue;
        };
        let count = counts.entry(value).or_default();
        *count = count.checked_add(1).ok_or(Error::Overflow)?;
    }
    Ok(counts
        .into_iter()
        .map(|(key, count)| Count { key, count })
        .collect())
}

fn analyze_all_charge_maps(trace: &[trace::Record]) -> Result<Vec<ChargeAnalysis>, Error> {
    [
        ChargeKind::SourceDepth,
        ChargeKind::LineageEvent,
        ChargeKind::SourceDepthEvent,
        ChargeKind::DirectedIncidenceTransition,
        ChargeKind::PortalSplitDescendant,
        ChargeKind::SnapshotSegmentEvent,
    ]
    .into_iter()
    .map(|map| analyze_charge_map(trace, map))
    .collect()
}

fn analyze_charge_map(trace: &[trace::Record], map: ChargeKind) -> Result<ChargeAnalysis, Error> {
    let mut fibers = BTreeMap::<String, Vec<u64>>::new();
    for event in trace.iter().filter(|event| !event.stale) {
        let key = match map {
            ChargeKind::SourceDepth => event
                .charge_source_depth
                .map(|value| format!("{}:{}", value[0], value[1])),
            ChargeKind::LineageEvent => event
                .charge_lineage_event
                .map(|value| format!("{}:{}", value[0], value[1])),
            ChargeKind::SourceDepthEvent => event
                .charge_source_depth_event
                .map(|value| format!("{}:{}:{}", value[0], value[1], value[2])),
            ChargeKind::DirectedIncidenceTransition => event
                .charge_incidence_transition
                .map(|value| format!("{}:{}", value[0], value[1])),
            ChargeKind::PortalSplitDescendant => event
                .charge_portal_descendant
                .map(|value| format!("{}:{}", value[0], value[1])),
            ChargeKind::SnapshotSegmentEvent => event
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
        *count = count.checked_add(1).ok_or(Error::Overflow)?;
    }
    Ok(ChargeAnalysis {
        map,
        charge_targets: u64::try_from(fibers.len()).map_err(|_| Error::Overflow)?,
        maximum_fiber_size: u64::try_from(maximum).map_err(|_| Error::Overflow)?,
        histogram: histogram
            .into_iter()
            .map(|(size, count)| Count {
                key: size.to_string(),
                count,
            })
            .collect(),
        worst_witness_event_sequence_numbers: worst,
        observed_growth_with_input_size: None,
    })
}
