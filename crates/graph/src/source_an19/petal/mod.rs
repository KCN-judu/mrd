use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::{
    CertifiedFixedPoint, ExactRatio, FixedPointConfig, FlowNodeId, SourceDynamicGraph, SourceEdgeId,
};

use super::experiment::projection;

#[cfg(test)]
use super::experiment::hierarchy;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PetalMetrics {
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

impl PetalMetrics {
    fn checked_add_assign(&mut self, other: &Self) -> Result<(), Error> {
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
pub struct UnweightedPetal {
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
    pub metrics: PetalMetrics,
}

impl UnweightedPetal {
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
    ) -> Result<Self, Error> {
        validate_domain(graph, cluster, remaining, center, target, budget.clone())?;
        let mut metrics = PetalMetrics::default();
        let cluster_paths = shortest_paths(graph, cluster, center, &mut metrics)?;
        let recovered_path = recover_path(center, target, &cluster_paths)?;
        if recovered_path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(Error::InvalidDomain);
        }
        let remaining_from_center = shortest_paths(graph, remaining, center, &mut metrics)?;
        for vertex in &recovered_path.vertices {
            if cluster_paths.distances[vertex.0] != remaining_from_center.distances[vertex.0] {
                return Err(Error::InvalidDomain);
            }
        }
        let thresholds = membership_thresholds(
            graph,
            remaining,
            &recovered_path.vertices,
            &remaining_from_center,
            target,
            budget.clone(),
            &mut metrics,
        )?;
        let cluster_edges = internal_edge_count(graph, cluster);
        let active_edges = (0..graph.edge_count())
            .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
            .count();
        if cluster_edges == 0 || active_edges < 2 {
            return Err(Error::InvalidDomain);
        }
        let levels = ceil_log_log(graph.node_count());
        let mut selected = None;
        for index in 1..=levels {
            let window_end = window_radius(budget.clone(), index, levels, true)?;
            let vertices = vertices_at_radius(remaining, &thresholds, window_end.clone())?;
            let internal = internal_edge_count(graph, &vertices);
            metrics.certified_comparisons = metrics
                .certified_comparisons
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            if certify_window_condition(active_edges, cluster_edges, internal, index, levels)? {
                selected = Some((index, window_end));
                break;
            }
        }
        let (window_index, window_end) = selected.ok_or(Error::InvalidRadius)?;
        let window_start = window_radius(budget.clone(), window_index, levels, false)?;
        let start_vertices = vertices_at_radius(remaining, &thresholds, window_start.clone())?;
        let start_edges = internal_edge_count(graph, &start_vertices);
        if start_edges == 0 || start_edges >= cluster_edges {
            return Err(Error::InvalidRadius);
        }
        let mut radius = window_start.clone();
        let (vertices, internal_edges, boundary_edges) = loop {
            let vertices = vertices_at_radius(remaining, &thresholds, radius.clone())?;
            let internal = internal_edge_count(graph, &vertices);
            let boundary = boundary_edge_count(graph, cluster, &vertices);
            metrics.certified_comparisons = metrics
                .certified_comparisons
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            if certify_stopping_condition(
                cluster_edges,
                start_edges,
                internal,
                count_ratio(boundary)?,
                levels,
                budget.clone(),
            )? {
                break (vertices, internal, boundary);
            }
            radius = next_radius_event(remaining, &thresholds, radius, window_end.clone())?
                .ok_or(Error::InvalidRadius)?;
            metrics.radius_events = metrics
                .radius_events
                .checked_add(1)
                .ok_or(Error::Overflow)?;
        };
        let center_vertex =
            recovered_path.vertices.iter().copied().find(|vertex| {
                thresholds.path_distance_from_target[vertex.0] == Some(radius.clone())
            });
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
pub enum PathPoint {
    Vertex(FlowNodeId),
    EdgeInterior {
        edge: SourceEdgeId,
        from: FlowNodeId,
        toward_center: FlowNodeId,
        offset_from: ExactRatio,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighwaySegment {
    pub edge: SourceEdgeId,
    pub from: FlowNodeId,
    pub toward_center: FlowNodeId,
    pub halved_length: ExactRatio,
    pub original_edge_length: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeightedPetalAtRadius {
    pub vertices: BTreeSet<FlowNodeId>,
    pub path_from_center: Vec<FlowNodeId>,
    pub path_edges: Vec<SourceEdgeId>,
    pub portal: PathPoint,
    pub radius: ExactRatio,
    pub highway_segments: Vec<HighwaySegment>,
    pub directed_distances: Vec<Option<ExactRatio>>,
    pub metrics: PetalMetrics,
}

impl WeightedPetalAtRadius {
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
    ) -> Result<Self, Error> {
        Self::construct_with_paths(graph, cluster, remaining, center, target, radius, false)
    }

    pub(super) fn construct_for_hierarchy(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        radius: ExactRatio,
    ) -> Result<Self, Error> {
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
    ) -> Result<Self, Error> {
        validate_weighted_domain(graph, cluster, remaining, center, target, radius.clone())?;
        let mut metrics = PetalMetrics::default();
        let cluster_paths =
            hierarchy_or_oracle_paths(graph, cluster, center, fast_paths, &mut metrics)?;
        let path = recover_path(center, target, &cluster_paths)?;
        if path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(Error::InvalidDomain);
        }
        let remaining_paths =
            hierarchy_or_oracle_paths(graph, remaining, center, fast_paths, &mut metrics)?;
        for vertex in &path.vertices {
            if cluster_paths.distances[vertex.0] != remaining_paths.distances[vertex.0] {
                return Err(Error::InvalidDomain);
            }
        }
        let (portal, highway_segments) =
            locate_portal_and_highway(graph, &path, target, radius.clone())?;
        let directed_distances = directed_petal_distances(
            graph,
            remaining,
            target,
            &remaining_paths.distances,
            &highway_segments,
            &mut metrics,
        )?;
        let half_radius = radius
            .checked_mul(&ratio(1, 2)?)
            .map_err(|_| Error::Overflow)?;
        let mut vertices = BTreeSet::new();
        for vertex in remaining {
            let distance = directed_distances[vertex.0]
                .clone()
                .ok_or(Error::Disconnected)?;
            if half_radius
                .at_least(&distance)
                .map_err(|_| Error::Overflow)?
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
pub struct WeightedPetal {
    pub at_radius: WeightedPetalAtRadius,
    pub window_index: usize,
    pub window_start: ExactRatio,
    pub window_end: ExactRatio,
    pub internal_edges: usize,
    pub boundary_edges: usize,
    pub cluster_edges: usize,
}

impl WeightedPetal {
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
    ) -> Result<Self, Error> {
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
    pub(super) fn construct_for_hierarchy(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        budget: ExactRatio,
        compact_weighted_portals: bool,
        level_node_count: usize,
    ) -> Result<Self, Error> {
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
    ) -> Result<Self, Error> {
        validate_weighted_domain(graph, cluster, remaining, center, target, budget.clone())?;
        if !budget.is_positive() {
            return Err(Error::InvalidDomain);
        }
        let mut metrics = PetalMetrics::default();
        let cluster_paths =
            hierarchy_or_oracle_paths(graph, cluster, center, fast_events, &mut metrics)?;
        let path = recover_path(center, target, &cluster_paths)?;
        if path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(Error::InvalidDomain);
        }
        let remaining_paths =
            hierarchy_or_oracle_paths(graph, remaining, center, fast_events, &mut metrics)?;
        for vertex in &path.vertices {
            if cluster_paths.distances[vertex.0] != remaining_paths.distances[vertex.0] {
                return Err(Error::InvalidDomain);
            }
        }
        let target_distance = remaining_paths.distances[target.0]
            .clone()
            .ok_or(Error::Disconnected)?;
        if ratio_less(target_distance, budget.clone())? {
            return Err(Error::InvalidRadius);
        }
        let thresholds = if fast_events {
            fast_weighted_membership_thresholds(
                graph,
                remaining,
                target,
                &path,
                &remaining_paths.distances,
                budget.clone(),
                &mut metrics,
            )?
        } else {
            weighted_membership_thresholds_oracle(
                graph,
                remaining,
                target,
                &path,
                &remaining_paths.distances,
                budget.clone(),
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
            WeightedPetalAtRadius::construct_for_hierarchy(
                graph,
                cluster,
                remaining,
                center,
                target,
                selection.radius,
            )?
        } else {
            WeightedPetalAtRadius::construct(
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

pub(super) struct FigureSixSelection {
    pub(super) radius: ExactRatio,
    pub(super) window_index: usize,
    pub(super) window_start: ExactRatio,
    pub(super) window_end: ExactRatio,
    pub(super) internal_edges: usize,
    pub(super) boundary_edges: usize,
    pub(super) cluster_edges: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_weighted_figure_six(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    budget: ExactRatio,
    compact_weighted_portals: bool,
    fast_events: bool,
    level_node_count: usize,
    metrics: &mut PetalMetrics,
) -> Result<FigureSixSelection, Error> {
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

#[derive(Clone)]
pub(super) struct RegionAdjacencyEdge {
    other: FlowNodeId,
    edge: SourceEdgeId,
    length: ExactRatio,
}

pub(super) struct RegionVolumeState {
    included: Vec<bool>,
    edge_seen: Vec<bool>,
    vertices: BTreeSet<FlowNodeId>,
    internal_edges: usize,
    incident_edges: usize,
    boundary_edges: usize,
    boundary_cost: ExactRatio,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn select_weighted_figure_six_fast(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    budget: ExactRatio,
    compact_weighted_portals: bool,
    level_node_count: usize,
    metrics: &mut PetalMetrics,
) -> Result<FigureSixSelection, Error> {
    let (base_cluster_edges, base_active_edges) = figure_six_base_edge_counts(graph, cluster)?;
    metrics.event_edge_touches = checked_metric_sum(
        metrics.event_edge_touches,
        u64::try_from(graph.edge_count())
            .ok()
            .and_then(|count| count.checked_mul(2))
            .ok_or(Error::Overflow)?,
    )?;
    let events = sorted_membership_events(remaining, thresholds, metrics)?;
    let adjacency = region_adjacency(graph, cluster, metrics)?;
    let levels = ceil_log_log(level_node_count);
    let mut state = RegionVolumeState::new(graph)?;
    let mut cursor = 0;
    let mut selected = None;
    for index in 1..=levels {
        let window_end = window_radius(budget.clone(), index, levels, true)?;
        advance_region_state(
            &events,
            &mut cursor,
            window_end.clone(),
            &adjacency,
            &mut state,
            metrics,
        )?;
        let portal_split = usize::from(
            compact_weighted_portals && portal_is_interior(thresholds, window_end.clone()),
        );
        let petal_edges = state.edge_measure(compact_weighted_portals, portal_split, metrics)?;
        let cluster_edges = checked_edge_sum(base_cluster_edges, portal_split)?;
        let active_edges = checked_edge_sum(base_active_edges, portal_split)?;
        metrics.certified_comparisons = checked_metric_sum(metrics.certified_comparisons, 1)?;
        if certify_window_condition(active_edges, cluster_edges, petal_edges, index, levels)? {
            selected = Some((index, window_end));
            break;
        }
    }
    let (window_index, window_end) = selected.ok_or(Error::InvalidRadius)?;
    let window_start = window_radius(budget.clone(), window_index, levels, false)?;
    let mut state = RegionVolumeState::new(graph)?;
    let mut cursor = 0;
    advance_region_state(
        &events,
        &mut cursor,
        window_start.clone(),
        &adjacency,
        &mut state,
        metrics,
    )?;
    let start_portal_split = usize::from(
        compact_weighted_portals && portal_is_interior(thresholds, window_start.clone()),
    );
    let start_edges = state.edge_measure(compact_weighted_portals, start_portal_split, metrics)?;
    let start_cluster_edges = checked_edge_sum(base_cluster_edges, start_portal_split)?;
    if start_edges == 0 || start_edges >= start_cluster_edges {
        return Err(Error::InvalidRadius);
    }
    let mut radius = window_start.clone();
    loop {
        let portal_split =
            usize::from(compact_weighted_portals && portal_is_interior(thresholds, radius.clone()));
        let petal_edges = state.edge_measure(compact_weighted_portals, portal_split, metrics)?;
        let cluster_edges = checked_edge_sum(base_cluster_edges, portal_split)?;
        let boundary_cost = if compact_weighted_portals {
            state.boundary_cost.clone()
        } else {
            count_ratio(state.boundary_edges)?
        };
        metrics.certified_comparisons = checked_metric_sum(metrics.certified_comparisons, 1)?;
        if let Some(selection) = fast_stopping_selection(
            radius.clone(),
            window_index,
            window_start.clone(),
            window_end.clone(),
            cluster_edges,
            start_edges,
            petal_edges,
            state.boundary_edges,
            boundary_cost,
            levels,
            budget.clone(),
        )? {
            return Ok(selection);
        }
        let next = events
            .get(cursor)
            .map(|event| event.distance.clone())
            .ok_or(Error::InvalidRadius)?;
        if ratio_less(window_end.clone(), next.clone())? {
            return Err(Error::InvalidRadius);
        }
        radius = next;
        advance_region_state(
            &events,
            &mut cursor,
            radius.clone(),
            &adjacency,
            &mut state,
            metrics,
        )?;
        metrics.radius_events = checked_metric_sum(metrics.radius_events, 1)?;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fast_stopping_selection(
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
) -> Result<Option<FigureSixSelection>, Error> {
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
    fn new(graph: &SourceDynamicGraph) -> Result<Self, Error> {
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
        metrics: &mut PetalMetrics,
    ) -> Result<usize, Error> {
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
        metrics: &mut PetalMetrics,
    ) -> Result<(), Error> {
        if self.included[vertex.0] {
            return Ok(());
        }
        self.included[vertex.0] = true;
        self.vertices.insert(vertex);
        metrics.event_vertex_activations = checked_metric_sum(metrics.event_vertex_activations, 1)?;
        for adjacent in &adjacency[vertex.0] {
            metrics.event_edge_touches = checked_metric_sum(metrics.event_edge_touches, 1)?;
            let weight = adjacent.length.reciprocal().map_err(|_| Error::Overflow)?;
            if !self.edge_seen[adjacent.edge.0] {
                self.edge_seen[adjacent.edge.0] = true;
                self.incident_edges = checked_edge_sum(self.incident_edges, 1)?;
                if self.included[adjacent.other.0] {
                    self.internal_edges = checked_edge_sum(self.internal_edges, 1)?;
                } else {
                    self.boundary_edges = checked_edge_sum(self.boundary_edges, 1)?;
                    self.boundary_cost = self
                        .boundary_cost
                        .checked_add(&weight)
                        .map_err(|_| Error::Overflow)?;
                }
            } else if self.included[adjacent.other.0] && adjacent.other != vertex {
                self.internal_edges = checked_edge_sum(self.internal_edges, 1)?;
                self.boundary_edges = self
                    .boundary_edges
                    .checked_sub(1)
                    .ok_or(Error::InvalidRadius)?;
                self.boundary_cost = self
                    .boundary_cost
                    .checked_sub(&weight)
                    .map_err(|_| Error::Overflow)?;
            }
        }
        Ok(())
    }
}

pub(super) fn sorted_membership_events(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    metrics: &mut PetalMetrics,
) -> Result<Vec<ExactHeapEntry>, Error> {
    if let Some(events) = &thresholds.ordered_events {
        return Ok(events.clone());
    }
    let mut heap = Vec::new();
    for vertex in remaining {
        if let Some(distance) = &thresholds.by_vertex[vertex.0] {
            event_heap_push(
                &mut heap,
                ExactHeapEntry {
                    distance: distance.clone(),
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

pub(super) fn region_adjacency(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    metrics: &mut PetalMetrics,
) -> Result<Vec<Vec<RegionAdjacencyEdge>>, Error> {
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
                length: edge.length.clone(),
            });
            if edge.first != edge.second {
                adjacency[edge.second.0].push(RegionAdjacencyEdge {
                    other: edge.first,
                    edge: edge_id,
                    length: edge.length.clone(),
                });
            }
        }
    }
    Ok(adjacency)
}

pub(super) fn advance_region_state(
    events: &[ExactHeapEntry],
    cursor: &mut usize,
    radius: ExactRatio,
    adjacency: &[Vec<RegionAdjacencyEdge>],
    state: &mut RegionVolumeState,
    metrics: &mut PetalMetrics,
) -> Result<(), Error> {
    while let Some(event) = events.get(*cursor) {
        if ratio_less(radius.clone(), event.distance.clone())? {
            break;
        }
        state.activate(event.vertex, adjacency, metrics)?;
        *cursor = (*cursor).checked_add(1).ok_or(Error::Overflow)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_weighted_figure_six_oracle(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    budget: ExactRatio,
    compact_weighted_portals: bool,
    level_node_count: usize,
    metrics: &mut PetalMetrics,
) -> Result<FigureSixSelection, Error> {
    let base_cluster_edges = internal_edge_count(graph, cluster);
    let base_active_edges = (0..graph.edge_count())
        .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
        .count();
    if base_cluster_edges == 0 || base_active_edges < 2 {
        return Err(Error::InvalidDomain);
    }
    let levels = ceil_log_log(level_node_count);
    let mut selected = None;
    for index in 1..=levels {
        let window_end = window_radius(budget.clone(), index, levels, true)?;
        let vertices = vertices_at_radius(remaining, thresholds, window_end.clone())?;
        let portal_split = usize::from(
            compact_weighted_portals && portal_is_interior(thresholds, window_end.clone()),
        );
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
            .ok_or(Error::Overflow)?;
        if certify_window_condition(active_edges, cluster_edges, internal, index, levels)? {
            selected = Some((index, window_end));
            break;
        }
    }
    let (window_index, window_end) = selected.ok_or(Error::InvalidRadius)?;
    let window_start = window_radius(budget.clone(), window_index, levels, false)?;
    let start_vertices = vertices_at_radius(remaining, thresholds, window_start.clone())?;
    let start_portal_split = usize::from(
        compact_weighted_portals && portal_is_interior(thresholds, window_start.clone()),
    );
    let start_edges = petal_edge_measure(
        graph,
        cluster,
        &start_vertices,
        compact_weighted_portals,
        start_portal_split,
    )?;
    let start_cluster_edges = checked_edge_sum(base_cluster_edges, start_portal_split)?;
    if start_edges == 0 || start_edges >= start_cluster_edges {
        return Err(Error::InvalidRadius);
    }
    let mut radius = window_start.clone();
    loop {
        let vertices = vertices_at_radius(remaining, thresholds, radius.clone())?;
        let portal_split =
            usize::from(compact_weighted_portals && portal_is_interior(thresholds, radius.clone()));
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
            .ok_or(Error::Overflow)?;
        if certify_stopping_condition(
            cluster_edges,
            start_edges,
            internal_edges,
            boundary_cost,
            levels,
            budget.clone(),
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
        radius = next_radius_event(remaining, thresholds, radius, window_end.clone())?
            .ok_or(Error::InvalidRadius)?;
        metrics.radius_events = metrics
            .radius_events
            .checked_add(1)
            .ok_or(Error::Overflow)?;
    }
}

pub(super) fn portal_is_interior(thresholds: &MembershipThresholds, radius: ExactRatio) -> bool {
    !thresholds.path_distance_from_target.contains(&Some(radius))
}

pub(super) fn petal_edge_measure(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    vertices: &BTreeSet<FlowNodeId>,
    use_incident_volume: bool,
    portal_split: usize,
) -> Result<usize, Error> {
    let base = if use_incident_volume {
        incident_edge_count(graph, cluster, vertices)
    } else {
        internal_edge_count(graph, vertices)
    };
    checked_edge_sum(base, portal_split)
}

pub(super) fn checked_edge_sum(first: usize, second: usize) -> Result<usize, Error> {
    first.checked_add(second).ok_or(Error::Overflow)
}

pub(super) fn figure_six_base_edge_counts(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
) -> Result<(usize, usize), Error> {
    let cluster_edges = internal_edge_count(graph, cluster);
    let active_edges = (0..graph.edge_count())
        .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
        .count();
    if cluster_edges == 0 || active_edges < 2 {
        return Err(Error::InvalidDomain);
    }
    Ok((cluster_edges, active_edges))
}

pub(super) fn split_provenance(
    edge: &projection::Edge,
    from: FlowNodeId,
    offset: ExactRatio,
) -> Result<
    (
        Option<projection::OriginalInterval>,
        Option<projection::OriginalInterval>,
    ),
    Error,
> {
    let Some(provenance) = &edge.provenance else {
        return Ok((None, None));
    };
    let (from_position, toward_position) = if edge.first == from {
        (
            provenance.first_position.clone(),
            provenance.second_position.clone(),
        )
    } else {
        (
            provenance.second_position.clone(),
            provenance.first_position.clone(),
        )
    };
    let direction = toward_position
        .checked_sub(&from_position)
        .map_err(|_| Error::Overflow)?;
    let fraction = offset
        .checked_mul(&edge.length.reciprocal().map_err(|_| Error::Overflow)?)
        .map_err(|_| Error::Overflow)?;
    let split_position = from_position
        .checked_add(
            &direction
                .checked_mul(&fraction)
                .map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::Overflow)?;
    let from_interval = projection::OriginalInterval {
        edge: provenance.edge,
        first_position: from_position,
        second_position: split_position.clone(),
    };
    let toward_interval = projection::OriginalInterval {
        edge: provenance.edge,
        first_position: split_position,
        second_position: toward_position,
    };
    Ok((Some(from_interval), Some(toward_interval)))
}

#[derive(Clone, Debug)]
pub(super) struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSet {
    pub(super) fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    pub(super) fn find(&mut self, vertex: usize) -> usize {
        if self.parent[vertex] != vertex {
            self.parent[vertex] = self.find(self.parent[vertex]);
        }
        self.parent[vertex]
    }

    pub(super) fn union(&mut self, first: usize, second: usize) -> bool {
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
pub(super) struct ShortestPaths {
    pub(super) distances: Vec<Option<ExactRatio>>,
    pub(super) predecessors: Vec<Option<(usize, SourceEdgeId)>>,
}

pub(super) struct HierarchyShortestPaths {
    pub(super) distances: BTreeMap<FlowNodeId, ExactRatio>,
    pub(super) predecessors: BTreeMap<FlowNodeId, (FlowNodeId, SourceEdgeId)>,
}

pub(super) struct RecoveredPath {
    pub(super) vertices: Vec<FlowNodeId>,
    pub(super) edges: Vec<SourceEdgeId>,
}

#[derive(Clone, Debug)]
pub(super) struct MembershipThresholds {
    pub(super) by_vertex: Vec<Option<ExactRatio>>,
    pub(super) path_distance_from_target: Vec<Option<ExactRatio>>,
    pub(super) ordered_events: Option<Vec<ExactHeapEntry>>,
}

pub(super) fn validate_domain(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
) -> Result<(), Error> {
    let one = ratio(1, 1)?;
    if cluster.is_empty()
        || !remaining.is_subset(cluster)
        || !remaining.contains(&center)
        || !remaining.contains(&target)
        || !budget.is_positive()
        || cluster.iter().any(|vertex| vertex.0 >= graph.node_count())
    {
        return Err(Error::InvalidDomain);
    }
    for index in 0..graph.edge_count() {
        if let Some(edge) = graph.edge(SourceEdgeId(index))
            && edge.length != one
        {
            return Err(Error::NonunitLength);
        }
    }
    Ok(())
}

pub(super) fn validate_weighted_domain(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    radius: ExactRatio,
) -> Result<(), Error> {
    if cluster.is_empty()
        || !remaining.is_subset(cluster)
        || !remaining.contains(&center)
        || !remaining.contains(&target)
        || radius.is_negative()
        || cluster.iter().any(|vertex| vertex.0 >= graph.node_count())
    {
        return Err(Error::InvalidDomain);
    }
    Ok(())
}

pub(super) fn shortest_paths(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    metrics: &mut PetalMetrics,
) -> Result<ShortestPaths, Error> {
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
                .ok_or(Error::Overflow)?;
            let candidate = distances[node]
                .clone()
                .ok_or(Error::Disconnected)?
                .checked_add(&edge.length)
                .map_err(|_| Error::Overflow)?;
            let mut key = path_keys[node].as_ref().ok_or(Error::Disconnected)?.clone();
            key.push(SourceEdgeId(index));
            let improves = match &distances[other] {
                None => true,
                Some(old) => {
                    ratio_less(candidate.clone(), old.clone())?
                        || (&candidate == old
                            && key < *path_keys[other].as_ref().ok_or(Error::Disconnected)?)
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
        .ok_or(Error::Overflow)?;
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(Error::Disconnected);
    }
    Ok(ShortestPaths {
        distances,
        predecessors,
    })
}

pub(super) fn fast_shortest_paths(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    metrics: &mut PetalMetrics,
) -> Result<ShortestPaths, Error> {
    let mut adjacency =
        vec![Vec::<(FlowNodeId, SourceEdgeId, ExactRatio, usize)>::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(String, String), usize>::new();
    for index in 0..graph.edge_count() {
        metrics.shortest_edge_scans = checked_metric_sum(metrics.shortest_edge_scans, 1)?;
        let edge_id = SourceEdgeId(index);
        let Some(edge) = graph.edge(edge_id) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let next_class = length_classes.len();
            let class = *length_classes
                .entry(ratio_key(&edge.length))
                .or_insert(next_class);
            adjacency[edge.first.0].push((edge.second, edge_id, edge.length.clone(), class));
            adjacency[edge.second.0].push((edge.first, edge_id, edge.length.clone(), class));
        }
    }
    let mut distances = vec![None; graph.node_count()];
    let mut predecessors = vec![None; graph.node_count()];
    let mut settled = vec![false; graph.node_count()];
    let zero = ratio(0, 1)?;
    distances[source.0] = Some(zero.clone());
    let source_class = length_classes.len();
    let mut queue =
        DistinctLengthQueue::new(source_class.checked_add(1).ok_or(Error::Overflow)?, metrics)?;
    queue.push(
        source_class,
        ExactHeapEntry {
            distance: zero,
            vertex: source,
        },
        metrics,
    )?;
    while let Some(entry) = queue.pop(metrics)? {
        if settled[entry.vertex.0] || distances[entry.vertex.0] != Some(entry.distance.clone()) {
            continue;
        }
        settled[entry.vertex.0] = true;
        for (other, edge_id, length, class) in &adjacency[entry.vertex.0] {
            metrics.shortest_edge_scans = metrics
                .shortest_edge_scans
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            if settled[other.0] {
                continue;
            }
            metrics.edge_relaxations = metrics
                .edge_relaxations
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            let candidate = entry
                .distance
                .checked_add(length)
                .map_err(|_| Error::Overflow)?;
            let old = distances[other.0].clone();
            let shorter = match &old {
                Some(distance) => ratio_less(candidate.clone(), distance.clone())?,
                None => true,
            };
            let equal_better = old == Some(candidate.clone())
                && predecessors[other.0].is_none_or(|(parent, old_edge)| {
                    (*edge_id, entry.vertex.0) < (old_edge, parent)
                });
            if shorter {
                distances[other.0] = Some(candidate.clone());
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
        .ok_or(Error::Overflow)?;
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(Error::Disconnected);
    }
    Ok(ShortestPaths {
        distances,
        predecessors,
    })
}

pub(super) fn hierarchy_or_oracle_paths(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    fast: bool,
    metrics: &mut PetalMetrics,
) -> Result<ShortestPaths, Error> {
    if fast {
        fast_shortest_paths(graph, allowed, source, metrics)
    } else {
        shortest_paths(graph, allowed, source, metrics)
    }
}

pub(super) fn path_state_is_better(
    candidate: usize,
    old: usize,
    distances: &[Option<ExactRatio>],
    path_keys: &[Option<Vec<SourceEdgeId>>],
) -> Result<bool, Error> {
    let candidate_distance = distances[candidate].clone().ok_or(Error::Disconnected)?;
    let old_distance = distances[old].clone().ok_or(Error::Disconnected)?;
    if ratio_less(candidate_distance.clone(), old_distance.clone())? {
        return Ok(true);
    }
    if candidate_distance != old_distance {
        return Ok(false);
    }
    let candidate_key = path_keys[candidate].as_ref().ok_or(Error::Disconnected)?;
    let old_key = path_keys[old].as_ref().ok_or(Error::Disconnected)?;
    Ok((candidate_key, candidate) < (old_key, old))
}

pub(super) fn recover_path(
    source: FlowNodeId,
    target: FlowNodeId,
    paths: &ShortestPaths,
) -> Result<RecoveredPath, Error> {
    let mut reversed = vec![target];
    let mut reversed_edges = Vec::new();
    let mut current = target.0;
    while current != source.0 {
        let (parent, edge) = paths.predecessors[current].ok_or(Error::Disconnected)?;
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

pub(super) fn recover_hierarchy_path(
    source: FlowNodeId,
    target: FlowNodeId,
    paths: &HierarchyShortestPaths,
) -> Result<RecoveredPath, Error> {
    let mut reversed = vec![target];
    let mut reversed_edges = Vec::new();
    let mut current = target;
    while current != source {
        let (parent, edge) = paths
            .predecessors
            .get(&current)
            .copied()
            .ok_or(Error::Disconnected)?;
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

pub(super) fn locate_portal_and_highway(
    graph: &SourceDynamicGraph,
    path: &RecoveredPath,
    target: FlowNodeId,
    radius: ExactRatio,
) -> Result<(PathPoint, Vec<HighwaySegment>), Error> {
    if path.vertices.last().copied() != Some(target) || path.vertices.len() != path.edges.len() + 1
    {
        return Err(Error::InvalidDomain);
    }
    if radius.is_zero() {
        return Ok((PathPoint::Vertex(target), Vec::new()));
    }
    let mut traversed = ratio(0, 1)?;
    let mut segments = Vec::new();
    for index in (0..path.edges.len()).rev() {
        let edge_id = path.edges[index];
        let edge = graph.edge(edge_id).ok_or(Error::InvalidDomain)?;
        let from = path.vertices[index + 1];
        let toward_center = path.vertices[index];
        let remaining = radius
            .checked_sub(&traversed)
            .map_err(|_| Error::Overflow)?;
        if !remaining.is_positive() {
            break;
        }
        let halved_length = if edge
            .length
            .at_least(&remaining)
            .map_err(|_| Error::Overflow)?
        {
            remaining
        } else {
            edge.length.clone()
        };
        segments.push(HighwaySegment {
            edge: edge_id,
            from,
            toward_center,
            halved_length: halved_length.clone(),
            original_edge_length: edge.length.clone(),
        });
        traversed = traversed
            .checked_add(&halved_length)
            .map_err(|_| Error::Overflow)?;
        if traversed == radius {
            let portal = if halved_length == edge.length {
                PathPoint::Vertex(toward_center)
            } else {
                PathPoint::EdgeInterior {
                    edge: edge_id,
                    from,
                    toward_center,
                    offset_from: halved_length,
                }
            };
            return Ok((portal, segments));
        }
    }
    Err(Error::InvalidRadius)
}

#[allow(clippy::too_many_lines)]
pub(super) fn directed_petal_distances(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    center_distances: &[Option<ExactRatio>],
    highway: &[HighwaySegment],
    metrics: &mut PetalMetrics,
) -> Result<Vec<Option<ExactRatio>>, Error> {
    let mut distances: Vec<Option<ExactRatio>> = vec![None; graph.node_count()];
    let mut settled = vec![false; graph.node_count()];
    let mut adjacency = vec![Vec::<(FlowNodeId, ExactRatio, usize)>::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(String, String), usize>::new();
    for edge_index in 0..graph.edge_count() {
        metrics.directed_edge_scans = checked_metric_sum(metrics.directed_edge_scans, 1)?;
        let edge_id = SourceEdgeId(edge_index);
        let Some(edge) = graph.edge(edge_id) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let next_class = length_classes.len();
            let class = *length_classes
                .entry(ratio_key(&edge.length))
                .or_insert(next_class);
            adjacency[edge.first.0].push((edge.second, edge.length.clone(), class));
            adjacency[edge.second.0].push((edge.first, edge.length.clone(), class));
        }
    }

    // For every ordinary arc, Claim 15's reduced length is
    // l(u,v) + d(x,u) - d(x,v). Adding the fixed center potential to a
    // tentative label therefore leaves the original undirected edge length.
    // The halved highway is represented by source labels at its path points.
    let half = ratio(1, 2)?;
    let target_potential = center_distances[target.0]
        .clone()
        .ok_or(Error::Disconnected)?;
    let mut descending_highway_sources = vec![ExactHeapEntry {
        distance: target_potential,
        vertex: target,
    }];
    let mut portal_sources = Vec::new();
    let mut traversed = ratio(0, 1)?;
    for segment in highway {
        let edge = graph.edge(segment.edge).ok_or(Error::InvalidHighway)?;
        if edge.length != segment.original_edge_length
            || segment.halved_length.is_negative()
            || ratio_less(edge.length.clone(), segment.halved_length.clone())?
        {
            return Err(Error::InvalidHighway);
        }
        traversed = traversed
            .checked_add(&segment.halved_length)
            .map_err(|_| Error::Overflow)?;
        let highway_distance = traversed.checked_mul(&half).map_err(|_| Error::Overflow)?;
        if segment.halved_length == edge.length {
            let transformed = highway_distance
                .checked_add(
                    &center_distances[segment.toward_center.0]
                        .clone()
                        .ok_or(Error::Disconnected)?,
                )
                .map_err(|_| Error::Overflow)?;
            descending_highway_sources.push(ExactHeapEntry {
                distance: transformed,
                vertex: segment.toward_center,
            });
        } else {
            let portal_potential = center_distances[segment.from.0]
                .clone()
                .ok_or(Error::Disconnected)?
                .checked_sub(&segment.halved_length)
                .map_err(|_| Error::Overflow)?;
            let portal_label = highway_distance
                .checked_add(&portal_potential)
                .map_err(|_| Error::Overflow)?;
            portal_sources.push(ExactHeapEntry {
                distance: portal_label
                    .checked_add(&segment.halved_length)
                    .map_err(|_| Error::Overflow)?,
                vertex: segment.from,
            });
            portal_sources.push(ExactHeapEntry {
                distance: portal_label
                    .checked_add(
                        &edge
                            .length
                            .checked_sub(&segment.halved_length)
                            .map_err(|_| Error::Overflow)?,
                    )
                    .map_err(|_| Error::Overflow)?,
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
        let improves = match &distances[source.vertex.0] {
            Some(old) => ratio_less(source.distance.clone(), old.clone())?,
            None => true,
        };
        if improves {
            distances[source.vertex.0] = Some(source.distance.clone());
        }
    }
    let source_class = length_classes.len();
    let mut queue =
        DistinctLengthQueue::new(source_class.checked_add(1).ok_or(Error::Overflow)?, metrics)?;
    for source in descending_highway_sources {
        queue.push(source_class, source, metrics)?;
    }
    while let Some(entry) = queue.pop(metrics)? {
        if settled[entry.vertex.0] || distances[entry.vertex.0] != Some(entry.distance.clone()) {
            continue;
        }
        settled[entry.vertex.0] = true;
        for (other, directed_length, class) in &adjacency[entry.vertex.0] {
            metrics.directed_edge_scans = metrics
                .directed_edge_scans
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            if settled[other.0] {
                continue;
            }
            metrics.edge_relaxations = metrics
                .edge_relaxations
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            let candidate = entry
                .distance
                .checked_add(directed_length)
                .map_err(|_| Error::Overflow)?;
            let improves = match &distances[other.0] {
                None => true,
                Some(old) => ratio_less(candidate.clone(), old.clone())?,
            };
            if improves {
                distances[other.0] = Some(candidate.clone());
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
        .ok_or(Error::Overflow)?;
    metrics.directed_region_runs = metrics
        .directed_region_runs
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(Error::Disconnected);
    }
    for vertex in allowed {
        distances[vertex.0] = Some(
            distances[vertex.0]
                .clone()
                .ok_or(Error::Disconnected)?
                .checked_sub(
                    &center_distances[vertex.0]
                        .clone()
                        .ok_or(Error::Disconnected)?,
                )
                .map_err(|_| Error::Overflow)?,
        );
    }
    Ok(distances)
}

pub(super) fn reduced_directed_length(
    edge_id: SourceEdgeId,
    from: FlowNodeId,
    to: FlowNodeId,
    edge_length: ExactRatio,
    center_distances: &[Option<ExactRatio>],
    highway: &[HighwaySegment],
) -> Result<ExactRatio, Error> {
    if let Some(segment) = highway
        .iter()
        .find(|segment| segment.edge == edge_id && segment.from == from)
    {
        if segment.toward_center != to {
            return Err(Error::InvalidHighway);
        }
        let unhalved = edge_length
            .checked_sub(&segment.halved_length)
            .map_err(|_| Error::Overflow)?;
        return segment
            .halved_length
            .checked_mul(&ratio(1, 2)?)
            .and_then(|value| {
                unhalved
                    .checked_mul_integer(2)
                    .and_then(|remainder| value.checked_add(&remainder))
            })
            .map_err(|_| Error::Overflow);
    }
    let from_distance = center_distances[from.0]
        .clone()
        .ok_or(Error::Disconnected)?;
    let to_distance = center_distances[to.0].clone().ok_or(Error::Disconnected)?;
    let reduced = edge_length
        .checked_sub(
            &to_distance
                .checked_sub(&from_distance)
                .map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::Overflow)?;
    if reduced.is_negative() {
        return Err(Error::InvalidHighway);
    }
    Ok(reduced)
}

#[cfg(test)]
pub(super) fn directed_petal_distances_oracle(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    center_distances: &[Option<ExactRatio>],
    highway: &[HighwaySegment],
) -> Result<Vec<Option<ExactRatio>>, Error> {
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
                    let candidate = distances[vertex.0].clone().ok_or(Error::Disconnected)?;
                    let old_distance = distances[old].clone().ok_or(Error::Disconnected)?;
                    ratio_less(candidate.clone(), old_distance.clone())?
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
                edge.length.clone(),
                center_distances,
                highway,
            )?;
            let candidate = distances[node]
                .clone()
                .ok_or(Error::Disconnected)?
                .checked_add(&length)
                .map_err(|_| Error::Overflow)?;
            let improves = match &distances[other.0] {
                Some(old) => ratio_less(candidate.clone(), old.clone())?,
                None => true,
            };
            if improves {
                distances[other.0] = Some(candidate);
            }
        }
    }
    Ok(distances)
}

pub(super) fn membership_thresholds(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    path: &[FlowNodeId],
    from_center: &ShortestPaths,
    target: FlowNodeId,
    budget: ExactRatio,
    metrics: &mut PetalMetrics,
) -> Result<MembershipThresholds, Error> {
    let mut by_vertex = vec![None; graph.node_count()];
    let mut path_distance_from_target = vec![None; graph.node_count()];
    let target_position = path
        .iter()
        .position(|vertex| *vertex == target)
        .ok_or(Error::InvalidDomain)?;
    for (position, point) in path.iter().enumerate().take(target_position + 1) {
        let point_paths = shortest_paths(graph, remaining, *point, metrics)?;
        let distance_from_target = ratio(
            i128::try_from(target_position - position).map_err(|_| Error::Overflow)?,
            1,
        )?;
        path_distance_from_target[point.0] = Some(distance_from_target.clone());
        if ratio_less(budget.clone(), distance_from_target.clone())? {
            continue;
        }
        let center_to_point = from_center.distances[point.0]
            .clone()
            .ok_or(Error::Disconnected)?;
        for vertex in remaining {
            let point_to_vertex = point_paths.distances[vertex.0]
                .clone()
                .ok_or(Error::Disconnected)?;
            let center_to_vertex = from_center.distances[vertex.0]
                .clone()
                .ok_or(Error::Disconnected)?;
            let excess = center_to_point
                .checked_add(&point_to_vertex)
                .and_then(|value| value.checked_sub(&center_to_vertex))
                .map_err(|_| Error::Overflow)?;
            if excess.is_negative() {
                return Err(Error::InvalidDomain);
            }
            let threshold = distance_from_target
                .checked_add(&excess.checked_mul_integer(2).map_err(|_| Error::Overflow)?)
                .map_err(|_| Error::Overflow)?;
            match &by_vertex[vertex.0] {
                None => by_vertex[vertex.0] = Some(threshold),
                Some(old) if ratio_less(threshold.clone(), old.clone())? => {
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
pub(super) struct ExactHeapEntry {
    pub(super) distance: ExactRatio,
    pub(super) vertex: FlowNodeId,
}

#[derive(Clone)]
pub(super) struct MonotoneFront {
    entry: ExactHeapEntry,
    class: usize,
}

pub(super) struct DistinctLengthQueue {
    queues: Vec<VecDeque<ExactHeapEntry>>,
    fronts: Vec<MonotoneFront>,
}

pub(super) type ClassifiedAdjacency = Vec<Vec<(FlowNodeId, ExactRatio, usize)>>;

impl DistinctLengthQueue {
    fn new(class_count: usize, metrics: &mut PetalMetrics) -> Result<Self, Error> {
        metrics.maximum_length_classes = metrics
            .maximum_length_classes
            .max(u64::try_from(class_count).map_err(|_| Error::Overflow)?);
        Ok(Self {
            queues: vec![VecDeque::new(); class_count],
            fronts: Vec::new(),
        })
    }

    fn push(
        &mut self,
        class: usize,
        entry: ExactHeapEntry,
        metrics: &mut PetalMetrics,
    ) -> Result<(), Error> {
        let queue = self
            .queues
            .get_mut(class)
            .ok_or(Error::InvalidWorkCertificate)?;
        if let Some(back) = queue.back()
            && ratio_less(entry.distance.clone(), back.distance.clone())?
        {
            return Err(Error::InvalidWorkCertificate);
        }
        let was_empty = queue.is_empty();
        queue.push_back(entry.clone());
        metrics.monotone_queue_pushes = checked_metric_sum(metrics.monotone_queue_pushes, 1)?;
        if was_empty {
            monotone_front_push(&mut self.fronts, MonotoneFront { entry, class }, metrics)?;
        }
        Ok(())
    }

    fn pop(&mut self, metrics: &mut PetalMetrics) -> Result<Option<ExactHeapEntry>, Error> {
        let Some(front) = monotone_front_pop(&mut self.fronts, metrics)? else {
            return Ok(None);
        };
        let queue = self
            .queues
            .get_mut(front.class)
            .ok_or(Error::InvalidWorkCertificate)?;
        if queue.front() != Some(&front.entry) {
            return Err(Error::InvalidWorkCertificate);
        }
        let result = queue.pop_front().ok_or(Error::InvalidWorkCertificate)?;
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

pub(super) fn fast_weighted_membership_thresholds(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    path: &RecoveredPath,
    center_distances: &[Option<ExactRatio>],
    maximum_radius: ExactRatio,
    metrics: &mut PetalMetrics,
) -> Result<MembershipThresholds, Error> {
    let mut thresholds = MembershipThresholds {
        by_vertex: vec![None; graph.node_count()],
        path_distance_from_target: vec![None; graph.node_count()],
        ordered_events: None,
    };
    let target_distance = center_distances[target.0]
        .clone()
        .ok_or(Error::Disconnected)?;
    let mut labels = vec![None; graph.node_count()];
    let mut sources = Vec::new();
    let mut distance_from_target = ratio(0, 1)?;
    add_membership_source(target, ratio(0, 1)?, &mut labels, &mut sources, metrics)?;
    thresholds.path_distance_from_target[target.0] = Some(distance_from_target.clone());
    for path_index in (0..path.edges.len()).rev() {
        let edge = graph
            .edge(path.edges[path_index])
            .ok_or(Error::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let next_distance = distance_from_target
            .checked_add(&edge.length)
            .map_err(|_| Error::Overflow)?;
        thresholds.path_distance_from_target[toward_center.0] = Some(next_distance.clone());
        if ratio_less(maximum_radius.clone(), next_distance.clone())? {
            if ratio_less(distance_from_target.clone(), maximum_radius.clone())? {
                add_interior_membership_source(
                    from,
                    toward_center,
                    edge.length.clone(),
                    maximum_radius
                        .checked_sub(&distance_from_target)
                        .map_err(|_| Error::Overflow)?,
                    target_distance,
                    maximum_radius.clone(),
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
            next_distance.clone(),
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
        let threshold = labels[vertex.0].clone().ok_or(Error::Disconnected)?;
        if threshold.is_negative() {
            return Err(Error::InvalidRadius);
        }
        if !ratio_less(maximum_radius.clone(), threshold.clone())? {
            thresholds.by_vertex[vertex.0] = Some(threshold);
        }
    }
    thresholds.ordered_events = Some(
        events
            .into_iter()
            .filter(|event| thresholds.by_vertex[event.vertex.0] == Some(event.distance.clone()))
            .collect(),
    );
    metrics.directed_region_runs = metrics
        .directed_region_runs
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    Ok(thresholds)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn add_interior_membership_source(
    from: FlowNodeId,
    toward_center: FlowNodeId,
    edge_length: ExactRatio,
    offset_from: ExactRatio,
    target_distance: ExactRatio,
    radius: ExactRatio,
    center_distances: &[Option<ExactRatio>],
    labels: &mut [Option<ExactRatio>],
    sources: &mut Vec<ExactHeapEntry>,
    metrics: &mut PetalMetrics,
) -> Result<(), Error> {
    let two = ratio(2, 1)?;
    let potential = target_distance
        .checked_mul(&two)
        .and_then(|value| value.checked_sub(&radius))
        .map_err(|_| Error::Overflow)?;
    let from_label = potential
        .checked_add(&offset_from.checked_mul(&two).map_err(|_| Error::Overflow)?)
        .map_err(|_| Error::Overflow)?;
    let toward_label = potential
        .checked_add(
            &edge_length
                .checked_sub(&offset_from)
                .and_then(|value| value.checked_mul(&two))
                .map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::Overflow)?;
    let from_threshold = from_label
        .checked_sub(
            &center_distances[from.0]
                .clone()
                .ok_or(Error::Disconnected)?
                .checked_mul(&two)
                .map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::Overflow)?;
    let toward_threshold = toward_label
        .checked_sub(
            &center_distances[toward_center.0]
                .clone()
                .ok_or(Error::Disconnected)?
                .checked_mul(&two)
                .map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::Overflow)?;
    add_membership_source(from, from_threshold, labels, sources, metrics)?;
    add_membership_source(toward_center, toward_threshold, labels, sources, metrics)?;
    metrics.membership_sources = metrics
        .membership_sources
        .checked_sub(1)
        .ok_or(Error::Overflow)?;
    Ok(())
}

pub(super) fn add_membership_source(
    vertex: FlowNodeId,
    distance: ExactRatio,
    labels: &mut [Option<ExactRatio>],
    sources: &mut Vec<ExactHeapEntry>,
    metrics: &mut PetalMetrics,
) -> Result<(), Error> {
    let improves = match &labels[vertex.0] {
        Some(old) => ratio_less(distance.clone(), old.clone())?,
        None => true,
    };
    if improves {
        labels[vertex.0] = Some(distance.clone());
        sources.push(ExactHeapEntry { distance, vertex });
    }
    metrics.membership_sources = metrics
        .membership_sources
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    Ok(())
}

pub(super) fn weighted_adjacency(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    center_distances: &[Option<ExactRatio>],
    metrics: &mut PetalMetrics,
) -> Result<(ClassifiedAdjacency, usize), Error> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(String, String), usize>::new();
    for index in 0..graph.edge_count() {
        metrics.directed_edge_scans = checked_metric_sum(metrics.directed_edge_scans, 1)?;
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let first_distance = center_distances[edge.first.0]
                .clone()
                .ok_or(Error::Disconnected)?;
            let second_distance = center_distances[edge.second.0]
                .clone()
                .ok_or(Error::Disconnected)?;
            let forward = edge
                .length
                .checked_add(&first_distance)
                .and_then(|value| value.checked_sub(&second_distance))
                .and_then(|value| value.checked_mul_integer(2))
                .map_err(|_| Error::Overflow)?;
            let reverse = edge
                .length
                .checked_add(&second_distance)
                .and_then(|value| value.checked_sub(&first_distance))
                .and_then(|value| value.checked_mul_integer(2))
                .map_err(|_| Error::Overflow)?;
            if forward.is_negative() || reverse.is_negative() {
                return Err(Error::InvalidHighway);
            }
            let next_forward = length_classes.len();
            let forward_class = *length_classes
                .entry(ratio_key(&forward))
                .or_insert(next_forward);
            let next_reverse = length_classes.len();
            let reverse_class = *length_classes
                .entry(ratio_key(&reverse))
                .or_insert(next_reverse);
            adjacency[edge.first.0].push((edge.second, forward, forward_class));
            adjacency[edge.second.0].push((edge.first, reverse, reverse_class));
        }
    }
    Ok((adjacency, length_classes.len()))
}

#[cfg(test)]
pub(super) fn transformed_weighted_adjacency(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    metrics: &mut PetalMetrics,
) -> Result<(ClassifiedAdjacency, usize), Error> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    let mut length_classes = BTreeMap::<(String, String), usize>::new();
    for index in 0..graph.edge_count() {
        metrics.directed_edge_scans = checked_metric_sum(metrics.directed_edge_scans, 1)?;
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        if allowed.contains(&edge.first) && allowed.contains(&edge.second) {
            let length = edge
                .length
                .checked_mul_integer(2)
                .map_err(|_| Error::Overflow)?;
            let next_class = length_classes.len();
            let class = *length_classes
                .entry(ratio_key(&length))
                .or_insert(next_class);
            adjacency[edge.first.0].push((edge.second, length.clone(), class));
            adjacency[edge.second.0].push((edge.first, length, class));
        }
    }
    Ok((adjacency, length_classes.len()))
}

#[cfg(test)]
#[allow(clippy::too_many_lines)]
pub(super) fn transformed_weighted_membership_thresholds_oracle(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    path: &RecoveredPath,
    center_distances: &[Option<ExactRatio>],
    maximum_radius: ExactRatio,
    metrics: &mut PetalMetrics,
) -> Result<MembershipThresholds, Error> {
    let mut reduced_labels = vec![None; graph.node_count()];
    let mut reduced_sources = Vec::new();
    let target_distance = center_distances[target.0]
        .clone()
        .ok_or(Error::Disconnected)?;
    let mut path_distance_from_target = vec![None; graph.node_count()];
    let mut distance_from_target = ratio(0, 1)?;
    add_membership_source(
        target,
        distance_from_target.clone(),
        &mut reduced_labels,
        &mut reduced_sources,
        metrics,
    )?;
    path_distance_from_target[target.0] = Some(distance_from_target.clone());
    for path_index in (0..path.edges.len()).rev() {
        let edge = graph
            .edge(path.edges[path_index])
            .ok_or(Error::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let next_distance = distance_from_target
            .checked_add(&edge.length)
            .map_err(|_| Error::Overflow)?;
        path_distance_from_target[toward_center.0] = Some(next_distance.clone());
        if ratio_less(maximum_radius.clone(), next_distance.clone())? {
            if ratio_less(distance_from_target.clone(), maximum_radius.clone())? {
                add_interior_membership_source(
                    from,
                    toward_center,
                    edge.length.clone(),
                    maximum_radius
                        .checked_sub(&distance_from_target)
                        .map_err(|_| Error::Overflow)?,
                    target_distance.clone(),
                    maximum_radius.clone(),
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
            next_distance.clone(),
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
    let mut transformed_labels: Vec<Option<ExactRatio>> = vec![None; graph.node_count()];
    let mut transformed_sources = Vec::with_capacity(reduced_sources.len());
    for source in reduced_sources {
        let potential = center_distances[source.vertex.0]
            .clone()
            .ok_or(Error::Disconnected)?
            .checked_mul(&two)
            .map_err(|_| Error::Overflow)?;
        let transformed = source
            .distance
            .checked_add(&potential)
            .map_err(|_| Error::Overflow)?;
        let improves = match &transformed_labels[source.vertex.0] {
            Some(old) => ratio_less(transformed.clone(), old.clone())?,
            None => true,
        };
        if improves {
            transformed_labels[source.vertex.0] = Some(transformed.clone());
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
            .clone()
            .ok_or(Error::Disconnected)?
            .checked_mul(&two)
            .map_err(|_| Error::Overflow)?;
        let threshold = transformed_labels[vertex.0]
            .clone()
            .ok_or(Error::Disconnected)?
            .checked_sub(&potential)
            .map_err(|_| Error::Overflow)?;
        if !ratio_less(maximum_radius.clone(), threshold.clone())? {
            by_vertex[vertex.0] = Some(threshold);
        }
    }
    Ok(MembershipThresholds {
        by_vertex,
        path_distance_from_target,
        ordered_events: None,
    })
}

pub(super) fn exact_multi_source_dijkstra(
    adjacency: &[Vec<(FlowNodeId, ExactRatio, usize)>],
    length_classes: usize,
    distances: &mut [Option<ExactRatio>],
    sources: &[ExactHeapEntry],
    metrics: &mut PetalMetrics,
) -> Result<Vec<ExactHeapEntry>, Error> {
    let source_class = length_classes;
    let mut queue =
        DistinctLengthQueue::new(source_class.checked_add(1).ok_or(Error::Overflow)?, metrics)?;
    for source in sources {
        queue.push(source_class, source.clone(), metrics)?;
    }
    let mut settled = vec![false; distances.len()];
    let mut events = Vec::new();
    while let Some(entry) = queue.pop(metrics)? {
        if settled[entry.vertex.0] || distances[entry.vertex.0] != Some(entry.distance.clone()) {
            continue;
        }
        settled[entry.vertex.0] = true;
        events.push(entry.clone());
        for (other, length, class) in &adjacency[entry.vertex.0] {
            metrics.directed_edge_scans = metrics
                .directed_edge_scans
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            let candidate = entry
                .distance
                .checked_add(length)
                .map_err(|_| Error::Overflow)?;
            if settled[other.0] {
                continue;
            }
            let improves = match &distances[other.0] {
                Some(old) => ratio_less(candidate.clone(), old.clone())?,
                None => true,
            };
            if improves {
                distances[other.0] = Some(candidate.clone());
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

pub(super) fn event_heap_push(
    heap: &mut Vec<ExactHeapEntry>,
    entry: ExactHeapEntry,
    metrics: &mut PetalMetrics,
) -> Result<(), Error> {
    heap_push(heap, entry, &mut metrics.heap_comparisons)?;
    metrics.event_heap_pushes = checked_metric_sum(metrics.event_heap_pushes, 1)?;
    Ok(())
}

pub(super) fn monotone_front_push(
    heap: &mut Vec<MonotoneFront>,
    entry: MonotoneFront,
    metrics: &mut PetalMetrics,
) -> Result<(), Error> {
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

pub(super) fn monotone_front_pop(
    heap: &mut Vec<MonotoneFront>,
    metrics: &mut PetalMetrics,
) -> Result<Option<MonotoneFront>, Error> {
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

pub(super) fn monotone_front_less(
    first: &MonotoneFront,
    second: &MonotoneFront,
) -> Result<bool, Error> {
    Ok(exact_heap_entry_less(&first.entry, &second.entry)?
        || (first.entry == second.entry && first.class < second.class))
}

pub(super) fn heap_push(
    heap: &mut Vec<ExactHeapEntry>,
    entry: ExactHeapEntry,
    comparisons: &mut u64,
) -> Result<(), Error> {
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

pub(super) fn event_heap_pop(
    heap: &mut Vec<ExactHeapEntry>,
    metrics: &mut PetalMetrics,
) -> Result<Option<ExactHeapEntry>, Error> {
    let result = heap_pop(heap, &mut metrics.heap_comparisons)?;
    if result.is_some() {
        metrics.event_heap_pops = checked_metric_sum(metrics.event_heap_pops, 1)?;
    }
    Ok(result)
}

pub(super) fn heap_pop(
    heap: &mut Vec<ExactHeapEntry>,
    comparisons: &mut u64,
) -> Result<Option<ExactHeapEntry>, Error> {
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

pub(super) fn exact_heap_entry_less(
    first: &ExactHeapEntry,
    second: &ExactHeapEntry,
) -> Result<bool, Error> {
    Ok(ratio_less(first.distance.clone(), second.distance.clone())?
        || (first.distance == second.distance && first.vertex < second.vertex))
}

pub(super) fn weighted_membership_thresholds_oracle(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    path: &RecoveredPath,
    center_distances: &[Option<ExactRatio>],
    maximum_radius: ExactRatio,
    metrics: &mut PetalMetrics,
) -> Result<MembershipThresholds, Error> {
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
        if !ratio_less(interval_start.clone(), maximum_radius.clone())? {
            break;
        }
        let edge_id = path.edges[path_index];
        let edge = graph.edge(edge_id).ok_or(Error::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let full_end = interval_start
            .checked_add(&edge.length)
            .map_err(|_| Error::Overflow)?;
        let interval_end = if ratio_less(maximum_radius.clone(), full_end.clone())? {
            maximum_radius.clone()
        } else {
            full_end.clone()
        };
        thresholds.path_distance_from_target[toward_center.0] = Some(full_end.clone());
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
        let source_to_from = without_current[from.0].clone().ok_or(Error::Disconnected)?;
        let three_halves = ratio(3, 2)?;
        let one_half = ratio(1, 2)?;
        let current_constant = edge
            .length
            .checked_mul_integer(2)
            .and_then(|value| {
                interval_start
                    .checked_mul(&three_halves)
                    .and_then(|offset| value.checked_add(&offset))
            })
            .map_err(|_| Error::Overflow)?;
        for vertex in remaining {
            if let Some(distance) = &without_current[vertex.0] {
                let entry = max_ratio(
                    interval_start.clone(),
                    distance
                        .checked_mul_integer(2)
                        .map_err(|_| Error::Overflow)?,
                )?;
                if !ratio_less(interval_end.clone(), entry.clone())? {
                    record_threshold(&mut thresholds.by_vertex[vertex.0], entry)?;
                }
            }
            if let Some(suffix) = &from_toward[vertex.0] {
                let entry = source_to_from
                    .checked_add(&current_constant)
                    .and_then(|value| value.checked_add(suffix))
                    .and_then(|value| value.checked_mul(&one_half))
                    .map_err(|_| Error::Overflow)?;
                let entry = max_ratio(interval_start.clone(), entry)?;
                if !ratio_less(interval_end.clone(), entry.clone())? {
                    record_threshold(&mut thresholds.by_vertex[vertex.0], entry)?;
                }
            }
        }
        if interval_end != full_end {
            break;
        }
        fully_halved.push(HighwaySegment {
            edge: edge_id,
            from,
            toward_center,
            halved_length: edge.length.clone(),
            original_edge_length: edge.length.clone(),
        });
        interval_start = full_end;
    }
    Ok(thresholds)
}

pub(super) fn constant_directed_distances(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    center_distances: &[Option<ExactRatio>],
    fully_halved: &[HighwaySegment],
    removed: Option<(SourceEdgeId, FlowNodeId, FlowNodeId)>,
    metrics: &mut PetalMetrics,
) -> Result<Vec<Option<ExactRatio>>, Error> {
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
                .ok_or(Error::Overflow)?;
            let directed_length = reduced_directed_length(
                edge_id,
                FlowNodeId(node),
                FlowNodeId(other),
                edge.length.clone(),
                center_distances,
                fully_halved,
            )?;
            let candidate = distances[node]
                .clone()
                .ok_or(Error::Disconnected)?
                .checked_add(&directed_length)
                .map_err(|_| Error::Overflow)?;
            let mut key = path_keys[node].as_ref().ok_or(Error::Disconnected)?.clone();
            key.push(edge_id);
            let improves = match &distances[other] {
                None => true,
                Some(old) => {
                    ratio_less(candidate.clone(), old.clone())?
                        || (&candidate == old
                            && key < *path_keys[other].as_ref().ok_or(Error::Disconnected)?)
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
        .ok_or(Error::Overflow)?;
    Ok(distances)
}

pub(super) fn record_threshold(
    current: &mut Option<ExactRatio>,
    candidate: ExactRatio,
) -> Result<(), Error> {
    let replace = match current.as_ref() {
        None => true,
        Some(old) => ratio_less(candidate.clone(), old.clone())?,
    };
    if replace {
        *current = Some(candidate);
    }
    Ok(())
}

pub(super) fn max_ratio(first: ExactRatio, second: ExactRatio) -> Result<ExactRatio, Error> {
    if ratio_less(first.clone(), second.clone())? {
        Ok(second)
    } else {
        Ok(first)
    }
}

pub(super) fn vertices_at_radius(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    radius: ExactRatio,
) -> Result<BTreeSet<FlowNodeId>, Error> {
    let mut result = BTreeSet::new();
    for vertex in remaining {
        if let Some(threshold) = &thresholds.by_vertex[vertex.0]
            && radius.at_least(threshold).map_err(|_| Error::Overflow)?
        {
            result.insert(*vertex);
        }
    }
    Ok(result)
}

pub(super) fn window_radius(
    budget: ExactRatio,
    index: usize,
    levels: usize,
    upper: bool,
) -> Result<ExactRatio, Error> {
    let numerator = if upper {
        levels + index
    } else {
        levels + index - 1
    };
    let denominator = i128::try_from(levels)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .ok_or(Error::Overflow)?;
    budget
        .checked_mul(&ratio(
            i128::try_from(numerator).map_err(|_| Error::Overflow)?,
            denominator,
        )?)
        .map_err(|_| Error::Overflow)
}

pub(super) fn certify_window_condition(
    active_edges: usize,
    cluster_edges: usize,
    petal_edges: usize,
    index: usize,
    levels: usize,
) -> Result<bool, Error> {
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
            Ok(None) | Err(Error::InsufficientPrecision) => {}
            Err(error) => return Err(error),
        }
    }
    Err(Error::InsufficientPrecision)
}

pub(super) fn try_window_comparison(
    active_edges: usize,
    cluster_edges: usize,
    petal_edges: usize,
    index: usize,
    levels: usize,
    precision: u32,
) -> Result<Option<bool>, Error> {
    let mut arithmetic = fixed_arithmetic(active_edges, cluster_edges, precision)?;
    let two = arithmetic
        .enclose_ratio(2, 1)
        .map_err(|_| Error::InsufficientPrecision)?;
    let log_two = arithmetic
        .logarithm(&two)
        .map_err(|_| Error::InsufficientPrecision)?;
    let m = arithmetic
        .enclose_ratio(
            i128::try_from(active_edges).map_err(|_| Error::Overflow)?,
            1,
        )
        .map_err(|_| Error::InsufficientPrecision)?;
    let ln_edge_count = arithmetic
        .logarithm(&m)
        .map_err(|_| Error::InsufficientPrecision)?;
    let binary_edge_logarithm = arithmetic
        .divide_intervals(&ln_edge_count, &log_two)
        .map_err(|_| Error::InsufficientPrecision)?;
    let iterated_logarithm = arithmetic
        .logarithm(&binary_edge_logarithm)
        .map_err(|_| Error::InsufficientPrecision)?;
    let alpha = arithmetic
        .enclose_ratio(
            i128::try_from(levels - index).map_err(|_| Error::Overflow)?,
            i128::try_from(levels).map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::InsufficientPrecision)?;
    let exponent_input = arithmetic
        .multiply_intervals(&alpha, &iterated_logarithm)
        .map_err(|_| Error::InsufficientPrecision)?;
    let exponent = arithmetic
        .exponential(&exponent_input)
        .map_err(|_| Error::InsufficientPrecision)?;
    let right = arithmetic
        .multiply_intervals(&exponent, &log_two)
        .map_err(|_| Error::InsufficientPrecision)?;
    let left_ratio_numerator = i128::try_from(cluster_edges)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .ok_or(Error::Overflow)?;
    let left_input = arithmetic
        .enclose_ratio(
            left_ratio_numerator,
            i128::try_from(petal_edges).map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::InsufficientPrecision)?;
    let left = arithmetic
        .logarithm(&left_input)
        .map_err(|_| Error::InsufficientPrecision)?;
    if left.lower_scaled() >= right.upper_scaled() {
        Ok(Some(true))
    } else if left.upper_scaled() < right.lower_scaled() {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

pub(super) fn certify_stopping_condition(
    cluster_edges: usize,
    start_edges: usize,
    petal_edges: usize,
    boundary_cost: ExactRatio,
    levels: usize,
    budget: ExactRatio,
) -> Result<bool, Error> {
    if boundary_cost.is_zero() {
        return Ok(true);
    }
    if petal_edges == 0 || start_edges == 0 || start_edges >= cluster_edges {
        return Err(Error::InvalidRadius);
    }
    let denominator = i128::try_from(petal_edges)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| value.checked_mul(i128::try_from(levels).ok()?))
        .ok_or(Error::Overflow)?;
    let exact_left = budget
        .checked_mul(&boundary_cost)
        .map_err(|_| Error::Overflow)?
        .checked_mul(&ratio(1, denominator)?)
        .map_err(|_| Error::Overflow)?;
    for precision in [48_u32, 96, 192, 384] {
        match try_stopping_comparison(
            cluster_edges,
            start_edges,
            petal_edges,
            exact_left.clone(),
            precision,
        ) {
            Ok(Some(result)) => return Ok(result),
            Ok(None) | Err(Error::InsufficientPrecision) => {}
            Err(error) => return Err(error),
        }
    }
    Err(Error::InsufficientPrecision)
}

pub(super) fn try_stopping_comparison(
    cluster_edges: usize,
    start_edges: usize,
    petal_edges: usize,
    exact_left: ExactRatio,
    precision: u32,
) -> Result<Option<bool>, Error> {
    let mut arithmetic = fixed_arithmetic(cluster_edges, petal_edges, precision)?;
    let chi = arithmetic
        .enclose_ratio(
            i128::try_from(cluster_edges).map_err(|_| Error::Overflow)?,
            i128::try_from(start_edges).map_err(|_| Error::Overflow)?,
        )
        .map_err(|_| Error::InsufficientPrecision)?;
    let log_chi = arithmetic
        .logarithm(&chi)
        .map_err(|_| Error::InsufficientPrecision)?;
    let left = arithmetic
        .enclose_big_ratio(exact_left.numerator(), exact_left.denominator())
        .map_err(|_| Error::InsufficientPrecision)?;
    if left.upper_scaled() < log_chi.lower_scaled() {
        Ok(Some(true))
    } else if left.lower_scaled() >= log_chi.upper_scaled() {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

pub(super) fn fixed_arithmetic(
    first_size: usize,
    second_size: usize,
    precision: u32,
) -> Result<CertifiedFixedPoint, Error> {
    let input_bits = u64::try_from(first_size)
        .ok()
        .and_then(|value| value.checked_add(u64::try_from(second_size).ok()?))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_mul(256))
        .ok_or(Error::Overflow)?;
    let terms = precision.checked_mul(2).ok_or(Error::Overflow)?;
    let config = FixedPointConfig::source_bounded(input_bits, precision, terms, 4)
        .map_err(|_| Error::InsufficientPrecision)?;
    CertifiedFixedPoint::new(config).map_err(|_| Error::InsufficientPrecision)
}

pub(super) fn next_radius_event(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    current: ExactRatio,
    limit: ExactRatio,
) -> Result<Option<ExactRatio>, Error> {
    let mut next = None;
    for vertex in remaining {
        let Some(ref candidate) = thresholds.by_vertex[vertex.0] else {
            continue;
        };
        if !ratio_less(current.clone(), candidate.clone())?
            || ratio_less(limit.clone(), candidate.clone())?
        {
            continue;
        }
        match next {
            None => next = Some(candidate),
            Some(old) if ratio_less(candidate.clone(), old.clone())? => next = Some(candidate),
            Some(_) => {}
        }
    }
    Ok(next.cloned())
}

pub(super) fn internal_edge_count(
    graph: &SourceDynamicGraph,
    vertices: &BTreeSet<FlowNodeId>,
) -> usize {
    (0..graph.edge_count())
        .filter_map(|index| graph.edge(SourceEdgeId(index)))
        .filter(|edge| vertices.contains(&edge.first) && vertices.contains(&edge.second))
        .count()
}

pub(super) fn boundary_edge_count(
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

pub(super) fn incident_edge_count(
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

pub(super) fn boundary_edge_cost(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    petal: &BTreeSet<FlowNodeId>,
) -> Result<ExactRatio, Error> {
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
                .checked_add(&edge.length.reciprocal().map_err(|_| Error::Overflow)?)
                .map_err(|_| Error::Overflow)?;
        }
    }
    Ok(cost)
}

pub(super) fn all_connected(connectivity: &mut DisjointSet, count: usize) -> bool {
    if count == 0 {
        return false;
    }
    let root = connectivity.find(0);
    (1..count).all(|vertex| connectivity.find(vertex) == root)
}

pub(super) fn all_cluster_connected(
    connectivity: &mut DisjointSet,
    cluster: &BTreeSet<FlowNodeId>,
) -> bool {
    let Some(first) = cluster.first() else {
        return false;
    };
    let root = connectivity.find(first.0);
    cluster
        .iter()
        .all(|vertex| connectivity.find(vertex.0) == root)
}

pub(super) fn intervals_overlap(
    first_start: ExactRatio,
    first_end: ExactRatio,
    second_start: ExactRatio,
    second_end: ExactRatio,
) -> Result<bool, Error> {
    Ok(ratio_less(first_start, second_end)? && ratio_less(second_start, first_end)?)
}

pub(super) fn sort_and_merge_touching(
    intervals: &mut Vec<projection::HalvedInterval>,
) -> Result<(), Error> {
    for index in 1..intervals.len() {
        let mut cursor = index;
        while cursor > 0
            && ratio_less(
                intervals[cursor].start_from_first.clone(),
                intervals[cursor - 1].start_from_first.clone(),
            )?
        {
            intervals.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
    let mut merged = Vec::<projection::HalvedInterval>::new();
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

pub(super) fn ceil_log_log(value: usize) -> usize {
    let log = usize::try_from(usize::BITS - value.saturating_sub(1).leading_zeros()).unwrap_or(1);
    usize::try_from(usize::BITS - log.saturating_sub(1).leading_zeros())
        .unwrap_or(1)
        .max(1)
}

pub(crate) fn round_length_to_power_of_two(
    length: ExactRatio,
    base: ExactRatio,
) -> Result<ExactRatio, Error> {
    let scaled = length
        .checked_mul(&base.reciprocal().map_err(|_| Error::Overflow)?)
        .map_err(|_| Error::Overflow)?;
    let mut power = ratio(1, 1)?;
    loop {
        let Ok(next) = power.checked_mul_integer(2) else {
            break;
        };
        if ratio_less(scaled.clone(), next.clone())? {
            break;
        }
        power = next;
    }
    base.checked_mul(&power).map_err(|_| Error::Overflow)
}

pub(super) fn ratio(numerator: i128, denominator: i128) -> Result<ExactRatio, Error> {
    ExactRatio::new(numerator, denominator).map_err(|_| Error::Overflow)
}

pub(super) fn count_ratio(value: usize) -> Result<ExactRatio, Error> {
    ratio(i128::try_from(value).map_err(|_| Error::Overflow)?, 1)
}

pub(super) fn checked_metric_sum(first: u64, second: u64) -> Result<u64, Error> {
    first.checked_add(second).ok_or(Error::Overflow)
}

pub(super) fn ratio_less(left: ExactRatio, right: ExactRatio) -> Result<bool, Error> {
    Ok(!left.at_least(&right).map_err(|_| Error::Overflow)?)
}

fn ratio_key(value: &ExactRatio) -> (String, String) {
    (
        value.numerator().to_string(),
        value.denominator().to_string(),
    )
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
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
    use super::UnweightedPetal;
    use super::hierarchy;
    use super::projection;
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
        hierarchy: &super::hierarchy::Lsst,
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
        hierarchy: &super::hierarchy::Lsst,
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
        hierarchy: &super::hierarchy::Lsst,
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
        hierarchy: &super::hierarchy::Lsst,
        graph: &SourceDynamicGraph,
    ) {
        let mutate = |field: fn(&mut super::hierarchy::Metrics) -> &mut u64| {
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

    fn assert_projection_charging_counts(audit: &super::projection::Audit, expected: [u64; 7]) {
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

    fn assert_projection_scan_counts(metrics: &super::hierarchy::Metrics, expected: [u64; 14]) {
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

    fn assert_workspace_scan_counts(metrics: &super::hierarchy::Metrics, expected: [u64; 5]) {
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
        let petal = UnweightedPetal::construct(
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
            UnweightedPetal::construct(
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
        let petal = UnweightedPetal::construct(
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
        let petal = UnweightedPetal::construct(
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
        use super::{HighwaySegment, PathPoint, WeightedPetalAtRadius};

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
        let petal = WeightedPetalAtRadius::construct(
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
            PathPoint::EdgeInterior {
                edge: crate::SourceEdgeId(1),
                from: FlowNodeId(2),
                toward_center: FlowNodeId(1),
                offset_from: ExactRatio::new(2, 1).unwrap(),
            }
        );
        assert_eq!(
            petal.highway_segments,
            vec![HighwaySegment {
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
            projection::ShortEdgeContraction::build(&graph, &vertices, FlowNodeId(0)).unwrap();
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
        use super::HighwaySegment;

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
        let mut ledger = projection::HighwayLedger::new(&graph);
        ledger
            .apply(
                &graph,
                &[HighwaySegment {
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
                &[HighwaySegment {
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
                    &[HighwaySegment {
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
        let graph = path_graph(2);
        let mut workspace = projection::Graph::from_source(&graph).unwrap();
        let cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let mut metrics = hierarchy::Metrics::default();
        let mut projection_audit = projection::Audit::new(graph.edge_count());
        hierarchy::halve_highway(
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
        hierarchy::halve_highway(
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
        use super::{WeightedPetal, WeightedPetalAtRadius};

        let graph = path_graph(10);
        let vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let cone_union = UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(3, 1).unwrap(),
        )
        .unwrap();
        let region_growing = WeightedPetalAtRadius::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            cone_union.radius.clone(),
        )
        .unwrap();
        assert_eq!(region_growing.vertices, cone_union.vertices);
        assert!(matches!(
            region_growing.portal,
            super::PathPoint::EdgeInterior { .. }
        ));
        let weighted_selector = WeightedPetal::construct(
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
        use super::{PathPoint, WeightedPetal};

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
        let petal = WeightedPetal::construct(
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
            PathPoint::EdgeInterior {
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
            PetalMetrics, fast_weighted_membership_thresholds, recover_path, shortest_paths,
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
        let mut setup_metrics = PetalMetrics::default();
        let center_paths =
            shortest_paths(&graph, &vertices, FlowNodeId(0), &mut setup_metrics).unwrap();
        let path = recover_path(FlowNodeId(0), FlowNodeId(2), &center_paths).unwrap();
        let maximum_radius = ExactRatio::new(2, 1).unwrap();
        let mut oracle_metrics = PetalMetrics::default();
        let oracle = weighted_membership_thresholds_oracle(
            &graph,
            &vertices,
            FlowNodeId(2),
            &path,
            &center_paths.distances,
            maximum_radius.clone(),
            &mut oracle_metrics,
        )
        .unwrap();
        let mut fast_metrics = PetalMetrics::default();
        let fast = fast_weighted_membership_thresholds(
            &graph,
            &vertices,
            FlowNodeId(2),
            &path,
            &center_paths.distances,
            maximum_radius.clone(),
            &mut fast_metrics,
        )
        .unwrap();
        let mut transformed_metrics = PetalMetrics::default();
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
            PetalMetrics, fast_weighted_membership_thresholds, recover_path, shortest_paths,
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
            let mut setup_metrics = PetalMetrics::default();
            let Ok(center_paths) =
                shortest_paths(&graph, &vertices, FlowNodeId(0), &mut setup_metrics)
            else {
                continue;
            };
            for target in 1..4 {
                let target = FlowNodeId(target);
                let path = recover_path(FlowNodeId(0), target, &center_paths).unwrap();
                let target_distance = center_paths.distances[target.0].clone().unwrap();
                for numerator in 1..=4 {
                    let maximum_radius = target_distance
                        .checked_mul(&ExactRatio::new(numerator, 4).unwrap())
                        .unwrap();
                    let mut oracle_metrics = PetalMetrics::default();
                    let oracle = weighted_membership_thresholds_oracle(
                        &graph,
                        &vertices,
                        target,
                        &path,
                        &center_paths.distances,
                        maximum_radius.clone(),
                        &mut oracle_metrics,
                    )
                    .unwrap();
                    let mut fast_metrics = PetalMetrics::default();
                    let fast = fast_weighted_membership_thresholds(
                        &graph,
                        &vertices,
                        target,
                        &path,
                        &center_paths.distances,
                        maximum_radius.clone(),
                        &mut fast_metrics,
                    )
                    .unwrap();
                    let mut transformed_metrics = PetalMetrics::default();
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
        use super::{PetalMetrics, fast_shortest_paths, shortest_paths};

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
                let mut oracle_metrics = PetalMetrics::default();
                let Ok(oracle) =
                    shortest_paths(&graph, &vertices, FlowNodeId(source), &mut oracle_metrics)
                else {
                    break;
                };
                let mut fast_metrics = PetalMetrics::default();
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
                            .clone()
                            .unwrap()
                            .checked_add(&edge.length)
                            .unwrap(),
                        fast.distances[vertex].clone().unwrap()
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
            PetalMetrics, directed_petal_distances, directed_petal_distances_oracle,
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
            let mut setup_metrics = PetalMetrics::default();
            let Ok(center_paths) =
                shortest_paths(&graph, &vertices, FlowNodeId(0), &mut setup_metrics)
            else {
                continue;
            };
            for target in 1..4 {
                let target = FlowNodeId(target);
                let path = recover_path(FlowNodeId(0), target, &center_paths).unwrap();
                let target_distance = center_paths.distances[target.0].clone().unwrap();
                for numerator in 1..=4 {
                    let radius = target_distance
                        .checked_mul(&ExactRatio::new(numerator, 4).unwrap())
                        .unwrap();
                    let (_, highway) =
                        locate_portal_and_highway(&graph, &path, target, radius).unwrap();
                    let mut metrics = PetalMetrics::default();
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
        use super::{UnweightedPetal, WeightedPetal};

        let unit = path_graph(10);
        let unit_vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let unit_oracle = UnweightedPetal::construct(
            &unit,
            &unit_vertices,
            &unit_vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(4, 1).unwrap(),
        )
        .unwrap();
        let unit_fast = WeightedPetal::construct_with_portal_volume(
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
        let weighted_oracle = WeightedPetal::construct_with_portal_volume(
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
        let weighted_fast = WeightedPetal::construct_with_portal_volume(
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
        let mut augmented = projection::Graph::from_source(&graph).unwrap();
        let (portal, from_edge, toward_edge) = augmented
            .split_edge(0, FlowNodeId(1), ExactRatio::new(1, 2).unwrap())
            .unwrap();

        assert_eq!(portal, FlowNodeId(3));
        assert_eq!(
            augmented.edges[from_edge].provenance,
            Some(projection::OriginalInterval {
                edge: crate::SourceEdgeId(0),
                first_position: ExactRatio::new(3, 2).unwrap(),
                second_position: ExactRatio::new(1, 1).unwrap(),
            })
        );
        assert_eq!(
            augmented.edges[toward_edge].provenance,
            Some(projection::OriginalInterval {
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
        let expected_label = projection::SymbolicLengthLabel {
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
        let graph = SourceDynamicGraph::new(
            8,
            vec![test_edge(0, 1, 1), test_edge(5, 6, 1), test_edge(6, 7, 1)],
            8,
        )
        .unwrap();
        let augmented = projection::Graph::from_source(&graph).unwrap();
        let cluster = BTreeSet::from([FlowNodeId(5), FlowNodeId(6), FlowNodeId(7)]);
        let mut metrics = hierarchy::Metrics::default();
        let mut audit = projection::Audit::new(graph.edge_count());
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
        let graph = SourceDynamicGraph::new(2, vec![test_edge(0, 1, 1)], 8).unwrap();
        let mut augmented = projection::Graph::from_source(&graph).unwrap();
        let mut cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let mut metrics = hierarchy::Metrics::default();
        let mut audit = projection::Audit::new(graph.edge_count());
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
        let graph = SourceDynamicGraph::new(2, vec![test_edge(0, 1, 1)], 8).unwrap();
        let mut augmented = projection::Graph::from_source(&graph).unwrap();
        let mut cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let mut metrics = hierarchy::Metrics::default();
        let mut audit = projection::Audit::new(graph.edge_count());
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
        let graph =
            SourceDynamicGraph::new(3, vec![test_edge(0, 1, 1), test_edge(1, 2, 1)], 8).unwrap();
        let source_label = projection::SymbolicLengthLabel {
            root_source: Some(SourceEdgeId(4)),
            unsplit_length: ExactRatio::new(8, 1).unwrap(),
            halved: true,
        };
        let virtual_label = projection::SymbolicLengthLabel {
            root_source: None,
            unsplit_length: ExactRatio::new(3, 1).unwrap(),
            halved: false,
        };
        let mismatched_label = projection::SymbolicLengthLabel {
            root_source: Some(SourceEdgeId(3)),
            ..source_label.clone()
        };
        assert!(
            projection::Graph::from_source_with_inherited_labels(
                &graph,
                hierarchy::LengthMode::ExactRational,
                &[Some(SourceEdgeId(4)), None],
                &[mismatched_label, virtual_label.clone()],
            )
            .is_err()
        );
        let mut augmented = projection::Graph::from_source_with_inherited_labels(
            &graph,
            hierarchy::LengthMode::ExactRational,
            &[Some(SourceEdgeId(4)), None],
            &[source_label.clone(), virtual_label.clone()],
        )
        .unwrap();
        let (portal, _, _) = augmented
            .split_edge(0, FlowNodeId(0), ExactRatio::new(1, 2).unwrap())
            .unwrap();
        let cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1), FlowNodeId(2), portal]);
        let mut metrics = hierarchy::Metrics::default();
        let mut audit = projection::Audit::new(5);
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
        let augmented = projection::Graph::from_source(&triangle).unwrap();
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
        let augmented = projection::Graph::from_source(&disconnected).unwrap();
        assert!(
            augmented
                .recover_original_tree(&BTreeSet::from([0, 1]))
                .is_err()
        );
    }

    #[test]
    fn hierarchical_base_case_matches_the_exact_tree_oracle() {
        use crate::source_lsf::oracle::Lsst;

        let graph = path_graph(5);
        let hierarchy = hierarchy::Lsst::construct(&graph, FlowNodeId(0)).unwrap();
        let oracle = Lsst::solve(&graph).unwrap();
        assert_eq!(hierarchy.tree_edges, oracle.edges);
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
        let graph = path_graph(500);
        let mut workspace = projection::Graph::from_source(&graph).unwrap();
        let cluster = (0..500).map(FlowNodeId).collect::<BTreeSet<_>>();
        let mut certificates = Vec::new();
        let mut metrics = hierarchy::Metrics::default();
        let mut projection_audit = projection::Audit::new(graph.edge_count());
        let selected = hierarchy::hierarchical_petal_decomposition(
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
        let graph = path_graph(500);
        let hierarchy = hierarchy::Lsst::construct(&graph, FlowNodeId(0)).unwrap();
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
        use crate::source_lsf::oracle::Lsst;

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
            let Ok(oracle) = Lsst::solve(&graph) else {
                continue;
            };
            let hierarchy = hierarchy::Lsst::construct(&graph, FlowNodeId(0)).unwrap();
            hierarchy.verify(&graph).unwrap();
            assert_eq!(hierarchy.total_weight, oracle.total_weight);
            assert!(
                hierarchy
                    .weighted_stretch
                    .at_least(&oracle.weighted_stretch)
                    .unwrap()
            );
            checked += 1;
        }
        assert_eq!(checked, 38);
    }

    #[test]
    fn weighted_hierarchies_differentiate_on_all_connected_four_node_graphs() {
        use crate::source_lsf::oracle::Lsst;

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
            let Ok(oracle) = Lsst::solve(&small) else {
                continue;
            };
            let large = make_graph(1_000);
            let small_hierarchy = hierarchy::Lsst::construct(&small, FlowNodeId(0)).unwrap();
            let large_hierarchy = hierarchy::Lsst::construct(&large, FlowNodeId(0)).unwrap();
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
                    .at_least(&oracle.weighted_stretch)
                    .unwrap()
            );
            checked += 1;
        }
        assert_eq!(checked, 38);
    }

    #[test]
    fn compact_rational_hierarchy_is_scale_invariant_without_length_expansion() {
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
        let small_hierarchy = hierarchy::Lsst::construct(&small, FlowNodeId(0)).unwrap();
        let large_hierarchy = hierarchy::Lsst::construct(&large, FlowNodeId(0)).unwrap();
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
            super::hierarchy::ProjectionMode::ClusterLocal
        );
        assert_eq!(
            small_hierarchy.work_certificate.length_mode,
            super::hierarchy::LengthMode::RoundedPowerOfTwo
        );
        assert_eq!(
            small_hierarchy.work_certificate.priority_queue_mode,
            super::hierarchy::PriorityQueueMode::ReducedLengthMonotone
        );
        assert_eq!(small_hierarchy.metrics.shortest_heap_pushes, 0);
        assert_eq!(small_hierarchy.metrics.directed_heap_pushes, 0);
        assert_eq!(small_hierarchy.metrics.event_heap_pushes, 0);
        assert_eq!(small_hierarchy.metrics.heap_comparisons, 0);
        assert!(!small_hierarchy.work_certificate.source_runtime_verified());
    }

    #[test]
    fn power_of_two_rounding_is_scale_relative_and_within_factor_two() {
        use super::round_length_to_power_of_two;

        let base = ExactRatio::new(2, 3).unwrap();
        let length = ExactRatio::new(3, 2).unwrap();
        let rounded = round_length_to_power_of_two(length.clone(), base.clone()).unwrap();
        assert_eq!(rounded, ExactRatio::new(4, 3).unwrap());
        assert!(length.at_least(&rounded).unwrap());
        assert!(
            rounded
                .checked_mul_integer(2)
                .unwrap()
                .at_least(&length)
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
        assert_eq!(hierarchy::source_scale_participation_bound(0).unwrap(), 4);
        assert_eq!(hierarchy::source_scale_participation_bound(9).unwrap(), 58);
        assert!(hierarchy::source_scale_participation_bound(u64::MAX).is_err());
    }

    #[test]
    fn weighted_hierarchy_contracts_recursively_and_expands_the_quotient_tree() {
        use crate::source_lsf::oracle::Lsst;

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
        let small_hierarchy = hierarchy::Lsst::construct(&small, FlowNodeId(0)).unwrap();
        let large_hierarchy = hierarchy::Lsst::construct(&large, FlowNodeId(0)).unwrap();
        let oracle = Lsst::solve(&small).unwrap();
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
                .at_least(&oracle.weighted_stretch)
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
        let small = alternating_path_graph(1);
        let large = alternating_path_graph(1_000);
        let small_hierarchy = hierarchy::Lsst::construct(&small, FlowNodeId(0)).unwrap();
        let large_hierarchy = hierarchy::Lsst::construct(&large, FlowNodeId(0)).unwrap();
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
            super::hierarchy::AmortizationMode::StructuralSourceBound;
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
        use super::{PetalMetrics, fast_shortest_paths, weighted_adjacency};

        for nodes in [16_usize, 32, 64, 128, 256] {
            let graph = power_of_two_chord_graph(nodes);
            let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
            let mut metrics = PetalMetrics::default();
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
                witnessed.insert((expected.numerator().clone(), expected.denominator().clone()));
            }
            assert_eq!(witnessed.len(), nodes / 2 - 1);
            assert!(reduced_classes >= witnessed.len());
            assert_eq!(
                usize::try_from(nodes.ilog2()).unwrap() + 1,
                (0..graph.edge_count())
                    .filter_map(|index| graph.edge(SourceEdgeId(index)))
                    .map(|edge| {
                        (
                            edge.length.numerator().clone(),
                            edge.length.denominator().clone(),
                        )
                    })
                    .collect::<BTreeSet<_>>()
                    .len()
            );
        }
    }

    #[test]
    fn reduced_length_queue_exposes_unbounded_source_classes() {
        use super::{
            PetalMetrics, WeightedPetal, WeightedPetalAtRadius, fast_shortest_paths, recover_path,
            transformed_weighted_membership_thresholds_oracle,
        };

        assert_power_of_two_chord_family_has_linear_reduced_length_classes();

        let nodes = 128_usize;
        let graph = power_of_two_chord_graph(nodes);
        let cluster = (0..nodes).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = WeightedPetal::construct_for_hierarchy(
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
        let fixed_radius = WeightedPetalAtRadius::construct_for_hierarchy(
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
        let mut setup_metrics = PetalMetrics::default();
        let center_paths =
            fast_shortest_paths(&graph, &cluster, FlowNodeId(0), &mut setup_metrics).unwrap();
        let path = recover_path(FlowNodeId(0), FlowNodeId(nodes - 1), &center_paths).unwrap();
        let mut transformed_metrics = PetalMetrics::default();
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
