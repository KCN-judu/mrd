use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::{
    backend::{Backend, Kind as EngineKind},
    certificate,
    model::{Problem, Ratio, Run},
};
use crate::source_an19::{experiment, oracle};
use crate::{
    ExactRatio,
    source_an19::petal::{Error, ratio_less},
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
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
pub enum Orientation {
    FirstToSecond,
    SecondToFirst,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleReason {
    SupersededDistance,
    SettledVertex,
    AfterStoppingRadius,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub active_vertices: usize,
    pub internal_edges: usize,
    pub boundary_edges: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub cluster_id: u64,
    pub projection_snapshot_id: u64,
    pub logical_partition_depth: u64,
    pub recursion_parent_id: Option<u64>,
    pub event_sequence_number: u64,
    pub event_type: Kind,
    pub source_edge_id: Option<usize>,
    pub active_segment_id: Option<usize>,
    pub segment_lineage_root_id: Option<usize>,
    pub orientation: Option<Orientation>,
    pub exact_materialized_segment_length: Option<Ratio>,
    pub symbolic_unsplit_rounded_length: Option<Ratio>,
    pub highway_halved: Option<bool>,
    pub exact_reduced_cost: Option<Ratio>,
    pub exact_event_radius: Ratio,
    pub queue_insertion_sequence: Option<u64>,
    pub queue_pop_sequence: Option<u64>,
    pub stale: bool,
    pub stale_reason: Option<StaleReason>,
    pub state_before: State,
    pub state_after: State,
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

impl Run {
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
    /// Returns [`Error::InvalidEventTrace`] when any trace invariant
    /// fails, or an exact-arithmetic error for a malformed ratio.
    pub fn verify_trace(&self) -> Result<(), Error> {
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
            return Err(Error::InvalidEventTrace);
        }
        let mut previous = None;
        let mut previous_state = State::default();
        let mut semantic_keys = BTreeSet::new();
        for event in &self.semantic_trace {
            let radius = ExactRatio::try_from(event.exact_event_radius)?;
            if let Some(old) = previous
                && ratio_less(radius, old)?
            {
                return Err(Error::InvalidEventTrace);
            }
            previous = Some(radius);
            if event.state_before != previous_state
                || event.stale != event.stale_reason.is_some()
                || event.state_after.active_vertices < event.state_before.active_vertices
                || event.state_after.internal_edges < event.state_before.internal_edges
                || (event.event_type == Kind::BoundaryToInternalEdgeTransition
                    && event.state_after.boundary_edges >= event.state_before.boundary_edges)
                || event.charge_source_depth.is_some() != event.source_edge_id.is_some()
                || event.charge_lineage_event.is_some() != event.segment_lineage_root_id.is_some()
                || event.charge_snapshot_segment_event.is_some()
                    != event.active_segment_id.is_some()
            {
                return Err(Error::InvalidEventTrace);
            }
            previous_state = event.state_after;
            let key = (
                event.event_type,
                event.exact_event_radius,
                event.affected_vertex_id,
                event.affected_directed_incidence_id,
            );
            if !event.stale && !semantic_keys.insert(key) {
                return Err(Error::InvalidEventTrace);
            }
        }
        if self.metrics.vertex_entry_count
            != u64::try_from(
                self.semantic_trace
                    .iter()
                    .filter(|event| event.event_type == Kind::VertexEntry && !event.stale)
                    .count(),
            )
            .map_err(|_| Error::Overflow)?
            || self.metrics.stale_queue_item_count
                != u64::try_from(
                    self.queue_trace
                        .iter()
                        .filter(|event| event.event_type == Kind::StaleQueueEvent)
                        .count(),
                )
                .map_err(|_| Error::Overflow)?
        {
            return Err(Error::InvalidEventTrace);
        }
        certificate::verify_local_event_bound(self)?;
        certificate::verify_practical_queue_bound(self)?;
        Ok(())
    }

    /// Reruns the selected exact backend and rejects any trace mutation.
    ///
    /// # Errors
    ///
    /// Returns an exact engine error or
    /// [`Error::InvalidEventTrace`] when the rerun differs.
    pub fn verify_against(&self, problem: &Problem<'_>) -> Result<(), Error> {
        let expected = match self.engine {
            EngineKind::Oracle => oracle::event::Engine.run(problem)?,
            EngineKind::Experiment => experiment::event::Engine.run(problem)?,
            EngineKind::ProvedUnavailable => {
                return Err(Error::UnprovedEventEngine);
            }
        };
        let mut actual = self.clone();
        let mut rebuilt = expected;
        actual.runtime_status.differential_verified = false;
        rebuilt.runtime_status.differential_verified = false;
        actual.runtime_status.exact_oracle_verified = rebuilt.runtime_status.exact_oracle_verified;
        if actual != rebuilt {
            return Err(Error::InvalidEventTrace);
        }
        self.verify_trace()
    }
}

#[allow(clippy::too_many_lines)]
type NormalizedEvent = (
    Kind,
    Ratio,
    Option<usize>,
    Option<usize>,
    bool,
    State,
    State,
);

fn normalized_semantic_trace(trace: &[Record]) -> Vec<NormalizedEvent> {
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
