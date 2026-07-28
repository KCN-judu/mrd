use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, VecDeque},
    rc::Rc,
};

use thiserror::Error;

use crate::{
    CertifiedFixedPoint, ExactRatio, FixedPointConfig, FlowNodeId, SourceDynamicGraph,
    SourceEdgeId, SourceWeightedEdge,
};

mod event_engine;

pub use event_engine::{
    An19AdversarialCampaign, An19AdversarialCaseResult, An19AdversarialFamily, An19ChargeAnalysis,
    An19ChargeMapKind, An19CountByKey, An19EventContext, An19EventEngine, An19EventEngineKind,
    An19EventOrientation, An19EventProblem, An19EventRun, An19EventRuntimeStatus,
    An19EventSegmentMetadata, An19EventState, An19EventTraceRecord, An19EventType,
    An19ExactRatioRecord, An19HierarchyEventMetrics, An19ReducedEventEngine, An19SnapshotMetrics,
    An19StaleReason, An19StoppingCertificate, ExactEventOracle, ProvedEventEngine,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct An19PetalMetrics {
    pub shortest_path_runs: u64,
    pub edge_relaxations: u64,
    pub shortest_heap_pushes: u64,
    pub shortest_heap_pops: u64,
    pub shortest_edge_scans: u64,
    pub radius_events: u64,
    pub certified_comparisons: u64,
    pub directed_region_runs: u64,
    pub directed_heap_pushes: u64,
    pub directed_heap_pops: u64,
    pub directed_edge_scans: u64,
    pub membership_sources: u64,
    pub event_heap_pushes: u64,
    pub event_heap_pops: u64,
    pub heap_comparisons: u64,
    pub monotone_queue_pushes: u64,
    pub monotone_queue_pops: u64,
    pub monotone_front_comparisons: u64,
    pub maximum_length_classes: u64,
    pub event_vertex_activations: u64,
    pub event_edge_touches: u64,
    pub volume_queries: u64,
}

impl An19PetalMetrics {
    fn checked_add_assign(&mut self, other: &Self) -> Result<(), An19PetalError> {
        self.shortest_path_runs =
            checked_metric_sum(self.shortest_path_runs, other.shortest_path_runs)?;
        self.edge_relaxations = checked_metric_sum(self.edge_relaxations, other.edge_relaxations)?;
        self.shortest_heap_pushes =
            checked_metric_sum(self.shortest_heap_pushes, other.shortest_heap_pushes)?;
        self.shortest_heap_pops =
            checked_metric_sum(self.shortest_heap_pops, other.shortest_heap_pops)?;
        self.shortest_edge_scans =
            checked_metric_sum(self.shortest_edge_scans, other.shortest_edge_scans)?;
        self.radius_events = checked_metric_sum(self.radius_events, other.radius_events)?;
        self.certified_comparisons =
            checked_metric_sum(self.certified_comparisons, other.certified_comparisons)?;
        self.directed_region_runs =
            checked_metric_sum(self.directed_region_runs, other.directed_region_runs)?;
        self.directed_heap_pushes =
            checked_metric_sum(self.directed_heap_pushes, other.directed_heap_pushes)?;
        self.directed_heap_pops =
            checked_metric_sum(self.directed_heap_pops, other.directed_heap_pops)?;
        self.directed_edge_scans =
            checked_metric_sum(self.directed_edge_scans, other.directed_edge_scans)?;
        self.membership_sources =
            checked_metric_sum(self.membership_sources, other.membership_sources)?;
        self.event_heap_pushes =
            checked_metric_sum(self.event_heap_pushes, other.event_heap_pushes)?;
        self.event_heap_pops = checked_metric_sum(self.event_heap_pops, other.event_heap_pops)?;
        self.heap_comparisons = checked_metric_sum(self.heap_comparisons, other.heap_comparisons)?;
        self.monotone_queue_pushes =
            checked_metric_sum(self.monotone_queue_pushes, other.monotone_queue_pushes)?;
        self.monotone_queue_pops =
            checked_metric_sum(self.monotone_queue_pops, other.monotone_queue_pops)?;
        self.monotone_front_comparisons = checked_metric_sum(
            self.monotone_front_comparisons,
            other.monotone_front_comparisons,
        )?;
        self.maximum_length_classes = self
            .maximum_length_classes
            .max(other.maximum_length_classes);
        self.event_vertex_activations = checked_metric_sum(
            self.event_vertex_activations,
            other.event_vertex_activations,
        )?;
        self.event_edge_touches =
            checked_metric_sum(self.event_edge_touches, other.event_edge_touches)?;
        self.volume_queries = checked_metric_sum(self.volume_queries, other.volume_queries)?;
        Ok(())
    }
}

/// Exact Figure 6 output on AN19's original unit-length vertex domain.
///
/// Interior weighted portal points and the hierarchical constructor are
/// intentionally outside this partial source gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19UnweightedPetal {
    pub vertices: BTreeSet<FlowNodeId>,
    pub path_from_center: Vec<FlowNodeId>,
    pub path_edges: Vec<SourceEdgeId>,
    pub center_vertex: Option<FlowNodeId>,
    pub radius: ExactRatio,
    pub window_index: usize,
    pub window_start: ExactRatio,
    pub window_end: ExactRatio,
    pub internal_edges: usize,
    pub boundary_edges: usize,
    pub cluster_edges: usize,
    pub metrics: An19PetalMetrics,
}

impl An19UnweightedPetal {
    /// Executes AN19 Figure 6 exactly on an induced unit-length cluster.
    ///
    /// `remaining` is Figure 5's current `Y`, while `cluster` is its fixed
    /// `X`. The fixed shortest path is selected lexicographically by edge ID,
    /// implementing the paper's deterministic unique-path perturbation.
    ///
    /// # Errors
    ///
    /// Returns an error unless all active edges have unit length, the supplied
    /// vertex sets satisfy the Figure 5 containment/connectivity contract, or
    /// the certified logarithmic comparisons can be resolved.
    pub fn construct(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        budget: ExactRatio,
    ) -> Result<Self, An19PetalError> {
        validate_domain(graph, cluster, remaining, center, target, budget)?;
        let mut metrics = An19PetalMetrics::default();
        let cluster_paths = shortest_paths(graph, cluster, center, &mut metrics)?;
        let recovered_path = recover_path(center, target, &cluster_paths)?;
        if recovered_path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(An19PetalError::InvalidDomain);
        }
        let remaining_from_center = shortest_paths(graph, remaining, center, &mut metrics)?;
        for vertex in &recovered_path.vertices {
            if cluster_paths.distances[vertex.0] != remaining_from_center.distances[vertex.0] {
                return Err(An19PetalError::InvalidDomain);
            }
        }
        let thresholds = membership_thresholds(
            graph,
            remaining,
            &recovered_path.vertices,
            &remaining_from_center,
            target,
            budget,
            &mut metrics,
        )?;
        let cluster_edges = internal_edge_count(graph, cluster);
        let active_edges = (0..graph.edge_count())
            .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
            .count();
        if cluster_edges == 0 || active_edges < 2 {
            return Err(An19PetalError::InvalidDomain);
        }
        let levels = ceil_log_log(graph.node_count());
        let mut selected = None;
        for index in 1..=levels {
            let window_end = window_radius(budget, index, levels, true)?;
            let vertices = vertices_at_radius(remaining, &thresholds, window_end)?;
            let internal = internal_edge_count(graph, &vertices);
            metrics.certified_comparisons = metrics
                .certified_comparisons
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            if certify_window_condition(active_edges, cluster_edges, internal, index, levels)? {
                selected = Some((index, window_end));
                break;
            }
        }
        let (window_index, window_end) = selected.ok_or(An19PetalError::InvalidRadius)?;
        let window_start = window_radius(budget, window_index, levels, false)?;
        let start_vertices = vertices_at_radius(remaining, &thresholds, window_start)?;
        let start_edges = internal_edge_count(graph, &start_vertices);
        if start_edges == 0 || start_edges >= cluster_edges {
            return Err(An19PetalError::InvalidRadius);
        }
        let mut radius = window_start;
        let (vertices, internal_edges, boundary_edges) = loop {
            let vertices = vertices_at_radius(remaining, &thresholds, radius)?;
            let internal = internal_edge_count(graph, &vertices);
            let boundary = boundary_edge_count(graph, cluster, &vertices);
            metrics.certified_comparisons = metrics
                .certified_comparisons
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            if certify_stopping_condition(
                cluster_edges,
                start_edges,
                internal,
                count_ratio(boundary)?,
                levels,
                budget,
            )? {
                break (vertices, internal, boundary);
            }
            radius = next_radius_event(remaining, &thresholds, radius, window_end)?
                .ok_or(An19PetalError::InvalidRadius)?;
            metrics.radius_events = metrics
                .radius_events
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
        };
        let center_vertex = recovered_path
            .vertices
            .iter()
            .copied()
            .find(|vertex| thresholds.path_distance_from_target[vertex.0] == Some(radius));
        Ok(Self {
            vertices,
            path_from_center: recovered_path.vertices,
            path_edges: recovered_path.edges,
            center_vertex,
            radius,
            window_index,
            window_start,
            window_end,
            internal_edges,
            boundary_edges,
            cluster_edges,
            metrics,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum An19PathPoint {
    Vertex(FlowNodeId),
    EdgeInterior {
        edge: SourceEdgeId,
        from: FlowNodeId,
        toward_center: FlowNodeId,
        offset_from: ExactRatio,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19HighwaySegment {
    pub edge: SourceEdgeId,
    pub from: FlowNodeId,
    pub toward_center: FlowNodeId,
    pub halved_length: ExactRatio,
    pub original_edge_length: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19WeightedPetalAtRadius {
    pub vertices: BTreeSet<FlowNodeId>,
    pub path_from_center: Vec<FlowNodeId>,
    pub path_edges: Vec<SourceEdgeId>,
    pub portal: An19PathPoint,
    pub radius: ExactRatio,
    pub highway_segments: Vec<An19HighwaySegment>,
    pub directed_distances: Vec<Option<ExactRatio>>,
    pub metrics: An19PetalMetrics,
}

impl An19WeightedPetalAtRadius {
    /// Evaluates AN19 Claim 15 at one exact rational radius without expanding
    /// edges into unit-length paths.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid/disconnected cluster, a radius beyond
    /// the fixed center-target path, or inconsistent reduced directed lengths.
    pub fn construct(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        radius: ExactRatio,
    ) -> Result<Self, An19PetalError> {
        Self::construct_with_paths(graph, cluster, remaining, center, target, radius, false)
    }

    fn construct_for_hierarchy(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        radius: ExactRatio,
    ) -> Result<Self, An19PetalError> {
        Self::construct_with_paths(graph, cluster, remaining, center, target, radius, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn construct_with_paths(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        radius: ExactRatio,
        fast_paths: bool,
    ) -> Result<Self, An19PetalError> {
        validate_weighted_domain(graph, cluster, remaining, center, target, radius)?;
        let mut metrics = An19PetalMetrics::default();
        let cluster_paths =
            hierarchy_or_oracle_paths(graph, cluster, center, fast_paths, &mut metrics)?;
        let path = recover_path(center, target, &cluster_paths)?;
        if path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(An19PetalError::InvalidDomain);
        }
        let remaining_paths =
            hierarchy_or_oracle_paths(graph, remaining, center, fast_paths, &mut metrics)?;
        for vertex in &path.vertices {
            if cluster_paths.distances[vertex.0] != remaining_paths.distances[vertex.0] {
                return Err(An19PetalError::InvalidDomain);
            }
        }
        let (portal, highway_segments) = locate_portal_and_highway(graph, &path, target, radius)?;
        let directed_distances = directed_petal_distances(
            graph,
            remaining,
            target,
            &remaining_paths.distances,
            &highway_segments,
            &mut metrics,
        )?;
        let half_radius = radius
            .checked_mul(ratio(1, 2)?)
            .map_err(|_| An19PetalError::Overflow)?;
        let mut vertices = BTreeSet::new();
        for vertex in remaining {
            let distance = directed_distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
            if half_radius
                .at_least(distance)
                .map_err(|_| An19PetalError::Overflow)?
            {
                vertices.insert(*vertex);
            }
        }
        Ok(Self {
            vertices,
            path_from_center: path.vertices,
            path_edges: path.edges,
            portal,
            radius,
            highway_segments,
            directed_distances,
            metrics,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19WeightedPetal {
    pub at_radius: An19WeightedPetalAtRadius,
    pub window_index: usize,
    pub window_start: ExactRatio,
    pub window_end: ExactRatio,
    pub internal_edges: usize,
    pub boundary_edges: usize,
    pub cluster_edges: usize,
}

impl An19WeightedPetal {
    /// Executes Figure 6 for arbitrary positive rational edge lengths using
    /// exact parametric Claim 15 membership events.
    ///
    /// This is a source-semantics baseline. Its repeated exact shortest paths
    /// do not establish AN19's fast region-growing runtime.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid source cluster, an insufficient target
    /// path, or an uncertifiable Figure 6 comparison.
    pub fn construct(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        budget: ExactRatio,
    ) -> Result<Self, An19PetalError> {
        Self::construct_with_portal_volume(
            graph,
            cluster,
            remaining,
            center,
            target,
            budget,
            false,
            false,
            graph.node_count(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn construct_for_hierarchy(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        budget: ExactRatio,
        compact_weighted_portals: bool,
        level_node_count: usize,
    ) -> Result<Self, An19PetalError> {
        Self::construct_with_portal_volume(
            graph,
            cluster,
            remaining,
            center,
            target,
            budget,
            compact_weighted_portals,
            true,
            level_node_count,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn construct_with_portal_volume(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        budget: ExactRatio,
        compact_weighted_portals: bool,
        fast_events: bool,
        level_node_count: usize,
    ) -> Result<Self, An19PetalError> {
        validate_weighted_domain(graph, cluster, remaining, center, target, budget)?;
        if !budget.is_positive() {
            return Err(An19PetalError::InvalidDomain);
        }
        let mut metrics = An19PetalMetrics::default();
        let cluster_paths =
            hierarchy_or_oracle_paths(graph, cluster, center, fast_events, &mut metrics)?;
        let path = recover_path(center, target, &cluster_paths)?;
        if path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(An19PetalError::InvalidDomain);
        }
        let remaining_paths =
            hierarchy_or_oracle_paths(graph, remaining, center, fast_events, &mut metrics)?;
        for vertex in &path.vertices {
            if cluster_paths.distances[vertex.0] != remaining_paths.distances[vertex.0] {
                return Err(An19PetalError::InvalidDomain);
            }
        }
        let target_distance =
            remaining_paths.distances[target.0].ok_or(An19PetalError::Disconnected)?;
        if ratio_less(target_distance, budget)? {
            return Err(An19PetalError::InvalidRadius);
        }
        let thresholds = if fast_events {
            fast_weighted_membership_thresholds(
                graph,
                remaining,
                target,
                &path,
                &remaining_paths.distances,
                budget,
                &mut metrics,
            )?
        } else {
            weighted_membership_thresholds_oracle(
                graph,
                remaining,
                target,
                &path,
                &remaining_paths.distances,
                budget,
                &mut metrics,
            )?
        };
        let selection = select_weighted_figure_six(
            graph,
            cluster,
            remaining,
            &thresholds,
            budget,
            compact_weighted_portals,
            fast_events,
            level_node_count,
            &mut metrics,
        )?;
        let mut at_radius = if fast_events {
            An19WeightedPetalAtRadius::construct_for_hierarchy(
                graph,
                cluster,
                remaining,
                center,
                target,
                selection.radius,
            )?
        } else {
            An19WeightedPetalAtRadius::construct(
                graph,
                cluster,
                remaining,
                center,
                target,
                selection.radius,
            )?
        };
        at_radius.metrics.checked_add_assign(&metrics)?;
        Ok(Self {
            at_radius,
            window_index: selection.window_index,
            window_start: selection.window_start,
            window_end: selection.window_end,
            internal_edges: selection.internal_edges,
            boundary_edges: selection.boundary_edges,
            cluster_edges: selection.cluster_edges,
        })
    }
}

struct FigureSixSelection {
    radius: ExactRatio,
    window_index: usize,
    window_start: ExactRatio,
    window_end: ExactRatio,
    internal_edges: usize,
    boundary_edges: usize,
    cluster_edges: usize,
}

#[allow(clippy::too_many_arguments)]
fn select_weighted_figure_six(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    budget: ExactRatio,
    compact_weighted_portals: bool,
    fast_events: bool,
    level_node_count: usize,
    metrics: &mut An19PetalMetrics,
) -> Result<FigureSixSelection, An19PetalError> {
    if fast_events {
        return select_weighted_figure_six_fast(
            graph,
            cluster,
            remaining,
            thresholds,
            budget,
            compact_weighted_portals,
            level_node_count,
            metrics,
        );
    }
    select_weighted_figure_six_oracle(
        graph,
        cluster,
        remaining,
        thresholds,
        budget,
        compact_weighted_portals,
        level_node_count,
        metrics,
    )
}

#[derive(Clone, Copy)]
struct RegionAdjacencyEdge {
    other: FlowNodeId,
    edge: SourceEdgeId,
    length: ExactRatio,
}

struct RegionVolumeState {
    included: Vec<bool>,
    edge_seen: Vec<bool>,
    vertices: BTreeSet<FlowNodeId>,
    internal_edges: usize,
    incident_edges: usize,
    boundary_edges: usize,
    boundary_cost: ExactRatio,
}

#[allow(clippy::too_many_arguments)]
fn select_weighted_figure_six_fast(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    budget: ExactRatio,
    compact_weighted_portals: bool,
    level_node_count: usize,
    metrics: &mut An19PetalMetrics,
) -> Result<FigureSixSelection, An19PetalError> {
    let (base_cluster_edges, base_active_edges) = figure_six_base_edge_counts(graph, cluster)?;
    metrics.event_edge_touches = checked_metric_sum(
        metrics.event_edge_touches,
        u64::try_from(graph.edge_count())
            .ok()
            .and_then(|count| count.checked_mul(2))
            .ok_or(An19PetalError::Overflow)?,
    )?;
    let events = sorted_membership_events(remaining, thresholds, metrics)?;
    let adjacency = region_adjacency(graph, cluster, metrics)?;
    let levels = ceil_log_log(level_node_count);
    let mut state = RegionVolumeState::new(graph)?;
    let mut cursor = 0;
    let mut selected = None;
    for index in 1..=levels {
        let window_end = window_radius(budget, index, levels, true)?;
        advance_region_state(
            &events,
            &mut cursor,
            window_end,
            &adjacency,
            &mut state,
            metrics,
        )?;
        let portal_split =
            usize::from(compact_weighted_portals && portal_is_interior(thresholds, window_end));
        let petal_edges = state.edge_measure(compact_weighted_portals, portal_split, metrics)?;
        let cluster_edges = checked_edge_sum(base_cluster_edges, portal_split)?;
        let active_edges = checked_edge_sum(base_active_edges, portal_split)?;
        metrics.certified_comparisons = checked_metric_sum(metrics.certified_comparisons, 1)?;
        if certify_window_condition(active_edges, cluster_edges, petal_edges, index, levels)? {
            selected = Some((index, window_end));
            break;
        }
    }
    let (window_index, window_end) = selected.ok_or(An19PetalError::InvalidRadius)?;
    let window_start = window_radius(budget, window_index, levels, false)?;
    let mut state = RegionVolumeState::new(graph)?;
    let mut cursor = 0;
    advance_region_state(
        &events,
        &mut cursor,
        window_start,
        &adjacency,
        &mut state,
        metrics,
    )?;
    let start_portal_split =
        usize::from(compact_weighted_portals && portal_is_interior(thresholds, window_start));
    let start_edges = state.edge_measure(compact_weighted_portals, start_portal_split, metrics)?;
    let start_cluster_edges = checked_edge_sum(base_cluster_edges, start_portal_split)?;
    if start_edges == 0 || start_edges >= start_cluster_edges {
        return Err(An19PetalError::InvalidRadius);
    }
    let mut radius = window_start;
    loop {
        let portal_split =
            usize::from(compact_weighted_portals && portal_is_interior(thresholds, radius));
        let petal_edges = state.edge_measure(compact_weighted_portals, portal_split, metrics)?;
        let cluster_edges = checked_edge_sum(base_cluster_edges, portal_split)?;
        let boundary_cost = if compact_weighted_portals {
            state.boundary_cost
        } else {
            count_ratio(state.boundary_edges)?
        };
        metrics.certified_comparisons = checked_metric_sum(metrics.certified_comparisons, 1)?;
        if let Some(selection) = fast_stopping_selection(
            radius,
            window_index,
            window_start,
            window_end,
            cluster_edges,
            start_edges,
            petal_edges,
            state.boundary_edges,
            boundary_cost,
            levels,
            budget,
        )? {
            return Ok(selection);
        }
        let next = events
            .get(cursor)
            .map(|event| event.distance)
            .ok_or(An19PetalError::InvalidRadius)?;
        if ratio_less(window_end, next)? {
            return Err(An19PetalError::InvalidRadius);
        }
        radius = next;
        advance_region_state(
            &events,
            &mut cursor,
            radius,
            &adjacency,
            &mut state,
            metrics,
        )?;
        metrics.radius_events = checked_metric_sum(metrics.radius_events, 1)?;
    }
}

#[allow(clippy::too_many_arguments)]
fn fast_stopping_selection(
    radius: ExactRatio,
    window_index: usize,
    window_start: ExactRatio,
    window_end: ExactRatio,
    cluster_edges: usize,
    start_edges: usize,
    petal_edges: usize,
    boundary_edges: usize,
    boundary_cost: ExactRatio,
    levels: usize,
    budget: ExactRatio,
) -> Result<Option<FigureSixSelection>, An19PetalError> {
    if !certify_stopping_condition(
        cluster_edges,
        start_edges,
        petal_edges,
        boundary_cost,
        levels,
        budget,
    )? {
        return Ok(None);
    }
    Ok(Some(FigureSixSelection {
        radius,
        window_index,
        window_start,
        window_end,
        internal_edges: petal_edges,
        boundary_edges,
        cluster_edges,
    }))
}

impl RegionVolumeState {
    fn new(graph: &SourceDynamicGraph) -> Result<Self, An19PetalError> {
        Ok(Self {
            included: vec![false; graph.node_count()],
            edge_seen: vec![false; graph.edge_count()],
            vertices: BTreeSet::new(),
            internal_edges: 0,
            incident_edges: 0,
            boundary_edges: 0,
            boundary_cost: ratio(0, 1)?,
        })
    }

    fn edge_measure(
        &self,
        incident_volume: bool,
        portal_split: usize,
        metrics: &mut An19PetalMetrics,
    ) -> Result<usize, An19PetalError> {
        metrics.volume_queries = checked_metric_sum(metrics.volume_queries, 1)?;
        checked_edge_sum(
            if incident_volume {
                self.incident_edges
            } else {
                self.internal_edges
            },
            portal_split,
        )
    }

    fn activate(
        &mut self,
        vertex: FlowNodeId,
        adjacency: &[Vec<RegionAdjacencyEdge>],
        metrics: &mut An19PetalMetrics,
    ) -> Result<(), An19PetalError> {
        if self.included[vertex.0] {
            return Ok(());
        }
        self.included[vertex.0] = true;
        self.vertices.insert(vertex);
        metrics.event_vertex_activations = checked_metric_sum(metrics.event_vertex_activations, 1)?;
        for adjacent in &adjacency[vertex.0] {
            metrics.event_edge_touches = checked_metric_sum(metrics.event_edge_touches, 1)?;
            let weight = adjacent
                .length
                .reciprocal()
                .map_err(|_| An19PetalError::Overflow)?;
            if !self.edge_seen[adjacent.edge.0] {
                self.edge_seen[adjacent.edge.0] = true;
                self.incident_edges = checked_edge_sum(self.incident_edges, 1)?;
                if self.included[adjacent.other.0] {
                    self.internal_edges = checked_edge_sum(self.internal_edges, 1)?;
                } else {
                    self.boundary_edges = checked_edge_sum(self.boundary_edges, 1)?;
                    self.boundary_cost = self
                        .boundary_cost
                        .checked_add(weight)
                        .map_err(|_| An19PetalError::Overflow)?;
                }
            } else if self.included[adjacent.other.0] && adjacent.other != vertex {
                self.internal_edges = checked_edge_sum(self.internal_edges, 1)?;
                self.boundary_edges = self
                    .boundary_edges
                    .checked_sub(1)
                    .ok_or(An19PetalError::InvalidRadius)?;
                self.boundary_cost = self
                    .boundary_cost
                    .checked_sub(weight)
                    .map_err(|_| An19PetalError::Overflow)?;
            }
        }
        Ok(())
    }
}

fn sorted_membership_events(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    metrics: &mut An19PetalMetrics,
) -> Result<Vec<ExactHeapEntry>, An19PetalError> {
    if let Some(events) = &thresholds.ordered_events {
        return Ok(events.clone());
    }
    let mut heap = Vec::new();
    for vertex in remaining {
        if let Some(distance) = thresholds.by_vertex[vertex.0] {
            event_heap_push(
                &mut heap,
                ExactHeapEntry {
                    distance,
                    vertex: *vertex,
                },
                metrics,
            )?;
        }
    }
    let mut events = Vec::with_capacity(heap.len());
    while let Some(event) = event_heap_pop(&mut heap, metrics)? {
        events.push(event);
    }
    Ok(events)
}

fn region_adjacency(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    metrics: &mut An19PetalMetrics,
) -> Result<Vec<Vec<RegionAdjacencyEdge>>, An19PetalError> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    for index in 0..graph.edge_count() {
        let edge_id = SourceEdgeId(index);
        let Some(edge) = graph.edge(edge_id) else {
            continue;
        };
        metrics.event_edge_touches = checked_metric_sum(metrics.event_edge_touches, 1)?;
        if cluster.contains(&edge.first) && cluster.contains(&edge.second) {
            adjacency[edge.first.0].push(RegionAdjacencyEdge {
                other: edge.second,
                edge: edge_id,
                length: edge.length,
            });
            if edge.first != edge.second {
                adjacency[edge.second.0].push(RegionAdjacencyEdge {
                    other: edge.first,
                    edge: edge_id,
                    length: edge.length,
                });
            }
        }
    }
    Ok(adjacency)
}

fn advance_region_state(
    events: &[ExactHeapEntry],
    cursor: &mut usize,
    radius: ExactRatio,
    adjacency: &[Vec<RegionAdjacencyEdge>],
    state: &mut RegionVolumeState,
    metrics: &mut An19PetalMetrics,
) -> Result<(), An19PetalError> {
    while let Some(event) = events.get(*cursor) {
        if ratio_less(radius, event.distance)? {
            break;
        }
        state.activate(event.vertex, adjacency, metrics)?;
        *cursor = (*cursor).checked_add(1).ok_or(An19PetalError::Overflow)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn select_weighted_figure_six_oracle(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    budget: ExactRatio,
    compact_weighted_portals: bool,
    level_node_count: usize,
    metrics: &mut An19PetalMetrics,
) -> Result<FigureSixSelection, An19PetalError> {
    let base_cluster_edges = internal_edge_count(graph, cluster);
    let base_active_edges = (0..graph.edge_count())
        .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
        .count();
    if base_cluster_edges == 0 || base_active_edges < 2 {
        return Err(An19PetalError::InvalidDomain);
    }
    let levels = ceil_log_log(level_node_count);
    let mut selected = None;
    for index in 1..=levels {
        let window_end = window_radius(budget, index, levels, true)?;
        let vertices = vertices_at_radius(remaining, thresholds, window_end)?;
        let portal_split =
            usize::from(compact_weighted_portals && portal_is_interior(thresholds, window_end));
        let internal = petal_edge_measure(
            graph,
            cluster,
            &vertices,
            compact_weighted_portals,
            portal_split,
        )?;
        let cluster_edges = checked_edge_sum(base_cluster_edges, portal_split)?;
        let active_edges = checked_edge_sum(base_active_edges, portal_split)?;
        metrics.certified_comparisons = metrics
            .certified_comparisons
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        if certify_window_condition(active_edges, cluster_edges, internal, index, levels)? {
            selected = Some((index, window_end));
            break;
        }
    }
    let (window_index, window_end) = selected.ok_or(An19PetalError::InvalidRadius)?;
    let window_start = window_radius(budget, window_index, levels, false)?;
    let start_vertices = vertices_at_radius(remaining, thresholds, window_start)?;
    let start_portal_split =
        usize::from(compact_weighted_portals && portal_is_interior(thresholds, window_start));
    let start_edges = petal_edge_measure(
        graph,
        cluster,
        &start_vertices,
        compact_weighted_portals,
        start_portal_split,
    )?;
    let start_cluster_edges = checked_edge_sum(base_cluster_edges, start_portal_split)?;
    if start_edges == 0 || start_edges >= start_cluster_edges {
        return Err(An19PetalError::InvalidRadius);
    }
    let mut radius = window_start;
    loop {
        let vertices = vertices_at_radius(remaining, thresholds, radius)?;
        let portal_split =
            usize::from(compact_weighted_portals && portal_is_interior(thresholds, radius));
        let internal_edges = petal_edge_measure(
            graph,
            cluster,
            &vertices,
            compact_weighted_portals,
            portal_split,
        )?;
        let cluster_edges = checked_edge_sum(base_cluster_edges, portal_split)?;
        let boundary_edges = boundary_edge_count(graph, cluster, &vertices);
        let boundary_cost = if compact_weighted_portals {
            boundary_edge_cost(graph, cluster, &vertices)?
        } else {
            count_ratio(boundary_edges)?
        };
        metrics.certified_comparisons = metrics
            .certified_comparisons
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        if certify_stopping_condition(
            cluster_edges,
            start_edges,
            internal_edges,
            boundary_cost,
            levels,
            budget,
        )? {
            return Ok(FigureSixSelection {
                radius,
                window_index,
                window_start,
                window_end,
                internal_edges,
                boundary_edges,
                cluster_edges,
            });
        }
        radius = next_radius_event(remaining, thresholds, radius, window_end)?
            .ok_or(An19PetalError::InvalidRadius)?;
        metrics.radius_events = metrics
            .radius_events
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
    }
}

fn portal_is_interior(thresholds: &MembershipThresholds, radius: ExactRatio) -> bool {
    !thresholds.path_distance_from_target.contains(&Some(radius))
}

fn petal_edge_measure(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    vertices: &BTreeSet<FlowNodeId>,
    use_incident_volume: bool,
    portal_split: usize,
) -> Result<usize, An19PetalError> {
    let base = if use_incident_volume {
        incident_edge_count(graph, cluster, vertices)
    } else {
        internal_edge_count(graph, vertices)
    };
    checked_edge_sum(base, portal_split)
}

fn checked_edge_sum(first: usize, second: usize) -> Result<usize, An19PetalError> {
    first.checked_add(second).ok_or(An19PetalError::Overflow)
}

fn figure_six_base_edge_counts(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
) -> Result<(usize, usize), An19PetalError> {
    let cluster_edges = internal_edge_count(graph, cluster);
    let active_edges = (0..graph.edge_count())
        .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
        .count();
    if cluster_edges == 0 || active_edges < 2 {
        return Err(An19PetalError::InvalidDomain);
    }
    Ok((cluster_edges, active_edges))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19ShortEdgeContraction {
    pub cluster: BTreeSet<FlowNodeId>,
    pub center: FlowNodeId,
    pub radius: ExactRatio,
    pub contraction_threshold: ExactRatio,
    pub component_of: Vec<Option<usize>>,
    pub components: Vec<BTreeSet<FlowNodeId>>,
    pub contracted_edges: BTreeSet<SourceEdgeId>,
    pub retained_edges: BTreeSet<SourceEdgeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19HalvedInterval {
    pub start_from_first: ExactRatio,
    pub end_from_first: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19HighwayLedger {
    original_lengths: Vec<Option<ExactRatio>>,
    halved_intervals: Vec<Vec<An19HalvedInterval>>,
    applications: u64,
}

impl An19HighwayLedger {
    /// Creates an empty interval ledger over every active original edge.
    #[must_use]
    pub fn new(graph: &SourceDynamicGraph) -> Self {
        Self {
            original_lengths: (0..graph.edge_count())
                .map(|index| graph.edge(SourceEdgeId(index)).map(|edge| edge.length))
                .collect(),
            halved_intervals: vec![Vec::new(); graph.edge_count()],
            applications: 0,
        }
    }

    #[must_use]
    pub const fn applications(&self) -> u64 {
        self.applications
    }

    #[must_use]
    pub fn intervals(&self, edge: SourceEdgeId) -> Option<&[An19HalvedInterval]> {
        self.halved_intervals.get(edge.0).map(Vec::as_slice)
    }

    /// Atomically records symbolic highway portions and rejects any positive
    /// overlap with a portion that has already been halved.
    ///
    /// # Errors
    ///
    /// Returns an error for stale edge data, an invalid orientation/length, a
    /// repeated interval, or exact arithmetic overflow.
    pub fn apply(
        &mut self,
        graph: &SourceDynamicGraph,
        highway: &[An19HighwaySegment],
    ) -> Result<(), An19PetalError> {
        let mut candidate = self.clone();
        for segment in highway {
            let edge = graph
                .edge(segment.edge)
                .ok_or(An19PetalError::InvalidHighway)?;
            let original = candidate
                .original_lengths
                .get(segment.edge.0)
                .copied()
                .flatten()
                .ok_or(An19PetalError::InvalidHighway)?;
            if original != edge.length
                || original != segment.original_edge_length
                || !segment.halved_length.is_positive()
                || ratio_less(original, segment.halved_length)?
            {
                return Err(An19PetalError::InvalidHighway);
            }
            let (start, end) = if segment.from == edge.first && segment.toward_center == edge.second
            {
                (ratio(0, 1)?, segment.halved_length)
            } else if segment.from == edge.second && segment.toward_center == edge.first {
                (
                    original
                        .checked_sub(segment.halved_length)
                        .map_err(|_| An19PetalError::Overflow)?,
                    original,
                )
            } else {
                return Err(An19PetalError::InvalidHighway);
            };
            let intervals = candidate
                .halved_intervals
                .get_mut(segment.edge.0)
                .ok_or(An19PetalError::InvalidHighway)?;
            for old in intervals.iter() {
                if intervals_overlap(start, end, old.start_from_first, old.end_from_first)? {
                    return Err(An19PetalError::RepeatedHighway);
                }
            }
            intervals.push(An19HalvedInterval {
                start_from_first: start,
                end_from_first: end,
            });
            sort_and_merge_touching(intervals)?;
        }
        candidate.applications = candidate
            .applications
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        *self = candidate;
        Ok(())
    }

    /// Returns the full endpoint-to-endpoint length after every recorded
    /// interval is halved once.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown edge or exact arithmetic overflow.
    pub fn effective_length(&self, edge: SourceEdgeId) -> Result<ExactRatio, An19PetalError> {
        let original = self
            .original_lengths
            .get(edge.0)
            .copied()
            .flatten()
            .ok_or(An19PetalError::InvalidHighway)?;
        let mut halved = ratio(0, 1)?;
        for interval in self
            .halved_intervals
            .get(edge.0)
            .ok_or(An19PetalError::InvalidHighway)?
        {
            halved = halved
                .checked_add(
                    interval
                        .end_from_first
                        .checked_sub(interval.start_from_first)
                        .map_err(|_| An19PetalError::Overflow)?,
                )
                .map_err(|_| An19PetalError::Overflow)?;
        }
        original
            .checked_sub(
                halved
                    .checked_mul(ratio(1, 2)?)
                    .map_err(|_| An19PetalError::Overflow)?,
            )
            .map_err(|_| An19PetalError::Overflow)
    }
}

impl An19ShortEdgeContraction {
    /// Contracts the edges shorter than `rad(X)/n^2` from AN19 Section 6.
    /// Original edge IDs are retained so a quotient tree can be expanded
    /// without choosing synthetic edges.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid/disconnected cluster or exact
    /// arithmetic overflow.
    pub fn build(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
    ) -> Result<Self, An19PetalError> {
        if cluster.is_empty()
            || !cluster.contains(&center)
            || cluster.iter().any(|vertex| vertex.0 >= graph.node_count())
        {
            return Err(An19PetalError::InvalidContraction);
        }
        let mut metrics = An19PetalMetrics::default();
        let paths = shortest_paths(graph, cluster, center, &mut metrics)?;
        let mut radius = ratio(0, 1)?;
        for vertex in cluster {
            let distance = paths.distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
            if ratio_less(radius, distance)? {
                radius = distance;
            }
        }
        if !radius.is_positive() && cluster.len() > 1 {
            return Err(An19PetalError::InvalidContraction);
        }
        Self::build_with_radius(graph, cluster, center, radius, graph.node_count())
    }

    fn build_with_radius(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        radius: ExactRatio,
        original_node_count: usize,
    ) -> Result<Self, An19PetalError> {
        let n = i128::try_from(original_node_count).map_err(|_| An19PetalError::Overflow)?;
        let n_squared = n.checked_mul(n).ok_or(An19PetalError::Overflow)?;
        let contraction_threshold = radius
            .checked_mul(ratio(1, n_squared)?)
            .map_err(|_| An19PetalError::Overflow)?;
        let mut connectivity = DisjointSet::new(graph.node_count());
        let mut contracted_edges = BTreeSet::new();
        for index in 0..graph.edge_count() {
            let edge_id = SourceEdgeId(index);
            let Some(edge) = graph.edge(edge_id) else {
                continue;
            };
            if cluster.contains(&edge.first)
                && cluster.contains(&edge.second)
                && ratio_less(edge.length, contraction_threshold)?
            {
                connectivity.union(edge.first.0, edge.second.0);
                contracted_edges.insert(edge_id);
            }
        }
        let mut root_to_component = BTreeMap::new();
        let mut component_of = vec![None; graph.node_count()];
        let mut components = Vec::<BTreeSet<FlowNodeId>>::new();
        for vertex in cluster {
            let root = connectivity.find(vertex.0);
            let component = if let Some(component) = root_to_component.get(&root) {
                *component
            } else {
                let component = components.len();
                root_to_component.insert(root, component);
                components.push(BTreeSet::new());
                component
            };
            component_of[vertex.0] = Some(component);
            components[component].insert(*vertex);
        }
        let retained_edges = (0..graph.edge_count())
            .filter_map(|index| {
                let edge_id = SourceEdgeId(index);
                graph.edge(edge_id).and_then(|edge| {
                    let first = component_of[edge.first.0]?;
                    let second = component_of[edge.second.0]?;
                    (first != second).then_some(edge_id)
                })
            })
            .collect();
        Ok(Self {
            cluster: cluster.clone(),
            center,
            radius,
            contraction_threshold,
            component_of,
            components,
            contracted_edges,
            retained_edges,
        })
    }

    /// Expands a tree of contracted components into a tree of original edges.
    ///
    /// # Errors
    ///
    /// Returns an error unless the supplied IDs form a tree of the quotient
    /// components using retained original edges.
    pub fn expand_quotient_tree(
        &self,
        graph: &SourceDynamicGraph,
        quotient_tree_edges: &BTreeSet<SourceEdgeId>,
    ) -> Result<BTreeSet<SourceEdgeId>, An19PetalError> {
        if self.component_of.len() != graph.node_count()
            || quotient_tree_edges.len() + 1 != self.components.len()
        {
            return Err(An19PetalError::InvalidContraction);
        }
        let mut quotient_connectivity = DisjointSet::new(self.components.len());
        for edge_id in quotient_tree_edges {
            if !self.retained_edges.contains(edge_id) {
                return Err(An19PetalError::InvalidContraction);
            }
            let edge = graph
                .edge(*edge_id)
                .ok_or(An19PetalError::InvalidContraction)?;
            let first =
                self.component_of[edge.first.0].ok_or(An19PetalError::InvalidContraction)?;
            let second =
                self.component_of[edge.second.0].ok_or(An19PetalError::InvalidContraction)?;
            if !quotient_connectivity.union(first, second) {
                return Err(An19PetalError::InvalidContraction);
            }
        }
        if !all_connected(&mut quotient_connectivity, self.components.len()) {
            return Err(An19PetalError::InvalidContraction);
        }
        let mut original_connectivity = DisjointSet::new(graph.node_count());
        let mut result = BTreeSet::new();
        for edge_id in &self.contracted_edges {
            let edge = graph
                .edge(*edge_id)
                .ok_or(An19PetalError::InvalidContraction)?;
            if original_connectivity.union(edge.first.0, edge.second.0) {
                result.insert(*edge_id);
            }
        }
        for edge_id in quotient_tree_edges {
            let edge = graph
                .edge(*edge_id)
                .ok_or(An19PetalError::InvalidContraction)?;
            if !original_connectivity.union(edge.first.0, edge.second.0) {
                return Err(An19PetalError::InvalidContraction);
            }
            result.insert(*edge_id);
        }
        if result.len() + 1 != self.cluster.len()
            || !all_cluster_connected(&mut original_connectivity, &self.cluster)
        {
            return Err(An19PetalError::InvalidContraction);
        }
        Ok(result)
    }
}

#[derive(Clone, Debug)]
struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct AugmentedAn19Graph {
    original_node_count: usize,
    original_endpoints: Vec<(FlowNodeId, FlowNodeId)>,
    unit_input: bool,
    length_mode: An19LengthMode,
    node_count: usize,
    edges: Vec<AugmentedAn19Edge>,
    incident_edges: Vec<Vec<usize>>,
    projection_cache: RefCell<Option<CachedAugmentedProjection>>,
}

#[derive(Clone, Debug)]
pub struct AugmentedAn19Edge {
    active: bool,
    halved: bool,
    first: FlowNodeId,
    second: FlowNodeId,
    length: ExactRatio,
    provenance: Option<OriginalEdgeInterval>,
    /// Top-level input edge charged by the runtime audit, independent of the
    /// current quotient workspace's local recovery provenance.
    root_source: Option<SourceEdgeId>,
    unsplit_length: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalEdgeInterval {
    edge: SourceEdgeId,
    first_position: ExactRatio,
    second_position: ExactRatio,
}

/// Symbolic source label retained when an augmented edge is split at portals.
///
/// Equal labels identify a common unsplit source length, but do not by
/// themselves prove that arbitrary candidate distances may share one monotone
/// queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct An19SymbolicLengthLabel {
    pub root_source: Option<SourceEdgeId>,
    pub unsplit_length: ExactRatio,
    pub halved: bool,
}

impl An19SymbolicLengthLabel {
    fn effective_length(self) -> Result<ExactRatio, An19PetalError> {
        if self.halved {
            self.unsplit_length
                .checked_mul(ratio(1, 2)?)
                .map_err(|_| An19PetalError::Overflow)
        } else {
            Ok(self.unsplit_length)
        }
    }
}

impl AugmentedAn19Edge {
    fn symbolic_length_label(&self) -> An19SymbolicLengthLabel {
        An19SymbolicLengthLabel {
            root_source: self.root_source,
            unsplit_length: self.unsplit_length,
            halved: self.halved,
        }
    }
}

#[derive(Clone, Debug)]
pub struct AugmentedProjection {
    graph: SourceDynamicGraph,
    dense_to_augmented: Vec<usize>,
    dense_root_sources: Vec<Option<SourceEdgeId>>,
    dense_symbolic_labels: Vec<An19SymbolicLengthLabel>,
    length_class_counts: BTreeMap<(i128, i128), usize>,
    symbolic_source_classes: BTreeSet<(i128, i128)>,
    symbolic_virtual_classes: BTreeSet<(i128, i128)>,
    local_to_augmented_node: Vec<FlowNodeId>,
    augmented_to_local_node: BTreeMap<FlowNodeId, FlowNodeId>,
}

#[derive(Clone, Debug)]
struct CachedAugmentedProjection {
    cluster: BTreeSet<FlowNodeId>,
    projection: Rc<AugmentedProjection>,
    pending_splits: Vec<ProjectionSplitUpdate>,
}

#[derive(Clone, Copy, Debug)]
struct ProjectionSplitUpdate {
    stable_edge: usize,
    from: FlowNodeId,
    portal: FlowNodeId,
    from_edge: usize,
    toward_edge: usize,
    offset: ExactRatio,
}

#[derive(Clone, Copy, Debug, Default)]
struct ProjectionIncidentScans {
    active_internal: u64,
    active_boundary: u64,
    inactive: u64,
}

impl ProjectionIncidentScans {
    fn observe(
        &mut self,
        edge: &AugmentedAn19Edge,
        cluster: &BTreeSet<FlowNodeId>,
    ) -> Result<bool, An19PetalError> {
        if !edge.active {
            self.inactive = checked_metric_sum(self.inactive, 1)?;
            return Ok(false);
        }
        if !cluster.contains(&edge.first) || !cluster.contains(&edge.second) {
            self.active_boundary = checked_metric_sum(self.active_boundary, 1)?;
            return Ok(false);
        }
        self.active_internal = checked_metric_sum(self.active_internal, 1)?;
        Ok(true)
    }

    fn record(self, metrics: &mut An19HierarchyMetrics) -> Result<(), An19PetalError> {
        metrics.projection_active_internal_incident_scans = checked_metric_sum(
            metrics.projection_active_internal_incident_scans,
            self.active_internal,
        )?;
        metrics.projection_active_boundary_incident_scans = checked_metric_sum(
            metrics.projection_active_boundary_incident_scans,
            self.active_boundary,
        )?;
        metrics.projection_inactive_incident_scans =
            checked_metric_sum(metrics.projection_inactive_incident_scans, self.inactive)?;
        let total = checked_metric_sum(
            checked_metric_sum(self.active_internal, self.active_boundary)?,
            self.inactive,
        )?;
        metrics.projection_incident_scans =
            checked_metric_sum(metrics.projection_incident_scans, total)?;
        metrics.workspace_edge_scans = checked_metric_sum(metrics.workspace_edge_scans, total)?;
        Ok(())
    }
}

impl AugmentedAn19Graph {
    /// Copies an exact source graph into a stable-edge hierarchy workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when an active source edge cannot be recovered or
    /// exact provenance coordinates overflow.
    pub fn from_source(graph: &SourceDynamicGraph) -> Result<Self, An19PetalError> {
        Self::from_source_with_length_mode(graph, An19LengthMode::ExactRational)
    }

    fn from_source_with_length_mode(
        graph: &SourceDynamicGraph,
        length_mode: An19LengthMode,
    ) -> Result<Self, An19PetalError> {
        let root_sources = (0..graph.edge_count())
            .map(|index| Some(SourceEdgeId(index)))
            .collect::<Vec<_>>();
        Self::from_source_with_root_sources_and_labels(graph, length_mode, &root_sources, None)
    }

    fn from_source_with_inherited_labels(
        graph: &SourceDynamicGraph,
        length_mode: An19LengthMode,
        root_sources: &[Option<SourceEdgeId>],
        symbolic_labels: &[An19SymbolicLengthLabel],
    ) -> Result<Self, An19PetalError> {
        Self::from_source_with_root_sources_and_labels(
            graph,
            length_mode,
            root_sources,
            Some(symbolic_labels),
        )
    }

    fn from_source_with_root_sources_and_labels(
        graph: &SourceDynamicGraph,
        length_mode: An19LengthMode,
        root_sources: &[Option<SourceEdgeId>],
        symbolic_labels: Option<&[An19SymbolicLengthLabel]>,
    ) -> Result<Self, An19PetalError> {
        if root_sources.len() != graph.edge_count()
            || symbolic_labels.is_some_and(|labels| labels.len() != graph.edge_count())
        {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let mut edges = Vec::new();
        let mut incident_edges = vec![Vec::new(); graph.node_count()];
        let mut original_endpoints = Vec::new();
        let one = ratio(1, 1)?;
        let mut unit_input = true;
        let minimum_length = (0..graph.edge_count())
            .filter_map(|index| graph.edge(SourceEdgeId(index)))
            .try_fold(None, |minimum, edge| {
                let replace = match minimum {
                    Some(value) => ratio_less(edge.length, value)?,
                    None => true,
                };
                Ok::<_, An19PetalError>(if replace { Some(edge.length) } else { minimum })
            })?
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        for (index, root_source) in root_sources.iter().copied().enumerate() {
            let edge = graph
                .edge(SourceEdgeId(index))
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            original_endpoints.push((edge.first, edge.second));
            unit_input &= edge.length == one;
            let workspace_length = match length_mode {
                An19LengthMode::ExactRational => edge.length,
                An19LengthMode::RoundedPowerOfTwo => {
                    round_length_to_power_of_two(edge.length, minimum_length)?
                }
            };
            let symbolic_label = symbolic_labels.map_or(
                An19SymbolicLengthLabel {
                    root_source,
                    unsplit_length: workspace_length,
                    halved: false,
                },
                |labels| labels[index],
            );
            if symbolic_label.root_source != root_source
                || !symbolic_label.unsplit_length.is_positive()
            {
                return Err(An19PetalError::InvalidAugmentedGraph);
            }
            let stable = edges.len();
            edges.push(AugmentedAn19Edge {
                active: true,
                halved: symbolic_label.halved,
                first: edge.first,
                second: edge.second,
                length: workspace_length,
                provenance: Some(OriginalEdgeInterval {
                    edge: SourceEdgeId(index),
                    first_position: ratio(0, 1)?,
                    second_position: edge.length,
                }),
                root_source,
                unsplit_length: symbolic_label.unsplit_length,
            });
            incident_edges[edge.first.0].push(stable);
            incident_edges[edge.second.0].push(stable);
        }
        Ok(Self {
            original_node_count: graph.node_count(),
            original_endpoints,
            unit_input,
            length_mode,
            node_count: graph.node_count(),
            edges,
            incident_edges,
            projection_cache: RefCell::new(None),
        })
    }

    fn invalidate_projection_cache(&mut self) {
        self.projection_cache.get_mut().take();
    }

    fn queue_projection_split(&mut self, edge: &AugmentedAn19Edge, update: ProjectionSplitUpdate) {
        let Some(cached) = self.projection_cache.get_mut().as_mut() else {
            return;
        };
        if !cached.cluster.contains(&edge.first) || !cached.cluster.contains(&edge.second) {
            self.invalidate_projection_cache();
            return;
        }
        cached.cluster.insert(update.portal);
        cached.pending_splits.push(update);
    }

    /// Attaches a positive-length provenance-free leaf used for Figure 5's
    /// imaginary first target.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid attachment, nonpositive length, or
    /// node-index overflow.
    pub fn add_virtual_leaf(
        &mut self,
        attached_to: FlowNodeId,
        length: ExactRatio,
    ) -> Result<(FlowNodeId, usize), An19PetalError> {
        if attached_to.0 >= self.node_count || !length.is_positive() {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let vertex = FlowNodeId(self.node_count);
        let next_node_count = self
            .node_count
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        self.invalidate_projection_cache();
        self.node_count = next_node_count;
        self.incident_edges.push(Vec::new());
        let edge = self.edges.len();
        self.edges.push(AugmentedAn19Edge {
            active: true,
            halved: false,
            first: attached_to,
            second: vertex,
            length,
            provenance: None,
            root_source: None,
            unsplit_length: length,
        });
        self.incident_edges[attached_to.0].push(edge);
        self.incident_edges[vertex.0].push(edge);
        Ok((vertex, edge))
    }

    /// Splits one active edge at an exact interior offset from either endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error for an inactive or nonincident edge, a noninterior
    /// offset, inconsistent provenance, or exact arithmetic overflow.
    pub fn split_edge(
        &mut self,
        edge_id: usize,
        from: FlowNodeId,
        offset: ExactRatio,
    ) -> Result<(FlowNodeId, usize, usize), An19PetalError> {
        let edge = self
            .edges
            .get(edge_id)
            .cloned()
            .filter(|edge| edge.active)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        let toward = if edge.first == from {
            edge.second
        } else if edge.second == from {
            edge.first
        } else {
            return Err(An19PetalError::InvalidAugmentedGraph);
        };
        if !offset.is_positive() || !ratio_less(offset, edge.length)? {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let remainder = edge
            .length
            .checked_sub(offset)
            .map_err(|_| An19PetalError::Overflow)?;
        let (from_provenance, toward_provenance) = split_provenance(&edge, from, offset)?;
        let vertex = FlowNodeId(self.node_count);
        let next_node_count = self
            .node_count
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        self.node_count = next_node_count;
        self.incident_edges.push(Vec::new());
        self.edges[edge_id].active = false;
        let from_edge = self.edges.len();
        self.edges.push(AugmentedAn19Edge {
            active: true,
            halved: edge.halved,
            first: from,
            second: vertex,
            length: offset,
            provenance: from_provenance,
            root_source: edge.root_source,
            unsplit_length: edge.unsplit_length,
        });
        self.incident_edges[from.0].push(from_edge);
        self.incident_edges[vertex.0].push(from_edge);
        let toward_edge = self.edges.len();
        self.edges.push(AugmentedAn19Edge {
            active: true,
            halved: edge.halved,
            first: vertex,
            second: toward,
            length: remainder,
            provenance: toward_provenance,
            root_source: edge.root_source,
            unsplit_length: edge.unsplit_length,
        });
        self.incident_edges[vertex.0].push(toward_edge);
        self.incident_edges[toward.0].push(toward_edge);
        self.queue_projection_split(
            &edge,
            ProjectionSplitUpdate {
                stable_edge: edge_id,
                from,
                portal: vertex,
                from_edge,
                toward_edge,
                offset,
            },
        );
        Ok((vertex, from_edge, toward_edge))
    }

    fn reuse_cluster_projection(
        &self,
        cluster: &BTreeSet<FlowNodeId>,
        metrics: &mut An19HierarchyMetrics,
        projection_audit: &mut An19ProjectionAudit,
    ) -> Result<Option<Rc<AugmentedProjection>>, An19PetalError> {
        let mut cache_slot = self.projection_cache.borrow_mut();
        let Some(mut cached) = cache_slot.take() else {
            return Ok(None);
        };
        if cached.cluster != *cluster {
            return Ok(None);
        }
        let incremental_splits =
            u64::try_from(cached.pending_splits.len()).map_err(|_| An19PetalError::Overflow)?;
        if !cached.pending_splits.is_empty() {
            let Some(projection) = Rc::get_mut(&mut cached.projection) else {
                return Ok(None);
            };
            for update in cached.pending_splits.drain(..) {
                projection.apply_split_update(update)?;
            }
        }
        metrics.projection_cache_hits = checked_metric_sum(metrics.projection_cache_hits, 1)?;
        metrics.projection_incremental_splits =
            checked_metric_sum(metrics.projection_incremental_splits, incremental_splits)?;
        if incremental_splits > 0 {
            projection_audit.observe_projection_shape(&cached.projection, metrics)?;
        }
        let projection = Rc::clone(&cached.projection);
        *cache_slot = Some(cached);
        Ok(Some(projection))
    }

    fn project_cluster(
        &self,
        cluster: &BTreeSet<FlowNodeId>,
        metrics: &mut An19HierarchyMetrics,
        projection_audit: &mut An19ProjectionAudit,
    ) -> Result<Rc<AugmentedProjection>, An19PetalError> {
        metrics.projection_calls = checked_metric_sum(metrics.projection_calls, 1)?;
        if let Some(projection) =
            self.reuse_cluster_projection(cluster, metrics, projection_audit)?
        {
            return Ok(projection);
        }
        metrics.projection_materializations =
            checked_metric_sum(metrics.projection_materializations, 1)?;
        let local_nodes = u64::try_from(cluster.len()).map_err(|_| An19PetalError::Overflow)?;
        metrics.projected_node_slots =
            checked_metric_sum(metrics.projected_node_slots, local_nodes)?;
        metrics.maximum_projection_nodes = metrics.maximum_projection_nodes.max(local_nodes);
        let local_to_augmented_node = cluster.iter().copied().collect::<Vec<_>>();
        let augmented_to_local_node = local_to_augmented_node
            .iter()
            .enumerate()
            .map(|(local, augmented)| (*augmented, FlowNodeId(local)))
            .collect::<BTreeMap<_, _>>();
        let mut dense_to_augmented = Vec::new();
        let mut dense_symbolic_labels = Vec::new();
        let mut edges = Vec::new();
        let mut length_class_counts = BTreeMap::new();
        let mut symbolic_source_classes = BTreeSet::new();
        let mut symbolic_virtual_classes = BTreeSet::new();
        let mut incident_scans = ProjectionIncidentScans::default();
        let mut bound = 1_i128;
        for vertex in cluster {
            let incident = self
                .incident_edges
                .get(vertex.0)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            for stable in incident {
                let edge = self
                    .edges
                    .get(*stable)
                    .ok_or(An19PetalError::InvalidAugmentedGraph)?;
                if !incident_scans.observe(edge, cluster)? || *vertex != edge.first.min(edge.second)
                {
                    continue;
                }
                bound = bound
                    .max(
                        edge.length
                            .numerator()
                            .checked_abs()
                            .ok_or(An19PetalError::Overflow)?,
                    )
                    .max(edge.length.denominator());
                dense_to_augmented.push(*stable);
                let symbolic_label = edge.symbolic_length_label();
                let symbolic_length = symbolic_label.effective_length()?;
                let symbolic_class = (symbolic_length.numerator(), symbolic_length.denominator());
                if symbolic_label.root_source.is_some() {
                    symbolic_source_classes.insert(symbolic_class);
                } else {
                    symbolic_virtual_classes.insert(symbolic_class);
                }
                dense_symbolic_labels.push(symbolic_label);
                *length_class_counts
                    .entry((edge.length.numerator(), edge.length.denominator()))
                    .or_insert(0) += 1;
                edges.push(SourceWeightedEdge {
                    first: *augmented_to_local_node
                        .get(&edge.first)
                        .ok_or(An19PetalError::InvalidAugmentedGraph)?,
                    second: *augmented_to_local_node
                        .get(&edge.second)
                        .ok_or(An19PetalError::InvalidAugmentedGraph)?,
                    length: edge.length,
                    weight: ratio(1, 1)?,
                });
            }
        }
        incident_scans.record(metrics)?;
        let graph = SourceDynamicGraph::new(cluster.len(), edges, bound)
            .map_err(|_| An19PetalError::InvalidAugmentedGraph)?;
        let dense_root_sources = dense_to_augmented
            .iter()
            .map(|stable| self.edges[*stable].root_source)
            .collect::<Vec<_>>();
        let projection = Rc::new(AugmentedProjection {
            graph,
            dense_to_augmented,
            dense_root_sources,
            dense_symbolic_labels,
            length_class_counts,
            symbolic_source_classes,
            symbolic_virtual_classes,
            local_to_augmented_node,
            augmented_to_local_node,
        });
        projection_audit.record(&projection, metrics)?;
        self.projection_cache
            .replace(Some(CachedAugmentedProjection {
                cluster: cluster.clone(),
                projection: Rc::clone(&projection),
                pending_splits: Vec::new(),
            }));
        Ok(projection)
    }

    /// Builds the dense active graph consumed by exact Figure 6 operations.
    ///
    /// # Errors
    ///
    /// Returns an error when an active edge violates the source graph domain
    /// or its rational encoding bound cannot be represented.
    pub fn project(&self) -> Result<AugmentedProjection, An19PetalError> {
        let local_to_augmented_node = (0..self.node_count).map(FlowNodeId).collect::<Vec<_>>();
        let augmented_to_local_node = local_to_augmented_node
            .iter()
            .copied()
            .map(|node| (node, node))
            .collect::<BTreeMap<_, _>>();
        let mut dense_to_augmented = Vec::new();
        let mut dense_symbolic_labels = Vec::new();
        let mut symbolic_source_classes = BTreeSet::new();
        let mut symbolic_virtual_classes = BTreeSet::new();
        let mut edges = Vec::new();
        let mut bound = 1_i128;
        for (index, edge) in self.edges.iter().enumerate() {
            if !edge.active {
                continue;
            }
            bound = bound
                .max(
                    edge.length
                        .numerator()
                        .checked_abs()
                        .ok_or(An19PetalError::Overflow)?,
                )
                .max(edge.length.denominator());
            dense_to_augmented.push(index);
            let symbolic_label = edge.symbolic_length_label();
            let symbolic_length = symbolic_label.effective_length()?;
            let symbolic_class = (symbolic_length.numerator(), symbolic_length.denominator());
            if symbolic_label.root_source.is_some() {
                symbolic_source_classes.insert(symbolic_class);
            } else {
                symbolic_virtual_classes.insert(symbolic_class);
            }
            dense_symbolic_labels.push(symbolic_label);
            edges.push(SourceWeightedEdge {
                first: edge.first,
                second: edge.second,
                length: edge.length,
                weight: ratio(1, 1)?,
            });
        }
        let graph = SourceDynamicGraph::new(self.node_count, edges, bound)
            .map_err(|_| An19PetalError::InvalidAugmentedGraph)?;
        let dense_root_sources = dense_to_augmented
            .iter()
            .map(|stable| self.edges[*stable].root_source)
            .collect();
        Ok(AugmentedProjection {
            graph,
            dense_to_augmented,
            dense_root_sources,
            dense_symbolic_labels,
            length_class_counts: self.edges.iter().filter(|edge| edge.active).fold(
                BTreeMap::new(),
                |mut counts, edge| {
                    *counts
                        .entry((edge.length.numerator(), edge.length.denominator()))
                        .or_insert(0) += 1;
                    counts
                },
            ),
            symbolic_source_classes,
            symbolic_virtual_classes,
            local_to_augmented_node,
            augmented_to_local_node,
        })
    }

    /// Suppresses complete provenance chains into a certified original tree.
    ///
    /// # Errors
    ///
    /// Returns an error for inactive selections, partial original edges, or a
    /// recovered original edge set that is cyclic or disconnected.
    pub fn recover_original_tree(
        &self,
        selected_augmented_edges: &BTreeSet<usize>,
    ) -> Result<BTreeSet<SourceEdgeId>, An19PetalError> {
        if selected_augmented_edges
            .iter()
            .any(|index| self.edges.get(*index).is_none_or(|edge| !edge.active))
        {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        if selected_augmented_edges.len() + 1 != self.node_count {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let mut augmented_connectivity = DisjointSet::new(self.node_count);
        for index in selected_augmented_edges {
            let edge = &self.edges[*index];
            if !augmented_connectivity.union(edge.first.0, edge.second.0) {
                return Err(An19PetalError::InvalidAugmentedGraph);
            }
        }
        if !all_connected(&mut augmented_connectivity, self.node_count) {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let original_edge_count = self.original_endpoints.len();
        let mut active_segments = vec![0_usize; original_edge_count];
        let mut selected_segments = vec![0_usize; original_edge_count];
        for (index, edge) in self.edges.iter().enumerate() {
            if !edge.active {
                continue;
            }
            if let Some(provenance) = &edge.provenance {
                if provenance.edge.0 >= original_edge_count {
                    return Err(An19PetalError::InvalidAugmentedGraph);
                }
                active_segments[provenance.edge.0] = active_segments[provenance.edge.0]
                    .checked_add(1)
                    .ok_or(An19PetalError::Overflow)?;
                if selected_augmented_edges.contains(&index) {
                    selected_segments[provenance.edge.0] = selected_segments[provenance.edge.0]
                        .checked_add(1)
                        .ok_or(An19PetalError::Overflow)?;
                }
            }
        }
        let mut result = BTreeSet::new();
        let mut connectivity = DisjointSet::new(self.original_node_count);
        for index in 0..original_edge_count {
            if selected_segments[index] == 0 {
                continue;
            }
            if active_segments[index] != selected_segments[index] {
                return Err(An19PetalError::InvalidAugmentedGraph);
            }
            let (first, second) = self.original_endpoints[index];
            if !connectivity.union(first.0, second.0) {
                return Err(An19PetalError::InvalidAugmentedGraph);
            }
            result.insert(SourceEdgeId(index));
        }
        if result.len() + 1 != self.original_node_count
            || !all_connected(&mut connectivity, self.original_node_count)
        {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        Ok(result)
    }
}

impl AugmentedProjection {
    fn apply_split_update(&mut self, update: ProjectionSplitUpdate) -> Result<(), An19PetalError> {
        let dense = self
            .dense_to_augmented
            .iter()
            .position(|stable| *stable == update.stable_edge)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        let local_from = self.local_node(update.from)?;
        if self.augmented_to_local_node.contains_key(&update.portal) {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let expected_portal = FlowNodeId(self.local_to_augmented_node.len());
        let root_source = *self
            .dense_root_sources
            .get(dense)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        let symbolic_label = *self
            .dense_symbolic_labels
            .get(dense)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        if symbolic_label.root_source != root_source {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let original_length = self
            .graph
            .edge(SourceEdgeId(dense))
            .ok_or(An19PetalError::InvalidAugmentedGraph)?
            .length;
        let remainder = original_length
            .checked_sub(update.offset)
            .map_err(|_| An19PetalError::Overflow)?;
        let original_class = (original_length.numerator(), original_length.denominator());
        let from_class = (update.offset.numerator(), update.offset.denominator());
        let toward_class = (remainder.numerator(), remainder.denominator());
        if self
            .length_class_counts
            .get(&original_class)
            .is_none_or(|count| *count == 0)
            || [from_class, toward_class].into_iter().any(|class| {
                self.length_class_counts
                    .get(&class)
                    .is_some_and(|count| *count > usize::MAX - 2)
            })
        {
            return Err(An19PetalError::Overflow);
        }
        let (portal, first, second) = self
            .graph
            .split_projection_edge(SourceEdgeId(dense), local_from, update.offset)
            .map_err(|_| An19PetalError::InvalidAugmentedGraph)?;
        if portal != expected_portal || first != SourceEdgeId(dense) {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let remove_original = {
            let count = self
                .length_class_counts
                .get_mut(&original_class)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            *count -= 1;
            *count == 0
        };
        if remove_original {
            self.length_class_counts.remove(&original_class);
        }
        *self.length_class_counts.entry(from_class).or_insert(0) += 1;
        *self.length_class_counts.entry(toward_class).or_insert(0) += 1;
        self.dense_to_augmented[dense] = update.from_edge;
        if second.0 != self.dense_to_augmented.len() {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        self.dense_to_augmented.push(update.toward_edge);
        self.dense_root_sources.push(root_source);
        self.dense_symbolic_labels.push(symbolic_label);
        self.local_to_augmented_node.push(update.portal);
        if self
            .augmented_to_local_node
            .insert(update.portal, portal)
            .is_some()
        {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        Ok(())
    }

    /// Returns the dense active source graph.
    #[must_use]
    pub const fn graph(&self) -> &SourceDynamicGraph {
        &self.graph
    }

    /// Maps every dense edge ID to its stable augmented edge ID.
    #[must_use]
    pub fn dense_to_augmented(&self) -> &[usize] {
        &self.dense_to_augmented
    }

    fn root_source(&self, dense: SourceEdgeId) -> Result<Option<SourceEdgeId>, An19PetalError> {
        self.dense_root_sources
            .get(dense.0)
            .copied()
            .ok_or(An19PetalError::InvalidAugmentedGraph)
    }

    fn symbolic_label(
        &self,
        dense: SourceEdgeId,
    ) -> Result<An19SymbolicLengthLabel, An19PetalError> {
        self.dense_symbolic_labels
            .get(dense.0)
            .copied()
            .ok_or(An19PetalError::InvalidAugmentedGraph)
    }

    fn local_node(&self, augmented: FlowNodeId) -> Result<FlowNodeId, An19PetalError> {
        self.augmented_to_local_node
            .get(&augmented)
            .copied()
            .ok_or(An19PetalError::InvalidAugmentedGraph)
    }

    fn augmented_node(&self, local: FlowNodeId) -> Result<FlowNodeId, An19PetalError> {
        self.local_to_augmented_node
            .get(local.0)
            .copied()
            .ok_or(An19PetalError::InvalidAugmentedGraph)
    }

    fn local_nodes(
        &self,
        augmented: &BTreeSet<FlowNodeId>,
    ) -> Result<BTreeSet<FlowNodeId>, An19PetalError> {
        augmented
            .iter()
            .map(|vertex| self.local_node(*vertex))
            .collect()
    }
}

/// Exact observed projection work grouped by top-level input edge.
///
/// The audit separates one source materialization per projection from extra
/// portal fragments, attributes every split, and checks both against certified
/// recursive scales. Provenance-free virtual fragments use a separate global
/// leaf-and-split charge. This does not prove the independent event-order gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19ProjectionAudit {
    pub original_edge_segment_occurrences: Vec<u64>,
    pub original_edge_materialization_occurrences: Vec<u64>,
    pub original_edge_portal_fragment_occurrences: Vec<u64>,
    pub original_edge_portal_splits: Vec<u64>,
    pub provenance_free_segment_occurrences: u64,
    pub provenance_free_portal_splits: u64,
    pub projected_edge_occurrences: u64,
    pub source_projection_materializations: u64,
    pub portal_fragment_materializations: u64,
    pub source_portal_splits: u64,
    pub maximum_projection_edges: u64,
    pub total_projection_length_classes: u64,
    pub maximum_projection_length_classes: u64,
    pub maximum_symbolic_source_label_classes: u64,
    pub maximum_symbolic_virtual_label_classes: u64,
    pub maximum_original_edge_segment_occurrences: u64,
    pub maximum_original_edge_materialization_occurrences: u64,
    pub maximum_original_edge_portal_fragment_occurrences: u64,
    pub maximum_original_edge_portal_splits: u64,
    pub original_edge_scale_occurrences: Vec<u64>,
    pub maximum_original_edge_scale_occurrences: u64,
    scale_observations: u64,
    source_last_scale_observation: Vec<u64>,
}

impl An19ProjectionAudit {
    fn new(original_edge_count: usize) -> Self {
        Self {
            original_edge_segment_occurrences: vec![0; original_edge_count],
            original_edge_materialization_occurrences: vec![0; original_edge_count],
            original_edge_portal_fragment_occurrences: vec![0; original_edge_count],
            original_edge_portal_splits: vec![0; original_edge_count],
            provenance_free_segment_occurrences: 0,
            provenance_free_portal_splits: 0,
            projected_edge_occurrences: 0,
            source_projection_materializations: 0,
            portal_fragment_materializations: 0,
            source_portal_splits: 0,
            maximum_projection_edges: 0,
            total_projection_length_classes: 0,
            maximum_projection_length_classes: 0,
            maximum_symbolic_source_label_classes: 0,
            maximum_symbolic_virtual_label_classes: 0,
            maximum_original_edge_segment_occurrences: 0,
            maximum_original_edge_materialization_occurrences: 0,
            maximum_original_edge_portal_fragment_occurrences: 0,
            maximum_original_edge_portal_splits: 0,
            original_edge_scale_occurrences: vec![0; original_edge_count],
            maximum_original_edge_scale_occurrences: 0,
            scale_observations: 0,
            source_last_scale_observation: vec![0; original_edge_count],
        }
    }

    fn record_portal_split(
        &mut self,
        root_source: Option<SourceEdgeId>,
        metrics: &mut An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        metrics.portal_splits = checked_metric_sum(metrics.portal_splits, 1)?;
        let Some(source) = root_source else {
            self.provenance_free_portal_splits =
                checked_metric_sum(self.provenance_free_portal_splits, 1)?;
            return Ok(());
        };
        let splits = self
            .original_edge_portal_splits
            .get_mut(source.0)
            .ok_or(An19PetalError::InvalidWorkCertificate)?;
        *splits = checked_metric_sum(*splits, 1)?;
        self.source_portal_splits = checked_metric_sum(self.source_portal_splits, 1)?;
        metrics.source_portal_splits = checked_metric_sum(metrics.source_portal_splits, 1)?;
        self.maximum_original_edge_portal_splits =
            self.maximum_original_edge_portal_splits.max(*splits);
        metrics.maximum_source_portal_splits = metrics.maximum_source_portal_splits.max(*splits);
        Ok(())
    }

    fn record_scale_sources(
        &mut self,
        projection: &AugmentedProjection,
        new_partition_scale: bool,
        metrics: &mut An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        if !new_partition_scale {
            return Ok(());
        }
        self.scale_observations = checked_metric_sum(self.scale_observations, 1)?;
        metrics.source_scale_attribution_scans = checked_metric_sum(
            metrics.source_scale_attribution_scans,
            u64::try_from(projection.dense_root_sources.len())
                .map_err(|_| An19PetalError::Overflow)?,
        )?;
        for source in projection.dense_root_sources.iter().copied().flatten() {
            let last_observation = self
                .source_last_scale_observation
                .get_mut(source.0)
                .ok_or(An19PetalError::InvalidWorkCertificate)?;
            if *last_observation == self.scale_observations {
                continue;
            }
            *last_observation = self.scale_observations;
            let occurrences = self
                .original_edge_scale_occurrences
                .get_mut(source.0)
                .ok_or(An19PetalError::InvalidWorkCertificate)?;
            *occurrences = checked_metric_sum(*occurrences, 1)?;
            self.maximum_original_edge_scale_occurrences = self
                .maximum_original_edge_scale_occurrences
                .max(*occurrences);
            metrics.source_scale_participations =
                checked_metric_sum(metrics.source_scale_participations, 1)?;
            metrics.maximum_source_scale_participations = metrics
                .maximum_source_scale_participations
                .max(*occurrences);
        }
        Ok(())
    }

    fn record_source_materializations(
        &mut self,
        source_segment_counts: Vec<u64>,
        metrics: &mut An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        for (source, segment_count) in source_segment_counts.into_iter().enumerate() {
            if segment_count == 0 {
                continue;
            }
            let materializations = self
                .original_edge_materialization_occurrences
                .get_mut(source)
                .ok_or(An19PetalError::InvalidWorkCertificate)?;
            *materializations = checked_metric_sum(*materializations, 1)?;
            self.source_projection_materializations =
                checked_metric_sum(self.source_projection_materializations, 1)?;
            metrics.source_projection_materializations =
                checked_metric_sum(metrics.source_projection_materializations, 1)?;
            self.maximum_original_edge_materialization_occurrences = self
                .maximum_original_edge_materialization_occurrences
                .max(*materializations);
            metrics.maximum_source_projection_materializations = metrics
                .maximum_source_projection_materializations
                .max(*materializations);

            let fragment_count = segment_count
                .checked_sub(1)
                .ok_or(An19PetalError::Overflow)?;
            let fragment_occurrences = self
                .original_edge_portal_fragment_occurrences
                .get_mut(source)
                .ok_or(An19PetalError::InvalidWorkCertificate)?;
            *fragment_occurrences = checked_metric_sum(*fragment_occurrences, fragment_count)?;
            self.portal_fragment_materializations =
                checked_metric_sum(self.portal_fragment_materializations, fragment_count)?;
            metrics.portal_fragment_materializations =
                checked_metric_sum(metrics.portal_fragment_materializations, fragment_count)?;
            self.maximum_original_edge_portal_fragment_occurrences = self
                .maximum_original_edge_portal_fragment_occurrences
                .max(*fragment_occurrences);
            metrics.maximum_source_portal_fragment_materializations = metrics
                .maximum_source_portal_fragment_materializations
                .max(*fragment_occurrences);
        }
        Ok(())
    }

    fn record(
        &mut self,
        projection: &AugmentedProjection,
        metrics: &mut An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        if projection.dense_root_sources.len() != projection.graph.edge_count()
            || projection.dense_symbolic_labels.len() != projection.graph.edge_count()
        {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        let edge_count =
            u64::try_from(projection.graph.edge_count()).map_err(|_| An19PetalError::Overflow)?;
        self.projected_edge_occurrences =
            checked_metric_sum(self.projected_edge_occurrences, edge_count)?;
        self.maximum_projection_edges = self.maximum_projection_edges.max(edge_count);
        metrics.projected_edge_slots =
            checked_metric_sum(metrics.projected_edge_slots, edge_count)?;
        metrics.maximum_projection_edges = metrics.maximum_projection_edges.max(edge_count);
        let mut length_classes = BTreeSet::new();
        let mut symbolic_source_classes = BTreeSet::new();
        let mut symbolic_virtual_classes = BTreeSet::new();
        let mut source_segment_counts = vec![0_u64; self.original_edge_segment_occurrences.len()];
        for index in 0..projection.graph.edge_count() {
            let edge = projection
                .graph
                .edge(SourceEdgeId(index))
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            length_classes.insert((edge.length.numerator(), edge.length.denominator()));
            let root_source = projection.root_source(SourceEdgeId(index))?;
            let symbolic_label = projection.symbolic_label(SourceEdgeId(index))?;
            if symbolic_label.root_source != root_source
                || !symbolic_label.unsplit_length.is_positive()
            {
                return Err(An19PetalError::InvalidWorkCertificate);
            }
            let symbolic_length = symbolic_label.effective_length()?;
            let symbolic_class = (symbolic_length.numerator(), symbolic_length.denominator());
            if root_source.is_some() {
                symbolic_source_classes.insert(symbolic_class);
            } else {
                symbolic_virtual_classes.insert(symbolic_class);
            }
            match root_source {
                Some(root) => {
                    let projection_occurrences = source_segment_counts
                        .get_mut(root.0)
                        .ok_or(An19PetalError::InvalidWorkCertificate)?;
                    *projection_occurrences = checked_metric_sum(*projection_occurrences, 1)?;
                    let occurrences = self
                        .original_edge_segment_occurrences
                        .get_mut(root.0)
                        .ok_or(An19PetalError::InvalidWorkCertificate)?;
                    *occurrences = checked_metric_sum(*occurrences, 1)?;
                    self.maximum_original_edge_segment_occurrences = self
                        .maximum_original_edge_segment_occurrences
                        .max(*occurrences);
                }
                None => {
                    self.provenance_free_segment_occurrences =
                        checked_metric_sum(self.provenance_free_segment_occurrences, 1)?;
                }
            }
        }
        self.record_source_materializations(source_segment_counts, metrics)?;
        if symbolic_source_classes != projection.symbolic_source_classes
            || symbolic_virtual_classes != projection.symbolic_virtual_classes
        {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        let class_count =
            u64::try_from(length_classes.len()).map_err(|_| An19PetalError::Overflow)?;
        let symbolic_source_class_count =
            u64::try_from(symbolic_source_classes.len()).map_err(|_| An19PetalError::Overflow)?;
        let symbolic_virtual_class_count =
            u64::try_from(symbolic_virtual_classes.len()).map_err(|_| An19PetalError::Overflow)?;
        self.total_projection_length_classes =
            checked_metric_sum(self.total_projection_length_classes, class_count)?;
        self.maximum_projection_length_classes =
            self.maximum_projection_length_classes.max(class_count);
        metrics.projection_length_class_sum =
            checked_metric_sum(metrics.projection_length_class_sum, class_count)?;
        metrics.maximum_projection_length_classes =
            metrics.maximum_projection_length_classes.max(class_count);
        self.maximum_symbolic_source_label_classes = self
            .maximum_symbolic_source_label_classes
            .max(symbolic_source_class_count);
        self.maximum_symbolic_virtual_label_classes = self
            .maximum_symbolic_virtual_label_classes
            .max(symbolic_virtual_class_count);
        metrics.maximum_symbolic_source_label_classes = metrics
            .maximum_symbolic_source_label_classes
            .max(symbolic_source_class_count);
        metrics.maximum_symbolic_virtual_label_classes = metrics
            .maximum_symbolic_virtual_label_classes
            .max(symbolic_virtual_class_count);
        Ok(())
    }

    fn observe_projection_shape(
        &mut self,
        projection: &AugmentedProjection,
        metrics: &mut An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        let edge_count =
            u64::try_from(projection.graph.edge_count()).map_err(|_| An19PetalError::Overflow)?;
        let class_count = u64::try_from(projection.length_class_counts.len())
            .map_err(|_| An19PetalError::Overflow)?;
        let symbolic_source_class_count = u64::try_from(projection.symbolic_source_classes.len())
            .map_err(|_| An19PetalError::Overflow)?;
        let symbolic_virtual_class_count = u64::try_from(projection.symbolic_virtual_classes.len())
            .map_err(|_| An19PetalError::Overflow)?;
        self.maximum_projection_edges = self.maximum_projection_edges.max(edge_count);
        self.maximum_projection_length_classes =
            self.maximum_projection_length_classes.max(class_count);
        metrics.maximum_projection_edges = metrics.maximum_projection_edges.max(edge_count);
        metrics.maximum_projection_length_classes =
            metrics.maximum_projection_length_classes.max(class_count);
        self.maximum_symbolic_source_label_classes = self
            .maximum_symbolic_source_label_classes
            .max(symbolic_source_class_count);
        self.maximum_symbolic_virtual_label_classes = self
            .maximum_symbolic_virtual_label_classes
            .max(symbolic_virtual_class_count);
        metrics.maximum_symbolic_source_label_classes = metrics
            .maximum_symbolic_source_label_classes
            .max(symbolic_source_class_count);
        metrics.maximum_symbolic_virtual_label_classes = metrics
            .maximum_symbolic_virtual_label_classes
            .max(symbolic_virtual_class_count);
        Ok(())
    }

    fn verify_structural_charges(
        &self,
        metrics: &An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        if self.scale_observations == 0 {
            return Ok(());
        }
        for (((materializations, fragments), splits), scales) in self
            .original_edge_materialization_occurrences
            .iter()
            .zip(&self.original_edge_portal_fragment_occurrences)
            .zip(&self.original_edge_portal_splits)
            .zip(&self.original_edge_scale_occurrences)
        {
            let scale_charge = source_materialization_charge(*scales)?;
            let fragment_charge = splits
                .checked_mul(scale_charge)
                .ok_or(An19PetalError::Overflow)?;
            if *materializations > scale_charge || *fragments > fragment_charge {
                return Err(An19PetalError::InvalidWorkCertificate);
            }
        }
        self.verify_structural_virtual_charges(metrics)
    }

    fn verify_structural_virtual_charges(
        &self,
        metrics: &An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        let virtual_fragments =
            checked_metric_sum(metrics.virtual_leaves, self.provenance_free_portal_splits)?;
        let active_scales = metrics
            .maximum_partition_depth
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        let bound = virtual_fragments
            .checked_mul(source_materialization_charge(active_scales)?)
            .ok_or(An19PetalError::Overflow)?;
        if self.provenance_free_segment_occurrences > bound {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        Ok(())
    }

    /// Recomputes aggregate projection relationships and cross-checks metrics.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing or out-of-range root attribution,
    /// inconsistent segment totals, or mismatched projection metrics.
    pub fn verify(
        &self,
        original_edge_count: usize,
        metrics: &An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        let original_occurrences = self
            .original_edge_segment_occurrences
            .iter()
            .try_fold(0_u64, |total, value| checked_metric_sum(total, *value))?;
        let maximum = self
            .original_edge_segment_occurrences
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let source_materializations = self
            .original_edge_materialization_occurrences
            .iter()
            .try_fold(0_u64, |total, value| checked_metric_sum(total, *value))?;
        let maximum_source_materializations = self
            .original_edge_materialization_occurrences
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let portal_fragments = self
            .original_edge_portal_fragment_occurrences
            .iter()
            .try_fold(0_u64, |total, value| checked_metric_sum(total, *value))?;
        let maximum_portal_fragments = self
            .original_edge_portal_fragment_occurrences
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let source_portal_splits = self
            .original_edge_portal_splits
            .iter()
            .try_fold(0_u64, |total, value| checked_metric_sum(total, *value))?;
        let maximum_source_portal_splits = self
            .original_edge_portal_splits
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let scale_occurrences = self
            .original_edge_scale_occurrences
            .iter()
            .try_fold(0_u64, |total, value| checked_metric_sum(total, *value))?;
        let maximum_scale_occurrences = self
            .original_edge_scale_occurrences
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        self.verify_structural_charges(metrics)?;
        if self.original_edge_segment_occurrences.len() != original_edge_count
            || self.original_edge_materialization_occurrences.len() != original_edge_count
            || self.original_edge_portal_fragment_occurrences.len() != original_edge_count
            || self.original_edge_portal_splits.len() != original_edge_count
            || self.original_edge_scale_occurrences.len() != original_edge_count
            || self.source_last_scale_observation.len() != original_edge_count
            || self.scale_observations != metrics.partition_recursion_calls
            || checked_metric_sum(
                original_occurrences,
                self.provenance_free_segment_occurrences,
            )? != self.projected_edge_occurrences
            || maximum != self.maximum_original_edge_segment_occurrences
            || source_materializations != self.source_projection_materializations
            || source_materializations != metrics.source_projection_materializations
            || maximum_source_materializations
                != self.maximum_original_edge_materialization_occurrences
            || maximum_source_materializations != metrics.maximum_source_projection_materializations
            || portal_fragments != self.portal_fragment_materializations
            || portal_fragments != metrics.portal_fragment_materializations
            || maximum_portal_fragments != self.maximum_original_edge_portal_fragment_occurrences
            || maximum_portal_fragments != metrics.maximum_source_portal_fragment_materializations
            || source_portal_splits != self.source_portal_splits
            || source_portal_splits != metrics.source_portal_splits
            || maximum_source_portal_splits != self.maximum_original_edge_portal_splits
            || maximum_source_portal_splits != metrics.maximum_source_portal_splits
            || checked_metric_sum(source_portal_splits, self.provenance_free_portal_splits)?
                != metrics.portal_splits
            || checked_metric_sum(source_materializations, portal_fragments)?
                != original_occurrences
            || self.projected_edge_occurrences != metrics.projected_edge_slots
            || self.maximum_projection_edges != metrics.maximum_projection_edges
            || self.total_projection_length_classes != metrics.projection_length_class_sum
            || self.maximum_projection_length_classes != metrics.maximum_projection_length_classes
            || self.maximum_symbolic_source_label_classes
                != metrics.maximum_symbolic_source_label_classes
            || self.maximum_symbolic_virtual_label_classes
                != metrics.maximum_symbolic_virtual_label_classes
            || self.total_projection_length_classes > self.projected_edge_occurrences
            || self.maximum_projection_length_classes > self.maximum_projection_edges
            || self.maximum_symbolic_source_label_classes > self.maximum_projection_edges
            || self.maximum_symbolic_virtual_label_classes > self.maximum_projection_edges
            || scale_occurrences != metrics.source_scale_participations
            || maximum_scale_occurrences != self.maximum_original_edge_scale_occurrences
            || self.maximum_original_edge_scale_occurrences
                != metrics.maximum_source_scale_participations
        {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct An19HierarchyMetrics {
    pub recursion_calls: u64,
    pub partition_recursion_calls: u64,
    pub maximum_partition_depth: u64,
    pub base_cases: u64,
    pub projection_calls: u64,
    pub projection_cache_hits: u64,
    pub projection_materializations: u64,
    pub projection_incremental_splits: u64,
    pub projected_node_slots: u64,
    pub maximum_projection_nodes: u64,
    pub projected_edge_slots: u64,
    pub maximum_projection_edges: u64,
    pub projection_incident_scans: u64,
    pub projection_active_internal_incident_scans: u64,
    pub projection_active_boundary_incident_scans: u64,
    pub projection_inactive_incident_scans: u64,
    pub projection_length_class_sum: u64,
    pub maximum_projection_length_classes: u64,
    pub maximum_symbolic_source_label_classes: u64,
    pub maximum_symbolic_virtual_label_classes: u64,
    pub source_projection_materializations: u64,
    pub maximum_source_projection_materializations: u64,
    pub portal_fragment_materializations: u64,
    pub maximum_source_portal_fragment_materializations: u64,
    pub source_portal_splits: u64,
    pub maximum_source_portal_splits: u64,
    pub source_scale_participations: u64,
    pub maximum_source_scale_participations: u64,
    pub source_scale_attribution_scans: u64,
    pub contraction_calls: u64,
    pub contracted_edges: u64,
    pub quotient_edges: u64,
    pub petals: u64,
    pub portal_splits: u64,
    pub virtual_leaves: u64,
    pub highway_edges_halved: u64,
    pub highway_edges_reused: u64,
    pub fixed_path_reuses: u64,
    pub shortest_path_runs: u64,
    pub edge_relaxations: u64,
    pub shortest_heap_pushes: u64,
    pub shortest_heap_pops: u64,
    pub shortest_edge_scans: u64,
    pub directed_region_runs: u64,
    pub directed_heap_pushes: u64,
    pub directed_heap_pops: u64,
    pub directed_edge_scans: u64,
    pub membership_sources: u64,
    pub event_heap_pushes: u64,
    pub event_heap_pops: u64,
    pub heap_comparisons: u64,
    pub monotone_queue_pushes: u64,
    pub monotone_queue_pops: u64,
    pub monotone_front_comparisons: u64,
    pub maximum_length_classes: u64,
    pub event_vertex_activations: u64,
    pub event_edge_touches: u64,
    pub volume_queries: u64,
    pub workspace_edge_scans: u64,
    pub radius_edge_scans: u64,
    pub contraction_input_edge_scans: u64,
    pub contraction_retained_edge_scans: u64,
    pub contraction_recovery_edge_scans: u64,
    pub final_recovery_edge_scans: u64,
    pub tree_audit_work_units: u64,
}

const AN19_WORK_BOUND_FACTOR: u64 = 1_024;
const AN19_PROJECTION_MATERIALIZATIONS_PER_SCALE: u64 = 4;

fn source_materialization_charge(scales: u64) -> Result<u64, An19PetalError> {
    // A source segment can enter one full projection at the recursive call,
    // one after the optional imaginary-path mutation, one while preparing its
    // child highway, and one after a same-scale quotient mutation. Cache hits
    // and incremental portal splits do not materialize another projection.
    scales
        .checked_mul(AN19_PROJECTION_MATERIALIZATIONS_PER_SCALE)
        .and_then(|value| value.checked_add(1))
        .ok_or(An19PetalError::Overflow)
}

fn projection_incident_scan_total(metrics: &An19HierarchyMetrics) -> Result<u64, An19PetalError> {
    checked_metric_sum(
        checked_metric_sum(
            metrics.projection_active_internal_incident_scans,
            metrics.projection_active_boundary_incident_scans,
        )?,
        metrics.projection_inactive_incident_scans,
    )
}

fn nonprojection_workspace_scan_total(
    metrics: &An19HierarchyMetrics,
) -> Result<u64, An19PetalError> {
    [
        metrics.radius_edge_scans,
        metrics.contraction_input_edge_scans,
        metrics.contraction_retained_edge_scans,
        metrics.contraction_recovery_edge_scans,
        metrics.final_recovery_edge_scans,
    ]
    .into_iter()
    .try_fold(0_u64, checked_metric_sum)
}

fn final_recovery_edge_scan_total(
    graph: &SourceDynamicGraph,
    metrics: &An19HierarchyMetrics,
) -> Result<u64, An19PetalError> {
    let stable_edges = u64::try_from(graph.edge_count())
        .map_err(|_| An19PetalError::Overflow)?
        .checked_add(metrics.virtual_leaves)
        .and_then(|value| value.checked_add(metrics.portal_splits.checked_mul(2)?))
        .ok_or(An19PetalError::Overflow)?;
    let selected_edges = u64::try_from(graph.node_count())
        .map_err(|_| An19PetalError::Overflow)?
        .checked_add(metrics.virtual_leaves)
        .and_then(|value| value.checked_add(metrics.portal_splits))
        .and_then(|value| value.checked_sub(1))
        .ok_or(An19PetalError::Overflow)?;
    stable_edges
        .checked_mul(2)
        .and_then(|value| value.checked_add(selected_edges))
        .ok_or(An19PetalError::Overflow)
}

fn projection_incident_scan_bounds(
    graph: &SourceDynamicGraph,
    metrics: &An19HierarchyMetrics,
    scale_charge: u64,
) -> Result<(u64, u64), An19PetalError> {
    // Each source, virtual leaf, or split creates one active segment lineage;
    // an active or inactive segment has two incident references.
    let active_lineages = u64::try_from(graph.edge_count())
        .map_err(|_| An19PetalError::Overflow)?
        .checked_add(metrics.virtual_leaves)
        .and_then(|value| value.checked_add(metrics.portal_splits))
        .ok_or(An19PetalError::Overflow)?;
    let boundary = active_lineages
        .checked_mul(2)
        .and_then(|value| value.checked_mul(scale_charge))
        .ok_or(An19PetalError::Overflow)?;
    let inactive = metrics
        .portal_splits
        .checked_mul(2)
        .and_then(|value| value.checked_mul(scale_charge))
        .ok_or(An19PetalError::Overflow)?;
    Ok((boundary, inactive))
}

fn source_scale_participation_bound(logarithmic_levels: u64) -> Result<u64, An19PetalError> {
    // AN19 Section 6 gives an active radius ratio of at most 2*n^2, while
    // Claims 5--6 shrink child radii by 3/4 and (3/4)^3 < 1/2. This is a
    // checked necessary gate for augmented runs; portal-fragment charging is
    // audited separately before the structural runtime claim can be enabled.
    logarithmic_levels
        .checked_mul(6)
        .and_then(|value| value.checked_add(4))
        .ok_or(An19PetalError::Overflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum An19ProjectionMode {
    ClusterLocal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum An19LengthMode {
    ExactRational,
    RoundedPowerOfTwo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum An19PriorityQueueMode {
    BinaryHeap,
    ReducedLengthMonotone,
    SourceMonotone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum An19AmortizationMode {
    AggregateRegressionOnly,
    StructuralSourceBound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct An19WorkCertificate {
    pub input_nodes: usize,
    pub input_edges: usize,
    pub logarithmic_levels: u64,
    pub iterated_logarithmic_levels: u64,
    pub source_scale_participation_bound: u64,
    pub observed_work_units: u64,
    pub maximum_work_units: u64,
    pub oracle_fallbacks: u64,
    pub numeric_length_expansions: u64,
    pub compact_weighted_input: bool,
    pub projection_mode: An19ProjectionMode,
    pub length_mode: An19LengthMode,
    pub priority_queue_mode: An19PriorityQueueMode,
    pub amortization_mode: An19AmortizationMode,
}

impl An19WorkCertificate {
    fn build(
        graph: &SourceDynamicGraph,
        unit_input: bool,
        length_mode: An19LengthMode,
        metrics: &An19HierarchyMetrics,
    ) -> Result<Self, An19PetalError> {
        let logarithmic_levels =
            u64::from(usize::BITS - graph.node_count().saturating_sub(1).leading_zeros());
        let iterated_logarithmic_levels = u64::try_from(ceil_log_log(graph.node_count()))
            .map_err(|_| An19PetalError::Overflow)?;
        let source_scale_participation_bound =
            source_scale_participation_bound(logarithmic_levels)?;
        let maximum_work_units = AN19_WORK_BOUND_FACTOR
            .checked_mul(
                u64::try_from(graph.edge_count().max(1)).map_err(|_| An19PetalError::Overflow)?,
            )
            .and_then(|value| value.checked_mul(logarithmic_levels.max(1)))
            .and_then(|value| value.checked_mul(iterated_logarithmic_levels.max(1)))
            .ok_or(An19PetalError::Overflow)?;
        Ok(Self {
            input_nodes: graph.node_count(),
            input_edges: graph.edge_count(),
            logarithmic_levels,
            iterated_logarithmic_levels,
            source_scale_participation_bound,
            observed_work_units: hierarchy_work_units(metrics)?,
            maximum_work_units,
            oracle_fallbacks: 0,
            numeric_length_expansions: 0,
            compact_weighted_input: !unit_input,
            projection_mode: An19ProjectionMode::ClusterLocal,
            length_mode,
            priority_queue_mode: An19PriorityQueueMode::ReducedLengthMonotone,
            amortization_mode: An19AmortizationMode::AggregateRegressionOnly,
        })
    }

    fn verify(
        &self,
        graph: &SourceDynamicGraph,
        metrics: &An19HierarchyMetrics,
    ) -> Result<(), An19PetalError> {
        let rebuilt = Self::build(
            graph,
            !self.compact_weighted_input,
            An19LengthMode::RoundedPowerOfTwo,
            metrics,
        )?;
        let scale_charge = source_materialization_charge(self.source_scale_participation_bound)?;
        let (boundary_scan_bound, inactive_scan_bound) =
            projection_incident_scan_bounds(graph, metrics, scale_charge)?;
        let classified_projection_scans = projection_incident_scan_total(metrics)?;
        if *self != rebuilt
            || self.oracle_fallbacks != 0
            || self.numeric_length_expansions != 0
            || self.projection_mode != An19ProjectionMode::ClusterLocal
            || self.length_mode != An19LengthMode::RoundedPowerOfTwo
            || self.priority_queue_mode != An19PriorityQueueMode::ReducedLengthMonotone
            || metrics.shortest_heap_pushes != 0
            || metrics.shortest_heap_pops != 0
            || metrics.directed_heap_pushes != 0
            || metrics.directed_heap_pops != 0
            || metrics.event_heap_pushes != 0
            || metrics.event_heap_pops != 0
            || metrics.heap_comparisons != 0
            || self.observed_work_units > self.maximum_work_units
            || metrics.event_heap_pushes != metrics.event_heap_pops
            || metrics.shortest_heap_pushes != metrics.shortest_heap_pops
            || metrics.monotone_queue_pushes != metrics.monotone_queue_pops
            || checked_metric_sum(metrics.partition_recursion_calls, metrics.contraction_calls)?
                != metrics.recursion_calls
            || metrics.maximum_partition_depth >= self.source_scale_participation_bound
            || metrics.projection_calls < metrics.recursion_calls
            || metrics.projection_cache_hits > metrics.projection_calls
            || checked_metric_sum(
                metrics.projection_cache_hits,
                metrics.projection_materializations,
            )? != metrics.projection_calls
            || metrics.projection_incremental_splits > metrics.portal_splits
            || metrics.projection_active_internal_incident_scans
                != metrics
                    .projected_edge_slots
                    .checked_mul(2)
                    .ok_or(An19PetalError::Overflow)?
            || metrics.projected_node_slots
                > checked_metric_sum(
                    metrics.projected_edge_slots,
                    metrics.projection_materializations,
                )?
            || metrics.projection_active_boundary_incident_scans > boundary_scan_bound
            || metrics.projection_inactive_incident_scans > inactive_scan_bound
            || classified_projection_scans != metrics.projection_incident_scans
            || metrics.projection_incident_scans > metrics.workspace_edge_scans
            || nonprojection_workspace_scan_total(metrics)?
                != metrics
                    .workspace_edge_scans
                    .checked_sub(metrics.projection_incident_scans)
                    .ok_or(An19PetalError::InvalidWorkCertificate)?
            || metrics.contraction_retained_edge_scans != metrics.quotient_edges
            || metrics.final_recovery_edge_scans != final_recovery_edge_scan_total(graph, metrics)?
            || metrics.maximum_source_scale_participations > self.source_scale_participation_bound
            || metrics.maximum_source_scale_participations
                > metrics
                    .maximum_partition_depth
                    .checked_add(1)
                    .ok_or(An19PetalError::Overflow)?
            || metrics.source_projection_materializations
                > metrics
                    .source_scale_participations
                    .checked_mul(AN19_PROJECTION_MATERIALIZATIONS_PER_SCALE)
                    .and_then(|value| value.checked_add(u64::try_from(graph.edge_count()).ok()?))
                    .ok_or(An19PetalError::Overflow)?
            || metrics.maximum_source_projection_materializations
                > source_materialization_charge(metrics.maximum_source_scale_participations)?
            || metrics.source_portal_splits > metrics.portal_splits
            || metrics.maximum_source_portal_splits > metrics.source_portal_splits
            || metrics.portal_fragment_materializations
                > metrics
                    .source_portal_splits
                    .checked_mul(source_materialization_charge(
                        self.source_scale_participation_bound,
                    )?)
                    .ok_or(An19PetalError::Overflow)?
            || metrics.portal_fragment_materializations > metrics.projected_edge_slots
            || metrics.source_scale_participations > metrics.source_scale_attribution_scans
            || metrics.maximum_projection_nodes == 0
            || metrics.maximum_projection_nodes > metrics.projected_node_slots
            || metrics.directed_region_runs
                != metrics
                    .petals
                    .checked_mul(2)
                    .ok_or(An19PetalError::Overflow)?
            || (self.compact_weighted_input && metrics.virtual_leaves > metrics.recursion_calls)
        {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        Ok(())
    }

    /// Reports whether the implementation satisfies AN19 Section 7's
    /// power-of-two rounding and monotone-queue runtime prerequisites.
    #[must_use]
    pub const fn source_runtime_verified(&self) -> bool {
        matches!(self.projection_mode, An19ProjectionMode::ClusterLocal)
            && matches!(self.length_mode, An19LengthMode::RoundedPowerOfTwo)
            && matches!(
                self.priority_queue_mode,
                An19PriorityQueueMode::SourceMonotone
            )
            && matches!(
                self.amortization_mode,
                An19AmortizationMode::StructuralSourceBound
            )
    }
}

fn hierarchy_work_units(metrics: &An19HierarchyMetrics) -> Result<u64, An19PetalError> {
    [
        metrics.recursion_calls,
        metrics.partition_recursion_calls,
        metrics.base_cases,
        metrics.projection_calls,
        metrics.projection_cache_hits,
        metrics.projection_incremental_splits,
        metrics.projected_node_slots,
        metrics.projected_edge_slots,
        metrics.projection_length_class_sum,
        metrics.source_projection_materializations,
        metrics.portal_fragment_materializations,
        metrics.source_scale_participations,
        metrics.source_scale_attribution_scans,
        metrics.contraction_calls,
        metrics.contracted_edges,
        metrics.quotient_edges,
        metrics.petals,
        metrics.portal_splits,
        metrics.virtual_leaves,
        metrics.highway_edges_halved,
        metrics.highway_edges_reused,
        metrics.fixed_path_reuses,
        metrics.shortest_path_runs,
        metrics.edge_relaxations,
        metrics.shortest_heap_pushes,
        metrics.shortest_heap_pops,
        metrics.shortest_edge_scans,
        metrics.directed_region_runs,
        metrics.directed_heap_pushes,
        metrics.directed_heap_pops,
        metrics.directed_edge_scans,
        metrics.membership_sources,
        metrics.event_heap_pushes,
        metrics.event_heap_pops,
        metrics.heap_comparisons,
        metrics.monotone_queue_pushes,
        metrics.monotone_queue_pops,
        metrics.monotone_front_comparisons,
        metrics.maximum_length_classes,
        metrics.event_vertex_activations,
        metrics.event_edge_touches,
        metrics.volume_queries,
        metrics.workspace_edge_scans,
        metrics.radius_edge_scans,
        metrics.contraction_input_edge_scans,
        metrics.contraction_retained_edge_scans,
        metrics.contraction_recovery_edge_scans,
        metrics.final_recovery_edge_scans,
        metrics.tree_audit_work_units,
    ]
    .into_iter()
    .try_fold(0_u64, |total, value| {
        total.checked_add(value).ok_or(An19PetalError::Overflow)
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19RadiusEdge {
    pub first: FlowNodeId,
    pub second: FlowNodeId,
    pub length: ExactRatio,
    pub root_source: Option<SourceEdgeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19RadiusCertificate {
    pub original_node_count: usize,
    pub recursion_parent: Option<usize>,
    pub partition_depth: u64,
    pub same_scale_contraction: bool,
    pub cluster_size: usize,
    pub base_vertex_limit: usize,
    pub center: FlowNodeId,
    pub target: FlowNodeId,
    pub radius: ExactRatio,
    pub base_threshold: ExactRatio,
    pub base_case: bool,
    pub contraction_threshold: Option<ExactRatio>,
    pub contraction_component_of: Vec<(FlowNodeId, usize)>,
    pub contracted_edge_count: usize,
    pub distances: Vec<(FlowNodeId, ExactRatio)>,
    pub edges: Vec<An19RadiusEdge>,
}

impl An19RadiusCertificate {
    /// Independently checks the recorded exact shortest-path radius witness.
    ///
    /// # Errors
    ///
    /// Returns an error for incomplete vertices, a violated edge triangle
    /// inequality, a missing tight predecessor, an incorrect maximum radius,
    /// or an inconsistent base-case decision.
    pub fn verify(&self) -> Result<(), An19PetalError> {
        if self.cluster_size == 0
            || self.cluster_size != self.distances.len()
            || !self
                .distances
                .iter()
                .any(|(vertex, distance)| *vertex == self.center && distance.is_zero())
            || !self
                .distances
                .iter()
                .any(|(vertex, _)| *vertex == self.target)
        {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        let distance_map = self.distances.iter().copied().collect::<BTreeMap<_, _>>();
        if distance_map.len() != self.cluster_size {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        let mut maximum = ratio(0, 1)?;
        for distance in distance_map.values() {
            if distance.is_negative() {
                return Err(An19PetalError::InvalidRadiusCertificate);
            }
            if ratio_less(maximum, *distance)? {
                maximum = *distance;
            }
        }
        if maximum != self.radius {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        for edge in &self.edges {
            let first = *distance_map
                .get(&edge.first)
                .ok_or(An19PetalError::InvalidRadiusCertificate)?;
            let second = *distance_map
                .get(&edge.second)
                .ok_or(An19PetalError::InvalidRadiusCertificate)?;
            if !edge.length.is_positive()
                || ratio_less(
                    first
                        .checked_add(edge.length)
                        .map_err(|_| An19PetalError::Overflow)?,
                    second,
                )?
                || ratio_less(
                    second
                        .checked_add(edge.length)
                        .map_err(|_| An19PetalError::Overflow)?,
                    first,
                )?
            {
                return Err(An19PetalError::InvalidRadiusCertificate);
            }
        }
        for (vertex, distance) in &self.distances {
            if *vertex == self.center {
                continue;
            }
            let has_tight_predecessor = self.edges.iter().any(|edge| {
                let neighbor = if edge.first == *vertex {
                    Some(edge.second)
                } else if edge.second == *vertex {
                    Some(edge.first)
                } else {
                    None
                };
                neighbor.is_some_and(|neighbor| {
                    distance_map
                        .get(&neighbor)
                        .is_some_and(|neighbor_distance| {
                            neighbor_distance
                                .checked_add(edge.length)
                                .is_ok_and(|candidate| candidate == *distance)
                        })
                })
            });
            if !has_tight_predecessor {
                return Err(An19PetalError::InvalidRadiusCertificate);
            }
        }
        let expected_base = self.cluster_size <= self.base_vertex_limit
            || self
                .base_threshold
                .at_least(self.radius)
                .map_err(|_| An19PetalError::Overflow)?;
        if expected_base != self.base_case {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        self.verify_contraction()?;
        Ok(())
    }

    fn verify_contraction(&self) -> Result<(), An19PetalError> {
        let Some(threshold) = self.contraction_threshold else {
            return if self.contraction_component_of.is_empty() && self.contracted_edge_count == 0 {
                Ok(())
            } else {
                Err(An19PetalError::InvalidRadiusCertificate)
            };
        };
        let n = i128::try_from(self.original_node_count).map_err(|_| An19PetalError::Overflow)?;
        let n_squared = n.checked_mul(n).ok_or(An19PetalError::Overflow)?;
        let expected_threshold = self
            .radius
            .checked_mul(ratio(1, n_squared)?)
            .map_err(|_| An19PetalError::Overflow)?;
        if threshold != expected_threshold
            || self.contraction_component_of.len() != self.cluster_size
        {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        let node_count = self
            .distances
            .iter()
            .map(|(vertex, _)| vertex.0)
            .max()
            .and_then(|value| value.checked_add(1))
            .ok_or(An19PetalError::InvalidRadiusCertificate)?;
        let mut connectivity = DisjointSet::new(node_count);
        let mut contracted_edge_count = 0_usize;
        for edge in &self.edges {
            if ratio_less(edge.length, threshold)? {
                connectivity.union(edge.first.0, edge.second.0);
                contracted_edge_count = contracted_edge_count
                    .checked_add(1)
                    .ok_or(An19PetalError::Overflow)?;
            }
        }
        let mut root_to_component = BTreeMap::new();
        let mut expected_components = Vec::new();
        for (vertex, _) in &self.distances {
            let root = connectivity.find(vertex.0);
            let next = root_to_component.len();
            let component = *root_to_component.entry(root).or_insert(next);
            expected_components.push((*vertex, component));
        }
        if contracted_edge_count == 0
            || contracted_edge_count != self.contracted_edge_count
            || expected_components != self.contraction_component_of
        {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        Ok(())
    }
}

/// Exact source-semantics implementation of AN19 Figures 4--6.
///
/// This constructor deliberately uses the repeated-shortest-path Figure 6
/// baseline. Its output and counters do not establish the fast region-growing
/// runtime claimed by AN19.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19HierarchicalLsst {
    pub tree_edges: BTreeSet<SourceEdgeId>,
    pub weighted_stretch: ExactRatio,
    pub total_weight: ExactRatio,
    pub radius_certificates: Vec<An19RadiusCertificate>,
    pub metrics: An19HierarchyMetrics,
    pub projection_audit: An19ProjectionAudit,
    pub work_certificate: An19WorkCertificate,
}

impl An19HierarchicalLsst {
    /// Runs the exact hierarchical petal decomposition from `root`.
    ///
    /// # Errors
    ///
    /// Returns an error for a disconnected source graph, an invalid root, a
    /// failed Figure 5 partition, an invalid recovered tree, or exact
    /// arithmetic overflow.
    pub fn construct(graph: &SourceDynamicGraph, root: FlowNodeId) -> Result<Self, An19PetalError> {
        if root.0 >= graph.node_count() {
            return Err(An19PetalError::InvalidDomain);
        }
        let mut workspace = AugmentedAn19Graph::from_source_with_length_mode(
            graph,
            An19LengthMode::RoundedPowerOfTwo,
        )?;
        let cluster = (0..graph.node_count())
            .map(FlowNodeId)
            .collect::<BTreeSet<_>>();
        let mut metrics = An19HierarchyMetrics::default();
        let mut radius_certificates = Vec::new();
        let mut projection_audit = An19ProjectionAudit::new(graph.edge_count());
        let selected = hierarchical_petal_decomposition(
            &mut workspace,
            cluster,
            root,
            root,
            graph.node_count(),
            None,
            0,
            false,
            &mut radius_certificates,
            &mut metrics,
            &mut projection_audit,
        )?;
        add_workspace_edge_scans(
            &mut metrics,
            WorkspaceScanClass::FinalRecovery,
            workspace.edges.len(),
            2,
        )?;
        add_workspace_edge_scans(
            &mut metrics,
            WorkspaceScanClass::FinalRecovery,
            selected.len(),
            1,
        )?;
        let tree_edges = workspace.recover_original_tree(&selected)?;
        metrics.tree_audit_work_units = tree_audit_work_units(graph)?
            .checked_mul(2)
            .ok_or(An19PetalError::Overflow)?;
        let (weighted_stretch, total_weight) = audit_original_tree_stretch(graph, &tree_edges)?;
        let work_certificate = An19WorkCertificate::build(
            graph,
            workspace.unit_input,
            workspace.length_mode,
            &metrics,
        )?;
        let result = Self {
            tree_edges,
            weighted_stretch,
            total_weight,
            radius_certificates,
            metrics,
            projection_audit,
            work_certificate,
        };
        result.verify(graph)?;
        Ok(result)
    }

    /// Recomputes the original-tree and every stored radius certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when the recovered original edge set is not a tree,
    /// its exact weighted stretch differs, or a recursive radius witness is
    /// invalid.
    #[allow(clippy::too_many_lines)]
    pub fn verify(&self, graph: &SourceDynamicGraph) -> Result<(), An19PetalError> {
        let (weighted_stretch, total_weight) =
            audit_original_tree_stretch(graph, &self.tree_edges)?;
        if weighted_stretch != self.weighted_stretch
            || total_weight != self.total_weight
            || self.radius_certificates.is_empty()
            || u64::try_from(self.radius_certificates.len())
                .map_err(|_| An19PetalError::Overflow)?
                != self.metrics.recursion_calls
        {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        self.work_certificate.verify(graph, &self.metrics)?;
        self.projection_audit
            .verify(graph.edge_count(), &self.metrics)?;
        if self
            .projection_audit
            .original_edge_segment_occurrences
            .contains(&0)
            || self
                .projection_audit
                .original_edge_scale_occurrences
                .contains(&0)
        {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        let mut partition_calls = 0_u64;
        let mut same_scale_contractions = 0_u64;
        let mut maximum_partition_depth = 0_u64;
        let mut scale_observation = 0_u64;
        let mut source_last_observation = vec![0_u64; graph.edge_count()];
        let mut rebuilt_source_scale_occurrences = vec![0_u64; graph.edge_count()];
        let mut rebuilt_source_scale_scans = 0_u64;
        for (index, certificate) in self.radius_certificates.iter().enumerate() {
            certificate.verify()?;
            if certificate.same_scale_contraction {
                same_scale_contractions = checked_metric_sum(same_scale_contractions, 1)?;
            } else {
                partition_calls = checked_metric_sum(partition_calls, 1)?;
                maximum_partition_depth = maximum_partition_depth.max(certificate.partition_depth);
                scale_observation = checked_metric_sum(scale_observation, 1)?;
                rebuilt_source_scale_scans = checked_metric_sum(
                    rebuilt_source_scale_scans,
                    u64::try_from(certificate.edges.len()).map_err(|_| An19PetalError::Overflow)?,
                )?;
                for edge in &certificate.edges {
                    let Some(source) = edge.root_source else {
                        continue;
                    };
                    let last_observation = source_last_observation
                        .get_mut(source.0)
                        .ok_or(An19PetalError::InvalidWorkCertificate)?;
                    if *last_observation == scale_observation {
                        continue;
                    }
                    *last_observation = scale_observation;
                    let occurrences = rebuilt_source_scale_occurrences
                        .get_mut(source.0)
                        .ok_or(An19PetalError::InvalidWorkCertificate)?;
                    *occurrences = checked_metric_sum(*occurrences, 1)?;
                }
            }
            match (index, certificate.recursion_parent) {
                (0, None)
                    if certificate.partition_depth == 0 && !certificate.same_scale_contraction => {}
                (0, _) | (_, None) => {
                    return Err(An19PetalError::InvalidRadiusCertificate);
                }
                (_, Some(parent_index)) => {
                    let parent = self
                        .radius_certificates
                        .get(parent_index)
                        .filter(|_| parent_index < index)
                        .ok_or(An19PetalError::InvalidRadiusCertificate)?;
                    if certificate.same_scale_contraction {
                        if parent.contraction_threshold.is_none()
                            || certificate.partition_depth != parent.partition_depth
                        {
                            return Err(An19PetalError::InvalidRadiusCertificate);
                        }
                    } else {
                        let expected_depth = parent
                            .partition_depth
                            .checked_add(1)
                            .ok_or(An19PetalError::Overflow)?;
                        let maximum_child_radius = parent
                            .radius
                            .checked_mul(ratio(3, 4)?)
                            .map_err(|_| An19PetalError::Overflow)?;
                        if certificate.partition_depth != expected_depth
                            || ratio_less(maximum_child_radius, certificate.radius)?
                        {
                            return Err(An19PetalError::InvalidRadiusCertificate);
                        }
                    }
                }
            }
        }
        let contraction_calls = self
            .radius_certificates
            .iter()
            .filter(|certificate| certificate.contraction_threshold.is_some())
            .count();
        let contracted_edges = self
            .radius_certificates
            .iter()
            .try_fold(0_usize, |total, certificate| {
                total.checked_add(certificate.contracted_edge_count)
            })
            .ok_or(An19PetalError::Overflow)?;
        let rebuilt_source_scale_total = rebuilt_source_scale_occurrences
            .iter()
            .try_fold(0_u64, |total, value| checked_metric_sum(total, *value))?;
        let rebuilt_source_scale_maximum = rebuilt_source_scale_occurrences
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        let rebuilt_radius_edge_scans =
            self.radius_certificates
                .iter()
                .try_fold(0_u64, |total, certificate| {
                    checked_metric_sum(
                        total,
                        u64::try_from(certificate.edges.len())
                            .map_err(|_| An19PetalError::Overflow)?
                            .checked_mul(2)
                            .ok_or(An19PetalError::Overflow)?,
                    )
                })?;
        let rebuilt_contraction_input_edge_scans = if self.work_certificate.compact_weighted_input {
            self.radius_certificates
                .iter()
                .filter(|certificate| !certificate.base_case)
                .try_fold(0_u64, |total, certificate| {
                    checked_metric_sum(
                        total,
                        u64::try_from(certificate.edges.len())
                            .map_err(|_| An19PetalError::Overflow)?,
                    )
                })?
        } else {
            0
        };
        let rebuilt_contraction_recovery_edge_scans = self
            .radius_certificates
            .iter()
            .filter(|certificate| certificate.contraction_threshold.is_some())
            .try_fold(0_u64, |total, certificate| {
                let components = certificate
                    .contraction_component_of
                    .iter()
                    .map(|(_, component)| *component)
                    .collect::<BTreeSet<_>>()
                    .len();
                let quotient_tree_edges = components
                    .checked_sub(1)
                    .ok_or(An19PetalError::InvalidContraction)?;
                checked_metric_sum(
                    total,
                    u64::try_from(certificate.contracted_edge_count)
                        .map_err(|_| An19PetalError::Overflow)?
                        .checked_add(
                            u64::try_from(quotient_tree_edges)
                                .map_err(|_| An19PetalError::Overflow)?
                                .checked_mul(2)
                                .ok_or(An19PetalError::Overflow)?,
                        )
                        .ok_or(An19PetalError::Overflow)?,
                )
            })?;
        if u64::try_from(contraction_calls).map_err(|_| An19PetalError::Overflow)?
            != self.metrics.contraction_calls
            || u64::try_from(contracted_edges).map_err(|_| An19PetalError::Overflow)?
                != self.metrics.contracted_edges
            || same_scale_contractions != self.metrics.contraction_calls
            || partition_calls != self.metrics.partition_recursion_calls
            || maximum_partition_depth != self.metrics.maximum_partition_depth
            || rebuilt_source_scale_occurrences
                != self.projection_audit.original_edge_scale_occurrences
            || rebuilt_source_scale_maximum
                != self
                    .projection_audit
                    .maximum_original_edge_scale_occurrences
            || rebuilt_source_scale_total != self.metrics.source_scale_participations
            || rebuilt_source_scale_maximum != self.metrics.maximum_source_scale_participations
            || rebuilt_source_scale_scans != self.metrics.source_scale_attribution_scans
            || rebuilt_radius_edge_scans != self.metrics.radius_edge_scans
            || rebuilt_contraction_input_edge_scans != self.metrics.contraction_input_edge_scans
            || rebuilt_contraction_recovery_edge_scans
                != self.metrics.contraction_recovery_edge_scans
        {
            return Err(An19PetalError::InvalidRadiusCertificate);
        }
        Ok(())
    }
}

struct An19HierarchyPiece {
    cluster: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    connection_edge: usize,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn hierarchical_petal_decomposition(
    workspace: &mut AugmentedAn19Graph,
    cluster: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    original_node_count: usize,
    recursion_parent: Option<usize>,
    partition_depth: u64,
    same_scale_contraction: bool,
    radius_certificates: &mut Vec<An19RadiusCertificate>,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<BTreeSet<usize>, An19PetalError> {
    metrics.recursion_calls = metrics
        .recursion_calls
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    if !same_scale_contraction {
        metrics.partition_recursion_calls =
            checked_metric_sum(metrics.partition_recursion_calls, 1)?;
        metrics.maximum_partition_depth = metrics.maximum_partition_depth.max(partition_depth);
    }
    let projection = hierarchy_projection(workspace, &cluster, metrics, projection_audit)?;
    projection_audit.record_scale_sources(&projection, !same_scale_contraction, metrics)?;
    let paths = hierarchy_shortest_paths(&projection, &cluster, center, metrics)?;
    let radius = hierarchy_radius(&cluster, &paths)?;
    add_workspace_edge_scans(
        metrics,
        WorkspaceScanClass::Radius,
        projection.graph().edge_count(),
        2,
    )?;
    let local_cluster = projection.local_nodes(&cluster)?;
    let local_center = projection.local_node(center)?;
    let threshold = hierarchy_base_threshold(original_node_count)?
        .checked_mul(minimum_cluster_edge_length(
            projection.graph(),
            &local_cluster,
        )?)
        .map_err(|_| An19PetalError::Overflow)?;
    let base_vertex_limit = 2;
    let base_case = cluster.len() <= base_vertex_limit
        || threshold
            .at_least(radius)
            .map_err(|_| An19PetalError::Overflow)?;
    let certificate_index = radius_certificates.len();
    radius_certificates.push(build_radius_certificate(
        &projection,
        original_node_count,
        recursion_parent,
        partition_depth,
        same_scale_contraction,
        &cluster,
        center,
        target,
        radius,
        threshold,
        base_vertex_limit,
        base_case,
        &paths,
    )?);
    if base_case {
        metrics.base_cases = metrics
            .base_cases
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        return hierarchy_shortest_path_tree(&projection, &cluster, center, &paths);
    }
    if !workspace.unit_input {
        add_workspace_edge_scans(
            metrics,
            WorkspaceScanClass::ContractionInput,
            projection.graph().edge_count(),
            1,
        )?;
        let contraction = An19ShortEdgeContraction::build_with_radius(
            projection.graph(),
            &local_cluster,
            local_center,
            radius,
            original_node_count,
        )?;
        if !contraction.contracted_edges.is_empty() {
            attach_contraction_certificate(
                radius_certificates
                    .last_mut()
                    .ok_or(An19PetalError::InvalidRadiusCertificate)?,
                &projection,
                &contraction,
            )?;
            return hierarchy_contracted_tree(
                &projection,
                &contraction,
                center,
                target,
                original_node_count,
                certificate_index,
                partition_depth,
                radius_certificates,
                metrics,
                projection_audit,
            );
        }
    }

    drop(projection);
    let (mut stigma, pieces, stigma_target) = petal_decomposition(
        workspace,
        cluster,
        center,
        target,
        radius,
        metrics,
        projection_audit,
    )?;
    let mut selected = BTreeSet::new();
    for piece in pieces {
        halve_highway(
            workspace,
            &piece.cluster,
            piece.center,
            piece.target,
            metrics,
            projection_audit,
        )?;
        let subtree = hierarchical_petal_decomposition(
            workspace,
            piece.cluster,
            piece.center,
            piece.target,
            original_node_count,
            Some(certificate_index),
            partition_depth
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?,
            false,
            radius_certificates,
            metrics,
            projection_audit,
        )?;
        selected.extend(subtree);
        if !selected.insert(piece.connection_edge) {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
    }
    halve_highway(
        workspace,
        &stigma,
        center,
        stigma_target,
        metrics,
        projection_audit,
    )?;
    let stigma_tree = hierarchical_petal_decomposition(
        workspace,
        std::mem::take(&mut stigma),
        center,
        stigma_target,
        original_node_count,
        Some(certificate_index),
        partition_depth
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?,
        false,
        radius_certificates,
        metrics,
        projection_audit,
    )?;
    selected.extend(stigma_tree);
    Ok(selected)
}

fn attach_contraction_certificate(
    certificate: &mut An19RadiusCertificate,
    projection: &AugmentedProjection,
    contraction: &An19ShortEdgeContraction,
) -> Result<(), An19PetalError> {
    certificate.contraction_threshold = Some(contraction.contraction_threshold);
    certificate.contraction_component_of = certificate
        .distances
        .iter()
        .map(|(vertex, _)| {
            let local = projection.local_node(*vertex)?;
            contraction
                .component_of
                .get(local.0)
                .copied()
                .flatten()
                .map(|component| (*vertex, component))
                .ok_or(An19PetalError::InvalidContraction)
        })
        .collect::<Result<_, _>>()?;
    certificate.contracted_edge_count = contraction.contracted_edges.len();
    certificate.verify()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn hierarchy_contracted_tree(
    projection: &AugmentedProjection,
    contraction: &An19ShortEdgeContraction,
    center: FlowNodeId,
    target: FlowNodeId,
    original_node_count: usize,
    recursion_parent: usize,
    partition_depth: u64,
    radius_certificates: &mut Vec<An19RadiusCertificate>,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<BTreeSet<usize>, An19PetalError> {
    add_workspace_edge_scans(
        metrics,
        WorkspaceScanClass::ContractionRetained,
        contraction.retained_edges.len(),
        1,
    )?;
    let mut quotient_edges = Vec::new();
    let mut quotient_to_dense = Vec::new();
    let mut quotient_root_sources = Vec::new();
    let mut quotient_symbolic_labels = Vec::new();
    let mut bound = 1_i128;
    for dense in &contraction.retained_edges {
        let edge = projection
            .graph()
            .edge(*dense)
            .ok_or(An19PetalError::InvalidContraction)?;
        let first = contraction
            .component_of
            .get(edge.first.0)
            .copied()
            .flatten()
            .ok_or(An19PetalError::InvalidContraction)?;
        let second = contraction
            .component_of
            .get(edge.second.0)
            .copied()
            .flatten()
            .ok_or(An19PetalError::InvalidContraction)?;
        bound = bound
            .max(
                edge.length
                    .numerator()
                    .checked_abs()
                    .ok_or(An19PetalError::Overflow)?,
            )
            .max(edge.length.denominator());
        quotient_edges.push(SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: edge.length,
            weight: edge.weight,
        });
        quotient_to_dense.push(*dense);
        quotient_root_sources.push(projection.root_source(*dense)?);
        quotient_symbolic_labels.push(projection.symbolic_label(*dense)?);
    }
    let quotient_graph =
        SourceDynamicGraph::new(contraction.components.len(), quotient_edges, bound)
            .map_err(|_| An19PetalError::InvalidContraction)?;
    let mut quotient_workspace = AugmentedAn19Graph::from_source_with_inherited_labels(
        &quotient_graph,
        An19LengthMode::ExactRational,
        &quotient_root_sources,
        &quotient_symbolic_labels,
    )?;
    let quotient_cluster = (0..contraction.components.len())
        .map(FlowNodeId)
        .collect::<BTreeSet<_>>();
    let quotient_center = contracted_vertex(contraction, projection.local_node(center)?)?;
    let quotient_target = contracted_vertex(contraction, projection.local_node(target)?)?;
    metrics.contraction_calls = metrics
        .contraction_calls
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    metrics.contracted_edges = metrics
        .contracted_edges
        .checked_add(
            u64::try_from(contraction.contracted_edges.len())
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .ok_or(An19PetalError::Overflow)?;
    metrics.quotient_edges = metrics
        .quotient_edges
        .checked_add(
            u64::try_from(contraction.retained_edges.len())
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .ok_or(An19PetalError::Overflow)?;
    let quotient_selected = hierarchical_petal_decomposition(
        &mut quotient_workspace,
        quotient_cluster,
        quotient_center,
        quotient_target,
        original_node_count,
        Some(recursion_parent),
        partition_depth,
        true,
        radius_certificates,
        metrics,
        projection_audit,
    )?;
    let quotient_tree = quotient_workspace.recover_original_tree(&quotient_selected)?;
    let dense_tree = quotient_tree
        .iter()
        .map(|edge| {
            quotient_to_dense
                .get(edge.0)
                .copied()
                .ok_or(An19PetalError::InvalidContraction)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    add_workspace_edge_scans(
        metrics,
        WorkspaceScanClass::ContractionRecovery,
        contraction.contracted_edges.len(),
        1,
    )?;
    add_workspace_edge_scans(
        metrics,
        WorkspaceScanClass::ContractionRecovery,
        dense_tree.len(),
        2,
    )?;
    contraction
        .expand_quotient_tree(projection.graph(), &dense_tree)?
        .iter()
        .map(|dense| {
            projection
                .dense_to_augmented()
                .get(dense.0)
                .copied()
                .ok_or(An19PetalError::InvalidContraction)
        })
        .collect()
}

fn contracted_vertex(
    contraction: &An19ShortEdgeContraction,
    vertex: FlowNodeId,
) -> Result<FlowNodeId, An19PetalError> {
    contraction
        .component_of
        .get(vertex.0)
        .copied()
        .flatten()
        .map(FlowNodeId)
        .ok_or(An19PetalError::InvalidContraction)
}

fn petal_decomposition(
    workspace: &mut AugmentedAn19Graph,
    mut cluster: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    delta: ExactRatio,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<(BTreeSet<FlowNodeId>, Vec<An19HierarchyPiece>, FlowNodeId), An19PetalError> {
    let half = ratio(1, 2)?;
    let r0 = delta
        .checked_mul(half)
        .map_err(|_| An19PetalError::Overflow)?;
    let mut remaining = cluster.clone();
    let projection = hierarchy_projection(workspace, &cluster, metrics, projection_audit)?;
    let paths = hierarchy_shortest_paths(&projection, &cluster, center, metrics)?;
    let target_distance = *paths
        .distances
        .get(&target)
        .ok_or(An19PetalError::Disconnected)?;
    let first_target = hierarchy_first_target(
        workspace,
        &mut cluster,
        &mut remaining,
        center,
        target,
        target_distance,
        r0,
        &projection,
        &paths,
        metrics,
        projection_audit,
    )?;
    drop(projection);
    let first_budget = delta
        .checked_mul(ratio(1, 4)?)
        .map_err(|_| An19PetalError::Overflow)?;
    let first = create_hierarchy_petal(
        workspace,
        &mut cluster,
        &mut remaining,
        center,
        first_target,
        first_budget,
        metrics,
        projection_audit,
    )?;
    let stigma_target = connection_predecessor(workspace, first.connection_edge, first.center)?;
    let mut pieces = vec![first];
    let later_budget = delta
        .checked_mul(ratio(1, 8)?)
        .map_err(|_| An19PetalError::Overflow)?;
    let projection = hierarchy_projection(workspace, &cluster, metrics, projection_audit)?;
    let fixed_paths = hierarchy_shortest_paths(&projection, &cluster, center, metrics)?;
    drop(projection);
    loop {
        let mut outside = None;
        for vertex in &remaining {
            let distance = *fixed_paths
                .distances
                .get(vertex)
                .ok_or(An19PetalError::Disconnected)?;
            if ratio_less(r0, distance)? {
                outside = Some(*vertex);
                break;
            }
        }
        let Some(outside) = outside else {
            break;
        };
        let next_target = ensure_vertex_at_distance(
            workspace,
            &mut cluster,
            &mut remaining,
            center,
            outside,
            r0,
            metrics,
            projection_audit,
        )?;
        let piece = create_hierarchy_petal(
            workspace,
            &mut cluster,
            &mut remaining,
            center,
            next_target,
            later_budget,
            metrics,
            projection_audit,
        )?;
        pieces.push(piece);
    }
    if remaining.is_empty() || !remaining.contains(&center) {
        return Err(An19PetalError::InvalidAugmentedGraph);
    }
    Ok((remaining, pieces, stigma_target))
}

#[allow(clippy::too_many_arguments)]
fn hierarchy_first_target(
    workspace: &mut AugmentedAn19Graph,
    cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    target_distance: ExactRatio,
    r0: ExactRatio,
    projection: &AugmentedProjection,
    paths: &HierarchyShortestPaths,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<FlowNodeId, An19PetalError> {
    if !ratio_less(target_distance, r0)? {
        metrics.fixed_path_reuses = checked_metric_sum(metrics.fixed_path_reuses, 1)?;
        return ensure_vertex_at_distance_from_paths(
            workspace,
            cluster,
            remaining,
            center,
            target,
            r0,
            projection,
            paths,
            metrics,
            projection_audit,
        );
    }
    let mut extension = r0
        .checked_sub(target_distance)
        .map_err(|_| An19PetalError::Overflow)?;
    let mut virtual_target = target;
    loop {
        let segment = if workspace.unit_input && ratio_less(ratio(1, 1)?, extension)? {
            ratio(1, 1)?
        } else {
            extension
        };
        let (next, _) = workspace.add_virtual_leaf(virtual_target, segment)?;
        cluster.insert(next);
        remaining.insert(next);
        virtual_target = next;
        metrics.virtual_leaves = metrics
            .virtual_leaves
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        extension = extension
            .checked_sub(segment)
            .map_err(|_| An19PetalError::Overflow)?;
        if !extension.is_positive() {
            return Ok(virtual_target);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn create_hierarchy_petal(
    workspace: &mut AugmentedAn19Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<An19HierarchyPiece, An19PetalError> {
    let projection = hierarchy_projection(workspace, fixed_cluster, metrics, projection_audit)?;
    let local_cluster = projection.local_nodes(fixed_cluster)?;
    let local_remaining = projection.local_nodes(remaining)?;
    let local_center = projection.local_node(center)?;
    let local_target = projection.local_node(target)?;
    let petal = An19WeightedPetal::construct_for_hierarchy(
        projection.graph(),
        &local_cluster,
        &local_remaining,
        local_center,
        local_target,
        budget,
        !workspace.unit_input,
        workspace.node_count,
    )?;
    add_petal_metrics(metrics, &petal.at_radius.metrics)?;
    let mut petal_vertices = petal
        .at_radius
        .vertices
        .iter()
        .map(|vertex| projection.augmented_node(*vertex))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let (petal_center, connection_edge) = match petal.at_radius.portal {
        An19PathPoint::Vertex(vertex) => {
            let position = petal
                .at_radius
                .path_from_center
                .iter()
                .position(|candidate| *candidate == vertex)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            let dense = position
                .checked_sub(1)
                .and_then(|index| petal.at_radius.path_edges.get(index))
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            let stable = *projection
                .dense_to_augmented()
                .get(dense.0)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            (projection.augmented_node(vertex)?, stable)
        }
        An19PathPoint::EdgeInterior {
            edge,
            from,
            offset_from,
            ..
        } => {
            let stable = *projection
                .dense_to_augmented()
                .get(edge.0)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            let root_source = workspace
                .edges
                .get(stable)
                .filter(|edge| edge.active)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?
                .root_source;
            let augmented_from = projection.augmented_node(from)?;
            let (portal, _, toward_center) =
                workspace.split_edge(stable, augmented_from, offset_from)?;
            fixed_cluster.insert(portal);
            petal_vertices.insert(portal);
            projection_audit.record_portal_split(root_source, metrics)?;
            (portal, toward_center)
        }
    };
    if !petal_vertices.contains(&target)
        || petal_vertices.contains(&center)
        || petal_vertices
            .iter()
            .any(|vertex| !fixed_cluster.contains(vertex))
    {
        return Err(An19PetalError::InvalidAugmentedGraph);
    }
    for vertex in &petal_vertices {
        remaining.remove(vertex);
    }
    let predecessor = connection_predecessor(workspace, connection_edge, petal_center)?;
    if !remaining.contains(&predecessor) {
        return Err(An19PetalError::InvalidAugmentedGraph);
    }
    metrics.petals = metrics
        .petals
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    Ok(An19HierarchyPiece {
        cluster: petal_vertices,
        center: petal_center,
        target,
        connection_edge,
    })
}

fn add_petal_metrics(
    hierarchy: &mut An19HierarchyMetrics,
    petal: &An19PetalMetrics,
) -> Result<(), An19PetalError> {
    hierarchy.shortest_path_runs = hierarchy
        .shortest_path_runs
        .checked_add(petal.shortest_path_runs)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.edge_relaxations = hierarchy
        .edge_relaxations
        .checked_add(petal.edge_relaxations)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.shortest_heap_pushes = hierarchy
        .shortest_heap_pushes
        .checked_add(petal.shortest_heap_pushes)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.shortest_heap_pops = hierarchy
        .shortest_heap_pops
        .checked_add(petal.shortest_heap_pops)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.shortest_edge_scans = hierarchy
        .shortest_edge_scans
        .checked_add(petal.shortest_edge_scans)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.directed_region_runs = hierarchy
        .directed_region_runs
        .checked_add(petal.directed_region_runs)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.directed_heap_pushes = hierarchy
        .directed_heap_pushes
        .checked_add(petal.directed_heap_pushes)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.directed_heap_pops = hierarchy
        .directed_heap_pops
        .checked_add(petal.directed_heap_pops)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.directed_edge_scans = hierarchy
        .directed_edge_scans
        .checked_add(petal.directed_edge_scans)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.membership_sources = hierarchy
        .membership_sources
        .checked_add(petal.membership_sources)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.event_heap_pushes = hierarchy
        .event_heap_pushes
        .checked_add(petal.event_heap_pushes)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.event_heap_pops = hierarchy
        .event_heap_pops
        .checked_add(petal.event_heap_pops)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.heap_comparisons = hierarchy
        .heap_comparisons
        .checked_add(petal.heap_comparisons)
        .ok_or(An19PetalError::Overflow)?;
    add_monotone_metrics(hierarchy, petal)?;
    hierarchy.event_vertex_activations = hierarchy
        .event_vertex_activations
        .checked_add(petal.event_vertex_activations)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.event_edge_touches = hierarchy
        .event_edge_touches
        .checked_add(petal.event_edge_touches)
        .ok_or(An19PetalError::Overflow)?;
    hierarchy.volume_queries = hierarchy
        .volume_queries
        .checked_add(petal.volume_queries)
        .ok_or(An19PetalError::Overflow)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ensure_vertex_at_distance(
    workspace: &mut AugmentedAn19Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    distance: ExactRatio,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<FlowNodeId, An19PetalError> {
    let projection = hierarchy_projection(workspace, fixed_cluster, metrics, projection_audit)?;
    let paths = hierarchy_shortest_paths(&projection, fixed_cluster, center, metrics)?;
    ensure_vertex_at_distance_from_paths(
        workspace,
        fixed_cluster,
        remaining,
        center,
        target,
        distance,
        &projection,
        &paths,
        metrics,
        projection_audit,
    )
}

#[allow(clippy::too_many_arguments)]
fn ensure_vertex_at_distance_from_paths(
    workspace: &mut AugmentedAn19Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    distance: ExactRatio,
    projection: &AugmentedProjection,
    paths: &HierarchyShortestPaths,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<FlowNodeId, An19PetalError> {
    let path = recover_hierarchy_path(center, target, paths)?;
    let mut traversed = ratio(0, 1)?;
    if distance == traversed {
        return Ok(center);
    }
    for (index, dense_edge) in path.edges.iter().enumerate() {
        let edge = projection
            .graph()
            .edge(*dense_edge)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        let next_distance = traversed
            .checked_add(edge.length)
            .map_err(|_| An19PetalError::Overflow)?;
        if distance == next_distance {
            return path
                .vertices
                .get(index + 1)
                .copied()
                .ok_or(An19PetalError::InvalidAugmentedGraph);
        }
        if ratio_less(traversed, distance)? && ratio_less(distance, next_distance)? {
            let from = path.vertices[index];
            let offset = distance
                .checked_sub(traversed)
                .map_err(|_| An19PetalError::Overflow)?;
            let stable = *projection
                .dense_to_augmented()
                .get(dense_edge.0)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            let root_source = workspace
                .edges
                .get(stable)
                .filter(|edge| edge.active)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?
                .root_source;
            let (vertex, _, _) = workspace.split_edge(stable, from, offset)?;
            fixed_cluster.insert(vertex);
            remaining.insert(vertex);
            projection_audit.record_portal_split(root_source, metrics)?;
            return Ok(vertex);
        }
        traversed = next_distance;
    }
    Err(An19PetalError::InvalidRadius)
}

fn connection_predecessor(
    workspace: &AugmentedAn19Graph,
    edge: usize,
    petal_center: FlowNodeId,
) -> Result<FlowNodeId, An19PetalError> {
    let edge = workspace
        .edges
        .get(edge)
        .filter(|candidate| candidate.active)
        .ok_or(An19PetalError::InvalidAugmentedGraph)?;
    if edge.first == petal_center {
        Ok(edge.second)
    } else if edge.second == petal_center {
        Ok(edge.first)
    } else {
        Err(An19PetalError::InvalidAugmentedGraph)
    }
}

fn halve_highway(
    workspace: &mut AugmentedAn19Graph,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<(), An19PetalError> {
    let projection = hierarchy_projection(workspace, cluster, metrics, projection_audit)?;
    let paths = hierarchy_shortest_paths(&projection, cluster, center, metrics)?;
    let path = recover_hierarchy_path(center, target, &paths)?;
    let stable_path = path
        .edges
        .iter()
        .map(|dense| {
            projection
                .dense_to_augmented()
                .get(dense.0)
                .copied()
                .ok_or(An19PetalError::InvalidAugmentedGraph)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let changes_length = stable_path.iter().try_fold(false, |changes, stable| {
        let edge = workspace
            .edges
            .get(*stable)
            .filter(|edge| edge.active)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        Ok::<_, An19PetalError>(changes || !edge.halved)
    })?;
    if changes_length {
        workspace.invalidate_projection_cache();
    }
    for stable in stable_path {
        let edge = workspace
            .edges
            .get_mut(stable)
            .filter(|edge| edge.active)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        if edge.halved {
            metrics.highway_edges_reused = metrics
                .highway_edges_reused
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            continue;
        }
        edge.length = edge
            .length
            .checked_mul(ratio(1, 2)?)
            .map_err(|_| An19PetalError::Overflow)?;
        edge.halved = true;
        metrics.highway_edges_halved = metrics
            .highway_edges_halved
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
    }
    Ok(())
}

fn hierarchy_shortest_paths(
    projection: &AugmentedProjection,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    metrics: &mut An19HierarchyMetrics,
) -> Result<HierarchyShortestPaths, An19PetalError> {
    let local_cluster = projection.local_nodes(cluster)?;
    let local_center = projection.local_node(center)?;
    let mut petal_metrics = An19PetalMetrics::default();
    let local_paths = fast_shortest_paths(
        projection.graph(),
        &local_cluster,
        local_center,
        &mut petal_metrics,
    )?;
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(petal_metrics.shortest_path_runs)
        .ok_or(An19PetalError::Overflow)?;
    metrics.edge_relaxations = metrics
        .edge_relaxations
        .checked_add(petal_metrics.edge_relaxations)
        .ok_or(An19PetalError::Overflow)?;
    metrics.shortest_heap_pushes = metrics
        .shortest_heap_pushes
        .checked_add(petal_metrics.shortest_heap_pushes)
        .ok_or(An19PetalError::Overflow)?;
    metrics.shortest_heap_pops = metrics
        .shortest_heap_pops
        .checked_add(petal_metrics.shortest_heap_pops)
        .ok_or(An19PetalError::Overflow)?;
    metrics.shortest_edge_scans = metrics
        .shortest_edge_scans
        .checked_add(petal_metrics.shortest_edge_scans)
        .ok_or(An19PetalError::Overflow)?;
    metrics.heap_comparisons = metrics
        .heap_comparisons
        .checked_add(petal_metrics.heap_comparisons)
        .ok_or(An19PetalError::Overflow)?;
    add_monotone_metrics(metrics, &petal_metrics)?;
    let mut distances = BTreeMap::new();
    let mut predecessors = BTreeMap::new();
    for augmented in cluster {
        let local = projection.local_node(*augmented)?;
        distances.insert(
            *augmented,
            local_paths.distances[local.0].ok_or(An19PetalError::Disconnected)?,
        );
        if let Some((parent, edge)) = local_paths.predecessors[local.0] {
            predecessors.insert(
                *augmented,
                (projection.augmented_node(FlowNodeId(parent))?, edge),
            );
        }
    }
    Ok(HierarchyShortestPaths {
        distances,
        predecessors,
    })
}

fn add_monotone_metrics(
    hierarchy: &mut An19HierarchyMetrics,
    petal: &An19PetalMetrics,
) -> Result<(), An19PetalError> {
    hierarchy.monotone_queue_pushes =
        checked_metric_sum(hierarchy.monotone_queue_pushes, petal.monotone_queue_pushes)?;
    hierarchy.monotone_queue_pops =
        checked_metric_sum(hierarchy.monotone_queue_pops, petal.monotone_queue_pops)?;
    hierarchy.monotone_front_comparisons = checked_metric_sum(
        hierarchy.monotone_front_comparisons,
        petal.monotone_front_comparisons,
    )?;
    hierarchy.maximum_length_classes = hierarchy
        .maximum_length_classes
        .max(petal.maximum_length_classes);
    Ok(())
}

fn hierarchy_projection(
    workspace: &AugmentedAn19Graph,
    cluster: &BTreeSet<FlowNodeId>,
    metrics: &mut An19HierarchyMetrics,
    projection_audit: &mut An19ProjectionAudit,
) -> Result<Rc<AugmentedProjection>, An19PetalError> {
    workspace.project_cluster(cluster, metrics, projection_audit)
}

#[derive(Clone, Copy)]
enum WorkspaceScanClass {
    Radius,
    ContractionInput,
    ContractionRetained,
    ContractionRecovery,
    FinalRecovery,
}

fn add_workspace_edge_scans(
    metrics: &mut An19HierarchyMetrics,
    class: WorkspaceScanClass,
    edge_count: usize,
    multiplier: u64,
) -> Result<(), An19PetalError> {
    let scans = u64::try_from(edge_count)
        .map_err(|_| An19PetalError::Overflow)?
        .checked_mul(multiplier)
        .ok_or(An19PetalError::Overflow)?;
    let classified = match class {
        WorkspaceScanClass::Radius => &mut metrics.radius_edge_scans,
        WorkspaceScanClass::ContractionInput => &mut metrics.contraction_input_edge_scans,
        WorkspaceScanClass::ContractionRetained => &mut metrics.contraction_retained_edge_scans,
        WorkspaceScanClass::ContractionRecovery => &mut metrics.contraction_recovery_edge_scans,
        WorkspaceScanClass::FinalRecovery => &mut metrics.final_recovery_edge_scans,
    };
    *classified = checked_metric_sum(*classified, scans)?;
    metrics.workspace_edge_scans = checked_metric_sum(metrics.workspace_edge_scans, scans)?;
    Ok(())
}

fn hierarchy_radius(
    cluster: &BTreeSet<FlowNodeId>,
    paths: &HierarchyShortestPaths,
) -> Result<ExactRatio, An19PetalError> {
    let mut radius = ratio(0, 1)?;
    for vertex in cluster {
        let distance = *paths
            .distances
            .get(vertex)
            .ok_or(An19PetalError::Disconnected)?;
        if ratio_less(radius, distance)? {
            radius = distance;
        }
    }
    Ok(radius)
}

#[allow(clippy::too_many_arguments)]
fn build_radius_certificate(
    projection: &AugmentedProjection,
    original_node_count: usize,
    recursion_parent: Option<usize>,
    partition_depth: u64,
    same_scale_contraction: bool,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    radius: ExactRatio,
    base_threshold: ExactRatio,
    base_vertex_limit: usize,
    base_case: bool,
    paths: &HierarchyShortestPaths,
) -> Result<An19RadiusCertificate, An19PetalError> {
    let distances = cluster
        .iter()
        .map(|vertex| {
            paths
                .distances
                .get(vertex)
                .copied()
                .map(|distance| (*vertex, distance))
                .ok_or(An19PetalError::Disconnected)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut edges = Vec::new();
    for index in 0..projection.graph().edge_count() {
        let edge = projection
            .graph()
            .edge(SourceEdgeId(index))
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        edges.push(An19RadiusEdge {
            first: projection.augmented_node(edge.first)?,
            second: projection.augmented_node(edge.second)?,
            length: edge.length,
            root_source: projection.root_source(SourceEdgeId(index))?,
        });
    }
    let certificate = An19RadiusCertificate {
        original_node_count,
        recursion_parent,
        partition_depth,
        same_scale_contraction,
        cluster_size: cluster.len(),
        base_vertex_limit,
        center,
        target,
        radius,
        base_threshold,
        base_case,
        contraction_threshold: None,
        contraction_component_of: Vec::new(),
        contracted_edge_count: 0,
        distances,
        edges,
    };
    certificate.verify()?;
    Ok(certificate)
}

fn hierarchy_shortest_path_tree(
    projection: &AugmentedProjection,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    paths: &HierarchyShortestPaths,
) -> Result<BTreeSet<usize>, An19PetalError> {
    let mut tree = BTreeSet::new();
    for vertex in cluster {
        if *vertex == center {
            continue;
        }
        let (_, dense) = paths
            .predecessors
            .get(vertex)
            .copied()
            .ok_or(An19PetalError::Disconnected)?;
        tree.insert(
            *projection
                .dense_to_augmented()
                .get(dense.0)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?,
        );
    }
    if tree.len() + 1 != cluster.len() {
        return Err(An19PetalError::InvalidAugmentedGraph);
    }
    Ok(tree)
}

fn hierarchy_base_threshold(node_count: usize) -> Result<ExactRatio, An19PetalError> {
    ratio(
        i128::try_from(hierarchy_base_vertex_limit(node_count)?)
            .map_err(|_| An19PetalError::Overflow)?,
        1,
    )
}

fn minimum_cluster_edge_length(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
) -> Result<ExactRatio, An19PetalError> {
    let mut minimum = None;
    for index in 0..graph.edge_count() {
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        if cluster.contains(&edge.first) && cluster.contains(&edge.second) {
            let replace = match minimum {
                Some(length) => ratio_less(edge.length, length)?,
                None => true,
            };
            if replace {
                minimum = Some(edge.length);
            }
        }
    }
    minimum.map_or_else(|| ratio(1, 1), Ok)
}

fn hierarchy_base_vertex_limit(node_count: usize) -> Result<usize, An19PetalError> {
    let log_n = usize::BITS - node_count.saturating_sub(1).leading_zeros();
    let log_log_n = ceil_log_log(node_count);
    usize::try_from(log_n)
        .ok()
        .and_then(|value| value.checked_mul(log_log_n))
        .and_then(|value| value.checked_mul(10))
        .ok_or(An19PetalError::Overflow)
}

fn audit_original_tree_stretch(
    graph: &SourceDynamicGraph,
    tree: &BTreeSet<SourceEdgeId>,
) -> Result<(ExactRatio, ExactRatio), An19PetalError> {
    if tree.len() + 1 != graph.node_count() {
        return Err(An19PetalError::InvalidAugmentedGraph);
    }
    let mut adjacency = vec![Vec::<(usize, SourceEdgeId)>::new(); graph.node_count()];
    let mut connectivity = DisjointSet::new(graph.node_count());
    for edge_id in tree {
        let edge = graph
            .edge(*edge_id)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        if !connectivity.union(edge.first.0, edge.second.0) {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        adjacency[edge.first.0].push((edge.second.0, *edge_id));
        adjacency[edge.second.0].push((edge.first.0, *edge_id));
    }
    if !all_connected(&mut connectivity, graph.node_count()) {
        return Err(An19PetalError::InvalidAugmentedGraph);
    }
    let distance_index = OriginalTreeDistanceIndex::build(graph, &adjacency)?;
    let mut weighted_stretch = ratio(0, 1)?;
    let mut total_weight = ratio(0, 1)?;
    for index in 0..graph.edge_count() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        let distance = distance_index.distance(edge.first, edge.second)?;
        let stretch = distance
            .checked_mul(
                edge.length
                    .reciprocal()
                    .map_err(|_| An19PetalError::Overflow)?,
            )
            .map_err(|_| An19PetalError::Overflow)?;
        let source_stretch = stretch
            .checked_add(ratio(1, 1)?)
            .map_err(|_| An19PetalError::Overflow)?;
        weighted_stretch = weighted_stretch
            .checked_add(
                edge.weight
                    .checked_mul(source_stretch)
                    .map_err(|_| An19PetalError::Overflow)?,
            )
            .map_err(|_| An19PetalError::Overflow)?;
        total_weight = total_weight
            .checked_add(edge.weight)
            .map_err(|_| An19PetalError::Overflow)?;
    }
    Ok((weighted_stretch, total_weight))
}

fn tree_audit_work_units(graph: &SourceDynamicGraph) -> Result<u64, An19PetalError> {
    let nodes = u64::try_from(graph.node_count()).map_err(|_| An19PetalError::Overflow)?;
    let edges = u64::try_from(graph.edge_count()).map_err(|_| An19PetalError::Overflow)?;
    let levels = u64::from(usize::BITS - graph.node_count().saturating_sub(1).leading_zeros())
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    nodes
        .checked_mul(levels)
        .and_then(|value| value.checked_add(nodes.saturating_sub(1).checked_mul(3)?))
        .and_then(|value| value.checked_add(edges.checked_mul(levels.checked_add(1)?)?))
        .ok_or(An19PetalError::Overflow)
}

struct OriginalTreeDistanceIndex {
    depth: Vec<usize>,
    distance_from_root: Vec<ExactRatio>,
    ancestors: Vec<Vec<usize>>,
}

impl OriginalTreeDistanceIndex {
    fn build(
        graph: &SourceDynamicGraph,
        adjacency: &[Vec<(usize, SourceEdgeId)>],
    ) -> Result<Self, An19PetalError> {
        let node_count = graph.node_count();
        let mut depth = vec![0_usize; node_count];
        let mut distance_from_root = vec![ratio(0, 1)?; node_count];
        let mut parent = vec![usize::MAX; node_count];
        let mut stack = vec![0];
        parent[0] = 0;
        while let Some(node) = stack.pop() {
            for (next, edge_id) in &adjacency[node] {
                if *next == parent[node] {
                    continue;
                }
                if parent[*next] != usize::MAX {
                    return Err(An19PetalError::InvalidAugmentedGraph);
                }
                let edge = graph
                    .edge(*edge_id)
                    .ok_or(An19PetalError::InvalidAugmentedGraph)?;
                parent[*next] = node;
                depth[*next] = depth[node].checked_add(1).ok_or(An19PetalError::Overflow)?;
                distance_from_root[*next] = distance_from_root[node]
                    .checked_add(edge.length)
                    .map_err(|_| An19PetalError::Overflow)?;
                stack.push(*next);
            }
        }
        if parent.contains(&usize::MAX) {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
        let levels: usize =
            usize::try_from(usize::BITS - node_count.saturating_sub(1).leading_zeros())
                .map_err(|_| An19PetalError::Overflow)?
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
        let mut ancestors = vec![parent];
        for level in 1..levels {
            let previous = &ancestors[level - 1];
            ancestors.push(
                previous
                    .iter()
                    .map(|ancestor| previous[*ancestor])
                    .collect(),
            );
        }
        Ok(Self {
            depth,
            distance_from_root,
            ancestors,
        })
    }

    fn distance(
        &self,
        first: FlowNodeId,
        second: FlowNodeId,
    ) -> Result<ExactRatio, An19PetalError> {
        let ancestor = self.lowest_common_ancestor(first.0, second.0)?;
        self.distance_from_root[first.0]
            .checked_add(self.distance_from_root[second.0])
            .and_then(|value| {
                self.distance_from_root[ancestor]
                    .checked_mul_integer(2)
                    .and_then(|shared| value.checked_sub(shared))
            })
            .map_err(|_| An19PetalError::Overflow)
    }

    fn lowest_common_ancestor(
        &self,
        mut first: usize,
        mut second: usize,
    ) -> Result<usize, An19PetalError> {
        if self.depth[first] < self.depth[second] {
            std::mem::swap(&mut first, &mut second);
        }
        let difference = self.depth[first] - self.depth[second];
        for level in 0..self.ancestors.len() {
            if difference & (1_usize << level) != 0 {
                first = self.ancestors[level][first];
            }
        }
        if first == second {
            return Ok(first);
        }
        for level in (0..self.ancestors.len()).rev() {
            if self.ancestors[level][first] != self.ancestors[level][second] {
                first = self.ancestors[level][first];
                second = self.ancestors[level][second];
            }
        }
        self.ancestors
            .first()
            .and_then(|parents| parents.get(first))
            .copied()
            .ok_or(An19PetalError::InvalidAugmentedGraph)
    }
}

fn split_provenance(
    edge: &AugmentedAn19Edge,
    from: FlowNodeId,
    offset: ExactRatio,
) -> Result<(Option<OriginalEdgeInterval>, Option<OriginalEdgeInterval>), An19PetalError> {
    let Some(provenance) = &edge.provenance else {
        return Ok((None, None));
    };
    let (from_position, toward_position) = if edge.first == from {
        (provenance.first_position, provenance.second_position)
    } else {
        (provenance.second_position, provenance.first_position)
    };
    let direction = toward_position
        .checked_sub(from_position)
        .map_err(|_| An19PetalError::Overflow)?;
    let fraction = offset
        .checked_mul(
            edge.length
                .reciprocal()
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::Overflow)?;
    let split_position = from_position
        .checked_add(
            direction
                .checked_mul(fraction)
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::Overflow)?;
    let from_interval = OriginalEdgeInterval {
        edge: provenance.edge,
        first_position: from_position,
        second_position: split_position,
    };
    let toward_interval = OriginalEdgeInterval {
        edge: provenance.edge,
        first_position: split_position,
        second_position: toward_position,
    };
    Ok((Some(from_interval), Some(toward_interval)))
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, vertex: usize) -> usize {
        if self.parent[vertex] != vertex {
            self.parent[vertex] = self.find(self.parent[vertex]);
        }
        self.parent[vertex]
    }

    fn union(&mut self, first: usize, second: usize) -> bool {
        let mut first_root = self.find(first);
        let mut second_root = self.find(second);
        if first_root == second_root {
            return false;
        }
        if self.rank[first_root] < self.rank[second_root] {
            std::mem::swap(&mut first_root, &mut second_root);
        }
        self.parent[second_root] = first_root;
        if self.rank[first_root] == self.rank[second_root] {
            self.rank[first_root] = self.rank[first_root].saturating_add(1);
        }
        true
    }
}

#[derive(Clone, Debug)]
struct ShortestPaths {
    distances: Vec<Option<ExactRatio>>,
    predecessors: Vec<Option<(usize, SourceEdgeId)>>,
}

struct HierarchyShortestPaths {
    distances: BTreeMap<FlowNodeId, ExactRatio>,
    predecessors: BTreeMap<FlowNodeId, (FlowNodeId, SourceEdgeId)>,
}

struct RecoveredPath {
    vertices: Vec<FlowNodeId>,
    edges: Vec<SourceEdgeId>,
}

#[derive(Clone, Debug)]
struct MembershipThresholds {
    by_vertex: Vec<Option<ExactRatio>>,
    path_distance_from_target: Vec<Option<ExactRatio>>,
    ordered_events: Option<Vec<ExactHeapEntry>>,
}

fn validate_domain(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
) -> Result<(), An19PetalError> {
    let one = ratio(1, 1)?;
    if cluster.is_empty()
        || !remaining.is_subset(cluster)
        || !remaining.contains(&center)
        || !remaining.contains(&target)
        || !budget.is_positive()
        || cluster.iter().any(|vertex| vertex.0 >= graph.node_count())
    {
        return Err(An19PetalError::InvalidDomain);
    }
    for index in 0..graph.edge_count() {
        if let Some(edge) = graph.edge(SourceEdgeId(index))
            && edge.length != one
        {
            return Err(An19PetalError::NonunitLength);
        }
    }
    Ok(())
}

fn validate_weighted_domain(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    radius: ExactRatio,
) -> Result<(), An19PetalError> {
    if cluster.is_empty()
        || !remaining.is_subset(cluster)
        || !remaining.contains(&center)
        || !remaining.contains(&target)
        || radius.is_negative()
        || cluster.iter().any(|vertex| vertex.0 >= graph.node_count())
    {
        return Err(An19PetalError::InvalidDomain);
    }
    Ok(())
}

fn shortest_paths(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    metrics: &mut An19PetalMetrics,
) -> Result<ShortestPaths, An19PetalError> {
    let n = graph.node_count();
    let mut distances = vec![None; n];
    let mut predecessors = vec![None; n];
    let mut path_keys = vec![None; n];
    let mut settled = vec![false; n];
    distances[source.0] = Some(ratio(0, 1)?);
    path_keys[source.0] = Some(Vec::new());
    loop {
        let mut next: Option<usize> = None;
        for vertex in allowed {
            if settled[vertex.0] || distances[vertex.0].is_none() {
                continue;
            }
            match next {
                None => next = Some(vertex.0),
                Some(old) if path_state_is_better(vertex.0, old, &distances, &path_keys)? => {
                    next = Some(vertex.0);
                }
                Some(_) => {}
            }
        }
        let Some(node) = next else {
            break;
        };
        settled[node] = true;
        for index in 0..graph.edge_count() {
            let Some(edge) = graph.edge(SourceEdgeId(index)) else {
                continue;
            };
            let other = if edge.first.0 == node {
                edge.second.0
            } else if edge.second.0 == node {
                edge.first.0
            } else {
                continue;
            };
            if !allowed.contains(&FlowNodeId(other)) || settled[other] {
                continue;
            }
            metrics.edge_relaxations = metrics
                .edge_relaxations
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            let candidate = distances[node]
                .ok_or(An19PetalError::Disconnected)?
                .checked_add(edge.length)
                .map_err(|_| An19PetalError::Overflow)?;
            let mut key = path_keys[node]
                .as_ref()
                .ok_or(An19PetalError::Disconnected)?
                .clone();
            key.push(SourceEdgeId(index));
            let improves = match distances[other] {
                None => true,
                Some(old) => {
                    ratio_less(candidate, old)?
                        || (candidate == old
                            && key
                                < *path_keys[other]
                                    .as_ref()
                                    .ok_or(An19PetalError::Disconnected)?)
                }
            };
            if improves {
                distances[other] = Some(candidate);
                predecessors[other] = Some((node, SourceEdgeId(index)));
                path_keys[other] = Some(key);
            }
        }
    }
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(An19PetalError::Disconnected);
    }
    Ok(ShortestPaths {
        distances,
        predecessors,
    })
}

fn fast_shortest_paths(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    metrics: &mut An19PetalMetrics,
) -> Result<ShortestPaths, An19PetalError> {
    let mut adjacency =
        vec![Vec::<(FlowNodeId, SourceEdgeId, ExactRatio, usize)>::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(i128, i128), usize>::new();
    for index in 0..graph.edge_count() {
        metrics.shortest_edge_scans = checked_metric_sum(metrics.shortest_edge_scans, 1)?;
        let edge_id = SourceEdgeId(index);
        let Some(edge) = graph.edge(edge_id) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let next_class = length_classes.len();
            let class = *length_classes
                .entry((edge.length.numerator(), edge.length.denominator()))
                .or_insert(next_class);
            adjacency[edge.first.0].push((edge.second, edge_id, edge.length, class));
            adjacency[edge.second.0].push((edge.first, edge_id, edge.length, class));
        }
    }
    let mut distances = vec![None; graph.node_count()];
    let mut predecessors = vec![None; graph.node_count()];
    let mut settled = vec![false; graph.node_count()];
    let zero = ratio(0, 1)?;
    distances[source.0] = Some(zero);
    let source_class = length_classes.len();
    let mut queue = DistinctLengthQueue::new(
        source_class
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?,
        metrics,
    )?;
    queue.push(
        source_class,
        ExactHeapEntry {
            distance: zero,
            vertex: source,
        },
        metrics,
    )?;
    while let Some(entry) = queue.pop(metrics)? {
        if settled[entry.vertex.0] || distances[entry.vertex.0] != Some(entry.distance) {
            continue;
        }
        settled[entry.vertex.0] = true;
        for (other, edge_id, length, class) in &adjacency[entry.vertex.0] {
            metrics.shortest_edge_scans = metrics
                .shortest_edge_scans
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            if settled[other.0] {
                continue;
            }
            metrics.edge_relaxations = metrics
                .edge_relaxations
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            let candidate = entry
                .distance
                .checked_add(*length)
                .map_err(|_| An19PetalError::Overflow)?;
            let old = distances[other.0];
            let shorter = match old {
                Some(distance) => ratio_less(candidate, distance)?,
                None => true,
            };
            let equal_better = old == Some(candidate)
                && predecessors[other.0].is_none_or(|(parent, old_edge)| {
                    (*edge_id, entry.vertex.0) < (old_edge, parent)
                });
            if shorter {
                distances[other.0] = Some(candidate);
                predecessors[other.0] = Some((entry.vertex.0, *edge_id));
                queue.push(
                    *class,
                    ExactHeapEntry {
                        distance: candidate,
                        vertex: *other,
                    },
                    metrics,
                )?;
            } else if equal_better {
                predecessors[other.0] = Some((entry.vertex.0, *edge_id));
            }
        }
    }
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(An19PetalError::Disconnected);
    }
    Ok(ShortestPaths {
        distances,
        predecessors,
    })
}

fn hierarchy_or_oracle_paths(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    fast: bool,
    metrics: &mut An19PetalMetrics,
) -> Result<ShortestPaths, An19PetalError> {
    if fast {
        fast_shortest_paths(graph, allowed, source, metrics)
    } else {
        shortest_paths(graph, allowed, source, metrics)
    }
}

fn path_state_is_better(
    candidate: usize,
    old: usize,
    distances: &[Option<ExactRatio>],
    path_keys: &[Option<Vec<SourceEdgeId>>],
) -> Result<bool, An19PetalError> {
    let candidate_distance = distances[candidate].ok_or(An19PetalError::Disconnected)?;
    let old_distance = distances[old].ok_or(An19PetalError::Disconnected)?;
    if ratio_less(candidate_distance, old_distance)? {
        return Ok(true);
    }
    if candidate_distance != old_distance {
        return Ok(false);
    }
    let candidate_key = path_keys[candidate]
        .as_ref()
        .ok_or(An19PetalError::Disconnected)?;
    let old_key = path_keys[old]
        .as_ref()
        .ok_or(An19PetalError::Disconnected)?;
    Ok((candidate_key, candidate) < (old_key, old))
}

fn recover_path(
    source: FlowNodeId,
    target: FlowNodeId,
    paths: &ShortestPaths,
) -> Result<RecoveredPath, An19PetalError> {
    let mut reversed = vec![target];
    let mut reversed_edges = Vec::new();
    let mut current = target.0;
    while current != source.0 {
        let (parent, edge) = paths.predecessors[current].ok_or(An19PetalError::Disconnected)?;
        reversed_edges.push(edge);
        current = parent;
        reversed.push(FlowNodeId(current));
    }
    reversed.reverse();
    reversed_edges.reverse();
    Ok(RecoveredPath {
        vertices: reversed,
        edges: reversed_edges,
    })
}

fn recover_hierarchy_path(
    source: FlowNodeId,
    target: FlowNodeId,
    paths: &HierarchyShortestPaths,
) -> Result<RecoveredPath, An19PetalError> {
    let mut reversed = vec![target];
    let mut reversed_edges = Vec::new();
    let mut current = target;
    while current != source {
        let (parent, edge) = paths
            .predecessors
            .get(&current)
            .copied()
            .ok_or(An19PetalError::Disconnected)?;
        reversed_edges.push(edge);
        current = parent;
        reversed.push(current);
    }
    reversed.reverse();
    reversed_edges.reverse();
    Ok(RecoveredPath {
        vertices: reversed,
        edges: reversed_edges,
    })
}

fn locate_portal_and_highway(
    graph: &SourceDynamicGraph,
    path: &RecoveredPath,
    target: FlowNodeId,
    radius: ExactRatio,
) -> Result<(An19PathPoint, Vec<An19HighwaySegment>), An19PetalError> {
    if path.vertices.last().copied() != Some(target) || path.vertices.len() != path.edges.len() + 1
    {
        return Err(An19PetalError::InvalidDomain);
    }
    if radius.is_zero() {
        return Ok((An19PathPoint::Vertex(target), Vec::new()));
    }
    let mut traversed = ratio(0, 1)?;
    let mut segments = Vec::new();
    for index in (0..path.edges.len()).rev() {
        let edge_id = path.edges[index];
        let edge = graph.edge(edge_id).ok_or(An19PetalError::InvalidDomain)?;
        let from = path.vertices[index + 1];
        let toward_center = path.vertices[index];
        let remaining = radius
            .checked_sub(traversed)
            .map_err(|_| An19PetalError::Overflow)?;
        if !remaining.is_positive() {
            break;
        }
        let halved_length = if edge
            .length
            .at_least(remaining)
            .map_err(|_| An19PetalError::Overflow)?
        {
            remaining
        } else {
            edge.length
        };
        segments.push(An19HighwaySegment {
            edge: edge_id,
            from,
            toward_center,
            halved_length,
            original_edge_length: edge.length,
        });
        traversed = traversed
            .checked_add(halved_length)
            .map_err(|_| An19PetalError::Overflow)?;
        if traversed == radius {
            let portal = if halved_length == edge.length {
                An19PathPoint::Vertex(toward_center)
            } else {
                An19PathPoint::EdgeInterior {
                    edge: edge_id,
                    from,
                    toward_center,
                    offset_from: halved_length,
                }
            };
            return Ok((portal, segments));
        }
    }
    Err(An19PetalError::InvalidRadius)
}

#[allow(clippy::too_many_lines)]
fn directed_petal_distances(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    center_distances: &[Option<ExactRatio>],
    highway: &[An19HighwaySegment],
    metrics: &mut An19PetalMetrics,
) -> Result<Vec<Option<ExactRatio>>, An19PetalError> {
    let mut distances = vec![None; graph.node_count()];
    let mut settled = vec![false; graph.node_count()];
    let mut adjacency = vec![Vec::<(FlowNodeId, ExactRatio, usize)>::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(i128, i128), usize>::new();
    for edge_index in 0..graph.edge_count() {
        metrics.directed_edge_scans = checked_metric_sum(metrics.directed_edge_scans, 1)?;
        let edge_id = SourceEdgeId(edge_index);
        let Some(edge) = graph.edge(edge_id) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let next_class = length_classes.len();
            let class = *length_classes
                .entry((edge.length.numerator(), edge.length.denominator()))
                .or_insert(next_class);
            adjacency[edge.first.0].push((edge.second, edge.length, class));
            adjacency[edge.second.0].push((edge.first, edge.length, class));
        }
    }

    // For every ordinary arc, Claim 15's reduced length is
    // l(u,v) + d(x,u) - d(x,v). Adding the fixed center potential to a
    // tentative label therefore leaves the original undirected edge length.
    // The halved highway is represented by source labels at its path points.
    let half = ratio(1, 2)?;
    let target_potential = center_distances[target.0].ok_or(An19PetalError::Disconnected)?;
    let mut descending_highway_sources = vec![ExactHeapEntry {
        distance: target_potential,
        vertex: target,
    }];
    let mut portal_sources = Vec::new();
    let mut traversed = ratio(0, 1)?;
    for segment in highway {
        let edge = graph
            .edge(segment.edge)
            .ok_or(An19PetalError::InvalidHighway)?;
        if edge.length != segment.original_edge_length
            || segment.halved_length.is_negative()
            || ratio_less(edge.length, segment.halved_length)?
        {
            return Err(An19PetalError::InvalidHighway);
        }
        traversed = traversed
            .checked_add(segment.halved_length)
            .map_err(|_| An19PetalError::Overflow)?;
        let highway_distance = traversed
            .checked_mul(half)
            .map_err(|_| An19PetalError::Overflow)?;
        if segment.halved_length == edge.length {
            let transformed = highway_distance
                .checked_add(
                    center_distances[segment.toward_center.0]
                        .ok_or(An19PetalError::Disconnected)?,
                )
                .map_err(|_| An19PetalError::Overflow)?;
            descending_highway_sources.push(ExactHeapEntry {
                distance: transformed,
                vertex: segment.toward_center,
            });
        } else {
            let portal_potential = center_distances[segment.from.0]
                .ok_or(An19PetalError::Disconnected)?
                .checked_sub(segment.halved_length)
                .map_err(|_| An19PetalError::Overflow)?;
            let portal_label = highway_distance
                .checked_add(portal_potential)
                .map_err(|_| An19PetalError::Overflow)?;
            portal_sources.push(ExactHeapEntry {
                distance: portal_label
                    .checked_add(segment.halved_length)
                    .map_err(|_| An19PetalError::Overflow)?,
                vertex: segment.from,
            });
            portal_sources.push(ExactHeapEntry {
                distance: portal_label
                    .checked_add(
                        edge.length
                            .checked_sub(segment.halved_length)
                            .map_err(|_| An19PetalError::Overflow)?,
                    )
                    .map_err(|_| An19PetalError::Overflow)?,
                vertex: segment.toward_center,
            });
        }
    }
    descending_highway_sources.reverse();
    for source in portal_sources {
        let mut position = descending_highway_sources.len();
        for (index, current) in descending_highway_sources.iter().enumerate() {
            if exact_heap_entry_less(&source, current)? {
                position = index;
                break;
            }
        }
        descending_highway_sources.insert(position, source);
    }
    for source in &descending_highway_sources {
        let improves = match distances[source.vertex.0] {
            Some(old) => ratio_less(source.distance, old)?,
            None => true,
        };
        if improves {
            distances[source.vertex.0] = Some(source.distance);
        }
    }
    let source_class = length_classes.len();
    let mut queue = DistinctLengthQueue::new(
        source_class
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?,
        metrics,
    )?;
    for source in descending_highway_sources {
        queue.push(source_class, source, metrics)?;
    }
    while let Some(entry) = queue.pop(metrics)? {
        if settled[entry.vertex.0] || distances[entry.vertex.0] != Some(entry.distance) {
            continue;
        }
        settled[entry.vertex.0] = true;
        for (other, directed_length, class) in &adjacency[entry.vertex.0] {
            metrics.directed_edge_scans = metrics
                .directed_edge_scans
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            if settled[other.0] {
                continue;
            }
            metrics.edge_relaxations = metrics
                .edge_relaxations
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            let candidate = entry
                .distance
                .checked_add(*directed_length)
                .map_err(|_| An19PetalError::Overflow)?;
            let improves = match distances[other.0] {
                None => true,
                Some(old) => ratio_less(candidate, old)?,
            };
            if improves {
                distances[other.0] = Some(candidate);
                queue.push(
                    *class,
                    ExactHeapEntry {
                        distance: candidate,
                        vertex: *other,
                    },
                    metrics,
                )?;
            }
        }
    }
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    metrics.directed_region_runs = metrics
        .directed_region_runs
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(An19PetalError::Disconnected);
    }
    for vertex in allowed {
        distances[vertex.0] = Some(
            distances[vertex.0]
                .ok_or(An19PetalError::Disconnected)?
                .checked_sub(center_distances[vertex.0].ok_or(An19PetalError::Disconnected)?)
                .map_err(|_| An19PetalError::Overflow)?,
        );
    }
    Ok(distances)
}

fn reduced_directed_length(
    edge_id: SourceEdgeId,
    from: FlowNodeId,
    to: FlowNodeId,
    edge_length: ExactRatio,
    center_distances: &[Option<ExactRatio>],
    highway: &[An19HighwaySegment],
) -> Result<ExactRatio, An19PetalError> {
    if let Some(segment) = highway
        .iter()
        .find(|segment| segment.edge == edge_id && segment.from == from)
    {
        if segment.toward_center != to {
            return Err(An19PetalError::InvalidHighway);
        }
        let unhalved = edge_length
            .checked_sub(segment.halved_length)
            .map_err(|_| An19PetalError::Overflow)?;
        return segment
            .halved_length
            .checked_mul(ratio(1, 2)?)
            .and_then(|value| {
                unhalved
                    .checked_mul_integer(2)
                    .and_then(|remainder| value.checked_add(remainder))
            })
            .map_err(|_| An19PetalError::Overflow);
    }
    let from_distance = center_distances[from.0].ok_or(An19PetalError::Disconnected)?;
    let to_distance = center_distances[to.0].ok_or(An19PetalError::Disconnected)?;
    let reduced = edge_length
        .checked_sub(
            to_distance
                .checked_sub(from_distance)
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::Overflow)?;
    if reduced.is_negative() {
        return Err(An19PetalError::InvalidHighway);
    }
    Ok(reduced)
}

#[cfg(test)]
fn directed_petal_distances_oracle(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    center_distances: &[Option<ExactRatio>],
    highway: &[An19HighwaySegment],
) -> Result<Vec<Option<ExactRatio>>, An19PetalError> {
    let mut distances = vec![None; graph.node_count()];
    let mut settled = vec![false; graph.node_count()];
    distances[target.0] = Some(ratio(0, 1)?);
    loop {
        let mut next: Option<usize> = None;
        for vertex in allowed {
            if settled[vertex.0] || distances[vertex.0].is_none() {
                continue;
            }
            let improves = match next {
                None => true,
                Some(old) => {
                    let candidate = distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
                    let old_distance = distances[old].ok_or(An19PetalError::Disconnected)?;
                    ratio_less(candidate, old_distance)?
                        || (candidate == old_distance && vertex.0 < old)
                }
            };
            if improves {
                next = Some(vertex.0);
            }
        }
        let Some(node) = next else {
            break;
        };
        settled[node] = true;
        for edge_index in 0..graph.edge_count() {
            let edge_id = SourceEdgeId(edge_index);
            let Some(edge) = graph.edge(edge_id) else {
                continue;
            };
            let other = if edge.first.0 == node {
                edge.second
            } else if edge.second.0 == node {
                edge.first
            } else {
                continue;
            };
            if !allowed.contains(&other) || settled[other.0] {
                continue;
            }
            let length = reduced_directed_length(
                edge_id,
                FlowNodeId(node),
                other,
                edge.length,
                center_distances,
                highway,
            )?;
            let candidate = distances[node]
                .ok_or(An19PetalError::Disconnected)?
                .checked_add(length)
                .map_err(|_| An19PetalError::Overflow)?;
            let improves = match distances[other.0] {
                Some(old) => ratio_less(candidate, old)?,
                None => true,
            };
            if improves {
                distances[other.0] = Some(candidate);
            }
        }
    }
    Ok(distances)
}

fn membership_thresholds(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    path: &[FlowNodeId],
    from_center: &ShortestPaths,
    target: FlowNodeId,
    budget: ExactRatio,
    metrics: &mut An19PetalMetrics,
) -> Result<MembershipThresholds, An19PetalError> {
    let mut by_vertex = vec![None; graph.node_count()];
    let mut path_distance_from_target = vec![None; graph.node_count()];
    let target_position = path
        .iter()
        .position(|vertex| *vertex == target)
        .ok_or(An19PetalError::InvalidDomain)?;
    for (position, point) in path.iter().enumerate().take(target_position + 1) {
        let point_paths = shortest_paths(graph, remaining, *point, metrics)?;
        let distance_from_target = ratio(
            i128::try_from(target_position - position).map_err(|_| An19PetalError::Overflow)?,
            1,
        )?;
        path_distance_from_target[point.0] = Some(distance_from_target);
        if ratio_less(budget, distance_from_target)? {
            continue;
        }
        let center_to_point = from_center.distances[point.0].ok_or(An19PetalError::Disconnected)?;
        for vertex in remaining {
            let point_to_vertex =
                point_paths.distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
            let center_to_vertex =
                from_center.distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
            let excess = center_to_point
                .checked_add(point_to_vertex)
                .and_then(|value| value.checked_sub(center_to_vertex))
                .map_err(|_| An19PetalError::Overflow)?;
            if excess.is_negative() {
                return Err(An19PetalError::InvalidDomain);
            }
            let threshold = distance_from_target
                .checked_add(
                    excess
                        .checked_mul_integer(2)
                        .map_err(|_| An19PetalError::Overflow)?,
                )
                .map_err(|_| An19PetalError::Overflow)?;
            match by_vertex[vertex.0] {
                None => by_vertex[vertex.0] = Some(threshold),
                Some(old) if ratio_less(threshold, old)? => {
                    by_vertex[vertex.0] = Some(threshold);
                }
                Some(_) => {}
            }
        }
    }
    Ok(MembershipThresholds {
        by_vertex,
        path_distance_from_target,
        ordered_events: None,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExactHeapEntry {
    distance: ExactRatio,
    vertex: FlowNodeId,
}

#[derive(Clone)]
struct MonotoneFront {
    entry: ExactHeapEntry,
    class: usize,
}

struct DistinctLengthQueue {
    queues: Vec<VecDeque<ExactHeapEntry>>,
    fronts: Vec<MonotoneFront>,
}

type ClassifiedAdjacency = Vec<Vec<(FlowNodeId, ExactRatio, usize)>>;

impl DistinctLengthQueue {
    fn new(class_count: usize, metrics: &mut An19PetalMetrics) -> Result<Self, An19PetalError> {
        metrics.maximum_length_classes = metrics
            .maximum_length_classes
            .max(u64::try_from(class_count).map_err(|_| An19PetalError::Overflow)?);
        Ok(Self {
            queues: vec![VecDeque::new(); class_count],
            fronts: Vec::new(),
        })
    }

    fn push(
        &mut self,
        class: usize,
        entry: ExactHeapEntry,
        metrics: &mut An19PetalMetrics,
    ) -> Result<(), An19PetalError> {
        let queue = self
            .queues
            .get_mut(class)
            .ok_or(An19PetalError::InvalidWorkCertificate)?;
        if let Some(back) = queue.back()
            && ratio_less(entry.distance, back.distance)?
        {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        let was_empty = queue.is_empty();
        queue.push_back(entry.clone());
        metrics.monotone_queue_pushes = checked_metric_sum(metrics.monotone_queue_pushes, 1)?;
        if was_empty {
            monotone_front_push(&mut self.fronts, MonotoneFront { entry, class }, metrics)?;
        }
        Ok(())
    }

    fn pop(
        &mut self,
        metrics: &mut An19PetalMetrics,
    ) -> Result<Option<ExactHeapEntry>, An19PetalError> {
        let Some(front) = monotone_front_pop(&mut self.fronts, metrics)? else {
            return Ok(None);
        };
        let queue = self
            .queues
            .get_mut(front.class)
            .ok_or(An19PetalError::InvalidWorkCertificate)?;
        if queue.front() != Some(&front.entry) {
            return Err(An19PetalError::InvalidWorkCertificate);
        }
        let result = queue
            .pop_front()
            .ok_or(An19PetalError::InvalidWorkCertificate)?;
        metrics.monotone_queue_pops = checked_metric_sum(metrics.monotone_queue_pops, 1)?;
        if let Some(next) = queue.front().cloned() {
            monotone_front_push(
                &mut self.fronts,
                MonotoneFront {
                    entry: next,
                    class: front.class,
                },
                metrics,
            )?;
        }
        Ok(Some(result))
    }
}

fn fast_weighted_membership_thresholds(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    path: &RecoveredPath,
    center_distances: &[Option<ExactRatio>],
    maximum_radius: ExactRatio,
    metrics: &mut An19PetalMetrics,
) -> Result<MembershipThresholds, An19PetalError> {
    let mut thresholds = MembershipThresholds {
        by_vertex: vec![None; graph.node_count()],
        path_distance_from_target: vec![None; graph.node_count()],
        ordered_events: None,
    };
    let target_distance = center_distances[target.0].ok_or(An19PetalError::Disconnected)?;
    let mut labels = vec![None; graph.node_count()];
    let mut sources = Vec::new();
    let mut distance_from_target = ratio(0, 1)?;
    add_membership_source(target, ratio(0, 1)?, &mut labels, &mut sources, metrics)?;
    thresholds.path_distance_from_target[target.0] = Some(distance_from_target);
    for path_index in (0..path.edges.len()).rev() {
        let edge = graph
            .edge(path.edges[path_index])
            .ok_or(An19PetalError::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let next_distance = distance_from_target
            .checked_add(edge.length)
            .map_err(|_| An19PetalError::Overflow)?;
        thresholds.path_distance_from_target[toward_center.0] = Some(next_distance);
        if ratio_less(maximum_radius, next_distance)? {
            if ratio_less(distance_from_target, maximum_radius)? {
                add_interior_membership_source(
                    from,
                    toward_center,
                    edge.length,
                    maximum_radius
                        .checked_sub(distance_from_target)
                        .map_err(|_| An19PetalError::Overflow)?,
                    target_distance,
                    maximum_radius,
                    center_distances,
                    &mut labels,
                    &mut sources,
                    metrics,
                )?;
            }
            break;
        }
        add_membership_source(
            toward_center,
            next_distance,
            &mut labels,
            &mut sources,
            metrics,
        )?;
        distance_from_target = next_distance;
        if distance_from_target == maximum_radius {
            break;
        }
    }
    let (adjacency, length_classes) =
        weighted_adjacency(graph, remaining, center_distances, metrics)?;
    let events =
        exact_multi_source_dijkstra(&adjacency, length_classes, &mut labels, &sources, metrics)?;
    for vertex in remaining {
        let threshold = labels[vertex.0].ok_or(An19PetalError::Disconnected)?;
        if threshold.is_negative() {
            return Err(An19PetalError::InvalidRadius);
        }
        if !ratio_less(maximum_radius, threshold)? {
            thresholds.by_vertex[vertex.0] = Some(threshold);
        }
    }
    thresholds.ordered_events = Some(
        events
            .into_iter()
            .filter(|event| thresholds.by_vertex[event.vertex.0] == Some(event.distance))
            .collect(),
    );
    metrics.directed_region_runs = metrics
        .directed_region_runs
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    Ok(thresholds)
}

#[allow(clippy::too_many_arguments)]
fn add_interior_membership_source(
    from: FlowNodeId,
    toward_center: FlowNodeId,
    edge_length: ExactRatio,
    offset_from: ExactRatio,
    target_distance: ExactRatio,
    radius: ExactRatio,
    center_distances: &[Option<ExactRatio>],
    labels: &mut [Option<ExactRatio>],
    sources: &mut Vec<ExactHeapEntry>,
    metrics: &mut An19PetalMetrics,
) -> Result<(), An19PetalError> {
    let two = ratio(2, 1)?;
    let potential = target_distance
        .checked_mul(two)
        .and_then(|value| value.checked_sub(radius))
        .map_err(|_| An19PetalError::Overflow)?;
    let from_label = potential
        .checked_add(
            offset_from
                .checked_mul(two)
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::Overflow)?;
    let toward_label = potential
        .checked_add(
            edge_length
                .checked_sub(offset_from)
                .and_then(|value| value.checked_mul(two))
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::Overflow)?;
    let from_threshold = from_label
        .checked_sub(
            center_distances[from.0]
                .ok_or(An19PetalError::Disconnected)?
                .checked_mul(two)
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::Overflow)?;
    let toward_threshold = toward_label
        .checked_sub(
            center_distances[toward_center.0]
                .ok_or(An19PetalError::Disconnected)?
                .checked_mul(two)
                .map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::Overflow)?;
    add_membership_source(from, from_threshold, labels, sources, metrics)?;
    add_membership_source(toward_center, toward_threshold, labels, sources, metrics)?;
    metrics.membership_sources = metrics
        .membership_sources
        .checked_sub(1)
        .ok_or(An19PetalError::Overflow)?;
    Ok(())
}

fn add_membership_source(
    vertex: FlowNodeId,
    distance: ExactRatio,
    labels: &mut [Option<ExactRatio>],
    sources: &mut Vec<ExactHeapEntry>,
    metrics: &mut An19PetalMetrics,
) -> Result<(), An19PetalError> {
    let improves = match labels[vertex.0] {
        Some(old) => ratio_less(distance, old)?,
        None => true,
    };
    if improves {
        labels[vertex.0] = Some(distance);
        sources.push(ExactHeapEntry { distance, vertex });
    }
    metrics.membership_sources = metrics
        .membership_sources
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    Ok(())
}

fn weighted_adjacency(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    center_distances: &[Option<ExactRatio>],
    metrics: &mut An19PetalMetrics,
) -> Result<(ClassifiedAdjacency, usize), An19PetalError> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(i128, i128), usize>::new();
    for index in 0..graph.edge_count() {
        metrics.directed_edge_scans = checked_metric_sum(metrics.directed_edge_scans, 1)?;
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let first_distance =
                center_distances[edge.first.0].ok_or(An19PetalError::Disconnected)?;
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
            let next_forward = length_classes.len();
            let forward_class = *length_classes
                .entry((forward.numerator(), forward.denominator()))
                .or_insert(next_forward);
            let next_reverse = length_classes.len();
            let reverse_class = *length_classes
                .entry((reverse.numerator(), reverse.denominator()))
                .or_insert(next_reverse);
            adjacency[edge.first.0].push((edge.second, forward, forward_class));
            adjacency[edge.second.0].push((edge.first, reverse, reverse_class));
        }
    }
    Ok((adjacency, length_classes.len()))
}

#[cfg(test)]
fn transformed_weighted_adjacency(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    metrics: &mut An19PetalMetrics,
) -> Result<(ClassifiedAdjacency, usize), An19PetalError> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(i128, i128), usize>::new();
    for index in 0..graph.edge_count() {
        metrics.directed_edge_scans = checked_metric_sum(metrics.directed_edge_scans, 1)?;
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let length = edge
                .length
                .checked_mul_integer(2)
                .map_err(|_| An19PetalError::Overflow)?;
            let next_class = length_classes.len();
            let class = *length_classes
                .entry((length.numerator(), length.denominator()))
                .or_insert(next_class);
            adjacency[edge.first.0].push((edge.second, length, class));
            adjacency[edge.second.0].push((edge.first, length, class));
        }
    }
    Ok((adjacency, length_classes.len()))
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
fn transformed_weighted_membership_thresholds_oracle(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    path: &RecoveredPath,
    center_distances: &[Option<ExactRatio>],
    maximum_radius: ExactRatio,
    metrics: &mut An19PetalMetrics,
) -> Result<MembershipThresholds, An19PetalError> {
    let mut reduced_labels = vec![None; graph.node_count()];
    let mut reduced_sources = Vec::new();
    let target_distance = center_distances[target.0].ok_or(An19PetalError::Disconnected)?;
    let mut path_distance_from_target = vec![None; graph.node_count()];
    let mut distance_from_target = ratio(0, 1)?;
    add_membership_source(
        target,
        distance_from_target,
        &mut reduced_labels,
        &mut reduced_sources,
        metrics,
    )?;
    path_distance_from_target[target.0] = Some(distance_from_target);
    for path_index in (0..path.edges.len()).rev() {
        let edge = graph
            .edge(path.edges[path_index])
            .ok_or(An19PetalError::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let next_distance = distance_from_target
            .checked_add(edge.length)
            .map_err(|_| An19PetalError::Overflow)?;
        path_distance_from_target[toward_center.0] = Some(next_distance);
        if ratio_less(maximum_radius, next_distance)? {
            if ratio_less(distance_from_target, maximum_radius)? {
                add_interior_membership_source(
                    from,
                    toward_center,
                    edge.length,
                    maximum_radius
                        .checked_sub(distance_from_target)
                        .map_err(|_| An19PetalError::Overflow)?,
                    target_distance,
                    maximum_radius,
                    center_distances,
                    &mut reduced_labels,
                    &mut reduced_sources,
                    metrics,
                )?;
            }
            break;
        }
        add_membership_source(
            toward_center,
            next_distance,
            &mut reduced_labels,
            &mut reduced_sources,
            metrics,
        )?;
        distance_from_target = next_distance;
        if distance_from_target == maximum_radius {
            break;
        }
    }

    let two = ratio(2, 1)?;
    let mut transformed_labels = vec![None; graph.node_count()];
    let mut transformed_sources = Vec::with_capacity(reduced_sources.len());
    for source in reduced_sources {
        let potential = center_distances[source.vertex.0]
            .ok_or(An19PetalError::Disconnected)?
            .checked_mul(two)
            .map_err(|_| An19PetalError::Overflow)?;
        let transformed = source
            .distance
            .checked_add(potential)
            .map_err(|_| An19PetalError::Overflow)?;
        let improves = match transformed_labels[source.vertex.0] {
            Some(old) => ratio_less(transformed, old)?,
            None => true,
        };
        if improves {
            transformed_labels[source.vertex.0] = Some(transformed);
            transformed_sources.push(ExactHeapEntry {
                distance: transformed,
                vertex: source.vertex,
            });
        }
    }
    let mut comparisons = 0;
    let mut source_heap = Vec::new();
    for source in transformed_sources {
        heap_push(&mut source_heap, source, &mut comparisons)?;
    }
    let mut sorted_sources = Vec::new();
    while let Some(source) = heap_pop(&mut source_heap, &mut comparisons)? {
        sorted_sources.push(source);
    }
    let (adjacency, length_classes) = transformed_weighted_adjacency(graph, remaining, metrics)?;
    exact_multi_source_dijkstra(
        &adjacency,
        length_classes,
        &mut transformed_labels,
        &sorted_sources,
        metrics,
    )?;

    let mut by_vertex = vec![None; graph.node_count()];
    for vertex in remaining {
        let potential = center_distances[vertex.0]
            .ok_or(An19PetalError::Disconnected)?
            .checked_mul(two)
            .map_err(|_| An19PetalError::Overflow)?;
        let threshold = transformed_labels[vertex.0]
            .ok_or(An19PetalError::Disconnected)?
            .checked_sub(potential)
            .map_err(|_| An19PetalError::Overflow)?;
        if !ratio_less(maximum_radius, threshold)? {
            by_vertex[vertex.0] = Some(threshold);
        }
    }
    Ok(MembershipThresholds {
        by_vertex,
        path_distance_from_target,
        ordered_events: None,
    })
}

fn exact_multi_source_dijkstra(
    adjacency: &[Vec<(FlowNodeId, ExactRatio, usize)>],
    length_classes: usize,
    distances: &mut [Option<ExactRatio>],
    sources: &[ExactHeapEntry],
    metrics: &mut An19PetalMetrics,
) -> Result<Vec<ExactHeapEntry>, An19PetalError> {
    let source_class = length_classes;
    let mut queue = DistinctLengthQueue::new(
        source_class
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?,
        metrics,
    )?;
    for source in sources {
        queue.push(source_class, source.clone(), metrics)?;
    }
    let mut settled = vec![false; distances.len()];
    let mut events = Vec::new();
    while let Some(entry) = queue.pop(metrics)? {
        if settled[entry.vertex.0] || distances[entry.vertex.0] != Some(entry.distance) {
            continue;
        }
        settled[entry.vertex.0] = true;
        events.push(entry.clone());
        for (other, length, class) in &adjacency[entry.vertex.0] {
            metrics.directed_edge_scans = metrics
                .directed_edge_scans
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            let candidate = entry
                .distance
                .checked_add(*length)
                .map_err(|_| An19PetalError::Overflow)?;
            if settled[other.0] {
                continue;
            }
            let improves = match distances[other.0] {
                Some(old) => ratio_less(candidate, old)?,
                None => true,
            };
            if improves {
                distances[other.0] = Some(candidate);
                queue.push(
                    *class,
                    ExactHeapEntry {
                        distance: candidate,
                        vertex: *other,
                    },
                    metrics,
                )?;
            }
        }
    }
    Ok(events)
}

fn event_heap_push(
    heap: &mut Vec<ExactHeapEntry>,
    entry: ExactHeapEntry,
    metrics: &mut An19PetalMetrics,
) -> Result<(), An19PetalError> {
    heap_push(heap, entry, &mut metrics.heap_comparisons)?;
    metrics.event_heap_pushes = checked_metric_sum(metrics.event_heap_pushes, 1)?;
    Ok(())
}

fn monotone_front_push(
    heap: &mut Vec<MonotoneFront>,
    entry: MonotoneFront,
    metrics: &mut An19PetalMetrics,
) -> Result<(), An19PetalError> {
    heap.push(entry);
    let mut index = heap.len() - 1;
    while index > 0 {
        let parent = (index - 1) / 2;
        metrics.monotone_front_comparisons =
            checked_metric_sum(metrics.monotone_front_comparisons, 1)?;
        if !monotone_front_less(&heap[index], &heap[parent])? {
            break;
        }
        heap.swap(index, parent);
        index = parent;
    }
    Ok(())
}

fn monotone_front_pop(
    heap: &mut Vec<MonotoneFront>,
    metrics: &mut An19PetalMetrics,
) -> Result<Option<MonotoneFront>, An19PetalError> {
    let Some(last) = heap.pop() else {
        return Ok(None);
    };
    if heap.is_empty() {
        return Ok(Some(last));
    }
    let result = std::mem::replace(&mut heap[0], last);
    let mut index = 0;
    loop {
        let left = index * 2 + 1;
        if left >= heap.len() {
            break;
        }
        let right = left + 1;
        let child = if right < heap.len() {
            metrics.monotone_front_comparisons =
                checked_metric_sum(metrics.monotone_front_comparisons, 1)?;
            if monotone_front_less(&heap[right], &heap[left])? {
                right
            } else {
                left
            }
        } else {
            left
        };
        metrics.monotone_front_comparisons =
            checked_metric_sum(metrics.monotone_front_comparisons, 1)?;
        if !monotone_front_less(&heap[child], &heap[index])? {
            break;
        }
        heap.swap(index, child);
        index = child;
    }
    Ok(Some(result))
}

fn monotone_front_less(
    first: &MonotoneFront,
    second: &MonotoneFront,
) -> Result<bool, An19PetalError> {
    Ok(exact_heap_entry_less(&first.entry, &second.entry)?
        || (first.entry == second.entry && first.class < second.class))
}

fn heap_push(
    heap: &mut Vec<ExactHeapEntry>,
    entry: ExactHeapEntry,
    comparisons: &mut u64,
) -> Result<(), An19PetalError> {
    heap.push(entry);
    let mut index = heap.len() - 1;
    while index > 0 {
        let parent = (index - 1) / 2;
        *comparisons = checked_metric_sum(*comparisons, 1)?;
        if !exact_heap_entry_less(&heap[index], &heap[parent])? {
            break;
        }
        heap.swap(index, parent);
        index = parent;
    }
    Ok(())
}

fn event_heap_pop(
    heap: &mut Vec<ExactHeapEntry>,
    metrics: &mut An19PetalMetrics,
) -> Result<Option<ExactHeapEntry>, An19PetalError> {
    let result = heap_pop(heap, &mut metrics.heap_comparisons)?;
    if result.is_some() {
        metrics.event_heap_pops = checked_metric_sum(metrics.event_heap_pops, 1)?;
    }
    Ok(result)
}

fn heap_pop(
    heap: &mut Vec<ExactHeapEntry>,
    comparisons: &mut u64,
) -> Result<Option<ExactHeapEntry>, An19PetalError> {
    let Some(last) = heap.pop() else {
        return Ok(None);
    };
    let result = if heap.is_empty() {
        last
    } else {
        let result = std::mem::replace(&mut heap[0], last);
        let mut index = 0;
        loop {
            let left = index * 2 + 1;
            if left >= heap.len() {
                break;
            }
            let right = left + 1;
            let child = if right < heap.len() {
                *comparisons = checked_metric_sum(*comparisons, 1)?;
                if exact_heap_entry_less(&heap[right], &heap[left])? {
                    right
                } else {
                    left
                }
            } else {
                left
            };
            *comparisons = checked_metric_sum(*comparisons, 1)?;
            if !exact_heap_entry_less(&heap[child], &heap[index])? {
                break;
            }
            heap.swap(index, child);
            index = child;
        }
        result
    };
    Ok(Some(result))
}

fn exact_heap_entry_less(
    first: &ExactHeapEntry,
    second: &ExactHeapEntry,
) -> Result<bool, An19PetalError> {
    Ok(ratio_less(first.distance, second.distance)?
        || (first.distance == second.distance && first.vertex < second.vertex))
}

fn weighted_membership_thresholds_oracle(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    path: &RecoveredPath,
    center_distances: &[Option<ExactRatio>],
    maximum_radius: ExactRatio,
    metrics: &mut An19PetalMetrics,
) -> Result<MembershipThresholds, An19PetalError> {
    let mut thresholds = MembershipThresholds {
        by_vertex: vec![None; graph.node_count()],
        path_distance_from_target: vec![None; graph.node_count()],
        ordered_events: None,
    };
    thresholds.by_vertex[target.0] = Some(ratio(0, 1)?);
    thresholds.path_distance_from_target[target.0] = Some(ratio(0, 1)?);
    let mut fully_halved = Vec::new();
    let mut interval_start = ratio(0, 1)?;
    for path_index in (0..path.edges.len()).rev() {
        if !ratio_less(interval_start, maximum_radius)? {
            break;
        }
        let edge_id = path.edges[path_index];
        let edge = graph.edge(edge_id).ok_or(An19PetalError::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let full_end = interval_start
            .checked_add(edge.length)
            .map_err(|_| An19PetalError::Overflow)?;
        let interval_end = if ratio_less(maximum_radius, full_end)? {
            maximum_radius
        } else {
            full_end
        };
        thresholds.path_distance_from_target[toward_center.0] = Some(full_end);
        let removed = Some((edge_id, from, toward_center));
        let without_current = constant_directed_distances(
            graph,
            remaining,
            target,
            center_distances,
            &fully_halved,
            removed,
            metrics,
        )?;
        let from_toward = constant_directed_distances(
            graph,
            remaining,
            toward_center,
            center_distances,
            &fully_halved,
            removed,
            metrics,
        )?;
        let source_to_from = without_current[from.0].ok_or(An19PetalError::Disconnected)?;
        let three_halves = ratio(3, 2)?;
        let one_half = ratio(1, 2)?;
        let current_constant = edge
            .length
            .checked_mul_integer(2)
            .and_then(|value| {
                interval_start
                    .checked_mul(three_halves)
                    .and_then(|offset| value.checked_add(offset))
            })
            .map_err(|_| An19PetalError::Overflow)?;
        for vertex in remaining {
            if let Some(distance) = without_current[vertex.0] {
                let entry = max_ratio(
                    interval_start,
                    distance
                        .checked_mul_integer(2)
                        .map_err(|_| An19PetalError::Overflow)?,
                )?;
                if !ratio_less(interval_end, entry)? {
                    record_threshold(&mut thresholds.by_vertex[vertex.0], entry)?;
                }
            }
            if let Some(suffix) = from_toward[vertex.0] {
                let entry = source_to_from
                    .checked_add(current_constant)
                    .and_then(|value| value.checked_add(suffix))
                    .and_then(|value| value.checked_mul(one_half))
                    .map_err(|_| An19PetalError::Overflow)?;
                let entry = max_ratio(interval_start, entry)?;
                if !ratio_less(interval_end, entry)? {
                    record_threshold(&mut thresholds.by_vertex[vertex.0], entry)?;
                }
            }
        }
        if interval_end != full_end {
            break;
        }
        fully_halved.push(An19HighwaySegment {
            edge: edge_id,
            from,
            toward_center,
            halved_length: edge.length,
            original_edge_length: edge.length,
        });
        interval_start = full_end;
    }
    Ok(thresholds)
}

fn constant_directed_distances(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    center_distances: &[Option<ExactRatio>],
    fully_halved: &[An19HighwaySegment],
    removed: Option<(SourceEdgeId, FlowNodeId, FlowNodeId)>,
    metrics: &mut An19PetalMetrics,
) -> Result<Vec<Option<ExactRatio>>, An19PetalError> {
    let mut distances = vec![None; graph.node_count()];
    let mut path_keys = vec![None; graph.node_count()];
    let mut settled = vec![false; graph.node_count()];
    distances[source.0] = Some(ratio(0, 1)?);
    path_keys[source.0] = Some(Vec::new());
    loop {
        let mut next = None;
        for vertex in allowed {
            if settled[vertex.0] || distances[vertex.0].is_none() {
                continue;
            }
            match next {
                None => next = Some(vertex.0),
                Some(old) if path_state_is_better(vertex.0, old, &distances, &path_keys)? => {
                    next = Some(vertex.0);
                }
                Some(_) => {}
            }
        }
        let Some(node) = next else {
            break;
        };
        settled[node] = true;
        for edge_index in 0..graph.edge_count() {
            let edge_id = SourceEdgeId(edge_index);
            let Some(edge) = graph.edge(edge_id) else {
                continue;
            };
            let other = if edge.first.0 == node {
                edge.second.0
            } else if edge.second.0 == node {
                edge.first.0
            } else {
                continue;
            };
            if !allowed.contains(&FlowNodeId(other)) || settled[other] {
                continue;
            }
            if removed == Some((edge_id, FlowNodeId(node), FlowNodeId(other))) {
                continue;
            }
            metrics.edge_relaxations = metrics
                .edge_relaxations
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            let directed_length = reduced_directed_length(
                edge_id,
                FlowNodeId(node),
                FlowNodeId(other),
                edge.length,
                center_distances,
                fully_halved,
            )?;
            let candidate = distances[node]
                .ok_or(An19PetalError::Disconnected)?
                .checked_add(directed_length)
                .map_err(|_| An19PetalError::Overflow)?;
            let mut key = path_keys[node]
                .as_ref()
                .ok_or(An19PetalError::Disconnected)?
                .clone();
            key.push(edge_id);
            let improves = match distances[other] {
                None => true,
                Some(old) => {
                    ratio_less(candidate, old)?
                        || (candidate == old
                            && key
                                < *path_keys[other]
                                    .as_ref()
                                    .ok_or(An19PetalError::Disconnected)?)
                }
            };
            if improves {
                distances[other] = Some(candidate);
                path_keys[other] = Some(key);
            }
        }
    }
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    Ok(distances)
}

fn record_threshold(
    current: &mut Option<ExactRatio>,
    candidate: ExactRatio,
) -> Result<(), An19PetalError> {
    match *current {
        None => *current = Some(candidate),
        Some(old) if ratio_less(candidate, old)? => *current = Some(candidate),
        Some(_) => {}
    }
    Ok(())
}

fn max_ratio(first: ExactRatio, second: ExactRatio) -> Result<ExactRatio, An19PetalError> {
    if ratio_less(first, second)? {
        Ok(second)
    } else {
        Ok(first)
    }
}

fn vertices_at_radius(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    radius: ExactRatio,
) -> Result<BTreeSet<FlowNodeId>, An19PetalError> {
    let mut result = BTreeSet::new();
    for vertex in remaining {
        if let Some(threshold) = thresholds.by_vertex[vertex.0]
            && radius
                .at_least(threshold)
                .map_err(|_| An19PetalError::Overflow)?
        {
            result.insert(*vertex);
        }
    }
    Ok(result)
}

fn window_radius(
    budget: ExactRatio,
    index: usize,
    levels: usize,
    upper: bool,
) -> Result<ExactRatio, An19PetalError> {
    let numerator = if upper {
        levels + index
    } else {
        levels + index - 1
    };
    let denominator = i128::try_from(levels)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .ok_or(An19PetalError::Overflow)?;
    budget
        .checked_mul(ratio(
            i128::try_from(numerator).map_err(|_| An19PetalError::Overflow)?,
            denominator,
        )?)
        .map_err(|_| An19PetalError::Overflow)
}

fn certify_window_condition(
    active_edges: usize,
    cluster_edges: usize,
    petal_edges: usize,
    index: usize,
    levels: usize,
) -> Result<bool, An19PetalError> {
    if petal_edges == 0 || index == levels {
        return Ok(true);
    }
    for precision in [48_u32, 96, 192, 384] {
        match try_window_comparison(
            active_edges,
            cluster_edges,
            petal_edges,
            index,
            levels,
            precision,
        ) {
            Ok(Some(result)) => return Ok(result),
            Ok(None) | Err(An19PetalError::InsufficientPrecision) => {}
            Err(error) => return Err(error),
        }
    }
    Err(An19PetalError::InsufficientPrecision)
}

fn try_window_comparison(
    active_edges: usize,
    cluster_edges: usize,
    petal_edges: usize,
    index: usize,
    levels: usize,
    precision: u32,
) -> Result<Option<bool>, An19PetalError> {
    let mut arithmetic = fixed_arithmetic(active_edges, cluster_edges, precision)?;
    let two = arithmetic
        .enclose_ratio(2, 1)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let log_two = arithmetic
        .logarithm(&two)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let m = arithmetic
        .enclose_ratio(
            i128::try_from(active_edges).map_err(|_| An19PetalError::Overflow)?,
            1,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let ln_edge_count = arithmetic
        .logarithm(&m)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let binary_edge_logarithm = arithmetic
        .divide_intervals(&ln_edge_count, &log_two)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let iterated_logarithm = arithmetic
        .logarithm(&binary_edge_logarithm)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let alpha = arithmetic
        .enclose_ratio(
            i128::try_from(levels - index).map_err(|_| An19PetalError::Overflow)?,
            i128::try_from(levels).map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let exponent_input = arithmetic
        .multiply_intervals(&alpha, &iterated_logarithm)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let exponent = arithmetic
        .exponential(&exponent_input)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let right = arithmetic
        .multiply_intervals(&exponent, &log_two)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let left_ratio_numerator = i128::try_from(cluster_edges)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .ok_or(An19PetalError::Overflow)?;
    let left_input = arithmetic
        .enclose_ratio(
            left_ratio_numerator,
            i128::try_from(petal_edges).map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let left = arithmetic
        .logarithm(&left_input)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    if left.lower_scaled() >= right.upper_scaled() {
        Ok(Some(true))
    } else if left.upper_scaled() < right.lower_scaled() {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

fn certify_stopping_condition(
    cluster_edges: usize,
    start_edges: usize,
    petal_edges: usize,
    boundary_cost: ExactRatio,
    levels: usize,
    budget: ExactRatio,
) -> Result<bool, An19PetalError> {
    if boundary_cost.is_zero() {
        return Ok(true);
    }
    if petal_edges == 0 || start_edges == 0 || start_edges >= cluster_edges {
        return Err(An19PetalError::InvalidRadius);
    }
    let denominator = i128::try_from(petal_edges)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| value.checked_mul(i128::try_from(levels).ok()?))
        .ok_or(An19PetalError::Overflow)?;
    let exact_left = budget
        .checked_mul(boundary_cost)
        .map_err(|_| An19PetalError::Overflow)?
        .checked_mul(ratio(1, denominator)?)
        .map_err(|_| An19PetalError::Overflow)?;
    for precision in [48_u32, 96, 192, 384] {
        match try_stopping_comparison(
            cluster_edges,
            start_edges,
            petal_edges,
            exact_left,
            precision,
        ) {
            Ok(Some(result)) => return Ok(result),
            Ok(None) | Err(An19PetalError::InsufficientPrecision) => {}
            Err(error) => return Err(error),
        }
    }
    Err(An19PetalError::InsufficientPrecision)
}

fn try_stopping_comparison(
    cluster_edges: usize,
    start_edges: usize,
    petal_edges: usize,
    exact_left: ExactRatio,
    precision: u32,
) -> Result<Option<bool>, An19PetalError> {
    let mut arithmetic = fixed_arithmetic(cluster_edges, petal_edges, precision)?;
    let chi = arithmetic
        .enclose_ratio(
            i128::try_from(cluster_edges).map_err(|_| An19PetalError::Overflow)?,
            i128::try_from(start_edges).map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let log_chi = arithmetic
        .logarithm(&chi)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let left = arithmetic
        .enclose_ratio(exact_left.numerator(), exact_left.denominator())
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    if left.upper_scaled() < log_chi.lower_scaled() {
        Ok(Some(true))
    } else if left.lower_scaled() >= log_chi.upper_scaled() {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

fn fixed_arithmetic(
    first_size: usize,
    second_size: usize,
    precision: u32,
) -> Result<CertifiedFixedPoint, An19PetalError> {
    let input_bits = u64::try_from(first_size)
        .ok()
        .and_then(|value| value.checked_add(u64::try_from(second_size).ok()?))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_mul(256))
        .ok_or(An19PetalError::Overflow)?;
    let terms = precision.checked_mul(2).ok_or(An19PetalError::Overflow)?;
    let config = FixedPointConfig::source_bounded(input_bits, precision, terms, 4)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    CertifiedFixedPoint::new(config).map_err(|_| An19PetalError::InsufficientPrecision)
}

fn next_radius_event(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    current: ExactRatio,
    limit: ExactRatio,
) -> Result<Option<ExactRatio>, An19PetalError> {
    let mut next = None;
    for vertex in remaining {
        let Some(candidate) = thresholds.by_vertex[vertex.0] else {
            continue;
        };
        if !ratio_less(current, candidate)? || ratio_less(limit, candidate)? {
            continue;
        }
        match next {
            None => next = Some(candidate),
            Some(old) if ratio_less(candidate, old)? => next = Some(candidate),
            Some(_) => {}
        }
    }
    Ok(next)
}

fn internal_edge_count(graph: &SourceDynamicGraph, vertices: &BTreeSet<FlowNodeId>) -> usize {
    (0..graph.edge_count())
        .filter_map(|index| graph.edge(SourceEdgeId(index)))
        .filter(|edge| vertices.contains(&edge.first) && vertices.contains(&edge.second))
        .count()
}

fn boundary_edge_count(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    petal: &BTreeSet<FlowNodeId>,
) -> usize {
    (0..graph.edge_count())
        .filter_map(|index| graph.edge(SourceEdgeId(index)))
        .filter(|edge| cluster.contains(&edge.first) && cluster.contains(&edge.second))
        .filter(|edge| petal.contains(&edge.first) != petal.contains(&edge.second))
        .count()
}

fn incident_edge_count(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    petal: &BTreeSet<FlowNodeId>,
) -> usize {
    (0..graph.edge_count())
        .filter_map(|index| graph.edge(SourceEdgeId(index)))
        .filter(|edge| cluster.contains(&edge.first) && cluster.contains(&edge.second))
        .filter(|edge| petal.contains(&edge.first) || petal.contains(&edge.second))
        .count()
}

fn boundary_edge_cost(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    petal: &BTreeSet<FlowNodeId>,
) -> Result<ExactRatio, An19PetalError> {
    let mut cost = ratio(0, 1)?;
    for index in 0..graph.edge_count() {
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        if cluster.contains(&edge.first)
            && cluster.contains(&edge.second)
            && petal.contains(&edge.first) != petal.contains(&edge.second)
        {
            cost = cost
                .checked_add(
                    edge.length
                        .reciprocal()
                        .map_err(|_| An19PetalError::Overflow)?,
                )
                .map_err(|_| An19PetalError::Overflow)?;
        }
    }
    Ok(cost)
}

fn all_connected(connectivity: &mut DisjointSet, count: usize) -> bool {
    if count == 0 {
        return false;
    }
    let root = connectivity.find(0);
    (1..count).all(|vertex| connectivity.find(vertex) == root)
}

fn all_cluster_connected(connectivity: &mut DisjointSet, cluster: &BTreeSet<FlowNodeId>) -> bool {
    let Some(first) = cluster.first() else {
        return false;
    };
    let root = connectivity.find(first.0);
    cluster
        .iter()
        .all(|vertex| connectivity.find(vertex.0) == root)
}

fn intervals_overlap(
    first_start: ExactRatio,
    first_end: ExactRatio,
    second_start: ExactRatio,
    second_end: ExactRatio,
) -> Result<bool, An19PetalError> {
    Ok(ratio_less(first_start, second_end)? && ratio_less(second_start, first_end)?)
}

fn sort_and_merge_touching(intervals: &mut Vec<An19HalvedInterval>) -> Result<(), An19PetalError> {
    for index in 1..intervals.len() {
        let mut cursor = index;
        while cursor > 0
            && ratio_less(
                intervals[cursor].start_from_first,
                intervals[cursor - 1].start_from_first,
            )?
        {
            intervals.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
    let mut merged = Vec::<An19HalvedInterval>::new();
    for interval in intervals.drain(..) {
        if let Some(previous) = merged.last_mut()
            && previous.end_from_first == interval.start_from_first
        {
            previous.end_from_first = interval.end_from_first;
            continue;
        }
        merged.push(interval);
    }
    *intervals = merged;
    Ok(())
}

fn ceil_log_log(value: usize) -> usize {
    let log = usize::try_from(usize::BITS - value.saturating_sub(1).leading_zeros()).unwrap_or(1);
    usize::try_from(usize::BITS - log.saturating_sub(1).leading_zeros())
        .unwrap_or(1)
        .max(1)
}

fn round_length_to_power_of_two(
    length: ExactRatio,
    base: ExactRatio,
) -> Result<ExactRatio, An19PetalError> {
    let scaled = length
        .checked_mul(base.reciprocal().map_err(|_| An19PetalError::Overflow)?)
        .map_err(|_| An19PetalError::Overflow)?;
    let mut power = ratio(1, 1)?;
    loop {
        let Ok(next) = power.checked_mul_integer(2) else {
            break;
        };
        if ratio_less(scaled, next)? {
            break;
        }
        power = next;
    }
    base.checked_mul(power)
        .map_err(|_| An19PetalError::Overflow)
}

fn ratio(numerator: i128, denominator: i128) -> Result<ExactRatio, An19PetalError> {
    ExactRatio::new(numerator, denominator).map_err(|_| An19PetalError::Overflow)
}

fn count_ratio(value: usize) -> Result<ExactRatio, An19PetalError> {
    ratio(
        i128::try_from(value).map_err(|_| An19PetalError::Overflow)?,
        1,
    )
}

fn checked_metric_sum(first: u64, second: u64) -> Result<u64, An19PetalError> {
    first.checked_add(second).ok_or(An19PetalError::Overflow)
}

fn ratio_less(left: ExactRatio, right: ExactRatio) -> Result<bool, An19PetalError> {
    Ok(!left.at_least(right).map_err(|_| An19PetalError::Overflow)?)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum An19PetalError {
    #[error("AN19 petal input violates the unit-length source domain")]
    InvalidDomain,
    #[error("AN19 unweighted petal construction received a nonunit edge")]
    NonunitLength,
    #[error("AN19 petal cluster is disconnected")]
    Disconnected,
    #[error("AN19 Figure 6 radius window or stopping event is invalid")]
    InvalidRadius,
    #[error("AN19 symbolic portal or highway certificate is invalid")]
    InvalidHighway,
    #[error("AN19 highway interval was already halved")]
    RepeatedHighway,
    #[error("AN19 short-edge contraction or tree expansion is invalid")]
    InvalidContraction,
    #[error("AN19 augmented hierarchy or original-tree recovery is invalid")]
    InvalidAugmentedGraph,
    #[error("AN19 recursive radius or tree-stretch certificate is invalid")]
    InvalidRadiusCertificate,
    #[error("AN19 source-shaped work certificate is invalid")]
    InvalidWorkCertificate,
    #[error(
        "AN19 proved event engine is unavailable until its ordering and work bounds are proved"
    )]
    UnprovedEventEngine,
    #[error("AN19 event trace or differential certificate is invalid")]
    InvalidEventTrace,
    #[error("certified AN19 logarithmic comparison needs more bounded precision")]
    InsufficientPrecision,
    #[error("checked AN19 petal arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::An19UnweightedPetal;
    use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge};
    use std::{collections::BTreeSet, rc::Rc};

    fn path_graph(nodes: usize) -> SourceDynamicGraph {
        let edges = (0..nodes - 1)
            .map(|node| SourceWeightedEdge {
                first: FlowNodeId(node),
                second: FlowNodeId(node + 1),
                length: ExactRatio::new(1, 1).unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            })
            .collect();
        SourceDynamicGraph::new(nodes, edges, 16).unwrap()
    }

    fn alternating_path_graph(scale: i128) -> SourceDynamicGraph {
        let edges = (0..499)
            .map(|index| SourceWeightedEdge {
                first: FlowNodeId(index),
                second: FlowNodeId(index + 1),
                length: ExactRatio::new(scale * i128::try_from(2 + index % 2).unwrap(), 1).unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            })
            .collect();
        SourceDynamicGraph::new(500, edges, 10_000).unwrap()
    }

    fn test_edge(first: usize, second: usize, length: i128) -> SourceWeightedEdge {
        SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(length, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        }
    }

    fn assert_contraction_recursion_mutations_rejected(
        hierarchy: &super::An19HierarchicalLsst,
        graph: &SourceDynamicGraph,
    ) {
        let mut invalid_parent = hierarchy.clone();
        invalid_parent
            .radius_certificates
            .iter_mut()
            .find(|certificate| certificate.same_scale_contraction)
            .unwrap()
            .recursion_parent = None;
        assert!(invalid_parent.verify(graph).is_err());

        let mut invalid_scale = hierarchy.clone();
        invalid_scale
            .radius_certificates
            .iter_mut()
            .find(|certificate| certificate.same_scale_contraction)
            .unwrap()
            .partition_depth += 1;
        assert!(invalid_scale.verify(graph).is_err());
    }

    fn assert_scale_audit_mutations_rejected(
        hierarchy: &super::An19HierarchicalLsst,
        graph: &SourceDynamicGraph,
    ) {
        let mut invalid_audit = hierarchy.clone();
        invalid_audit
            .projection_audit
            .original_edge_scale_occurrences[0] += 1;
        assert!(invalid_audit.verify(graph).is_err());

        let mut invalid_root = hierarchy.clone();
        invalid_root
            .radius_certificates
            .iter_mut()
            .filter(|certificate| !certificate.same_scale_contraction)
            .flat_map(|certificate| certificate.edges.iter_mut())
            .find(|edge| edge.root_source.is_some())
            .unwrap()
            .root_source = Some(SourceEdgeId(graph.edge_count()));
        assert!(invalid_root.verify(graph).is_err());

        let mut invalid_bound = hierarchy.clone();
        invalid_bound
            .work_certificate
            .source_scale_participation_bound += 1;
        assert!(invalid_bound.verify(graph).is_err());
    }

    fn assert_projection_charging_mutations_rejected(
        hierarchy: &super::An19HierarchicalLsst,
        graph: &SourceDynamicGraph,
    ) {
        let mut invalid_materialization = hierarchy.clone();
        invalid_materialization
            .projection_audit
            .original_edge_materialization_occurrences[0] += 1;
        assert!(invalid_materialization.verify(graph).is_err());

        let mut invalid_fragment = hierarchy.clone();
        invalid_fragment
            .projection_audit
            .original_edge_portal_fragment_occurrences[0] += 1;
        assert!(invalid_fragment.verify(graph).is_err());

        let split_source = hierarchy
            .projection_audit
            .original_edge_portal_splits
            .iter()
            .position(|splits| *splits > 0)
            .unwrap();
        let mut invalid_split = hierarchy.clone();
        invalid_split.projection_audit.original_edge_portal_splits[split_source] += 1;
        assert!(invalid_split.verify(graph).is_err());

        let mut invalid_metric = hierarchy.clone();
        invalid_metric.metrics.source_projection_materializations += 1;
        assert!(invalid_metric.verify(graph).is_err());

        let mut invalid_builds = hierarchy.clone();
        invalid_builds.metrics.projection_materializations += 1;
        assert!(invalid_builds.verify(graph).is_err());

        let mut invalid_internal = hierarchy.clone();
        invalid_internal
            .metrics
            .projection_active_internal_incident_scans += 1;
        assert!(invalid_internal.verify(graph).is_err());

        let mut invalid_boundary = hierarchy.clone();
        invalid_boundary
            .metrics
            .projection_active_boundary_incident_scans += 1;
        assert!(invalid_boundary.verify(graph).is_err());

        let mut invalid_inactive = hierarchy.clone();
        invalid_inactive.metrics.projection_inactive_incident_scans += 1;
        assert!(invalid_inactive.verify(graph).is_err());

        let mut invalid_incident_total = hierarchy.clone();
        invalid_incident_total.metrics.projection_incident_scans += 1;
        assert!(invalid_incident_total.verify(graph).is_err());
    }

    fn assert_workspace_scan_mutations_rejected(
        hierarchy: &super::An19HierarchicalLsst,
        graph: &SourceDynamicGraph,
    ) {
        let mutate = |field: fn(&mut super::An19HierarchyMetrics) -> &mut u64| {
            let mut invalid = hierarchy.clone();
            *field(&mut invalid.metrics) += 1;
            invalid.metrics.workspace_edge_scans += 1;
            assert!(invalid.verify(graph).is_err());
        };
        mutate(|metrics| &mut metrics.radius_edge_scans);
        mutate(|metrics| &mut metrics.contraction_input_edge_scans);
        mutate(|metrics| &mut metrics.contraction_retained_edge_scans);
        mutate(|metrics| &mut metrics.contraction_recovery_edge_scans);
        mutate(|metrics| &mut metrics.final_recovery_edge_scans);
    }

    fn assert_projection_charging_counts(audit: &super::An19ProjectionAudit, expected: [u64; 7]) {
        assert_eq!(audit.source_projection_materializations, expected[0]);
        assert_eq!(
            audit.maximum_original_edge_materialization_occurrences,
            expected[1]
        );
        assert_eq!(audit.portal_fragment_materializations, expected[2]);
        assert_eq!(
            audit.maximum_original_edge_portal_fragment_occurrences,
            expected[3]
        );
        assert_eq!(audit.source_portal_splits, expected[4]);
        assert_eq!(audit.maximum_original_edge_portal_splits, expected[5]);
        assert_eq!(audit.provenance_free_portal_splits, expected[6]);
    }

    fn assert_projection_scan_counts(metrics: &super::An19HierarchyMetrics, expected: [u64; 14]) {
        assert_eq!(metrics.projection_calls, expected[0]);
        assert_eq!(metrics.projection_cache_hits, expected[1]);
        assert_eq!(metrics.projection_materializations, expected[2]);
        assert_eq!(metrics.projected_node_slots, expected[3]);
        assert_eq!(metrics.projected_edge_slots, expected[4]);
        assert_eq!(
            metrics.projection_active_internal_incident_scans,
            expected[5]
        );
        assert_eq!(
            metrics.projection_active_boundary_incident_scans,
            expected[6]
        );
        assert_eq!(metrics.projection_inactive_incident_scans, expected[7]);
        assert_eq!(metrics.workspace_edge_scans, expected[8]);
        assert_eq!(
            metrics.projection_incident_scans,
            expected[5] + expected[6] + expected[7]
        );
        assert_eq!(metrics.radius_edge_scans, expected[9]);
        assert_eq!(metrics.contraction_input_edge_scans, expected[10]);
        assert_eq!(metrics.contraction_retained_edge_scans, expected[11]);
        assert_eq!(metrics.contraction_recovery_edge_scans, expected[12]);
        assert_eq!(metrics.final_recovery_edge_scans, expected[13]);
    }

    fn assert_workspace_scan_counts(metrics: &super::An19HierarchyMetrics, expected: [u64; 5]) {
        assert_eq!(metrics.radius_edge_scans, expected[0]);
        assert_eq!(metrics.contraction_input_edge_scans, expected[1]);
        assert_eq!(metrics.contraction_retained_edge_scans, expected[2]);
        assert_eq!(metrics.contraction_recovery_edge_scans, expected[3]);
        assert_eq!(metrics.final_recovery_edge_scans, expected[4]);
    }

    #[test]
    fn constructs_exact_path_petal_and_radius_window() {
        let graph = path_graph(10);
        let vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(4, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(petal.window_index, 1);
        assert_eq!(petal.window_start, ExactRatio::new(2, 1).unwrap());
        assert_eq!(petal.window_end, ExactRatio::new(3, 1).unwrap());
        assert_eq!(petal.radius, ExactRatio::new(2, 1).unwrap());
        assert_eq!(petal.center_vertex, Some(FlowNodeId(7)));
        assert_eq!(
            petal.vertices,
            BTreeSet::from([FlowNodeId(7), FlowNodeId(8), FlowNodeId(9)])
        );
        assert_eq!(petal.internal_edges, 2);
        assert_eq!(petal.boundary_edges, 1);
        assert_eq!(petal.metrics.radius_events, 0);
        assert!(petal.metrics.certified_comparisons >= 2);
    }

    #[test]
    fn rejects_nonunit_source_edges() {
        let graph = SourceDynamicGraph::new(
            2,
            vec![SourceWeightedEdge {
                first: FlowNodeId(0),
                second: FlowNodeId(1),
                length: ExactRatio::new(2, 1).unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            }],
            16,
        )
        .unwrap();
        let vertices = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        assert!(
            An19UnweightedPetal::construct(
                &graph,
                &vertices,
                &vertices,
                FlowNodeId(0),
                FlowNodeId(1),
                ExactRatio::new(1, 1).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn uses_lexicographic_edge_ids_for_equal_shortest_paths() {
        let graph = SourceDynamicGraph::new(
            4,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(2),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let vertices = (0..4).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(3),
            ExactRatio::new(2, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(
            petal.path_from_center,
            vec![FlowNodeId(0), FlowNodeId(1), FlowNodeId(3)]
        );
    }

    #[test]
    fn preserves_fractional_center_as_an_unresolved_portal() {
        let graph = path_graph(10);
        let vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(3, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(petal.radius, ExactRatio::new(3, 2).unwrap());
        assert_eq!(petal.center_vertex, None);
        assert_eq!(
            petal.vertices,
            BTreeSet::from([FlowNodeId(8), FlowNodeId(9)])
        );
    }

    #[test]
    fn represents_a_rational_portal_inside_an_original_edge() {
        use super::{An19HighwaySegment, An19PathPoint, An19WeightedPetalAtRadius};

        let graph = SourceDynamicGraph::new(
            3,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(3, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(5, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let vertices = BTreeSet::from([FlowNodeId(0), FlowNodeId(1), FlowNodeId(2)]);
        let petal = An19WeightedPetalAtRadius::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(2),
            ExactRatio::new(2, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(
            petal.portal,
            An19PathPoint::EdgeInterior {
                edge: crate::SourceEdgeId(1),
                from: FlowNodeId(2),
                toward_center: FlowNodeId(1),
                offset_from: ExactRatio::new(2, 1).unwrap(),
            }
        );
        assert_eq!(
            petal.highway_segments,
            vec![An19HighwaySegment {
                edge: crate::SourceEdgeId(1),
                from: FlowNodeId(2),
                toward_center: FlowNodeId(1),
                halved_length: ExactRatio::new(2, 1).unwrap(),
                original_edge_length: ExactRatio::new(5, 2).unwrap(),
            }]
        );
        assert_eq!(petal.vertices, BTreeSet::from([FlowNodeId(2)]));
        assert_eq!(
            petal.directed_distances[1],
            Some(ExactRatio::new(2, 1).unwrap())
        );
    }

    #[test]
    fn expands_a_short_edge_contraction_to_original_ids() {
        use super::An19ShortEdgeContraction;

        let graph = SourceDynamicGraph::new(
            4,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(2),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(100, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(200, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            256,
        )
        .unwrap();
        let vertices = (0..4).map(FlowNodeId).collect::<BTreeSet<_>>();
        let contraction =
            An19ShortEdgeContraction::build(&graph, &vertices, FlowNodeId(0)).unwrap();
        assert_eq!(contraction.radius, ExactRatio::new(102, 1).unwrap());
        assert_eq!(
            contraction.contraction_threshold,
            ExactRatio::new(51, 8).unwrap()
        );
        assert_eq!(contraction.components.len(), 2);
        assert_eq!(
            contraction.contracted_edges,
            BTreeSet::from([crate::SourceEdgeId(0), crate::SourceEdgeId(1)])
        );
        let expanded = contraction
            .expand_quotient_tree(&graph, &BTreeSet::from([crate::SourceEdgeId(2)]))
            .unwrap();
        assert_eq!(
            expanded,
            BTreeSet::from([
                crate::SourceEdgeId(0),
                crate::SourceEdgeId(1),
                crate::SourceEdgeId(2)
            ])
        );
    }

    #[test]
    fn highway_ledger_merges_touching_intervals_and_rejects_overlap() {
        use super::{An19HighwayLedger, An19HighwaySegment};

        let graph = SourceDynamicGraph::new(
            2,
            vec![SourceWeightedEdge {
                first: FlowNodeId(0),
                second: FlowNodeId(1),
                length: ExactRatio::new(5, 2).unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            }],
            16,
        )
        .unwrap();
        let mut ledger = An19HighwayLedger::new(&graph);
        ledger
            .apply(
                &graph,
                &[An19HighwaySegment {
                    edge: crate::SourceEdgeId(0),
                    from: FlowNodeId(1),
                    toward_center: FlowNodeId(0),
                    halved_length: ExactRatio::new(2, 1).unwrap(),
                    original_edge_length: ExactRatio::new(5, 2).unwrap(),
                }],
            )
            .unwrap();
        ledger
            .apply(
                &graph,
                &[An19HighwaySegment {
                    edge: crate::SourceEdgeId(0),
                    from: FlowNodeId(0),
                    toward_center: FlowNodeId(1),
                    halved_length: ExactRatio::new(1, 2).unwrap(),
                    original_edge_length: ExactRatio::new(5, 2).unwrap(),
                }],
            )
            .unwrap();
        assert_eq!(ledger.intervals(crate::SourceEdgeId(0)).unwrap().len(), 1);
        assert_eq!(
            ledger.effective_length(crate::SourceEdgeId(0)).unwrap(),
            ExactRatio::new(5, 4).unwrap()
        );
        let before = ledger.clone();
        assert!(
            ledger
                .apply(
                    &graph,
                    &[An19HighwaySegment {
                        edge: crate::SourceEdgeId(0),
                        from: FlowNodeId(0),
                        toward_center: FlowNodeId(1),
                        halved_length: ExactRatio::new(3, 4).unwrap(),
                        original_edge_length: ExactRatio::new(5, 2).unwrap(),
                    }],
                )
                .is_err()
        );
        assert_eq!(ledger, before);
    }

    #[test]
    fn weighted_boundary_volume_and_cost_are_scale_covariant() {
        use super::{boundary_edge_cost, incident_edge_count};

        let make_graph = |scale: i128| {
            SourceDynamicGraph::new(
                4,
                vec![
                    SourceWeightedEdge {
                        first: FlowNodeId(0),
                        second: FlowNodeId(1),
                        length: ExactRatio::new(2 * scale, 1).unwrap(),
                        weight: ExactRatio::new(1, 1).unwrap(),
                    },
                    SourceWeightedEdge {
                        first: FlowNodeId(1),
                        second: FlowNodeId(2),
                        length: ExactRatio::new(4 * scale, 1).unwrap(),
                        weight: ExactRatio::new(1, 1).unwrap(),
                    },
                    SourceWeightedEdge {
                        first: FlowNodeId(2),
                        second: FlowNodeId(3),
                        length: ExactRatio::new(8 * scale, 1).unwrap(),
                        weight: ExactRatio::new(1, 1).unwrap(),
                    },
                ],
                10_000,
            )
            .unwrap()
        };
        let cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1), FlowNodeId(2)]);
        let petal = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let small = make_graph(1);
        let large = make_graph(1_000);
        assert_eq!(incident_edge_count(&small, &cluster, &petal), 2);
        assert_eq!(incident_edge_count(&large, &cluster, &petal), 2);
        assert_eq!(
            boundary_edge_cost(&small, &cluster, &petal).unwrap(),
            ExactRatio::new(1, 4).unwrap()
        );
        assert_eq!(
            boundary_edge_cost(&large, &cluster, &petal).unwrap(),
            ExactRatio::new(1, 4_000).unwrap()
        );
    }

    #[test]
    fn hierarchy_highway_reuse_preserves_the_original_half_length() {
        use super::{An19HierarchyMetrics, An19ProjectionAudit, AugmentedAn19Graph, halve_highway};

        let graph = path_graph(2);
        let mut workspace = AugmentedAn19Graph::from_source(&graph).unwrap();
        let cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let mut metrics = An19HierarchyMetrics::default();
        let mut projection_audit = An19ProjectionAudit::new(graph.edge_count());
        halve_highway(
            &mut workspace,
            &cluster,
            FlowNodeId(0),
            FlowNodeId(1),
            &mut metrics,
            &mut projection_audit,
        )
        .unwrap();
        let half = ExactRatio::new(1, 2).unwrap();
        assert_eq!(workspace.edges[0].length, half);
        halve_highway(
            &mut workspace,
            &cluster,
            FlowNodeId(0),
            FlowNodeId(1),
            &mut metrics,
            &mut projection_audit,
        )
        .unwrap();
        let cached_projection = workspace
            .project_cluster(&cluster, &mut metrics, &mut projection_audit)
            .unwrap();
        assert_eq!(workspace.edges[0].length, half);
        assert_eq!(
            cached_projection
                .graph()
                .edge(SourceEdgeId(0))
                .unwrap()
                .length,
            half
        );
        assert_eq!(metrics.highway_edges_halved, 1);
        assert_eq!(metrics.highway_edges_reused, 1);
        assert_eq!(metrics.projection_cache_hits, 1);
    }

    #[test]
    fn claim_15_region_growing_matches_the_cone_union() {
        use super::{An19WeightedPetal, An19WeightedPetalAtRadius};

        let graph = path_graph(10);
        let vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let cone_union = An19UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(3, 1).unwrap(),
        )
        .unwrap();
        let region_growing = An19WeightedPetalAtRadius::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            cone_union.radius,
        )
        .unwrap();
        assert_eq!(region_growing.vertices, cone_union.vertices);
        assert!(matches!(
            region_growing.portal,
            super::An19PathPoint::EdgeInterior { .. }
        ));
        let weighted_selector = An19WeightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(3, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(weighted_selector.window_index, cone_union.window_index);
        assert_eq!(weighted_selector.at_radius.radius, cone_union.radius);
        assert_eq!(weighted_selector.at_radius.vertices, cone_union.vertices);
    }

    #[test]
    fn selects_a_weighted_figure_6_radius_from_parametric_events() {
        use super::{An19PathPoint, An19WeightedPetal};

        let graph = SourceDynamicGraph::new(
            3,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(7, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let vertices = (0..3).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19WeightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(2),
            ExactRatio::new(4, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(petal.window_index, 1);
        assert_eq!(petal.window_start, ExactRatio::new(2, 1).unwrap());
        assert_eq!(petal.at_radius.radius, ExactRatio::new(2, 1).unwrap());
        assert_eq!(
            petal.at_radius.vertices,
            BTreeSet::from([FlowNodeId(1), FlowNodeId(2)])
        );
        assert_eq!(
            petal.at_radius.portal,
            An19PathPoint::EdgeInterior {
                edge: crate::SourceEdgeId(0),
                from: FlowNodeId(1),
                toward_center: FlowNodeId(0),
                offset_from: ExactRatio::new(3, 2).unwrap(),
            }
        );
    }

    #[test]
    fn fast_weighted_events_match_the_parametric_oracle_at_an_interior_cut() {
        use super::{
            An19PetalMetrics, fast_weighted_membership_thresholds, recover_path, shortest_paths,
            transformed_weighted_membership_thresholds_oracle,
            weighted_membership_thresholds_oracle,
        };

        let graph = SourceDynamicGraph::new(
            4,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(7, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(3, 4).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let vertices = (0..4).map(FlowNodeId).collect::<BTreeSet<_>>();
        let mut setup_metrics = An19PetalMetrics::default();
        let center_paths =
            shortest_paths(&graph, &vertices, FlowNodeId(0), &mut setup_metrics).unwrap();
        let path = recover_path(FlowNodeId(0), FlowNodeId(2), &center_paths).unwrap();
        let maximum_radius = ExactRatio::new(2, 1).unwrap();
        let mut oracle_metrics = An19PetalMetrics::default();
        let oracle = weighted_membership_thresholds_oracle(
            &graph,
            &vertices,
            FlowNodeId(2),
            &path,
            &center_paths.distances,
            maximum_radius,
            &mut oracle_metrics,
        )
        .unwrap();
        let mut fast_metrics = An19PetalMetrics::default();
        let fast = fast_weighted_membership_thresholds(
            &graph,
            &vertices,
            FlowNodeId(2),
            &path,
            &center_paths.distances,
            maximum_radius,
            &mut fast_metrics,
        )
        .unwrap();
        let mut transformed_metrics = An19PetalMetrics::default();
        let transformed = transformed_weighted_membership_thresholds_oracle(
            &graph,
            &vertices,
            FlowNodeId(2),
            &path,
            &center_paths.distances,
            maximum_radius,
            &mut transformed_metrics,
        )
        .unwrap();
        assert_eq!(fast.by_vertex, oracle.by_vertex);
        assert_eq!(transformed.by_vertex, oracle.by_vertex);
        assert_eq!(
            transformed.path_distance_from_target,
            oracle.path_distance_from_target
        );
        assert_eq!(
            fast.path_distance_from_target,
            oracle.path_distance_from_target
        );
        assert_eq!(fast_metrics.directed_region_runs, 1);
        assert_eq!(fast_metrics.shortest_path_runs, 0);
        assert!(fast_metrics.directed_edge_scans <= 3 * 3);
        assert!(oracle_metrics.shortest_path_runs > 1);
        assert!(transformed_metrics.maximum_length_classes <= 4);
    }

    #[test]
    fn fast_weighted_events_match_oracle_on_all_connected_four_node_graphs() {
        use super::{
            An19PetalMetrics, fast_weighted_membership_thresholds, recover_path, shortest_paths,
            transformed_weighted_membership_thresholds_oracle,
            weighted_membership_thresholds_oracle,
        };

        let endpoints = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        let mut checked = 0;
        for mask in 0_u32..(1_u32 << endpoints.len()) {
            let edges = endpoints
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1_u32 << index) != 0)
                .map(|(index, (first, second))| SourceWeightedEdge {
                    first: FlowNodeId(*first),
                    second: FlowNodeId(*second),
                    length: ExactRatio::new(i128::try_from(index % 3 + 1).unwrap(), 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                })
                .collect::<Vec<_>>();
            let graph = SourceDynamicGraph::new(4, edges, 16).unwrap();
            let vertices = (0..4).map(FlowNodeId).collect::<BTreeSet<_>>();
            let mut setup_metrics = An19PetalMetrics::default();
            let Ok(center_paths) =
                shortest_paths(&graph, &vertices, FlowNodeId(0), &mut setup_metrics)
            else {
                continue;
            };
            for target in 1..4 {
                let target = FlowNodeId(target);
                let path = recover_path(FlowNodeId(0), target, &center_paths).unwrap();
                let target_distance = center_paths.distances[target.0].unwrap();
                for numerator in 1..=4 {
                    let maximum_radius = target_distance
                        .checked_mul(ExactRatio::new(numerator, 4).unwrap())
                        .unwrap();
                    let mut oracle_metrics = An19PetalMetrics::default();
                    let oracle = weighted_membership_thresholds_oracle(
                        &graph,
                        &vertices,
                        target,
                        &path,
                        &center_paths.distances,
                        maximum_radius,
                        &mut oracle_metrics,
                    )
                    .unwrap();
                    let mut fast_metrics = An19PetalMetrics::default();
                    let fast = fast_weighted_membership_thresholds(
                        &graph,
                        &vertices,
                        target,
                        &path,
                        &center_paths.distances,
                        maximum_radius,
                        &mut fast_metrics,
                    )
                    .unwrap();
                    let mut transformed_metrics = An19PetalMetrics::default();
                    let transformed = transformed_weighted_membership_thresholds_oracle(
                        &graph,
                        &vertices,
                        target,
                        &path,
                        &center_paths.distances,
                        maximum_radius,
                        &mut transformed_metrics,
                    )
                    .unwrap();
                    assert_eq!(fast.by_vertex, oracle.by_vertex);
                    assert_eq!(transformed.by_vertex, oracle.by_vertex);
                    assert_eq!(
                        fast.path_distance_from_target,
                        oracle.path_distance_from_target
                    );
                    assert_eq!(fast_metrics.directed_region_runs, 1);
                    assert!(
                        transformed_metrics.maximum_length_classes
                            <= u64::try_from(graph.edge_count() + 1).unwrap()
                    );
                    assert!(
                        fast_metrics.directed_edge_scans
                            <= u64::try_from(graph.edge_count() * 3).unwrap()
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 456);
    }

    #[test]
    fn fast_shortest_paths_match_oracle_distances_on_connected_four_node_graphs() {
        use super::{An19PetalMetrics, fast_shortest_paths, shortest_paths};

        let endpoints = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        let mut checked = 0;
        for mask in 0_u32..(1_u32 << endpoints.len()) {
            let edges = endpoints
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1_u32 << index) != 0)
                .map(|(index, (first, second))| SourceWeightedEdge {
                    first: FlowNodeId(*first),
                    second: FlowNodeId(*second),
                    length: ExactRatio::new(i128::try_from(index % 3 + 1).unwrap(), 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                })
                .collect::<Vec<_>>();
            let graph = SourceDynamicGraph::new(4, edges, 16).unwrap();
            let vertices = (0..4).map(FlowNodeId).collect::<BTreeSet<_>>();
            for source in 0..4 {
                let mut oracle_metrics = An19PetalMetrics::default();
                let Ok(oracle) =
                    shortest_paths(&graph, &vertices, FlowNodeId(source), &mut oracle_metrics)
                else {
                    break;
                };
                let mut fast_metrics = An19PetalMetrics::default();
                let fast =
                    fast_shortest_paths(&graph, &vertices, FlowNodeId(source), &mut fast_metrics)
                        .unwrap();
                assert_eq!(fast.distances, oracle.distances);
                for vertex in 0..4 {
                    if vertex == source {
                        assert!(fast.predecessors[vertex].is_none());
                        continue;
                    }
                    let (parent, edge_id) = fast.predecessors[vertex].unwrap();
                    let edge = graph.edge(edge_id).unwrap();
                    assert!(
                        (edge.first == FlowNodeId(parent) && edge.second == FlowNodeId(vertex))
                            || (edge.second == FlowNodeId(parent)
                                && edge.first == FlowNodeId(vertex))
                    );
                    assert_eq!(
                        fast.distances[parent]
                            .unwrap()
                            .checked_add(edge.length)
                            .unwrap(),
                        fast.distances[vertex].unwrap()
                    );
                }
                assert!(
                    fast_metrics.shortest_edge_scans
                        <= u64::try_from(graph.edge_count() * 3).unwrap()
                );
                assert_eq!(
                    fast_metrics.shortest_heap_pushes,
                    fast_metrics.shortest_heap_pops
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 152);
    }

    #[test]
    fn source_class_directed_distances_match_reduced_oracle_on_four_node_graphs() {
        use super::{
            An19PetalMetrics, directed_petal_distances, directed_petal_distances_oracle,
            locate_portal_and_highway, recover_path, shortest_paths,
        };

        let endpoints = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        let mut checked = 0;
        for mask in 0_u32..(1_u32 << endpoints.len()) {
            let edges = endpoints
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1_u32 << index) != 0)
                .map(|(index, (first, second))| SourceWeightedEdge {
                    first: FlowNodeId(*first),
                    second: FlowNodeId(*second),
                    length: ExactRatio::new(i128::try_from(index % 3 + 1).unwrap(), 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                })
                .collect::<Vec<_>>();
            let graph = SourceDynamicGraph::new(4, edges, 16).unwrap();
            let vertices = (0..4).map(FlowNodeId).collect::<BTreeSet<_>>();
            let mut setup_metrics = An19PetalMetrics::default();
            let Ok(center_paths) =
                shortest_paths(&graph, &vertices, FlowNodeId(0), &mut setup_metrics)
            else {
                continue;
            };
            for target in 1..4 {
                let target = FlowNodeId(target);
                let path = recover_path(FlowNodeId(0), target, &center_paths).unwrap();
                let target_distance = center_paths.distances[target.0].unwrap();
                for numerator in 1..=4 {
                    let radius = target_distance
                        .checked_mul(ExactRatio::new(numerator, 4).unwrap())
                        .unwrap();
                    let (_, highway) =
                        locate_portal_and_highway(&graph, &path, target, radius).unwrap();
                    let mut metrics = An19PetalMetrics::default();
                    let source_class = directed_petal_distances(
                        &graph,
                        &vertices,
                        target,
                        &center_paths.distances,
                        &highway,
                        &mut metrics,
                    )
                    .unwrap();
                    let reduced = directed_petal_distances_oracle(
                        &graph,
                        &vertices,
                        target,
                        &center_paths.distances,
                        &highway,
                    )
                    .unwrap();
                    assert_eq!(source_class, reduced);
                    assert!(
                        metrics.maximum_length_classes
                            <= u64::try_from(graph.edge_count() + 1).unwrap()
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 456);
    }

    #[test]
    fn fast_figure_six_selection_matches_unit_and_weighted_oracles() {
        use super::{An19UnweightedPetal, An19WeightedPetal};

        let unit = path_graph(10);
        let unit_vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let unit_oracle = An19UnweightedPetal::construct(
            &unit,
            &unit_vertices,
            &unit_vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(4, 1).unwrap(),
        )
        .unwrap();
        let unit_fast = An19WeightedPetal::construct_with_portal_volume(
            &unit,
            &unit_vertices,
            &unit_vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(4, 1).unwrap(),
            false,
            true,
            unit.node_count(),
        )
        .unwrap();
        assert_eq!(unit_fast.window_index, unit_oracle.window_index);
        assert_eq!(unit_fast.window_start, unit_oracle.window_start);
        assert_eq!(unit_fast.window_end, unit_oracle.window_end);
        assert_eq!(unit_fast.at_radius.radius, unit_oracle.radius);
        assert_eq!(unit_fast.at_radius.vertices, unit_oracle.vertices);
        assert_eq!(unit_fast.internal_edges, unit_oracle.internal_edges);
        assert_eq!(unit_fast.boundary_edges, unit_oracle.boundary_edges);
        assert_eq!(unit_fast.cluster_edges, unit_oracle.cluster_edges);

        let weighted = SourceDynamicGraph::new(
            10,
            (0..9)
                .map(|index| SourceWeightedEdge {
                    first: FlowNodeId(index),
                    second: FlowNodeId(index + 1),
                    length: ExactRatio::new(if index % 2 == 0 { 1 } else { 3 }, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                })
                .collect(),
            16,
        )
        .unwrap();
        let weighted_vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let weighted_oracle = An19WeightedPetal::construct_with_portal_volume(
            &weighted,
            &weighted_vertices,
            &weighted_vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(4, 1).unwrap(),
            true,
            false,
            weighted.node_count(),
        )
        .unwrap();
        let weighted_fast = An19WeightedPetal::construct_with_portal_volume(
            &weighted,
            &weighted_vertices,
            &weighted_vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(4, 1).unwrap(),
            true,
            true,
            weighted.node_count(),
        )
        .unwrap();
        assert_eq!(weighted_fast.window_index, weighted_oracle.window_index);
        assert_eq!(weighted_fast.window_start, weighted_oracle.window_start);
        assert_eq!(weighted_fast.window_end, weighted_oracle.window_end);
        assert_eq!(
            weighted_fast.at_radius.radius,
            weighted_oracle.at_radius.radius
        );
        assert_eq!(
            weighted_fast.at_radius.vertices,
            weighted_oracle.at_radius.vertices
        );
        assert_eq!(weighted_fast.internal_edges, weighted_oracle.internal_edges);
        assert_eq!(weighted_fast.boundary_edges, weighted_oracle.boundary_edges);
        assert_eq!(weighted_fast.cluster_edges, weighted_oracle.cluster_edges);
        assert_eq!(
            weighted_fast.at_radius.metrics.event_heap_pushes,
            weighted_fast.at_radius.metrics.event_heap_pops
        );
        assert!(
            weighted_fast.at_radius.metrics.event_vertex_activations
                <= u64::try_from(weighted.node_count() * 2).unwrap()
        );
        assert!(
            weighted_fast.at_radius.metrics.event_edge_touches
                <= u64::try_from(weighted.edge_count() * 5).unwrap()
        );
    }

    #[test]
    fn splits_augmented_provenance_in_both_orientations_and_projects_dense_ids() {
        use super::{An19SymbolicLengthLabel, AugmentedAn19Graph, OriginalEdgeInterval};

        let graph = SourceDynamicGraph::new(
            3,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(3, 2).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let mut augmented = AugmentedAn19Graph::from_source(&graph).unwrap();
        let (portal, from_edge, toward_edge) = augmented
            .split_edge(0, FlowNodeId(1), ExactRatio::new(1, 2).unwrap())
            .unwrap();

        assert_eq!(portal, FlowNodeId(3));
        assert_eq!(
            augmented.edges[from_edge].provenance,
            Some(OriginalEdgeInterval {
                edge: crate::SourceEdgeId(0),
                first_position: ExactRatio::new(3, 2).unwrap(),
                second_position: ExactRatio::new(1, 1).unwrap(),
            })
        );
        assert_eq!(
            augmented.edges[toward_edge].provenance,
            Some(OriginalEdgeInterval {
                edge: crate::SourceEdgeId(0),
                first_position: ExactRatio::new(1, 1).unwrap(),
                second_position: ExactRatio::new(0, 1).unwrap(),
            })
        );
        assert_eq!(
            augmented.edges[from_edge].root_source,
            Some(SourceEdgeId(0))
        );
        assert_eq!(
            augmented.edges[toward_edge].root_source,
            Some(SourceEdgeId(0))
        );
        let expected_label = An19SymbolicLengthLabel {
            root_source: Some(SourceEdgeId(0)),
            unsplit_length: ExactRatio::new(3, 2).unwrap(),
            halved: false,
        };
        assert_eq!(
            augmented.edges[from_edge].symbolic_length_label(),
            expected_label
        );
        assert_eq!(
            augmented.edges[toward_edge].symbolic_length_label(),
            expected_label
        );
        let projection = augmented.project().unwrap();
        assert_eq!(projection.graph.node_count(), 4);
        assert_eq!(
            projection.dense_to_augmented,
            vec![1, from_edge, toward_edge]
        );
        assert_eq!(projection.dense_symbolic_labels[1], expected_label);
        assert_eq!(projection.dense_symbolic_labels[2], expected_label);

        let selected = BTreeSet::from([1, from_edge, toward_edge]);
        assert_eq!(
            augmented.recover_original_tree(&selected).unwrap(),
            BTreeSet::from([crate::SourceEdgeId(0), crate::SourceEdgeId(1)])
        );
        assert!(
            augmented
                .recover_original_tree(&BTreeSet::from([1, from_edge]))
                .is_err()
        );
    }

    #[test]
    fn cluster_projection_uses_dense_local_node_slots() {
        use super::{An19HierarchyMetrics, An19ProjectionAudit, AugmentedAn19Graph};

        let graph = SourceDynamicGraph::new(
            8,
            vec![test_edge(0, 1, 1), test_edge(5, 6, 1), test_edge(6, 7, 1)],
            8,
        )
        .unwrap();
        let augmented = AugmentedAn19Graph::from_source(&graph).unwrap();
        let cluster = BTreeSet::from([FlowNodeId(5), FlowNodeId(6), FlowNodeId(7)]);
        let mut metrics = An19HierarchyMetrics::default();
        let mut audit = An19ProjectionAudit::new(graph.edge_count());
        let projection = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();
        let cached_projection = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();

        assert!(Rc::ptr_eq(&projection, &cached_projection));
        assert_eq!(projection.graph().node_count(), cluster.len());
        assert_eq!(
            projection.local_to_augmented_node,
            cluster.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(projection.local_node(FlowNodeId(5)).unwrap(), FlowNodeId(0));
        assert_eq!(projection.local_node(FlowNodeId(7)).unwrap(), FlowNodeId(2));
        assert_eq!(
            projection.augmented_node(FlowNodeId(1)).unwrap(),
            FlowNodeId(6)
        );
        assert_eq!(
            (0..projection.graph().edge_count())
                .map(|index| {
                    let edge = projection.graph().edge(SourceEdgeId(index)).unwrap();
                    (edge.first, edge.second)
                })
                .collect::<Vec<_>>(),
            vec![
                (FlowNodeId(0), FlowNodeId(1)),
                (FlowNodeId(1), FlowNodeId(2)),
            ]
        );
        assert_eq!(metrics.projection_calls, 2);
        assert_eq!(metrics.projection_cache_hits, 1);
        assert_eq!(metrics.projected_node_slots, 3);
        assert_eq!(metrics.maximum_projection_nodes, 3);
        assert_eq!(metrics.projected_edge_slots, 2);
        assert_eq!(metrics.maximum_projection_edges, 2);
        assert_eq!(metrics.projection_length_class_sum, 1);
        assert_eq!(metrics.maximum_projection_length_classes, 1);
        assert_eq!(metrics.maximum_symbolic_source_label_classes, 1);
        assert_eq!(metrics.maximum_symbolic_virtual_label_classes, 0);
        assert_eq!(audit.original_edge_segment_occurrences, vec![0, 1, 1]);
        assert_eq!(audit.provenance_free_segment_occurrences, 0);
        assert_eq!(audit.projected_edge_occurrences, 2);
        assert_eq!(audit.maximum_projection_edges, 2);
        assert_eq!(audit.total_projection_length_classes, 1);
        assert_eq!(audit.maximum_projection_length_classes, 1);
        assert_eq!(audit.maximum_symbolic_source_label_classes, 1);
        assert_eq!(audit.maximum_symbolic_virtual_label_classes, 0);
        assert_eq!(audit.maximum_original_edge_segment_occurrences, 1);
        audit.verify(graph.edge_count(), &metrics).unwrap();
    }

    #[test]
    fn cluster_projection_cache_applies_portal_split_incrementally() {
        use super::{An19HierarchyMetrics, An19ProjectionAudit, AugmentedAn19Graph};

        let graph = SourceDynamicGraph::new(2, vec![test_edge(0, 1, 1)], 8).unwrap();
        let mut augmented = AugmentedAn19Graph::from_source(&graph).unwrap();
        let mut cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let mut metrics = An19HierarchyMetrics::default();
        let mut audit = An19ProjectionAudit::new(graph.edge_count());
        let original = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();
        let original_pointer = Rc::as_ptr(&original);
        drop(original);
        let (portal, _, _) = augmented
            .split_edge(0, FlowNodeId(0), ExactRatio::new(1, 2).unwrap())
            .unwrap();
        cluster.insert(portal);
        let split = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();
        let cached_split = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();

        assert_eq!(original_pointer, Rc::as_ptr(&split));
        assert!(Rc::ptr_eq(&split, &cached_split));
        assert_eq!(split.graph().node_count(), 3);
        assert_eq!(split.graph().edge_count(), 2);
        assert_eq!(split.dense_symbolic_labels.len(), 2);
        assert_eq!(
            split.dense_symbolic_labels[0],
            split.dense_symbolic_labels[1]
        );
        assert_eq!(
            split.dense_symbolic_labels[0].unsplit_length,
            ExactRatio::new(1, 1).unwrap()
        );
        assert_eq!(metrics.projection_calls, 3);
        assert_eq!(metrics.projection_cache_hits, 2);
        assert_eq!(metrics.projection_incremental_splits, 1);
        assert_eq!(metrics.projected_node_slots, 2);
        assert_eq!(metrics.projected_edge_slots, 1);
        assert_eq!(audit.original_edge_segment_occurrences, vec![1]);
        audit.verify(graph.edge_count(), &metrics).unwrap();
    }

    #[test]
    fn cluster_projection_rebuilds_when_a_split_snapshot_is_still_borrowed() {
        use super::{An19HierarchyMetrics, An19ProjectionAudit, AugmentedAn19Graph};

        let graph = SourceDynamicGraph::new(2, vec![test_edge(0, 1, 1)], 8).unwrap();
        let mut augmented = AugmentedAn19Graph::from_source(&graph).unwrap();
        let mut cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let mut metrics = An19HierarchyMetrics::default();
        let mut audit = An19ProjectionAudit::new(graph.edge_count());
        let borrowed = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();
        let (portal, _, _) = augmented
            .split_edge(0, FlowNodeId(0), ExactRatio::new(1, 2).unwrap())
            .unwrap();
        cluster.insert(portal);
        let rebuilt = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();

        assert!(!Rc::ptr_eq(&borrowed, &rebuilt));
        assert_eq!(metrics.projection_calls, 2);
        assert_eq!(metrics.projection_cache_hits, 0);
        assert_eq!(metrics.projection_incremental_splits, 0);
        assert_eq!(metrics.projected_node_slots, 5);
        assert_eq!(metrics.projected_edge_slots, 3);
        assert_eq!(audit.original_edge_segment_occurrences, vec![3]);
        audit.verify(graph.edge_count(), &metrics).unwrap();
    }

    #[test]
    fn quotient_projection_retains_symbolic_labels_through_splits() {
        use super::{
            An19HierarchyMetrics, An19LengthMode, An19ProjectionAudit, An19SymbolicLengthLabel,
            AugmentedAn19Graph,
        };

        let graph =
            SourceDynamicGraph::new(3, vec![test_edge(0, 1, 1), test_edge(1, 2, 1)], 8).unwrap();
        let source_label = An19SymbolicLengthLabel {
            root_source: Some(SourceEdgeId(4)),
            unsplit_length: ExactRatio::new(8, 1).unwrap(),
            halved: true,
        };
        let virtual_label = An19SymbolicLengthLabel {
            root_source: None,
            unsplit_length: ExactRatio::new(3, 1).unwrap(),
            halved: false,
        };
        let mismatched_label = An19SymbolicLengthLabel {
            root_source: Some(SourceEdgeId(3)),
            ..source_label
        };
        assert!(
            AugmentedAn19Graph::from_source_with_inherited_labels(
                &graph,
                An19LengthMode::ExactRational,
                &[Some(SourceEdgeId(4)), None],
                &[mismatched_label, virtual_label],
            )
            .is_err()
        );
        let mut augmented = AugmentedAn19Graph::from_source_with_inherited_labels(
            &graph,
            An19LengthMode::ExactRational,
            &[Some(SourceEdgeId(4)), None],
            &[source_label, virtual_label],
        )
        .unwrap();
        let (portal, _, _) = augmented
            .split_edge(0, FlowNodeId(0), ExactRatio::new(1, 2).unwrap())
            .unwrap();
        let cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1), FlowNodeId(2), portal]);
        let mut metrics = An19HierarchyMetrics::default();
        let mut audit = An19ProjectionAudit::new(5);
        let projection = augmented
            .project_cluster(&cluster, &mut metrics, &mut audit)
            .unwrap();

        assert_eq!(projection.dense_symbolic_labels.len(), 3);
        assert_eq!(
            projection
                .dense_symbolic_labels
                .iter()
                .filter(|label| **label == source_label)
                .count(),
            2
        );
        assert_eq!(
            projection
                .dense_symbolic_labels
                .iter()
                .filter(|label| **label == virtual_label)
                .count(),
            1
        );
        assert_eq!(
            source_label.effective_length().unwrap(),
            ExactRatio::new(4, 1).unwrap()
        );
        assert_eq!(audit.original_edge_segment_occurrences, vec![0, 0, 0, 0, 2]);
        assert_eq!(audit.provenance_free_segment_occurrences, 1);
        assert_eq!(audit.projected_edge_occurrences, 3);
        assert_eq!(audit.maximum_projection_edges, 3);
        assert_eq!(audit.maximum_symbolic_source_label_classes, 1);
        assert_eq!(audit.maximum_symbolic_virtual_label_classes, 1);
        assert_eq!(audit.maximum_original_edge_segment_occurrences, 2);
        audit.verify(5, &metrics).unwrap();
    }

    #[test]
    fn rejects_cyclic_and_disconnected_augmented_tree_recovery() {
        use super::AugmentedAn19Graph;

        let triangle = SourceDynamicGraph::new(
            3,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(2),
                    second: FlowNodeId(0),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let augmented = AugmentedAn19Graph::from_source(&triangle).unwrap();
        assert!(
            augmented
                .recover_original_tree(&BTreeSet::from([0, 1, 2]))
                .is_err()
        );

        let disconnected = SourceDynamicGraph::new(
            4,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(2),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let augmented = AugmentedAn19Graph::from_source(&disconnected).unwrap();
        assert!(
            augmented
                .recover_original_tree(&BTreeSet::from([0, 1]))
                .is_err()
        );
    }

    #[test]
    fn hierarchical_base_case_matches_the_exact_tree_oracle() {
        use super::An19HierarchicalLsst;
        use crate::ExactStaticLsstOracle;

        let graph = path_graph(5);
        let hierarchy = An19HierarchicalLsst::construct(&graph, FlowNodeId(0)).unwrap();
        let oracle = ExactStaticLsstOracle::solve(&graph).unwrap();
        assert_eq!(hierarchy.tree_edges, oracle.tree_edges);
        assert_eq!(hierarchy.weighted_stretch, oracle.weighted_stretch);
        assert_eq!(hierarchy.total_weight, oracle.total_weight);
        assert_eq!(hierarchy.metrics.recursion_calls, 1);
        assert_eq!(hierarchy.metrics.base_cases, 1);
        assert_eq!(hierarchy.radius_certificates.len(), 1);
        assert!(hierarchy.radius_certificates[0].base_case);
        hierarchy.verify(&graph).unwrap();
        let mut invalid_radius = hierarchy.radius_certificates[0].clone();
        invalid_radius.radius = ExactRatio::new(5, 1).unwrap();
        assert!(invalid_radius.verify().is_err());
        let mut invalid_stretch = hierarchy.clone();
        invalid_stretch.weighted_stretch = ExactRatio::new(1, 1).unwrap();
        assert!(invalid_stretch.verify(&graph).is_err());
    }

    #[test]
    fn hierarchical_recursion_without_a_virtual_first_target_recovers_original_edges() {
        use super::{
            An19HierarchyMetrics, An19ProjectionAudit, AugmentedAn19Graph,
            hierarchical_petal_decomposition,
        };

        let graph = path_graph(500);
        let mut workspace = AugmentedAn19Graph::from_source(&graph).unwrap();
        let cluster = (0..500).map(FlowNodeId).collect::<BTreeSet<_>>();
        let mut certificates = Vec::new();
        let mut metrics = An19HierarchyMetrics::default();
        let mut projection_audit = An19ProjectionAudit::new(graph.edge_count());
        let selected = hierarchical_petal_decomposition(
            &mut workspace,
            cluster,
            FlowNodeId(0),
            FlowNodeId(499),
            500,
            None,
            0,
            false,
            &mut certificates,
            &mut metrics,
            &mut projection_audit,
        )
        .unwrap();
        let recovered = workspace.recover_original_tree(&selected).unwrap();
        assert_eq!(recovered.len(), 499);
        assert!(metrics.recursion_calls > 1);
        assert!(metrics.petals > 0);
        assert!(metrics.portal_splits > 0);
        assert!(metrics.fixed_path_reuses > 0);
        assert!(metrics.projection_cache_hits > 0);
        assert!(metrics.projection_incremental_splits > 0);
        assert!(projection_audit.provenance_free_segment_occurrences > 0);
        assert!(projection_audit.maximum_projection_length_classes > 1);
        assert!(
            projection_audit.maximum_projection_length_classes
                > projection_audit.maximum_symbolic_source_label_classes
        );
        assert!(projection_audit.maximum_symbolic_source_label_classes > 0);
        let logarithmic_levels =
            u64::from(usize::BITS - graph.node_count().saturating_sub(1).leading_zeros());
        assert_eq!(
            projection_audit.maximum_original_edge_scale_occurrences,
            logarithmic_levels
        );
        assert_eq!(
            projection_audit.maximum_original_edge_scale_occurrences,
            metrics.maximum_source_scale_participations
        );
        assert_eq!(
            projection_audit
                .original_edge_scale_occurrences
                .iter()
                .copied()
                .sum::<u64>(),
            metrics.source_scale_participations
        );
        assert_eq!(metrics.source_scale_participations, 2_256);
        assert_eq!(metrics.source_scale_attribution_scans, 2_919);
        assert_eq!(metrics.recursion_calls, 46);
        assert_eq!(metrics.partition_recursion_calls, 46);
        assert_eq!(metrics.maximum_partition_depth, 8);
        assert_projection_charging_counts(&projection_audit, [4_533, 17, 61, 16, 27, 2, 22]);
        assert_projection_scan_counts(
            &metrics,
            [
                165, 83, 82, 6_056, 5_974, 11_948, 172, 332, 18_290, 5_838, 0, 0, 0, 0,
            ],
        );
        assert!(projection_audit.maximum_original_edge_segment_occurrences > logarithmic_levels);
        projection_audit
            .verify(graph.edge_count(), &metrics)
            .unwrap();
        assert!(
            certificates
                .iter()
                .any(|certificate| !certificate.base_case)
        );
    }

    #[test]
    fn hierarchical_constructor_suppresses_the_unit_virtual_first_path() {
        use super::An19HierarchicalLsst;

        let graph = path_graph(500);
        let hierarchy = An19HierarchicalLsst::construct(&graph, FlowNodeId(0)).unwrap();
        assert_eq!(
            hierarchy.tree_edges,
            (0..499).map(crate::SourceEdgeId).collect()
        );
        assert!(hierarchy.metrics.virtual_leaves > 0);
        assert!(
            hierarchy
                .projection_audit
                .provenance_free_segment_occurrences
                > 0
        );
        assert!(hierarchy.metrics.recursion_calls > 1);
        assert!(hierarchy.metrics.petals > 0);
        assert_eq!(hierarchy.metrics.maximum_partition_depth, 9);
        let mut invalid_parent = hierarchy.clone();
        invalid_parent
            .radius_certificates
            .iter_mut()
            .find(|certificate| certificate.partition_depth >= 2)
            .unwrap()
            .recursion_parent = Some(0);
        assert!(invalid_parent.verify(&graph).is_err());
    }

    #[test]
    fn hierarchical_base_cases_differentiate_against_all_connected_four_node_graphs() {
        use super::An19HierarchicalLsst;
        use crate::ExactStaticLsstOracle;

        let endpoints = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        let mut checked = 0;
        for mask in 0_u32..(1_u32 << endpoints.len()) {
            let edges = endpoints
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1_u32 << index) != 0)
                .map(|(index, (first, second))| SourceWeightedEdge {
                    first: FlowNodeId(*first),
                    second: FlowNodeId(*second),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(i128::try_from(index + 1).unwrap(), 1).unwrap(),
                })
                .collect::<Vec<_>>();
            let graph = SourceDynamicGraph::new(4, edges, 16).unwrap();
            let Ok(oracle) = ExactStaticLsstOracle::solve(&graph) else {
                continue;
            };
            let hierarchy = An19HierarchicalLsst::construct(&graph, FlowNodeId(0)).unwrap();
            hierarchy.verify(&graph).unwrap();
            assert_eq!(hierarchy.total_weight, oracle.total_weight);
            assert!(
                hierarchy
                    .weighted_stretch
                    .at_least(oracle.weighted_stretch)
                    .unwrap()
            );
            checked += 1;
        }
        assert_eq!(checked, 38);
    }

    #[test]
    fn weighted_hierarchies_differentiate_on_all_connected_four_node_graphs() {
        use super::An19HierarchicalLsst;
        use crate::ExactStaticLsstOracle;

        let endpoints = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
        let mut checked = 0;
        for mask in 0_u32..(1_u32 << endpoints.len()) {
            let make_graph = |scale: i128| {
                let edges = endpoints
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| mask & (1_u32 << index) != 0)
                    .map(|(index, (first, second))| SourceWeightedEdge {
                        first: FlowNodeId(*first),
                        second: FlowNodeId(*second),
                        length: ExactRatio::new(scale * i128::try_from(index % 3 + 1).unwrap(), 2)
                            .unwrap(),
                        weight: ExactRatio::new(i128::try_from(index + 1).unwrap(), 1).unwrap(),
                    })
                    .collect::<Vec<_>>();
                SourceDynamicGraph::new(4, edges, 100_000).unwrap()
            };
            let small = make_graph(1);
            let Ok(oracle) = ExactStaticLsstOracle::solve(&small) else {
                continue;
            };
            let large = make_graph(1_000);
            let small_hierarchy = An19HierarchicalLsst::construct(&small, FlowNodeId(0)).unwrap();
            let large_hierarchy = An19HierarchicalLsst::construct(&large, FlowNodeId(0)).unwrap();
            small_hierarchy.verify(&small).unwrap();
            large_hierarchy.verify(&large).unwrap();
            assert_eq!(small_hierarchy.tree_edges, large_hierarchy.tree_edges);
            assert_eq!(
                small_hierarchy.weighted_stretch,
                large_hierarchy.weighted_stretch
            );
            assert_eq!(small_hierarchy.metrics, large_hierarchy.metrics);
            assert_eq!(
                small_hierarchy.projection_audit,
                large_hierarchy.projection_audit
            );
            assert_eq!(
                small_hierarchy.work_certificate,
                large_hierarchy.work_certificate
            );
            assert_eq!(small_hierarchy.total_weight, oracle.total_weight);
            assert!(
                small_hierarchy
                    .weighted_stretch
                    .at_least(oracle.weighted_stretch)
                    .unwrap()
            );
            checked += 1;
        }
        assert_eq!(checked, 38);
    }

    #[test]
    fn compact_rational_hierarchy_is_scale_invariant_without_length_expansion() {
        use super::An19HierarchicalLsst;

        let make_graph = |scale: i128| {
            SourceDynamicGraph::new(
                3,
                vec![
                    SourceWeightedEdge {
                        first: FlowNodeId(0),
                        second: FlowNodeId(1),
                        length: ExactRatio::new(2 * scale, 1).unwrap(),
                        weight: ExactRatio::new(2, 1).unwrap(),
                    },
                    SourceWeightedEdge {
                        first: FlowNodeId(1),
                        second: FlowNodeId(2),
                        length: ExactRatio::new(2 * scale, 1).unwrap(),
                        weight: ExactRatio::new(3, 1).unwrap(),
                    },
                    SourceWeightedEdge {
                        first: FlowNodeId(0),
                        second: FlowNodeId(2),
                        length: ExactRatio::new(3 * scale, 1).unwrap(),
                        weight: ExactRatio::new(5, 1).unwrap(),
                    },
                ],
                10_000,
            )
            .unwrap()
        };
        let small = make_graph(1);
        let large = make_graph(1_000);
        let small_hierarchy = An19HierarchicalLsst::construct(&small, FlowNodeId(0)).unwrap();
        let large_hierarchy = An19HierarchicalLsst::construct(&large, FlowNodeId(0)).unwrap();
        small_hierarchy.verify(&small).unwrap();
        large_hierarchy.verify(&large).unwrap();
        assert_eq!(small_hierarchy.tree_edges, large_hierarchy.tree_edges);
        assert_eq!(
            small_hierarchy.weighted_stretch,
            large_hierarchy.weighted_stretch
        );
        assert!(small_hierarchy.metrics.virtual_leaves <= 3);
        assert_eq!(
            small_hierarchy.metrics.virtual_leaves,
            large_hierarchy.metrics.virtual_leaves
        );
        assert_eq!(
            small_hierarchy.metrics.portal_splits,
            large_hierarchy.metrics.portal_splits
        );
        assert_eq!(
            small_hierarchy.work_certificate,
            large_hierarchy.work_certificate
        );
        assert_eq!(
            small_hierarchy.projection_audit,
            large_hierarchy.projection_audit
        );
        assert_eq!(
            small_hierarchy
                .projection_audit
                .maximum_symbolic_source_label_classes,
            large_hierarchy
                .projection_audit
                .maximum_symbolic_source_label_classes
        );
        assert_eq!(
            small_hierarchy
                .projection_audit
                .maximum_symbolic_virtual_label_classes,
            large_hierarchy
                .projection_audit
                .maximum_symbolic_virtual_label_classes
        );
        assert_eq!(small_hierarchy.work_certificate.oracle_fallbacks, 0);
        assert_eq!(
            small_hierarchy.work_certificate.numeric_length_expansions,
            0
        );
        assert_eq!(
            small_hierarchy.work_certificate.projection_mode,
            super::An19ProjectionMode::ClusterLocal
        );
        assert_eq!(
            small_hierarchy.work_certificate.length_mode,
            super::An19LengthMode::RoundedPowerOfTwo
        );
        assert_eq!(
            small_hierarchy.work_certificate.priority_queue_mode,
            super::An19PriorityQueueMode::ReducedLengthMonotone
        );
        assert_eq!(small_hierarchy.metrics.shortest_heap_pushes, 0);
        assert_eq!(small_hierarchy.metrics.directed_heap_pushes, 0);
        assert_eq!(small_hierarchy.metrics.event_heap_pushes, 0);
        assert_eq!(small_hierarchy.metrics.heap_comparisons, 0);
        assert!(!small_hierarchy.work_certificate.source_runtime_verified());
    }

    #[test]
    fn power_of_two_rounding_is_scale_relative_and_within_factor_two() {
        use super::{round_length_to_power_of_two, source_scale_participation_bound};

        let base = ExactRatio::new(2, 3).unwrap();
        let length = ExactRatio::new(3, 2).unwrap();
        let rounded = round_length_to_power_of_two(length, base).unwrap();
        assert_eq!(rounded, ExactRatio::new(4, 3).unwrap());
        assert!(length.at_least(rounded).unwrap());
        assert!(
            rounded
                .checked_mul_integer(2)
                .unwrap()
                .at_least(length)
                .unwrap()
        );
        assert_eq!(
            round_length_to_power_of_two(
                length.checked_mul_integer(1_000).unwrap(),
                base.checked_mul_integer(1_000).unwrap(),
            )
            .unwrap(),
            rounded.checked_mul_integer(1_000).unwrap()
        );
        assert_eq!(source_scale_participation_bound(0).unwrap(), 4);
        assert_eq!(source_scale_participation_bound(9).unwrap(), 58);
        assert!(source_scale_participation_bound(u64::MAX).is_err());
    }

    #[test]
    fn weighted_hierarchy_contracts_recursively_and_expands_the_quotient_tree() {
        use super::An19HierarchicalLsst;
        use crate::ExactStaticLsstOracle;

        let make_graph = |scale: i128| {
            SourceDynamicGraph::new(
                4,
                vec![
                    SourceWeightedEdge {
                        first: FlowNodeId(0),
                        second: FlowNodeId(1),
                        length: ExactRatio::new(scale, 1).unwrap(),
                        weight: ExactRatio::new(1, 1).unwrap(),
                    },
                    SourceWeightedEdge {
                        first: FlowNodeId(2),
                        second: FlowNodeId(3),
                        length: ExactRatio::new(scale, 1).unwrap(),
                        weight: ExactRatio::new(1, 1).unwrap(),
                    },
                    SourceWeightedEdge {
                        first: FlowNodeId(1),
                        second: FlowNodeId(2),
                        length: ExactRatio::new(100 * scale, 1).unwrap(),
                        weight: ExactRatio::new(1, 1).unwrap(),
                    },
                    SourceWeightedEdge {
                        first: FlowNodeId(0),
                        second: FlowNodeId(3),
                        length: ExactRatio::new(200 * scale, 1).unwrap(),
                        weight: ExactRatio::new(1, 1).unwrap(),
                    },
                ],
                1_000_000,
            )
            .unwrap()
        };
        let small = make_graph(1);
        let large = make_graph(1_000);
        let small_hierarchy = An19HierarchicalLsst::construct(&small, FlowNodeId(0)).unwrap();
        let large_hierarchy = An19HierarchicalLsst::construct(&large, FlowNodeId(0)).unwrap();
        let oracle = ExactStaticLsstOracle::solve(&small).unwrap();
        small_hierarchy.verify(&small).unwrap();
        large_hierarchy.verify(&large).unwrap();
        assert_eq!(
            small_hierarchy.tree_edges,
            BTreeSet::from([
                crate::SourceEdgeId(0),
                crate::SourceEdgeId(1),
                crate::SourceEdgeId(2)
            ])
        );
        assert_eq!(small_hierarchy.tree_edges, large_hierarchy.tree_edges);
        assert_eq!(
            small_hierarchy.weighted_stretch,
            large_hierarchy.weighted_stretch
        );
        assert_eq!(small_hierarchy.metrics, large_hierarchy.metrics);
        assert_eq!(small_hierarchy.metrics.contraction_calls, 1);
        assert_eq!(small_hierarchy.metrics.recursion_calls, 2);
        assert_eq!(small_hierarchy.metrics.partition_recursion_calls, 1);
        assert_eq!(small_hierarchy.metrics.maximum_partition_depth, 0);
        assert_eq!(
            small_hierarchy
                .radius_certificates
                .iter()
                .filter(|certificate| certificate.same_scale_contraction)
                .count(),
            1
        );
        assert_eq!(small_hierarchy.metrics.contracted_edges, 2);
        assert_eq!(small_hierarchy.metrics.quotient_edges, 2);
        assert_workspace_scan_counts(&small_hierarchy.metrics, [12, 4, 2, 4, 11]);
        assert!(
            small_hierarchy
                .projection_audit
                .original_edge_segment_occurrences
                .iter()
                .all(|occurrences| *occurrences > 0)
        );
        assert_eq!(
            small_hierarchy.projection_audit,
            large_hierarchy.projection_audit
        );
        assert_workspace_scan_mutations_rejected(&small_hierarchy, &small);
        assert_eq!(small_hierarchy.total_weight, oracle.total_weight);
        assert!(
            small_hierarchy
                .weighted_stretch
                .at_least(oracle.weighted_stretch)
                .unwrap()
        );
        let mut invalid = small_hierarchy.clone();
        invalid
            .radius_certificates
            .iter_mut()
            .find(|certificate| certificate.contraction_threshold.is_some())
            .unwrap()
            .contraction_threshold = Some(ExactRatio::new(1, 1).unwrap());
        assert!(invalid.verify(&small).is_err());
        assert_contraction_recursion_mutations_rejected(&small_hierarchy, &small);
    }

    #[test]
    fn compact_rational_hierarchy_recurses_with_scale_independent_counters() {
        use super::An19HierarchicalLsst;

        let small = alternating_path_graph(1);
        let large = alternating_path_graph(1_000);
        let small_hierarchy = An19HierarchicalLsst::construct(&small, FlowNodeId(0)).unwrap();
        let large_hierarchy = An19HierarchicalLsst::construct(&large, FlowNodeId(0)).unwrap();
        assert_projection_charging_counts(
            &small_hierarchy.projection_audit,
            [4_181, 16, 45, 12, 22, 1, 10],
        );
        assert_projection_scan_counts(
            &small_hierarchy.metrics,
            [
                118, 55, 63, 4_319, 4_256, 8_512, 113, 203, 16_043, 4_032, 1_496, 0, 0, 1_687,
            ],
        );
        small_hierarchy.verify(&small).unwrap();
        large_hierarchy.verify(&large).unwrap();
        assert_eq!(small_hierarchy.tree_edges, large_hierarchy.tree_edges);
        assert_eq!(small_hierarchy.metrics, large_hierarchy.metrics);
        assert!(small_hierarchy.metrics.recursion_calls > 1);
        assert!(small_hierarchy.metrics.virtual_leaves > 0);
        assert!(small_hierarchy.metrics.virtual_leaves < 500);
        assert!(small_hierarchy.metrics.virtual_leaves <= small_hierarchy.metrics.recursion_calls);
        assert_eq!(
            small_hierarchy.metrics.directed_region_runs,
            small_hierarchy.metrics.petals * 2
        );
        assert!(small_hierarchy.metrics.event_edge_touches > 0);
        assert!(small_hierarchy.metrics.workspace_edge_scans > 0);
        assert_eq!(
            small_hierarchy.metrics.monotone_queue_pushes,
            small_hierarchy.metrics.monotone_queue_pops
        );
        assert!(small_hierarchy.metrics.maximum_length_classes > 0);
        assert!(small_hierarchy.metrics.projected_edge_slots > 0);
        assert!(small_hierarchy.metrics.projection_length_class_sum > 0);
        assert!(small_hierarchy.metrics.source_scale_participations > 0);
        assert_eq!(small_hierarchy.metrics.source_scale_participations, 1_983);
        assert_eq!(
            small_hierarchy.metrics.maximum_source_scale_participations,
            7
        );
        assert_eq!(
            small_hierarchy.metrics.source_scale_attribution_scans,
            2_016
        );
        assert!(
            small_hierarchy.metrics.maximum_source_scale_participations
                <= small_hierarchy
                    .work_certificate
                    .source_scale_participation_bound
        );
        assert_eq!(
            small_hierarchy.metrics.maximum_projection_length_classes,
            small_hierarchy
                .projection_audit
                .maximum_projection_length_classes
        );
        assert!(
            small_hierarchy.work_certificate.observed_work_units
                <= small_hierarchy.work_certificate.maximum_work_units
        );
        let mut invalid_fallback = small_hierarchy.clone();
        invalid_fallback.work_certificate.oracle_fallbacks = 1;
        assert!(invalid_fallback.verify(&small).is_err());
        let mut invalid_bound = small_hierarchy.clone();
        invalid_bound.work_certificate.maximum_work_units = invalid_bound
            .work_certificate
            .observed_work_units
            .saturating_sub(1);
        assert!(invalid_bound.verify(&small).is_err());
        let mut invalid_projection = small_hierarchy.clone();
        invalid_projection.work_certificate.amortization_mode =
            super::An19AmortizationMode::StructuralSourceBound;
        assert!(invalid_projection.verify(&small).is_err());
        let mut invalid_audit = small_hierarchy.clone();
        invalid_audit
            .projection_audit
            .original_edge_segment_occurrences[0] += 1;
        assert!(invalid_audit.verify(&small).is_err());
        let mut invalid_symbolic_audit = small_hierarchy.clone();
        invalid_symbolic_audit
            .projection_audit
            .maximum_symbolic_source_label_classes += 1;
        assert!(invalid_symbolic_audit.verify(&small).is_err());
        assert_scale_audit_mutations_rejected(&small_hierarchy, &small);
        assert_projection_charging_mutations_rejected(&small_hierarchy, &small);
        assert_workspace_scan_mutations_rejected(&small_hierarchy, &small);
        let mut invalid_queue = small_hierarchy.clone();
        invalid_queue.metrics.monotone_queue_pops =
            invalid_queue.metrics.monotone_queue_pops.saturating_sub(1);
        assert!(invalid_queue.verify(&small).is_err());
        let mut invalid_binary_heap = small_hierarchy.clone();
        invalid_binary_heap.metrics.event_heap_pushes = 1;
        invalid_binary_heap.metrics.event_heap_pops = 1;
        assert!(invalid_binary_heap.verify(&small).is_err());
    }

    fn power_of_two_chord_graph(nodes: usize) -> SourceDynamicGraph {
        let mut edges = (0..nodes - 1)
            .map(|index| SourceWeightedEdge {
                first: FlowNodeId(index),
                second: FlowNodeId(index + 1),
                length: ExactRatio::new(1, 1).unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            })
            .collect::<Vec<_>>();
        for index in 0..nodes - 2 {
            let distance = nodes - 1 - index;
            edges.push(SourceWeightedEdge {
                first: FlowNodeId(index),
                second: FlowNodeId(nodes - 1),
                length: ExactRatio::new(i128::try_from(distance.next_power_of_two()).unwrap(), 1)
                    .unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            });
        }
        SourceDynamicGraph::new(nodes, edges, 4_096).unwrap()
    }

    fn assert_power_of_two_chord_family_has_linear_reduced_length_classes() {
        use super::{An19PetalMetrics, fast_shortest_paths, weighted_adjacency};

        for nodes in [16_usize, 32, 64, 128, 256] {
            let graph = power_of_two_chord_graph(nodes);
            let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
            let mut metrics = An19PetalMetrics::default();
            let paths = fast_shortest_paths(&graph, &cluster, FlowNodeId(0), &mut metrics).unwrap();
            for vertex in 0..nodes {
                assert_eq!(
                    paths.distances[vertex],
                    Some(ExactRatio::new(i128::try_from(vertex).unwrap(), 1).unwrap())
                );
            }
            let (adjacency, reduced_classes) =
                weighted_adjacency(&graph, &cluster, &paths.distances, &mut metrics).unwrap();

            // For r in (N/2, N), the chord from v_(N-1-r) to v_(N-1)
            // has length N and forward reduced cost 2 * (N-r). These N/2-1
            // costs are distinct although the graph has only log2(N)+1
            // original power-of-two length classes.
            let mut witnessed = BTreeSet::new();
            for distance in nodes / 2 + 1..nodes {
                let from = nodes - 1 - distance;
                let expected =
                    ExactRatio::new(i128::try_from(2 * (nodes - distance)).unwrap(), 1).unwrap();
                assert!(adjacency[from].iter().any(|(other, length, _)| *other
                    == FlowNodeId(nodes - 1)
                    && *length == expected));
                witnessed.insert((expected.numerator(), expected.denominator()));
            }
            assert_eq!(witnessed.len(), nodes / 2 - 1);
            assert!(reduced_classes >= witnessed.len());
            assert_eq!(
                usize::try_from(nodes.ilog2()).unwrap() + 1,
                (0..graph.edge_count())
                    .filter_map(|index| graph.edge(SourceEdgeId(index)))
                    .map(|edge| (edge.length.numerator(), edge.length.denominator()))
                    .collect::<BTreeSet<_>>()
                    .len()
            );
        }
    }

    #[test]
    fn reduced_length_queue_exposes_unbounded_source_classes() {
        use super::{
            An19PetalMetrics, An19WeightedPetal, An19WeightedPetalAtRadius, fast_shortest_paths,
            recover_path, transformed_weighted_membership_thresholds_oracle,
        };

        assert_power_of_two_chord_family_has_linear_reduced_length_classes();

        let nodes = 128_usize;
        let graph = power_of_two_chord_graph(nodes);
        let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19WeightedPetal::construct_for_hierarchy(
            &graph,
            &cluster,
            &cluster,
            FlowNodeId(0),
            FlowNodeId(nodes - 1),
            ExactRatio::new(32, 1).unwrap(),
            true,
            graph.node_count(),
        )
        .unwrap();
        assert!(
            petal.at_radius.metrics.maximum_length_classes > u64::try_from(nodes).unwrap(),
            "reduced lengths are not bounded by the original power-of-two classes"
        );
        let fixed_radius = An19WeightedPetalAtRadius::construct_for_hierarchy(
            &graph,
            &cluster,
            &cluster,
            FlowNodeId(0),
            FlowNodeId(nodes - 1),
            ExactRatio::new(32, 1).unwrap(),
        )
        .unwrap();
        assert!(
            fixed_radius.metrics.maximum_length_classes <= 9,
            "fixed-radius Claim 15 now uses only original edge-length classes plus sources"
        );
        let mut setup_metrics = An19PetalMetrics::default();
        let center_paths =
            fast_shortest_paths(&graph, &cluster, FlowNodeId(0), &mut setup_metrics).unwrap();
        let path = recover_path(FlowNodeId(0), FlowNodeId(nodes - 1), &center_paths).unwrap();
        let mut transformed_metrics = An19PetalMetrics::default();
        let transformed = transformed_weighted_membership_thresholds_oracle(
            &graph,
            &cluster,
            FlowNodeId(nodes - 1),
            &path,
            &center_paths.distances,
            ExactRatio::new(32, 1).unwrap(),
            &mut transformed_metrics,
        )
        .unwrap();
        assert!(transformed.by_vertex.iter().any(Option::is_some));
        assert!(
            transformed_metrics.maximum_length_classes <= 9,
            "the transformed queue uses only doubled original edge-length classes plus sources"
        );
    }
}
