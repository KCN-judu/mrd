use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{
    CertifiedFixedPoint, ExactRatio, FixedPointConfig, FlowNodeId, SourceDynamicGraph,
    SourceEdgeId, SourceWeightedEdge,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct An19PetalMetrics {
    pub shortest_path_runs: u64,
    pub edge_relaxations: u64,
    pub radius_events: u64,
    pub certified_comparisons: u64,
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
        validate_weighted_domain(graph, cluster, remaining, center, target, radius)?;
        let mut metrics = An19PetalMetrics::default();
        let cluster_paths = shortest_paths(graph, cluster, center, &mut metrics)?;
        let path = recover_path(center, target, &cluster_paths)?;
        if path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(An19PetalError::InvalidDomain);
        }
        let remaining_paths = shortest_paths(graph, remaining, center, &mut metrics)?;
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
        Self::construct_with_portal_volume(graph, cluster, remaining, center, target, budget, false)
    }

    fn construct_for_hierarchy(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        budget: ExactRatio,
        compact_weighted_portals: bool,
    ) -> Result<Self, An19PetalError> {
        Self::construct_with_portal_volume(
            graph,
            cluster,
            remaining,
            center,
            target,
            budget,
            compact_weighted_portals,
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
    ) -> Result<Self, An19PetalError> {
        validate_weighted_domain(graph, cluster, remaining, center, target, budget)?;
        if !budget.is_positive() {
            return Err(An19PetalError::InvalidDomain);
        }
        let mut metrics = An19PetalMetrics::default();
        let cluster_paths = shortest_paths(graph, cluster, center, &mut metrics)?;
        let path = recover_path(center, target, &cluster_paths)?;
        if path
            .vertices
            .iter()
            .any(|vertex| !remaining.contains(vertex))
        {
            return Err(An19PetalError::InvalidDomain);
        }
        let remaining_paths = shortest_paths(graph, remaining, center, &mut metrics)?;
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
        let thresholds = weighted_membership_thresholds(
            graph,
            remaining,
            target,
            &path,
            &remaining_paths.distances,
            budget,
            &mut metrics,
        )?;
        let selection = select_weighted_figure_six(
            graph,
            cluster,
            remaining,
            &thresholds,
            budget,
            compact_weighted_portals,
            &mut metrics,
        )?;
        let mut at_radius = An19WeightedPetalAtRadius::construct(
            graph,
            cluster,
            remaining,
            center,
            target,
            selection.radius,
        )?;
        at_radius.metrics.shortest_path_runs = at_radius
            .metrics
            .shortest_path_runs
            .checked_add(metrics.shortest_path_runs)
            .ok_or(An19PetalError::Overflow)?;
        at_radius.metrics.edge_relaxations = at_radius
            .metrics
            .edge_relaxations
            .checked_add(metrics.edge_relaxations)
            .ok_or(An19PetalError::Overflow)?;
        at_radius.metrics.radius_events = metrics.radius_events;
        at_radius.metrics.certified_comparisons = metrics.certified_comparisons;
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

fn select_weighted_figure_six(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    budget: ExactRatio,
    compact_weighted_portals: bool,
    metrics: &mut An19PetalMetrics,
) -> Result<FigureSixSelection, An19PetalError> {
    let base_cluster_edges = internal_edge_count(graph, cluster);
    let base_active_edges = (0..graph.edge_count())
        .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
        .count();
    if base_cluster_edges == 0 || base_active_edges < 2 {
        return Err(An19PetalError::InvalidDomain);
    }
    let levels = ceil_log_log(graph.node_count());
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
    node_count: usize,
    edges: Vec<AugmentedAn19Edge>,
}

#[derive(Clone, Debug)]
pub struct AugmentedAn19Edge {
    active: bool,
    halved: bool,
    first: FlowNodeId,
    second: FlowNodeId,
    length: ExactRatio,
    provenance: Option<OriginalEdgeInterval>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalEdgeInterval {
    edge: SourceEdgeId,
    first_position: ExactRatio,
    second_position: ExactRatio,
}

pub struct AugmentedProjection {
    graph: SourceDynamicGraph,
    dense_to_augmented: Vec<usize>,
}

impl AugmentedAn19Graph {
    /// Copies an exact source graph into a stable-edge hierarchy workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when an active source edge cannot be recovered or
    /// exact provenance coordinates overflow.
    pub fn from_source(graph: &SourceDynamicGraph) -> Result<Self, An19PetalError> {
        let mut edges = Vec::new();
        let mut original_endpoints = Vec::new();
        let one = ratio(1, 1)?;
        let mut unit_input = true;
        for index in 0..graph.edge_count() {
            let edge = graph
                .edge(SourceEdgeId(index))
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            original_endpoints.push((edge.first, edge.second));
            unit_input &= edge.length == one;
            edges.push(AugmentedAn19Edge {
                active: true,
                halved: false,
                first: edge.first,
                second: edge.second,
                length: edge.length,
                provenance: Some(OriginalEdgeInterval {
                    edge: SourceEdgeId(index),
                    first_position: ratio(0, 1)?,
                    second_position: edge.length,
                }),
            });
        }
        Ok(Self {
            original_node_count: graph.node_count(),
            original_endpoints,
            unit_input,
            node_count: graph.node_count(),
            edges,
        })
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
        self.node_count = self
            .node_count
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        let edge = self.edges.len();
        self.edges.push(AugmentedAn19Edge {
            active: true,
            halved: false,
            first: attached_to,
            second: vertex,
            length,
            provenance: None,
        });
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
        let vertex = FlowNodeId(self.node_count);
        self.node_count = self
            .node_count
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        let (from_provenance, toward_provenance) = split_provenance(&edge, from, offset)?;
        self.edges[edge_id].active = false;
        let from_edge = self.edges.len();
        self.edges.push(AugmentedAn19Edge {
            active: true,
            halved: edge.halved,
            first: from,
            second: vertex,
            length: offset,
            provenance: from_provenance,
        });
        let toward_edge = self.edges.len();
        self.edges.push(AugmentedAn19Edge {
            active: true,
            halved: edge.halved,
            first: vertex,
            second: toward,
            length: remainder,
            provenance: toward_provenance,
        });
        Ok((vertex, from_edge, toward_edge))
    }

    /// Builds the dense active graph consumed by exact Figure 6 operations.
    ///
    /// # Errors
    ///
    /// Returns an error when an active edge violates the source graph domain
    /// or its rational encoding bound cannot be represented.
    pub fn project(&self) -> Result<AugmentedProjection, An19PetalError> {
        let mut dense_to_augmented = Vec::new();
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
            edges.push(SourceWeightedEdge {
                first: edge.first,
                second: edge.second,
                length: edge.length,
                weight: ratio(1, 1)?,
            });
        }
        let graph = SourceDynamicGraph::new(self.node_count, edges, bound)
            .map_err(|_| An19PetalError::InvalidAugmentedGraph)?;
        Ok(AugmentedProjection {
            graph,
            dense_to_augmented,
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
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct An19HierarchyMetrics {
    pub recursion_calls: u64,
    pub base_cases: u64,
    pub contraction_calls: u64,
    pub contracted_edges: u64,
    pub quotient_edges: u64,
    pub petals: u64,
    pub portal_splits: u64,
    pub virtual_leaves: u64,
    pub highway_edges_halved: u64,
    pub highway_edges_reused: u64,
    pub shortest_path_runs: u64,
    pub edge_relaxations: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19RadiusEdge {
    pub first: FlowNodeId,
    pub second: FlowNodeId,
    pub length: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19RadiusCertificate {
    pub original_node_count: usize,
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
        let mut workspace = AugmentedAn19Graph::from_source(graph)?;
        let cluster = (0..graph.node_count())
            .map(FlowNodeId)
            .collect::<BTreeSet<_>>();
        let mut metrics = An19HierarchyMetrics::default();
        let mut radius_certificates = Vec::new();
        let selected = hierarchical_petal_decomposition(
            &mut workspace,
            cluster,
            root,
            root,
            graph.node_count(),
            &mut radius_certificates,
            &mut metrics,
        )?;
        let tree_edges = workspace.recover_original_tree(&selected)?;
        let (weighted_stretch, total_weight) = audit_original_tree_stretch(graph, &tree_edges)?;
        let result = Self {
            tree_edges,
            weighted_stretch,
            total_weight,
            radius_certificates,
            metrics,
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
        for certificate in &self.radius_certificates {
            certificate.verify()?;
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
        if u64::try_from(contraction_calls).map_err(|_| An19PetalError::Overflow)?
            != self.metrics.contraction_calls
            || u64::try_from(contracted_edges).map_err(|_| An19PetalError::Overflow)?
                != self.metrics.contracted_edges
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

fn hierarchical_petal_decomposition(
    workspace: &mut AugmentedAn19Graph,
    cluster: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    original_node_count: usize,
    radius_certificates: &mut Vec<An19RadiusCertificate>,
    metrics: &mut An19HierarchyMetrics,
) -> Result<BTreeSet<usize>, An19PetalError> {
    metrics.recursion_calls = metrics
        .recursion_calls
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    let projection = workspace.project()?;
    let paths = hierarchy_shortest_paths(projection.graph(), &cluster, center, metrics)?;
    let radius = hierarchy_radius(&cluster, &paths)?;
    let threshold = hierarchy_base_threshold(original_node_count)?
        .checked_mul(minimum_cluster_edge_length(projection.graph(), &cluster)?)
        .map_err(|_| An19PetalError::Overflow)?;
    let base_vertex_limit = 2;
    let base_case = cluster.len() <= base_vertex_limit
        || threshold
            .at_least(radius)
            .map_err(|_| An19PetalError::Overflow)?;
    radius_certificates.push(build_radius_certificate(
        &projection,
        original_node_count,
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
        let contraction = An19ShortEdgeContraction::build_with_radius(
            projection.graph(),
            &cluster,
            center,
            radius,
            original_node_count,
        )?;
        if !contraction.contracted_edges.is_empty() {
            attach_contraction_certificate(
                radius_certificates
                    .last_mut()
                    .ok_or(An19PetalError::InvalidRadiusCertificate)?,
                &contraction,
            )?;
            return hierarchy_contracted_tree(
                &projection,
                &contraction,
                center,
                target,
                original_node_count,
                radius_certificates,
                metrics,
            );
        }
    }

    let (mut stigma, pieces, stigma_target) =
        petal_decomposition(workspace, cluster, center, target, radius, metrics)?;
    let mut selected = BTreeSet::new();
    for piece in pieces {
        halve_highway(
            workspace,
            &piece.cluster,
            piece.center,
            piece.target,
            metrics,
        )?;
        let subtree = hierarchical_petal_decomposition(
            workspace,
            piece.cluster,
            piece.center,
            piece.target,
            original_node_count,
            radius_certificates,
            metrics,
        )?;
        selected.extend(subtree);
        if !selected.insert(piece.connection_edge) {
            return Err(An19PetalError::InvalidAugmentedGraph);
        }
    }
    halve_highway(workspace, &stigma, center, stigma_target, metrics)?;
    let stigma_tree = hierarchical_petal_decomposition(
        workspace,
        std::mem::take(&mut stigma),
        center,
        stigma_target,
        original_node_count,
        radius_certificates,
        metrics,
    )?;
    selected.extend(stigma_tree);
    Ok(selected)
}

fn attach_contraction_certificate(
    certificate: &mut An19RadiusCertificate,
    contraction: &An19ShortEdgeContraction,
) -> Result<(), An19PetalError> {
    certificate.contraction_threshold = Some(contraction.contraction_threshold);
    certificate.contraction_component_of = certificate
        .distances
        .iter()
        .map(|(vertex, _)| {
            contraction
                .component_of
                .get(vertex.0)
                .copied()
                .flatten()
                .map(|component| (*vertex, component))
                .ok_or(An19PetalError::InvalidContraction)
        })
        .collect::<Result<_, _>>()?;
    certificate.contracted_edge_count = contraction.contracted_edges.len();
    certificate.verify()
}

#[allow(clippy::too_many_arguments)]
fn hierarchy_contracted_tree(
    projection: &AugmentedProjection,
    contraction: &An19ShortEdgeContraction,
    center: FlowNodeId,
    target: FlowNodeId,
    original_node_count: usize,
    radius_certificates: &mut Vec<An19RadiusCertificate>,
    metrics: &mut An19HierarchyMetrics,
) -> Result<BTreeSet<usize>, An19PetalError> {
    let mut quotient_edges = Vec::new();
    let mut quotient_to_dense = Vec::new();
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
    }
    let quotient_graph =
        SourceDynamicGraph::new(contraction.components.len(), quotient_edges, bound)
            .map_err(|_| An19PetalError::InvalidContraction)?;
    let mut quotient_workspace = AugmentedAn19Graph::from_source(&quotient_graph)?;
    let quotient_cluster = (0..contraction.components.len())
        .map(FlowNodeId)
        .collect::<BTreeSet<_>>();
    let quotient_center = contracted_vertex(contraction, center)?;
    let quotient_target = contracted_vertex(contraction, target)?;
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
        radius_certificates,
        metrics,
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
) -> Result<(BTreeSet<FlowNodeId>, Vec<An19HierarchyPiece>, FlowNodeId), An19PetalError> {
    let half = ratio(1, 2)?;
    let r0 = delta
        .checked_mul(half)
        .map_err(|_| An19PetalError::Overflow)?;
    let mut remaining = cluster.clone();
    let projection = workspace.project()?;
    let paths = hierarchy_shortest_paths(projection.graph(), &cluster, center, metrics)?;
    let target_distance = paths.distances[target.0].ok_or(An19PetalError::Disconnected)?;
    let first_target = hierarchy_first_target(
        workspace,
        &mut cluster,
        &mut remaining,
        center,
        target,
        target_distance,
        r0,
        metrics,
    )?;
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
    )?;
    let stigma_target = connection_predecessor(workspace, first.connection_edge, first.center)?;
    let mut pieces = vec![first];
    let later_budget = delta
        .checked_mul(ratio(1, 8)?)
        .map_err(|_| An19PetalError::Overflow)?;
    loop {
        let projection = workspace.project()?;
        let paths = hierarchy_shortest_paths(projection.graph(), &cluster, center, metrics)?;
        let mut outside = None;
        for vertex in &remaining {
            let distance = paths.distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
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
        )?;
        let piece = create_hierarchy_petal(
            workspace,
            &mut cluster,
            &mut remaining,
            center,
            next_target,
            later_budget,
            metrics,
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
    metrics: &mut An19HierarchyMetrics,
) -> Result<FlowNodeId, An19PetalError> {
    if !ratio_less(target_distance, r0)? {
        return ensure_vertex_at_distance(
            workspace, cluster, remaining, center, target, r0, metrics,
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

fn create_hierarchy_petal(
    workspace: &mut AugmentedAn19Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
    metrics: &mut An19HierarchyMetrics,
) -> Result<An19HierarchyPiece, An19PetalError> {
    let projection = workspace.project()?;
    let petal = An19WeightedPetal::construct_for_hierarchy(
        projection.graph(),
        fixed_cluster,
        remaining,
        center,
        target,
        budget,
        !workspace.unit_input,
    )?;
    let mut petal_vertices = petal.at_radius.vertices;
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
            (vertex, stable)
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
            let (portal, _, toward_center) = workspace.split_edge(stable, from, offset_from)?;
            fixed_cluster.insert(portal);
            petal_vertices.insert(portal);
            metrics.portal_splits = metrics
                .portal_splits
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
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

fn ensure_vertex_at_distance(
    workspace: &mut AugmentedAn19Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    distance: ExactRatio,
    metrics: &mut An19HierarchyMetrics,
) -> Result<FlowNodeId, An19PetalError> {
    let projection = workspace.project()?;
    let paths = hierarchy_shortest_paths(projection.graph(), remaining, center, metrics)?;
    let path = recover_path(center, target, &paths)?;
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
            let (vertex, _, _) = workspace.split_edge(stable, from, offset)?;
            fixed_cluster.insert(vertex);
            remaining.insert(vertex);
            metrics.portal_splits = metrics
                .portal_splits
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
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
) -> Result<(), An19PetalError> {
    let projection = workspace.project()?;
    let paths = hierarchy_shortest_paths(projection.graph(), cluster, center, metrics)?;
    let path = recover_path(center, target, &paths)?;
    for dense in path.edges {
        let stable = *projection
            .dense_to_augmented()
            .get(dense.0)
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
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
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    metrics: &mut An19HierarchyMetrics,
) -> Result<ShortestPaths, An19PetalError> {
    let mut petal_metrics = An19PetalMetrics::default();
    let result = shortest_paths(graph, cluster, center, &mut petal_metrics)?;
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(petal_metrics.shortest_path_runs)
        .ok_or(An19PetalError::Overflow)?;
    metrics.edge_relaxations = metrics
        .edge_relaxations
        .checked_add(petal_metrics.edge_relaxations)
        .ok_or(An19PetalError::Overflow)?;
    Ok(result)
}

fn hierarchy_radius(
    cluster: &BTreeSet<FlowNodeId>,
    paths: &ShortestPaths,
) -> Result<ExactRatio, An19PetalError> {
    let mut radius = ratio(0, 1)?;
    for vertex in cluster {
        let distance = paths.distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
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
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    radius: ExactRatio,
    base_threshold: ExactRatio,
    base_vertex_limit: usize,
    base_case: bool,
    paths: &ShortestPaths,
) -> Result<An19RadiusCertificate, An19PetalError> {
    let distances = cluster
        .iter()
        .map(|vertex| {
            paths.distances[vertex.0]
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
        if cluster.contains(&edge.first) && cluster.contains(&edge.second) {
            edges.push(An19RadiusEdge {
                first: edge.first,
                second: edge.second,
                length: edge.length,
            });
        }
    }
    let certificate = An19RadiusCertificate {
        original_node_count,
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
    paths: &ShortestPaths,
) -> Result<BTreeSet<usize>, An19PetalError> {
    let mut tree = BTreeSet::new();
    for vertex in cluster {
        if *vertex == center {
            continue;
        }
        let (_, dense) = paths.predecessors[vertex.0].ok_or(An19PetalError::Disconnected)?;
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
    let mut weighted_stretch = ratio(0, 1)?;
    let mut total_weight = ratio(0, 1)?;
    for index in 0..graph.edge_count() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(An19PetalError::InvalidAugmentedGraph)?;
        let distance = original_tree_distance(graph, &adjacency, edge.first, edge.second)?;
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

fn original_tree_distance(
    graph: &SourceDynamicGraph,
    adjacency: &[Vec<(usize, SourceEdgeId)>],
    source: FlowNodeId,
    target: FlowNodeId,
) -> Result<ExactRatio, An19PetalError> {
    let mut stack = vec![(source.0, usize::MAX, ratio(0, 1)?)];
    while let Some((node, parent, distance)) = stack.pop() {
        if node == target.0 {
            return Ok(distance);
        }
        for (next, edge_id) in &adjacency[node] {
            if *next == parent {
                continue;
            }
            let edge = graph
                .edge(*edge_id)
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            stack.push((
                *next,
                node,
                distance
                    .checked_add(edge.length)
                    .map_err(|_| An19PetalError::Overflow)?,
            ));
        }
    }
    Err(An19PetalError::InvalidAugmentedGraph)
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

struct RecoveredPath {
    vertices: Vec<FlowNodeId>,
    edges: Vec<SourceEdgeId>,
}

#[derive(Clone, Debug)]
struct MembershipThresholds {
    by_vertex: Vec<Option<ExactRatio>>,
    path_distance_from_target: Vec<Option<ExactRatio>>,
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

fn directed_petal_distances(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    target: FlowNodeId,
    center_distances: &[Option<ExactRatio>],
    highway: &[An19HighwaySegment],
    metrics: &mut An19PetalMetrics,
) -> Result<Vec<Option<ExactRatio>>, An19PetalError> {
    let mut distances = vec![None; graph.node_count()];
    let mut path_keys = vec![None; graph.node_count()];
    let mut settled = vec![false; graph.node_count()];
    distances[target.0] = Some(ratio(0, 1)?);
    path_keys[target.0] = Some(Vec::new());
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
                highway,
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
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(An19PetalError::Disconnected);
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
    })
}

fn weighted_membership_thresholds(
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

fn ratio(numerator: i128, denominator: i128) -> Result<ExactRatio, An19PetalError> {
    ExactRatio::new(numerator, denominator).map_err(|_| An19PetalError::Overflow)
}

fn count_ratio(value: usize) -> Result<ExactRatio, An19PetalError> {
    ratio(
        i128::try_from(value).map_err(|_| An19PetalError::Overflow)?,
        1,
    )
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
    #[error("certified AN19 logarithmic comparison needs more bounded precision")]
    InsufficientPrecision,
    #[error("checked AN19 petal arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::An19UnweightedPetal;
    use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceWeightedEdge};
    use std::collections::BTreeSet;

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
        use super::{An19HierarchyMetrics, AugmentedAn19Graph, halve_highway};

        let graph = path_graph(2);
        let mut workspace = AugmentedAn19Graph::from_source(&graph).unwrap();
        let cluster = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        let mut metrics = An19HierarchyMetrics::default();
        halve_highway(
            &mut workspace,
            &cluster,
            FlowNodeId(0),
            FlowNodeId(1),
            &mut metrics,
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
        )
        .unwrap();
        assert_eq!(workspace.edges[0].length, half);
        assert_eq!(metrics.highway_edges_halved, 1);
        assert_eq!(metrics.highway_edges_reused, 1);
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
    fn splits_augmented_provenance_in_both_orientations_and_projects_dense_ids() {
        use super::{AugmentedAn19Graph, OriginalEdgeInterval};

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
        let projection = augmented.project().unwrap();
        assert_eq!(projection.graph.node_count(), 4);
        assert_eq!(
            projection.dense_to_augmented,
            vec![1, from_edge, toward_edge]
        );

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
        use super::{An19HierarchyMetrics, AugmentedAn19Graph, hierarchical_petal_decomposition};

        let graph = path_graph(500);
        let mut workspace = AugmentedAn19Graph::from_source(&graph).unwrap();
        let cluster = (0..500).map(FlowNodeId).collect::<BTreeSet<_>>();
        let mut certificates = Vec::new();
        let mut metrics = An19HierarchyMetrics::default();
        let selected = hierarchical_petal_decomposition(
            &mut workspace,
            cluster,
            FlowNodeId(0),
            FlowNodeId(499),
            500,
            &mut certificates,
            &mut metrics,
        )
        .unwrap();
        let recovered = workspace.recover_original_tree(&selected).unwrap();
        assert_eq!(recovered.len(), 499);
        assert!(metrics.recursion_calls > 1);
        assert!(metrics.petals > 0);
        assert!(metrics.portal_splits > 0);
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
        assert!(hierarchy.metrics.recursion_calls > 1);
        assert!(hierarchy.metrics.petals > 0);
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
        assert_eq!(small_hierarchy.metrics.contracted_edges, 2);
        assert_eq!(small_hierarchy.metrics.quotient_edges, 2);
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
    }

    #[test]
    fn compact_rational_hierarchy_recurses_with_scale_independent_counters() {
        use super::An19HierarchicalLsst;

        let make_path = |scale: i128| {
            let edges = (0..499)
                .map(|index| SourceWeightedEdge {
                    first: FlowNodeId(index),
                    second: FlowNodeId(index + 1),
                    length: ExactRatio::new(scale * i128::try_from(2 + index % 2).unwrap(), 1)
                        .unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                })
                .collect();
            SourceDynamicGraph::new(500, edges, 10_000).unwrap()
        };
        let small = make_path(1);
        let large = make_path(1_000);
        let small_hierarchy = An19HierarchicalLsst::construct(&small, FlowNodeId(0)).unwrap();
        let large_hierarchy = An19HierarchicalLsst::construct(&large, FlowNodeId(0)).unwrap();
        small_hierarchy.verify(&small).unwrap();
        large_hierarchy.verify(&large).unwrap();
        assert_eq!(small_hierarchy.tree_edges, large_hierarchy.tree_edges);
        assert_eq!(small_hierarchy.metrics, large_hierarchy.metrics);
        assert!(small_hierarchy.metrics.recursion_calls > 1);
        assert!(small_hierarchy.metrics.virtual_leaves > 0);
        assert!(small_hierarchy.metrics.virtual_leaves < 500);
        assert!(small_hierarchy.metrics.virtual_leaves <= small_hierarchy.metrics.recursion_calls);
    }
}
