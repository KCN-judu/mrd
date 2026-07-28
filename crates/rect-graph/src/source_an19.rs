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
                boundary,
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
    metrics: &mut An19PetalMetrics,
) -> Result<FigureSixSelection, An19PetalError> {
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
        let vertices = vertices_at_radius(remaining, thresholds, window_end)?;
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
    let start_vertices = vertices_at_radius(remaining, thresholds, window_start)?;
    let start_edges = internal_edge_count(graph, &start_vertices);
    if start_edges == 0 || start_edges >= cluster_edges {
        return Err(An19PetalError::InvalidRadius);
    }
    let mut radius = window_start;
    loop {
        let vertices = vertices_at_radius(remaining, thresholds, radius)?;
        let internal_edges = internal_edge_count(graph, &vertices);
        let boundary_edges = boundary_edge_count(graph, cluster, &vertices);
        metrics.certified_comparisons = metrics
            .certified_comparisons
            .checked_add(1)
            .ok_or(An19PetalError::Overflow)?;
        if certify_stopping_condition(
            cluster_edges,
            start_edges,
            internal_edges,
            boundary_edges,
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
        let n = i128::try_from(graph.node_count()).map_err(|_| An19PetalError::Overflow)?;
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
    node_count: usize,
    edges: Vec<AugmentedAn19Edge>,
}

#[derive(Clone, Debug)]
pub struct AugmentedAn19Edge {
    active: bool,
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
        for index in 0..graph.edge_count() {
            let edge = graph
                .edge(SourceEdgeId(index))
                .ok_or(An19PetalError::InvalidAugmentedGraph)?;
            original_endpoints.push((edge.first, edge.second));
            edges.push(AugmentedAn19Edge {
                active: true,
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
            first: from,
            second: vertex,
            length: offset,
            provenance: from_provenance,
        });
        let toward_edge = self.edges.len();
        self.edges.push(AugmentedAn19Edge {
            active: true,
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
    boundary_edges: usize,
    levels: usize,
    budget: ExactRatio,
) -> Result<bool, An19PetalError> {
    if boundary_edges == 0 {
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
        .checked_mul(ratio(
            i128::try_from(boundary_edges).map_err(|_| An19PetalError::Overflow)?,
            denominator,
        )?)
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
}
