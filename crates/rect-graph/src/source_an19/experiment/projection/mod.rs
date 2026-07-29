use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge};

use super::super::{
    experiment::hierarchy::{
        LengthMode, Metrics as HierarchyMetrics, source_materialization_charge,
    },
    petal::{
        DisjointSet, Error, HighwaySegment, PetalMetrics, all_cluster_connected, all_connected,
        checked_metric_sum, intervals_overlap, ratio, ratio_less, round_length_to_power_of_two,
        shortest_paths, sort_and_merge_touching, split_provenance,
    },
};

pub struct ShortEdgeContraction {
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
pub struct HalvedInterval {
    pub start_from_first: ExactRatio,
    pub end_from_first: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighwayLedger {
    original_lengths: Vec<Option<ExactRatio>>,
    halved_intervals: Vec<Vec<HalvedInterval>>,
    applications: u64,
}

impl HighwayLedger {
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
    pub fn intervals(&self, edge: SourceEdgeId) -> Option<&[HalvedInterval]> {
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
        highway: &[HighwaySegment],
    ) -> Result<(), Error> {
        let mut candidate = self.clone();
        for segment in highway {
            let edge = graph.edge(segment.edge).ok_or(Error::InvalidHighway)?;
            let original = candidate
                .original_lengths
                .get(segment.edge.0)
                .copied()
                .flatten()
                .ok_or(Error::InvalidHighway)?;
            if original != edge.length
                || original != segment.original_edge_length
                || !segment.halved_length.is_positive()
                || ratio_less(original, segment.halved_length)?
            {
                return Err(Error::InvalidHighway);
            }
            let (start, end) = if segment.from == edge.first && segment.toward_center == edge.second
            {
                (ratio(0, 1)?, segment.halved_length)
            } else if segment.from == edge.second && segment.toward_center == edge.first {
                (
                    original
                        .checked_sub(segment.halved_length)
                        .map_err(|_| Error::Overflow)?,
                    original,
                )
            } else {
                return Err(Error::InvalidHighway);
            };
            let intervals = candidate
                .halved_intervals
                .get_mut(segment.edge.0)
                .ok_or(Error::InvalidHighway)?;
            for old in intervals.iter() {
                if intervals_overlap(start, end, old.start_from_first, old.end_from_first)? {
                    return Err(Error::RepeatedHighway);
                }
            }
            intervals.push(HalvedInterval {
                start_from_first: start,
                end_from_first: end,
            });
            sort_and_merge_touching(intervals)?;
        }
        candidate.applications = candidate
            .applications
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        *self = candidate;
        Ok(())
    }

    /// Returns the full endpoint-to-endpoint length after every recorded
    /// interval is halved once.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown edge or exact arithmetic overflow.
    pub fn effective_length(&self, edge: SourceEdgeId) -> Result<ExactRatio, Error> {
        let original = self
            .original_lengths
            .get(edge.0)
            .copied()
            .flatten()
            .ok_or(Error::InvalidHighway)?;
        let mut halved = ratio(0, 1)?;
        for interval in self
            .halved_intervals
            .get(edge.0)
            .ok_or(Error::InvalidHighway)?
        {
            halved = halved
                .checked_add(
                    interval
                        .end_from_first
                        .checked_sub(interval.start_from_first)
                        .map_err(|_| Error::Overflow)?,
                )
                .map_err(|_| Error::Overflow)?;
        }
        original
            .checked_sub(
                halved
                    .checked_mul(ratio(1, 2)?)
                    .map_err(|_| Error::Overflow)?,
            )
            .map_err(|_| Error::Overflow)
    }
}

impl ShortEdgeContraction {
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
    ) -> Result<Self, Error> {
        if cluster.is_empty()
            || !cluster.contains(&center)
            || cluster.iter().any(|vertex| vertex.0 >= graph.node_count())
        {
            return Err(Error::InvalidContraction);
        }
        let mut metrics = PetalMetrics::default();
        let paths = shortest_paths(graph, cluster, center, &mut metrics)?;
        let mut radius = ratio(0, 1)?;
        for vertex in cluster {
            let distance = paths.distances[vertex.0].ok_or(Error::Disconnected)?;
            if ratio_less(radius, distance)? {
                radius = distance;
            }
        }
        if !radius.is_positive() && cluster.len() > 1 {
            return Err(Error::InvalidContraction);
        }
        Self::build_with_radius(graph, cluster, center, radius, graph.node_count())
    }

    pub(in crate::source_an19) fn build_with_radius(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        radius: ExactRatio,
        original_node_count: usize,
    ) -> Result<Self, Error> {
        let n = i128::try_from(original_node_count).map_err(|_| Error::Overflow)?;
        let n_squared = n.checked_mul(n).ok_or(Error::Overflow)?;
        let contraction_threshold = radius
            .checked_mul(ratio(1, n_squared)?)
            .map_err(|_| Error::Overflow)?;
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
    ) -> Result<BTreeSet<SourceEdgeId>, Error> {
        if self.component_of.len() != graph.node_count()
            || quotient_tree_edges.len() + 1 != self.components.len()
        {
            return Err(Error::InvalidContraction);
        }
        let mut quotient_connectivity = DisjointSet::new(self.components.len());
        for edge_id in quotient_tree_edges {
            if !self.retained_edges.contains(edge_id) {
                return Err(Error::InvalidContraction);
            }
            let edge = graph.edge(*edge_id).ok_or(Error::InvalidContraction)?;
            let first = self.component_of[edge.first.0].ok_or(Error::InvalidContraction)?;
            let second = self.component_of[edge.second.0].ok_or(Error::InvalidContraction)?;
            if !quotient_connectivity.union(first, second) {
                return Err(Error::InvalidContraction);
            }
        }
        if !all_connected(&mut quotient_connectivity, self.components.len()) {
            return Err(Error::InvalidContraction);
        }
        let mut original_connectivity = DisjointSet::new(graph.node_count());
        let mut result = BTreeSet::new();
        for edge_id in &self.contracted_edges {
            let edge = graph.edge(*edge_id).ok_or(Error::InvalidContraction)?;
            if original_connectivity.union(edge.first.0, edge.second.0) {
                result.insert(*edge_id);
            }
        }
        for edge_id in quotient_tree_edges {
            let edge = graph.edge(*edge_id).ok_or(Error::InvalidContraction)?;
            if !original_connectivity.union(edge.first.0, edge.second.0) {
                return Err(Error::InvalidContraction);
            }
            result.insert(*edge_id);
        }
        if result.len() + 1 != self.cluster.len()
            || !all_cluster_connected(&mut original_connectivity, &self.cluster)
        {
            return Err(Error::InvalidContraction);
        }
        Ok(result)
    }
}

#[derive(Clone, Debug)]
pub struct Graph {
    pub(in crate::source_an19) original_node_count: usize,
    pub(in crate::source_an19) original_endpoints: Vec<(FlowNodeId, FlowNodeId)>,
    pub(in crate::source_an19) unit_input: bool,
    pub(in crate::source_an19) length_mode: LengthMode,
    pub(in crate::source_an19) node_count: usize,
    pub(in crate::source_an19) edges: Vec<Edge>,
    pub(in crate::source_an19) incident_edges: Vec<Vec<usize>>,
    projection_cache: RefCell<Option<CachedSnapshot>>,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub(in crate::source_an19) active: bool,
    pub(in crate::source_an19) halved: bool,
    pub(in crate::source_an19) first: FlowNodeId,
    pub(in crate::source_an19) second: FlowNodeId,
    pub(in crate::source_an19) length: ExactRatio,
    pub(in crate::source_an19) provenance: Option<OriginalInterval>,
    /// Top-level input edge charged by the runtime audit, independent of the
    /// current quotient workspace's local recovery provenance.
    pub(in crate::source_an19) root_source: Option<SourceEdgeId>,
    pub(in crate::source_an19) unsplit_length: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginalInterval {
    pub(in crate::source_an19) edge: SourceEdgeId,
    pub(in crate::source_an19) first_position: ExactRatio,
    pub(in crate::source_an19) second_position: ExactRatio,
}

/// Symbolic source label retained when an augmented edge is split at portals.
///
/// Equal labels identify a common unsplit source length, but do not by
/// themselves prove that arbitrary candidate distances may share one monotone
/// queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SymbolicLengthLabel {
    pub root_source: Option<SourceEdgeId>,
    pub unsplit_length: ExactRatio,
    pub halved: bool,
}

impl SymbolicLengthLabel {
    pub(in crate::source_an19) fn effective_length(self) -> Result<ExactRatio, Error> {
        if self.halved {
            self.unsplit_length
                .checked_mul(ratio(1, 2)?)
                .map_err(|_| Error::Overflow)
        } else {
            Ok(self.unsplit_length)
        }
    }
}

impl Edge {
    pub(in crate::source_an19) fn symbolic_length_label(&self) -> SymbolicLengthLabel {
        SymbolicLengthLabel {
            root_source: self.root_source,
            unsplit_length: self.unsplit_length,
            halved: self.halved,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub(in crate::source_an19) graph: SourceDynamicGraph,
    pub(in crate::source_an19) dense_to_augmented: Vec<usize>,
    pub(in crate::source_an19) dense_root_sources: Vec<Option<SourceEdgeId>>,
    pub(in crate::source_an19) dense_symbolic_labels: Vec<SymbolicLengthLabel>,
    pub(in crate::source_an19) length_class_counts: BTreeMap<(i128, i128), usize>,
    pub(in crate::source_an19) symbolic_source_classes: BTreeSet<(i128, i128)>,
    pub(in crate::source_an19) symbolic_virtual_classes: BTreeSet<(i128, i128)>,
    pub(in crate::source_an19) local_to_augmented_node: Vec<FlowNodeId>,
    pub(in crate::source_an19) augmented_to_local_node: BTreeMap<FlowNodeId, FlowNodeId>,
}

#[derive(Clone, Debug)]
struct CachedSnapshot {
    cluster: BTreeSet<FlowNodeId>,
    projection: Rc<Snapshot>,
    pending_splits: Vec<SplitUpdate>,
}

#[derive(Clone, Copy, Debug)]
struct SplitUpdate {
    stable_edge: usize,
    from: FlowNodeId,
    portal: FlowNodeId,
    from_edge: usize,
    toward_edge: usize,
    offset: ExactRatio,
}

#[derive(Clone, Copy, Debug, Default)]
struct IncidentScans {
    active_internal: u64,
    active_boundary: u64,
    inactive: u64,
}

impl IncidentScans {
    pub(in crate::source_an19) fn observe(
        &mut self,
        edge: &Edge,
        cluster: &BTreeSet<FlowNodeId>,
    ) -> Result<bool, Error> {
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

    pub(in crate::source_an19) fn record(
        self,
        metrics: &mut HierarchyMetrics,
    ) -> Result<(), Error> {
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

impl Graph {
    /// Copies an exact source graph into a stable-edge hierarchy workspace.
    ///
    /// # Errors
    ///
    /// Returns an error when an active source edge cannot be recovered or
    /// exact provenance coordinates overflow.
    pub fn from_source(graph: &SourceDynamicGraph) -> Result<Self, Error> {
        Self::from_source_with_length_mode(graph, LengthMode::ExactRational)
    }

    pub(in crate::source_an19) fn from_source_with_length_mode(
        graph: &SourceDynamicGraph,
        length_mode: LengthMode,
    ) -> Result<Self, Error> {
        let root_sources = (0..graph.edge_count())
            .map(|index| Some(SourceEdgeId(index)))
            .collect::<Vec<_>>();
        Self::from_source_with_root_sources_and_labels(graph, length_mode, &root_sources, None)
    }

    pub(in crate::source_an19) fn from_source_with_inherited_labels(
        graph: &SourceDynamicGraph,
        length_mode: LengthMode,
        root_sources: &[Option<SourceEdgeId>],
        symbolic_labels: &[SymbolicLengthLabel],
    ) -> Result<Self, Error> {
        Self::from_source_with_root_sources_and_labels(
            graph,
            length_mode,
            root_sources,
            Some(symbolic_labels),
        )
    }

    pub(in crate::source_an19) fn from_source_with_root_sources_and_labels(
        graph: &SourceDynamicGraph,
        length_mode: LengthMode,
        root_sources: &[Option<SourceEdgeId>],
        symbolic_labels: Option<&[SymbolicLengthLabel]>,
    ) -> Result<Self, Error> {
        if root_sources.len() != graph.edge_count()
            || symbolic_labels.is_some_and(|labels| labels.len() != graph.edge_count())
        {
            return Err(Error::InvalidAugmentedGraph);
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
                Ok::<_, Error>(if replace { Some(edge.length) } else { minimum })
            })?
            .ok_or(Error::InvalidAugmentedGraph)?;
        for (index, root_source) in root_sources.iter().copied().enumerate() {
            let edge = graph
                .edge(SourceEdgeId(index))
                .ok_or(Error::InvalidAugmentedGraph)?;
            original_endpoints.push((edge.first, edge.second));
            unit_input &= edge.length == one;
            let workspace_length = match length_mode {
                LengthMode::ExactRational => edge.length,
                LengthMode::RoundedPowerOfTwo => {
                    round_length_to_power_of_two(edge.length, minimum_length)?
                }
            };
            let symbolic_label = symbolic_labels.map_or(
                SymbolicLengthLabel {
                    root_source,
                    unsplit_length: workspace_length,
                    halved: false,
                },
                |labels| labels[index],
            );
            if symbolic_label.root_source != root_source
                || !symbolic_label.unsplit_length.is_positive()
            {
                return Err(Error::InvalidAugmentedGraph);
            }
            let stable = edges.len();
            edges.push(Edge {
                active: true,
                halved: symbolic_label.halved,
                first: edge.first,
                second: edge.second,
                length: workspace_length,
                provenance: Some(OriginalInterval {
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

    pub(in crate::source_an19) fn invalidate_projection_cache(&mut self) {
        self.projection_cache.get_mut().take();
    }

    fn queue_projection_split(&mut self, edge: &Edge, update: SplitUpdate) {
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
    ) -> Result<(FlowNodeId, usize), Error> {
        if attached_to.0 >= self.node_count || !length.is_positive() {
            return Err(Error::InvalidAugmentedGraph);
        }
        let vertex = FlowNodeId(self.node_count);
        let next_node_count = self.node_count.checked_add(1).ok_or(Error::Overflow)?;
        self.invalidate_projection_cache();
        self.node_count = next_node_count;
        self.incident_edges.push(Vec::new());
        let edge = self.edges.len();
        self.edges.push(Edge {
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
    ) -> Result<(FlowNodeId, usize, usize), Error> {
        let edge = self
            .edges
            .get(edge_id)
            .cloned()
            .filter(|edge| edge.active)
            .ok_or(Error::InvalidAugmentedGraph)?;
        let toward = if edge.first == from {
            edge.second
        } else if edge.second == from {
            edge.first
        } else {
            return Err(Error::InvalidAugmentedGraph);
        };
        if !offset.is_positive() || !ratio_less(offset, edge.length)? {
            return Err(Error::InvalidAugmentedGraph);
        }
        let remainder = edge
            .length
            .checked_sub(offset)
            .map_err(|_| Error::Overflow)?;
        let (from_provenance, toward_provenance) = split_provenance(&edge, from, offset)?;
        let vertex = FlowNodeId(self.node_count);
        let next_node_count = self.node_count.checked_add(1).ok_or(Error::Overflow)?;
        self.node_count = next_node_count;
        self.incident_edges.push(Vec::new());
        self.edges[edge_id].active = false;
        let from_edge = self.edges.len();
        self.edges.push(Edge {
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
        self.edges.push(Edge {
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
            SplitUpdate {
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

    pub(in crate::source_an19) fn reuse_cluster_projection(
        &self,
        cluster: &BTreeSet<FlowNodeId>,
        metrics: &mut HierarchyMetrics,
        projection_audit: &mut Audit,
    ) -> Result<Option<Rc<Snapshot>>, Error> {
        let mut cache_slot = self.projection_cache.borrow_mut();
        let Some(mut cached) = cache_slot.take() else {
            return Ok(None);
        };
        if cached.cluster != *cluster {
            return Ok(None);
        }
        let incremental_splits =
            u64::try_from(cached.pending_splits.len()).map_err(|_| Error::Overflow)?;
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

    pub(in crate::source_an19) fn project_cluster(
        &self,
        cluster: &BTreeSet<FlowNodeId>,
        metrics: &mut HierarchyMetrics,
        projection_audit: &mut Audit,
    ) -> Result<Rc<Snapshot>, Error> {
        metrics.projection_calls = checked_metric_sum(metrics.projection_calls, 1)?;
        if let Some(projection) =
            self.reuse_cluster_projection(cluster, metrics, projection_audit)?
        {
            return Ok(projection);
        }
        metrics.projection_materializations =
            checked_metric_sum(metrics.projection_materializations, 1)?;
        let local_nodes = u64::try_from(cluster.len()).map_err(|_| Error::Overflow)?;
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
        let mut incident_scans = IncidentScans::default();
        let mut bound = 1_i128;
        for vertex in cluster {
            let incident = self
                .incident_edges
                .get(vertex.0)
                .ok_or(Error::InvalidAugmentedGraph)?;
            for stable in incident {
                let edge = self
                    .edges
                    .get(*stable)
                    .ok_or(Error::InvalidAugmentedGraph)?;
                if !incident_scans.observe(edge, cluster)? || *vertex != edge.first.min(edge.second)
                {
                    continue;
                }
                bound = bound
                    .max(
                        edge.length
                            .numerator()
                            .checked_abs()
                            .ok_or(Error::Overflow)?,
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
                        .ok_or(Error::InvalidAugmentedGraph)?,
                    second: *augmented_to_local_node
                        .get(&edge.second)
                        .ok_or(Error::InvalidAugmentedGraph)?,
                    length: edge.length,
                    weight: ratio(1, 1)?,
                });
            }
        }
        incident_scans.record(metrics)?;
        let graph = SourceDynamicGraph::new(cluster.len(), edges, bound)
            .map_err(|_| Error::InvalidAugmentedGraph)?;
        let dense_root_sources = dense_to_augmented
            .iter()
            .map(|stable| self.edges[*stable].root_source)
            .collect::<Vec<_>>();
        let projection = Rc::new(Snapshot {
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
        self.projection_cache.replace(Some(CachedSnapshot {
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
    pub fn project(&self) -> Result<Snapshot, Error> {
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
                        .ok_or(Error::Overflow)?,
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
            .map_err(|_| Error::InvalidAugmentedGraph)?;
        let dense_root_sources = dense_to_augmented
            .iter()
            .map(|stable| self.edges[*stable].root_source)
            .collect();
        Ok(Snapshot {
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
    ) -> Result<BTreeSet<SourceEdgeId>, Error> {
        if selected_augmented_edges
            .iter()
            .any(|index| self.edges.get(*index).is_none_or(|edge| !edge.active))
        {
            return Err(Error::InvalidAugmentedGraph);
        }
        if selected_augmented_edges.len() + 1 != self.node_count {
            return Err(Error::InvalidAugmentedGraph);
        }
        let mut augmented_connectivity = DisjointSet::new(self.node_count);
        for index in selected_augmented_edges {
            let edge = &self.edges[*index];
            if !augmented_connectivity.union(edge.first.0, edge.second.0) {
                return Err(Error::InvalidAugmentedGraph);
            }
        }
        if !all_connected(&mut augmented_connectivity, self.node_count) {
            return Err(Error::InvalidAugmentedGraph);
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
                    return Err(Error::InvalidAugmentedGraph);
                }
                active_segments[provenance.edge.0] = active_segments[provenance.edge.0]
                    .checked_add(1)
                    .ok_or(Error::Overflow)?;
                if selected_augmented_edges.contains(&index) {
                    selected_segments[provenance.edge.0] = selected_segments[provenance.edge.0]
                        .checked_add(1)
                        .ok_or(Error::Overflow)?;
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
                return Err(Error::InvalidAugmentedGraph);
            }
            let (first, second) = self.original_endpoints[index];
            if !connectivity.union(first.0, second.0) {
                return Err(Error::InvalidAugmentedGraph);
            }
            result.insert(SourceEdgeId(index));
        }
        if result.len() + 1 != self.original_node_count
            || !all_connected(&mut connectivity, self.original_node_count)
        {
            return Err(Error::InvalidAugmentedGraph);
        }
        Ok(result)
    }
}

impl Snapshot {
    fn apply_split_update(&mut self, update: SplitUpdate) -> Result<(), Error> {
        let dense = self
            .dense_to_augmented
            .iter()
            .position(|stable| *stable == update.stable_edge)
            .ok_or(Error::InvalidAugmentedGraph)?;
        let local_from = self.local_node(update.from)?;
        if self.augmented_to_local_node.contains_key(&update.portal) {
            return Err(Error::InvalidAugmentedGraph);
        }
        let expected_portal = FlowNodeId(self.local_to_augmented_node.len());
        let root_source = *self
            .dense_root_sources
            .get(dense)
            .ok_or(Error::InvalidAugmentedGraph)?;
        let symbolic_label = *self
            .dense_symbolic_labels
            .get(dense)
            .ok_or(Error::InvalidAugmentedGraph)?;
        if symbolic_label.root_source != root_source {
            return Err(Error::InvalidAugmentedGraph);
        }
        let original_length = self
            .graph
            .edge(SourceEdgeId(dense))
            .ok_or(Error::InvalidAugmentedGraph)?
            .length;
        let remainder = original_length
            .checked_sub(update.offset)
            .map_err(|_| Error::Overflow)?;
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
            return Err(Error::Overflow);
        }
        let (portal, first, second) = self
            .graph
            .split_projection_edge(SourceEdgeId(dense), local_from, update.offset)
            .map_err(|_| Error::InvalidAugmentedGraph)?;
        if portal != expected_portal || first != SourceEdgeId(dense) {
            return Err(Error::InvalidAugmentedGraph);
        }
        let remove_original = {
            let count = self
                .length_class_counts
                .get_mut(&original_class)
                .ok_or(Error::InvalidAugmentedGraph)?;
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
            return Err(Error::InvalidAugmentedGraph);
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
            return Err(Error::InvalidAugmentedGraph);
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

    pub(in crate::source_an19) fn root_source(
        &self,
        dense: SourceEdgeId,
    ) -> Result<Option<SourceEdgeId>, Error> {
        self.dense_root_sources
            .get(dense.0)
            .copied()
            .ok_or(Error::InvalidAugmentedGraph)
    }

    pub(in crate::source_an19) fn symbolic_label(
        &self,
        dense: SourceEdgeId,
    ) -> Result<SymbolicLengthLabel, Error> {
        self.dense_symbolic_labels
            .get(dense.0)
            .copied()
            .ok_or(Error::InvalidAugmentedGraph)
    }

    pub(in crate::source_an19) fn local_node(
        &self,
        augmented: FlowNodeId,
    ) -> Result<FlowNodeId, Error> {
        self.augmented_to_local_node
            .get(&augmented)
            .copied()
            .ok_or(Error::InvalidAugmentedGraph)
    }

    pub(in crate::source_an19) fn augmented_node(
        &self,
        local: FlowNodeId,
    ) -> Result<FlowNodeId, Error> {
        self.local_to_augmented_node
            .get(local.0)
            .copied()
            .ok_or(Error::InvalidAugmentedGraph)
    }

    pub(in crate::source_an19) fn local_nodes(
        &self,
        augmented: &BTreeSet<FlowNodeId>,
    ) -> Result<BTreeSet<FlowNodeId>, Error> {
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
pub struct Audit {
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

impl Audit {
    pub(in crate::source_an19) fn new(original_edge_count: usize) -> Self {
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

    pub(in crate::source_an19) fn record_portal_split(
        &mut self,
        root_source: Option<SourceEdgeId>,
        metrics: &mut HierarchyMetrics,
    ) -> Result<(), Error> {
        metrics.portal_splits = checked_metric_sum(metrics.portal_splits, 1)?;
        let Some(source) = root_source else {
            self.provenance_free_portal_splits =
                checked_metric_sum(self.provenance_free_portal_splits, 1)?;
            return Ok(());
        };
        let splits = self
            .original_edge_portal_splits
            .get_mut(source.0)
            .ok_or(Error::InvalidWorkCertificate)?;
        *splits = checked_metric_sum(*splits, 1)?;
        self.source_portal_splits = checked_metric_sum(self.source_portal_splits, 1)?;
        metrics.source_portal_splits = checked_metric_sum(metrics.source_portal_splits, 1)?;
        self.maximum_original_edge_portal_splits =
            self.maximum_original_edge_portal_splits.max(*splits);
        metrics.maximum_source_portal_splits = metrics.maximum_source_portal_splits.max(*splits);
        Ok(())
    }

    pub(in crate::source_an19) fn record_scale_sources(
        &mut self,
        projection: &Snapshot,
        new_partition_scale: bool,
        metrics: &mut HierarchyMetrics,
    ) -> Result<(), Error> {
        if !new_partition_scale {
            return Ok(());
        }
        self.scale_observations = checked_metric_sum(self.scale_observations, 1)?;
        metrics.source_scale_attribution_scans = checked_metric_sum(
            metrics.source_scale_attribution_scans,
            u64::try_from(projection.dense_root_sources.len()).map_err(|_| Error::Overflow)?,
        )?;
        for source in projection.dense_root_sources.iter().copied().flatten() {
            let last_observation = self
                .source_last_scale_observation
                .get_mut(source.0)
                .ok_or(Error::InvalidWorkCertificate)?;
            if *last_observation == self.scale_observations {
                continue;
            }
            *last_observation = self.scale_observations;
            let occurrences = self
                .original_edge_scale_occurrences
                .get_mut(source.0)
                .ok_or(Error::InvalidWorkCertificate)?;
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

    pub(in crate::source_an19) fn record_source_materializations(
        &mut self,
        source_segment_counts: Vec<u64>,
        metrics: &mut HierarchyMetrics,
    ) -> Result<(), Error> {
        for (source, segment_count) in source_segment_counts.into_iter().enumerate() {
            if segment_count == 0 {
                continue;
            }
            let materializations = self
                .original_edge_materialization_occurrences
                .get_mut(source)
                .ok_or(Error::InvalidWorkCertificate)?;
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

            let fragment_count = segment_count.checked_sub(1).ok_or(Error::Overflow)?;
            let fragment_occurrences = self
                .original_edge_portal_fragment_occurrences
                .get_mut(source)
                .ok_or(Error::InvalidWorkCertificate)?;
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

    pub(in crate::source_an19) fn record(
        &mut self,
        projection: &Snapshot,
        metrics: &mut HierarchyMetrics,
    ) -> Result<(), Error> {
        if projection.dense_root_sources.len() != projection.graph.edge_count()
            || projection.dense_symbolic_labels.len() != projection.graph.edge_count()
        {
            return Err(Error::InvalidWorkCertificate);
        }
        let edge_count =
            u64::try_from(projection.graph.edge_count()).map_err(|_| Error::Overflow)?;
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
                .ok_or(Error::InvalidAugmentedGraph)?;
            length_classes.insert((edge.length.numerator(), edge.length.denominator()));
            let root_source = projection.root_source(SourceEdgeId(index))?;
            let symbolic_label = projection.symbolic_label(SourceEdgeId(index))?;
            if symbolic_label.root_source != root_source
                || !symbolic_label.unsplit_length.is_positive()
            {
                return Err(Error::InvalidWorkCertificate);
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
                        .ok_or(Error::InvalidWorkCertificate)?;
                    *projection_occurrences = checked_metric_sum(*projection_occurrences, 1)?;
                    let occurrences = self
                        .original_edge_segment_occurrences
                        .get_mut(root.0)
                        .ok_or(Error::InvalidWorkCertificate)?;
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
            return Err(Error::InvalidWorkCertificate);
        }
        let class_count = u64::try_from(length_classes.len()).map_err(|_| Error::Overflow)?;
        let symbolic_source_class_count =
            u64::try_from(symbolic_source_classes.len()).map_err(|_| Error::Overflow)?;
        let symbolic_virtual_class_count =
            u64::try_from(symbolic_virtual_classes.len()).map_err(|_| Error::Overflow)?;
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

    pub(in crate::source_an19) fn observe_projection_shape(
        &mut self,
        projection: &Snapshot,
        metrics: &mut HierarchyMetrics,
    ) -> Result<(), Error> {
        let edge_count =
            u64::try_from(projection.graph.edge_count()).map_err(|_| Error::Overflow)?;
        let class_count =
            u64::try_from(projection.length_class_counts.len()).map_err(|_| Error::Overflow)?;
        let symbolic_source_class_count =
            u64::try_from(projection.symbolic_source_classes.len()).map_err(|_| Error::Overflow)?;
        let symbolic_virtual_class_count = u64::try_from(projection.symbolic_virtual_classes.len())
            .map_err(|_| Error::Overflow)?;
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

    pub(in crate::source_an19) fn verify_structural_charges(
        &self,
        metrics: &HierarchyMetrics,
    ) -> Result<(), Error> {
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
            let fragment_charge = splits.checked_mul(scale_charge).ok_or(Error::Overflow)?;
            if *materializations > scale_charge || *fragments > fragment_charge {
                return Err(Error::InvalidWorkCertificate);
            }
        }
        self.verify_structural_virtual_charges(metrics)
    }

    pub(in crate::source_an19) fn verify_structural_virtual_charges(
        &self,
        metrics: &HierarchyMetrics,
    ) -> Result<(), Error> {
        let virtual_fragments =
            checked_metric_sum(metrics.virtual_leaves, self.provenance_free_portal_splits)?;
        let active_scales = metrics
            .maximum_partition_depth
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        let bound = virtual_fragments
            .checked_mul(source_materialization_charge(active_scales)?)
            .ok_or(Error::Overflow)?;
        if self.provenance_free_segment_occurrences > bound {
            return Err(Error::InvalidWorkCertificate);
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
        metrics: &HierarchyMetrics,
    ) -> Result<(), Error> {
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
            return Err(Error::InvalidWorkCertificate);
        }
        Ok(())
    }
}
