use serde::{Deserialize, Serialize};

use super::{
    backend::Kind,
    model::{Problem, Run, SnapshotMetrics},
    queue, trace,
};
use crate::source_an19::petal::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalBound {
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
#[serde(rename_all = "snake_case")]
pub enum QueueStrategy {
    StableBinaryMinHeap,
    LinearMinimumScan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueProofScope {
    ReducedEngineFixedSnapshot,
    #[serde(rename = "an19_runtime_target")]
    SourceRuntimeTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QueueBound {
    pub schema_version: u32,
    pub strategy: QueueStrategy,
    pub proof_scope: QueueProofScope,
    pub queue_insertion_count: u64,
    pub queue_pop_count: u64,
    pub edge_count: u64,
    pub heap_height_bound: u32,
    pub push_comparison_bound: u64,
    pub pop_comparison_bound: u64,
    pub relaxation_label_comparison_bound: u64,
    pub total_comparison_bound: u64,
    pub observed_push_comparisons: u64,
    pub observed_pop_comparisons: u64,
    pub observed_relaxation_label_comparisons: u64,
    pub observed_total_comparisons: u64,
    pub an19_priority_queue_target_proved: bool,
}

struct LocalCounts {
    semantic: u64,
    vertex_entries: u64,
    highway_endpoints: u64,
    stopping_checks: u64,
    transitions: u64,
    virtual_events: u64,
    structural_events: u64,
    categorized: u64,
    queue_insertions: u64,
    queue_pops: u64,
    stale_queue_items: u64,
}

fn local_counts(run: &Run) -> Result<LocalCounts, Error> {
    let semantic = u64::try_from(run.semantic_trace.len()).map_err(|_| Error::Overflow)?;
    let count_semantic = |event_type| {
        u64::try_from(
            run.semantic_trace
                .iter()
                .filter(|event| event.event_type == event_type)
                .count(),
        )
        .map_err(|_| Error::Overflow)
    };
    let vertex_entries = count_semantic(trace::Kind::VertexEntry)?;
    let highway_endpoints = count_semantic(trace::Kind::HighwayEndpoint)?;
    let stopping_checks = count_semantic(trace::Kind::StoppingConditionCheck)?;
    let transitions = count_semantic(trace::Kind::OutsideToBoundaryEdgeTransition)?
        .checked_add(count_semantic(
            trace::Kind::BoundaryToInternalEdgeTransition,
        )?)
        .ok_or(Error::Overflow)?;
    let virtual_events = count_semantic(trace::Kind::VirtualSegmentEvent)?;
    let structural_events = count_semantic(trace::Kind::PortalSplit)?
        .checked_add(count_semantic(trace::Kind::ContractionRelatedEvent)?)
        .ok_or(Error::Overflow)?;
    let categorized = vertex_entries
        .checked_add(highway_endpoints)
        .and_then(|value| value.checked_add(stopping_checks))
        .and_then(|value| value.checked_add(transitions))
        .and_then(|value| value.checked_add(virtual_events))
        .and_then(|value| value.checked_add(structural_events))
        .ok_or(Error::Overflow)?;
    let count_queue = |predicate: fn(&trace::Record) -> bool| {
        u64::try_from(
            run.queue_trace
                .iter()
                .filter(|event| predicate(event))
                .count(),
        )
        .map_err(|_| Error::Overflow)
    };
    Ok(LocalCounts {
        semantic,
        vertex_entries,
        highway_endpoints,
        stopping_checks,
        transitions,
        virtual_events,
        structural_events,
        categorized,
        queue_insertions: count_queue(|event| event.event_type == trace::Kind::QueueInsertion)?,
        queue_pops: count_queue(|event| event.queue_pop_sequence.is_some())?,
        stale_queue_items: count_queue(|event| event.event_type == trace::Kind::StaleQueueEvent)?,
    })
}

pub(super) fn verify_local_event_bound(run: &Run) -> Result<(), Error> {
    let certificate = run.local_event_bound;
    let counts = local_counts(run)?;
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
        .ok_or(Error::Overflow)?;
    let queue_bound = certificate
        .edge_count
        .checked_mul(2)
        .and_then(|edges| certificate.vertex_count.checked_add(edges))
        .and_then(|value| value.checked_add(2))
        .ok_or(Error::Overflow)?;
    let twice_edges = certificate
        .edge_count
        .checked_mul(2)
        .ok_or(Error::Overflow)?;
    if certificate.schema_version != 1
        || certificate.priority_queue_comparison_bound_included
        || certificate.semantic_event_bound != semantic_bound
        || certificate.queue_item_bound != queue_bound
        || certificate.semantic_event_count != counts.semantic
        || certificate.candidate_vertex_event_count != run.metrics.candidate_event_count
        || certificate.vertex_entry_count != counts.vertex_entries
        || certificate.highway_endpoint_count != counts.highway_endpoints
        || certificate.stopping_check_count != counts.stopping_checks
        || certificate.directed_transition_count != counts.transitions
        || certificate.virtual_segment_event_count != counts.virtual_events
        || certificate.structural_event_count != counts.structural_events
        || certificate.queue_insertion_count != counts.queue_insertions
        || certificate.queue_pop_count != counts.queue_pops
        || certificate.stale_queue_item_count != counts.stale_queue_items
        || counts.categorized != counts.semantic
        || certificate.candidate_vertex_event_count > certificate.vertex_count
        || counts.vertex_entries > certificate.vertex_count
        || counts.highway_endpoints > certificate.vertex_count
        || counts.stopping_checks > certificate.vertex_count
        || counts.transitions > twice_edges
        || counts.virtual_events > counts.transitions
        || counts.structural_events > 2
        || counts.semantic > semantic_bound
        || counts.queue_insertions > queue_bound
        || counts.queue_pops != counts.queue_insertions
        || counts.stale_queue_items > counts.queue_pops
        || run.metrics.inserted_queue_item_count != counts.queue_insertions
        || run.metrics.popped_queue_item_count != counts.queue_pops
        || run.metrics.stale_queue_item_count != counts.stale_queue_items
        || run.metrics.directed_incidence_transition_count != counts.transitions
    {
        return Err(Error::InvalidEventTrace);
    }
    Ok(())
}

pub(super) fn verify_practical_queue_bound(run: &Run) -> Result<(), Error> {
    let Some(certificate) = run.practical_queue_bound else {
        return if run.engine == Kind::Oracle {
            Ok(())
        } else {
            Err(Error::InvalidEventTrace)
        };
    };
    if run.engine != Kind::Experiment {
        return Err(Error::InvalidEventTrace);
    }
    let expected_height = if certificate.queue_insertion_count <= 1 {
        0
    } else {
        u64::BITS - (certificate.queue_insertion_count - 1).leading_zeros()
    };
    let height = u64::from(expected_height);
    let expected_push_bound = certificate
        .queue_insertion_count
        .checked_mul(height)
        .ok_or(Error::Overflow)?;
    let expected_pop_bound = expected_push_bound.checked_mul(2).ok_or(Error::Overflow)?;
    let expected_label_bound = certificate
        .edge_count
        .checked_mul(2)
        .ok_or(Error::Overflow)?;
    let expected_total_bound = expected_push_bound
        .checked_add(expected_pop_bound)
        .and_then(|value| value.checked_add(expected_label_bound))
        .ok_or(Error::Overflow)?;
    let observed_total = certificate
        .observed_push_comparisons
        .checked_add(certificate.observed_pop_comparisons)
        .and_then(|value| value.checked_add(certificate.observed_relaxation_label_comparisons))
        .ok_or(Error::Overflow)?;
    if certificate.schema_version != 1
        || certificate.strategy != QueueStrategy::StableBinaryMinHeap
        || certificate.proof_scope != QueueProofScope::ReducedEngineFixedSnapshot
        || certificate.an19_priority_queue_target_proved
        || certificate.queue_insertion_count != run.local_event_bound.queue_insertion_count
        || certificate.queue_pop_count != run.local_event_bound.queue_pop_count
        || certificate.edge_count != run.local_event_bound.edge_count
        || certificate.heap_height_bound != expected_height
        || certificate.push_comparison_bound != expected_push_bound
        || certificate.pop_comparison_bound != expected_pop_bound
        || certificate.relaxation_label_comparison_bound != expected_label_bound
        || certificate.total_comparison_bound != expected_total_bound
        || certificate.observed_push_comparisons > certificate.push_comparison_bound
        || certificate.observed_pop_comparisons > certificate.pop_comparison_bound
        || certificate.observed_relaxation_label_comparisons
            > certificate.relaxation_label_comparison_bound
        || certificate.observed_total_comparisons != observed_total
        || certificate.observed_total_comparisons != run.metrics.exact_comparison_count
        || certificate.observed_total_comparisons > certificate.total_comparison_bound
    {
        return Err(Error::InvalidEventTrace);
    }
    Ok(())
}

pub(super) fn build_local_event_bound(
    problem: &Problem<'_>,
    semantic_trace: &[trace::Record],
    queue_trace: &[trace::Record],
    metrics: &SnapshotMetrics,
) -> Result<LocalBound, Error> {
    let vertex_count = u64::try_from(problem.remaining.len()).map_err(|_| Error::Overflow)?;
    let edge_count = u64::try_from(problem.graph.edge_count()).map_err(|_| Error::Overflow)?;
    let semantic_event_bound = vertex_count
        .checked_mul(3)
        .and_then(|value| {
            edge_count
                .checked_mul(4)
                .and_then(|edges| value.checked_add(edges))
        })
        .and_then(|value| value.checked_add(2))
        .ok_or(Error::Overflow)?;
    let queue_item_bound = edge_count
        .checked_mul(2)
        .and_then(|edges| vertex_count.checked_add(edges))
        .and_then(|value| value.checked_add(2))
        .ok_or(Error::Overflow)?;
    let count_semantic = |event_type| {
        u64::try_from(
            semantic_trace
                .iter()
                .filter(|event| event.event_type == event_type)
                .count(),
        )
        .map_err(|_| Error::Overflow)
    };
    let directed_transition_count = count_semantic(trace::Kind::OutsideToBoundaryEdgeTransition)?
        .checked_add(count_semantic(
            trace::Kind::BoundaryToInternalEdgeTransition,
        )?)
        .ok_or(Error::Overflow)?;
    let structural_event_count = count_semantic(trace::Kind::PortalSplit)?
        .checked_add(count_semantic(trace::Kind::ContractionRelatedEvent)?)
        .ok_or(Error::Overflow)?;
    Ok(LocalBound {
        schema_version: 1,
        vertex_count,
        edge_count,
        semantic_event_bound,
        queue_item_bound,
        semantic_event_count: u64::try_from(semantic_trace.len()).map_err(|_| Error::Overflow)?,
        candidate_vertex_event_count: metrics.candidate_event_count,
        vertex_entry_count: count_semantic(trace::Kind::VertexEntry)?,
        highway_endpoint_count: count_semantic(trace::Kind::HighwayEndpoint)?,
        stopping_check_count: count_semantic(trace::Kind::StoppingConditionCheck)?,
        directed_transition_count,
        virtual_segment_event_count: count_semantic(trace::Kind::VirtualSegmentEvent)?,
        structural_event_count,
        queue_insertion_count: u64::try_from(
            queue_trace
                .iter()
                .filter(|event| event.event_type == trace::Kind::QueueInsertion)
                .count(),
        )
        .map_err(|_| Error::Overflow)?,
        queue_pop_count: u64::try_from(
            queue_trace
                .iter()
                .filter(|event| event.queue_pop_sequence.is_some())
                .count(),
        )
        .map_err(|_| Error::Overflow)?,
        stale_queue_item_count: u64::try_from(
            queue_trace
                .iter()
                .filter(|event| event.event_type == trace::Kind::StaleQueueEvent)
                .count(),
        )
        .map_err(|_| Error::Overflow)?,
        priority_queue_comparison_bound_included: false,
    })
}

pub(super) fn build_practical_queue_bound(
    problem: &Problem<'_>,
    statistics: &queue::Statistics,
) -> Result<QueueBound, Error> {
    let insertions = statistics.inserted;
    let heap_height_bound = if insertions <= 1 {
        0
    } else {
        u64::BITS - (insertions - 1).leading_zeros()
    };
    let height = u64::from(heap_height_bound);
    let push_comparison_bound = insertions.checked_mul(height).ok_or(Error::Overflow)?;
    let pop_comparison_bound = push_comparison_bound
        .checked_mul(2)
        .ok_or(Error::Overflow)?;
    let edge_count = u64::try_from(problem.graph.edge_count()).map_err(|_| Error::Overflow)?;
    let relaxation_label_comparison_bound = edge_count.checked_mul(2).ok_or(Error::Overflow)?;
    let total_comparison_bound = push_comparison_bound
        .checked_add(pop_comparison_bound)
        .and_then(|value| value.checked_add(relaxation_label_comparison_bound))
        .ok_or(Error::Overflow)?;
    Ok(QueueBound {
        schema_version: 1,
        strategy: QueueStrategy::StableBinaryMinHeap,
        proof_scope: QueueProofScope::ReducedEngineFixedSnapshot,
        queue_insertion_count: insertions,
        queue_pop_count: statistics.popped,
        edge_count,
        heap_height_bound,
        push_comparison_bound,
        pop_comparison_bound,
        relaxation_label_comparison_bound,
        total_comparison_bound,
        observed_push_comparisons: statistics.heap_push_comparisons,
        observed_pop_comparisons: statistics.heap_pop_comparisons,
        observed_relaxation_label_comparisons: statistics.relaxation_label_comparisons,
        observed_total_comparisons: statistics.comparisons,
        an19_priority_queue_target_proved: false,
    })
}
