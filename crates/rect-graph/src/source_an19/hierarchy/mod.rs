use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge};

use super::{
    petal::{
        DisjointSet, Error, HierarchyShortestPaths, PathPoint, PetalMetrics, WeightedPetal,
        all_connected, ceil_log_log, checked_metric_sum, fast_shortest_paths, ratio, ratio_less,
        recover_hierarchy_path,
    },
    projection,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Metrics {
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

pub(super) fn source_materialization_charge(scales: u64) -> Result<u64, Error> {
    // A source segment can enter one full projection at the recursive call,
    // one after the optional imaginary-path mutation, one while preparing its
    // child highway, and one after a same-scale quotient mutation. Cache hits
    // and incremental portal splits do not materialize another projection.
    scales
        .checked_mul(AN19_PROJECTION_MATERIALIZATIONS_PER_SCALE)
        .and_then(|value| value.checked_add(1))
        .ok_or(Error::Overflow)
}

fn projection_incident_scan_total(metrics: &Metrics) -> Result<u64, Error> {
    checked_metric_sum(
        checked_metric_sum(
            metrics.projection_active_internal_incident_scans,
            metrics.projection_active_boundary_incident_scans,
        )?,
        metrics.projection_inactive_incident_scans,
    )
}

fn nonprojection_workspace_scan_total(metrics: &Metrics) -> Result<u64, Error> {
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
    metrics: &Metrics,
) -> Result<u64, Error> {
    let stable_edges = u64::try_from(graph.edge_count())
        .map_err(|_| Error::Overflow)?
        .checked_add(metrics.virtual_leaves)
        .and_then(|value| value.checked_add(metrics.portal_splits.checked_mul(2)?))
        .ok_or(Error::Overflow)?;
    let selected_edges = u64::try_from(graph.node_count())
        .map_err(|_| Error::Overflow)?
        .checked_add(metrics.virtual_leaves)
        .and_then(|value| value.checked_add(metrics.portal_splits))
        .and_then(|value| value.checked_sub(1))
        .ok_or(Error::Overflow)?;
    stable_edges
        .checked_mul(2)
        .and_then(|value| value.checked_add(selected_edges))
        .ok_or(Error::Overflow)
}

fn projection_incident_scan_bounds(
    graph: &SourceDynamicGraph,
    metrics: &Metrics,
    scale_charge: u64,
) -> Result<(u64, u64), Error> {
    // Each source, virtual leaf, or split creates one active segment lineage;
    // an active or inactive segment has two incident references.
    let active_lineages = u64::try_from(graph.edge_count())
        .map_err(|_| Error::Overflow)?
        .checked_add(metrics.virtual_leaves)
        .and_then(|value| value.checked_add(metrics.portal_splits))
        .ok_or(Error::Overflow)?;
    let boundary = active_lineages
        .checked_mul(2)
        .and_then(|value| value.checked_mul(scale_charge))
        .ok_or(Error::Overflow)?;
    let inactive = metrics
        .portal_splits
        .checked_mul(2)
        .and_then(|value| value.checked_mul(scale_charge))
        .ok_or(Error::Overflow)?;
    Ok((boundary, inactive))
}

pub(super) fn source_scale_participation_bound(logarithmic_levels: u64) -> Result<u64, Error> {
    // AN19 Section 6 gives an active radius ratio of at most 2*n^2, while
    // Claims 5--6 shrink child radii by 3/4 and (3/4)^3 < 1/2. This is a
    // checked necessary gate for augmented runs; portal-fragment charging is
    // audited separately before the structural runtime claim can be enabled.
    logarithmic_levels
        .checked_mul(6)
        .and_then(|value| value.checked_add(4))
        .ok_or(Error::Overflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionMode {
    ClusterLocal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LengthMode {
    ExactRational,
    RoundedPowerOfTwo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PriorityQueueMode {
    BinaryHeap,
    ReducedLengthMonotone,
    SourceMonotone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AmortizationMode {
    AggregateRegressionOnly,
    StructuralSourceBound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkCertificate {
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
    pub projection_mode: ProjectionMode,
    pub length_mode: LengthMode,
    pub priority_queue_mode: PriorityQueueMode,
    pub amortization_mode: AmortizationMode,
}

impl WorkCertificate {
    fn build(
        graph: &SourceDynamicGraph,
        unit_input: bool,
        length_mode: LengthMode,
        metrics: &Metrics,
    ) -> Result<Self, Error> {
        let logarithmic_levels =
            u64::from(usize::BITS - graph.node_count().saturating_sub(1).leading_zeros());
        let iterated_logarithmic_levels =
            u64::try_from(ceil_log_log(graph.node_count())).map_err(|_| Error::Overflow)?;
        let source_scale_participation_bound =
            source_scale_participation_bound(logarithmic_levels)?;
        let maximum_work_units = AN19_WORK_BOUND_FACTOR
            .checked_mul(u64::try_from(graph.edge_count().max(1)).map_err(|_| Error::Overflow)?)
            .and_then(|value| value.checked_mul(logarithmic_levels.max(1)))
            .and_then(|value| value.checked_mul(iterated_logarithmic_levels.max(1)))
            .ok_or(Error::Overflow)?;
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
            projection_mode: ProjectionMode::ClusterLocal,
            length_mode,
            priority_queue_mode: PriorityQueueMode::ReducedLengthMonotone,
            amortization_mode: AmortizationMode::AggregateRegressionOnly,
        })
    }

    fn verify(&self, graph: &SourceDynamicGraph, metrics: &Metrics) -> Result<(), Error> {
        let rebuilt = Self::build(
            graph,
            !self.compact_weighted_input,
            LengthMode::RoundedPowerOfTwo,
            metrics,
        )?;
        let scale_charge = source_materialization_charge(self.source_scale_participation_bound)?;
        let (boundary_scan_bound, inactive_scan_bound) =
            projection_incident_scan_bounds(graph, metrics, scale_charge)?;
        let classified_projection_scans = projection_incident_scan_total(metrics)?;
        if *self != rebuilt
            || self.oracle_fallbacks != 0
            || self.numeric_length_expansions != 0
            || self.projection_mode != ProjectionMode::ClusterLocal
            || self.length_mode != LengthMode::RoundedPowerOfTwo
            || self.priority_queue_mode != PriorityQueueMode::ReducedLengthMonotone
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
                    .ok_or(Error::Overflow)?
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
                    .ok_or(Error::InvalidWorkCertificate)?
            || metrics.contraction_retained_edge_scans != metrics.quotient_edges
            || metrics.final_recovery_edge_scans != final_recovery_edge_scan_total(graph, metrics)?
            || metrics.maximum_source_scale_participations > self.source_scale_participation_bound
            || metrics.maximum_source_scale_participations
                > metrics
                    .maximum_partition_depth
                    .checked_add(1)
                    .ok_or(Error::Overflow)?
            || metrics.source_projection_materializations
                > metrics
                    .source_scale_participations
                    .checked_mul(AN19_PROJECTION_MATERIALIZATIONS_PER_SCALE)
                    .and_then(|value| value.checked_add(u64::try_from(graph.edge_count()).ok()?))
                    .ok_or(Error::Overflow)?
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
                    .ok_or(Error::Overflow)?
            || metrics.portal_fragment_materializations > metrics.projected_edge_slots
            || metrics.source_scale_participations > metrics.source_scale_attribution_scans
            || metrics.maximum_projection_nodes == 0
            || metrics.maximum_projection_nodes > metrics.projected_node_slots
            || metrics.directed_region_runs
                != metrics.petals.checked_mul(2).ok_or(Error::Overflow)?
            || (self.compact_weighted_input && metrics.virtual_leaves > metrics.recursion_calls)
        {
            return Err(Error::InvalidWorkCertificate);
        }
        Ok(())
    }

    /// Reports whether the implementation satisfies AN19 Section 7's
    /// power-of-two rounding and monotone-queue runtime prerequisites.
    #[must_use]
    pub const fn source_runtime_verified(&self) -> bool {
        matches!(self.projection_mode, ProjectionMode::ClusterLocal)
            && matches!(self.length_mode, LengthMode::RoundedPowerOfTwo)
            && matches!(self.priority_queue_mode, PriorityQueueMode::SourceMonotone)
            && matches!(
                self.amortization_mode,
                AmortizationMode::StructuralSourceBound
            )
    }
}

fn hierarchy_work_units(metrics: &Metrics) -> Result<u64, Error> {
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
        total.checked_add(value).ok_or(Error::Overflow)
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadiusEdge {
    pub first: FlowNodeId,
    pub second: FlowNodeId,
    pub length: ExactRatio,
    pub root_source: Option<SourceEdgeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RadiusCertificate {
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
    pub edges: Vec<RadiusEdge>,
}

impl RadiusCertificate {
    /// Independently checks the recorded exact shortest-path radius witness.
    ///
    /// # Errors
    ///
    /// Returns an error for incomplete vertices, a violated edge triangle
    /// inequality, a missing tight predecessor, an incorrect maximum radius,
    /// or an inconsistent base-case decision.
    pub fn verify(&self) -> Result<(), Error> {
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
            return Err(Error::InvalidRadiusCertificate);
        }
        let distance_map = self.distances.iter().copied().collect::<BTreeMap<_, _>>();
        if distance_map.len() != self.cluster_size {
            return Err(Error::InvalidRadiusCertificate);
        }
        let mut maximum = ratio(0, 1)?;
        for distance in distance_map.values() {
            if distance.is_negative() {
                return Err(Error::InvalidRadiusCertificate);
            }
            if ratio_less(maximum, *distance)? {
                maximum = *distance;
            }
        }
        if maximum != self.radius {
            return Err(Error::InvalidRadiusCertificate);
        }
        for edge in &self.edges {
            let first = *distance_map
                .get(&edge.first)
                .ok_or(Error::InvalidRadiusCertificate)?;
            let second = *distance_map
                .get(&edge.second)
                .ok_or(Error::InvalidRadiusCertificate)?;
            if !edge.length.is_positive()
                || ratio_less(
                    first
                        .checked_add(edge.length)
                        .map_err(|_| Error::Overflow)?,
                    second,
                )?
                || ratio_less(
                    second
                        .checked_add(edge.length)
                        .map_err(|_| Error::Overflow)?,
                    first,
                )?
            {
                return Err(Error::InvalidRadiusCertificate);
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
                return Err(Error::InvalidRadiusCertificate);
            }
        }
        let expected_base = self.cluster_size <= self.base_vertex_limit
            || self
                .base_threshold
                .at_least(self.radius)
                .map_err(|_| Error::Overflow)?;
        if expected_base != self.base_case {
            return Err(Error::InvalidRadiusCertificate);
        }
        self.verify_contraction()?;
        Ok(())
    }

    fn verify_contraction(&self) -> Result<(), Error> {
        let Some(threshold) = self.contraction_threshold else {
            return if self.contraction_component_of.is_empty() && self.contracted_edge_count == 0 {
                Ok(())
            } else {
                Err(Error::InvalidRadiusCertificate)
            };
        };
        let n = i128::try_from(self.original_node_count).map_err(|_| Error::Overflow)?;
        let n_squared = n.checked_mul(n).ok_or(Error::Overflow)?;
        let expected_threshold = self
            .radius
            .checked_mul(ratio(1, n_squared)?)
            .map_err(|_| Error::Overflow)?;
        if threshold != expected_threshold
            || self.contraction_component_of.len() != self.cluster_size
        {
            return Err(Error::InvalidRadiusCertificate);
        }
        let node_count = self
            .distances
            .iter()
            .map(|(vertex, _)| vertex.0)
            .max()
            .and_then(|value| value.checked_add(1))
            .ok_or(Error::InvalidRadiusCertificate)?;
        let mut connectivity = DisjointSet::new(node_count);
        let mut contracted_edge_count = 0_usize;
        for edge in &self.edges {
            if ratio_less(edge.length, threshold)? {
                connectivity.union(edge.first.0, edge.second.0);
                contracted_edge_count = contracted_edge_count
                    .checked_add(1)
                    .ok_or(Error::Overflow)?;
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
            return Err(Error::InvalidRadiusCertificate);
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
pub struct Lsst {
    pub tree_edges: BTreeSet<SourceEdgeId>,
    pub weighted_stretch: ExactRatio,
    pub total_weight: ExactRatio,
    pub radius_certificates: Vec<RadiusCertificate>,
    pub metrics: Metrics,
    pub projection_audit: projection::Audit,
    pub work_certificate: WorkCertificate,
}

impl Lsst {
    /// Runs the exact hierarchical petal decomposition from `root`.
    ///
    /// # Errors
    ///
    /// Returns an error for a disconnected source graph, an invalid root, a
    /// failed Figure 5 partition, an invalid recovered tree, or exact
    /// arithmetic overflow.
    pub fn construct(graph: &SourceDynamicGraph, root: FlowNodeId) -> Result<Self, Error> {
        if root.0 >= graph.node_count() {
            return Err(Error::InvalidDomain);
        }
        let mut workspace =
            projection::Graph::from_source_with_length_mode(graph, LengthMode::RoundedPowerOfTwo)?;
        let cluster = (0..graph.node_count())
            .map(FlowNodeId)
            .collect::<BTreeSet<_>>();
        let mut metrics = Metrics::default();
        let mut radius_certificates = Vec::new();
        let mut projection_audit = projection::Audit::new(graph.edge_count());
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
            .ok_or(Error::Overflow)?;
        let (weighted_stretch, total_weight) = audit_original_tree_stretch(graph, &tree_edges)?;
        let work_certificate =
            WorkCertificate::build(graph, workspace.unit_input, workspace.length_mode, &metrics)?;
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
    pub fn verify(&self, graph: &SourceDynamicGraph) -> Result<(), Error> {
        let (weighted_stretch, total_weight) =
            audit_original_tree_stretch(graph, &self.tree_edges)?;
        if weighted_stretch != self.weighted_stretch
            || total_weight != self.total_weight
            || self.radius_certificates.is_empty()
            || u64::try_from(self.radius_certificates.len()).map_err(|_| Error::Overflow)?
                != self.metrics.recursion_calls
        {
            return Err(Error::InvalidRadiusCertificate);
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
            return Err(Error::InvalidWorkCertificate);
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
                    u64::try_from(certificate.edges.len()).map_err(|_| Error::Overflow)?,
                )?;
                for edge in &certificate.edges {
                    let Some(source) = edge.root_source else {
                        continue;
                    };
                    let last_observation = source_last_observation
                        .get_mut(source.0)
                        .ok_or(Error::InvalidWorkCertificate)?;
                    if *last_observation == scale_observation {
                        continue;
                    }
                    *last_observation = scale_observation;
                    let occurrences = rebuilt_source_scale_occurrences
                        .get_mut(source.0)
                        .ok_or(Error::InvalidWorkCertificate)?;
                    *occurrences = checked_metric_sum(*occurrences, 1)?;
                }
            }
            match (index, certificate.recursion_parent) {
                (0, None)
                    if certificate.partition_depth == 0 && !certificate.same_scale_contraction => {}
                (0, _) | (_, None) => {
                    return Err(Error::InvalidRadiusCertificate);
                }
                (_, Some(parent_index)) => {
                    let parent = self
                        .radius_certificates
                        .get(parent_index)
                        .filter(|_| parent_index < index)
                        .ok_or(Error::InvalidRadiusCertificate)?;
                    if certificate.same_scale_contraction {
                        if parent.contraction_threshold.is_none()
                            || certificate.partition_depth != parent.partition_depth
                        {
                            return Err(Error::InvalidRadiusCertificate);
                        }
                    } else {
                        let expected_depth = parent
                            .partition_depth
                            .checked_add(1)
                            .ok_or(Error::Overflow)?;
                        let maximum_child_radius = parent
                            .radius
                            .checked_mul(ratio(3, 4)?)
                            .map_err(|_| Error::Overflow)?;
                        if certificate.partition_depth != expected_depth
                            || ratio_less(maximum_child_radius, certificate.radius)?
                        {
                            return Err(Error::InvalidRadiusCertificate);
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
            .ok_or(Error::Overflow)?;
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
                            .map_err(|_| Error::Overflow)?
                            .checked_mul(2)
                            .ok_or(Error::Overflow)?,
                    )
                })?;
        let rebuilt_contraction_input_edge_scans = if self.work_certificate.compact_weighted_input {
            self.radius_certificates
                .iter()
                .filter(|certificate| !certificate.base_case)
                .try_fold(0_u64, |total, certificate| {
                    checked_metric_sum(
                        total,
                        u64::try_from(certificate.edges.len()).map_err(|_| Error::Overflow)?,
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
                let quotient_tree_edges =
                    components.checked_sub(1).ok_or(Error::InvalidContraction)?;
                checked_metric_sum(
                    total,
                    u64::try_from(certificate.contracted_edge_count)
                        .map_err(|_| Error::Overflow)?
                        .checked_add(
                            u64::try_from(quotient_tree_edges)
                                .map_err(|_| Error::Overflow)?
                                .checked_mul(2)
                                .ok_or(Error::Overflow)?,
                        )
                        .ok_or(Error::Overflow)?,
                )
            })?;
        if u64::try_from(contraction_calls).map_err(|_| Error::Overflow)?
            != self.metrics.contraction_calls
            || u64::try_from(contracted_edges).map_err(|_| Error::Overflow)?
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
            return Err(Error::InvalidRadiusCertificate);
        }
        Ok(())
    }
}

struct Piece {
    cluster: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    connection_edge: usize,
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn hierarchical_petal_decomposition(
    workspace: &mut projection::Graph,
    cluster: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    original_node_count: usize,
    recursion_parent: Option<usize>,
    partition_depth: u64,
    same_scale_contraction: bool,
    radius_certificates: &mut Vec<RadiusCertificate>,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<BTreeSet<usize>, Error> {
    metrics.recursion_calls = metrics
        .recursion_calls
        .checked_add(1)
        .ok_or(Error::Overflow)?;
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
        .map_err(|_| Error::Overflow)?;
    let base_vertex_limit = 2;
    let base_case = cluster.len() <= base_vertex_limit
        || threshold.at_least(radius).map_err(|_| Error::Overflow)?;
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
        metrics.base_cases = metrics.base_cases.checked_add(1).ok_or(Error::Overflow)?;
        return hierarchy_shortest_path_tree(&projection, &cluster, center, &paths);
    }
    if !workspace.unit_input {
        add_workspace_edge_scans(
            metrics,
            WorkspaceScanClass::ContractionInput,
            projection.graph().edge_count(),
            1,
        )?;
        let contraction = projection::ShortEdgeContraction::build_with_radius(
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
                    .ok_or(Error::InvalidRadiusCertificate)?,
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
            partition_depth.checked_add(1).ok_or(Error::Overflow)?,
            false,
            radius_certificates,
            metrics,
            projection_audit,
        )?;
        selected.extend(subtree);
        if !selected.insert(piece.connection_edge) {
            return Err(Error::InvalidAugmentedGraph);
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
        partition_depth.checked_add(1).ok_or(Error::Overflow)?,
        false,
        radius_certificates,
        metrics,
        projection_audit,
    )?;
    selected.extend(stigma_tree);
    Ok(selected)
}

fn attach_contraction_certificate(
    certificate: &mut RadiusCertificate,
    projection: &projection::Snapshot,
    contraction: &projection::ShortEdgeContraction,
) -> Result<(), Error> {
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
                .ok_or(Error::InvalidContraction)
        })
        .collect::<Result<_, _>>()?;
    certificate.contracted_edge_count = contraction.contracted_edges.len();
    certificate.verify()
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn hierarchy_contracted_tree(
    projection: &projection::Snapshot,
    contraction: &projection::ShortEdgeContraction,
    center: FlowNodeId,
    target: FlowNodeId,
    original_node_count: usize,
    recursion_parent: usize,
    partition_depth: u64,
    radius_certificates: &mut Vec<RadiusCertificate>,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<BTreeSet<usize>, Error> {
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
            .ok_or(Error::InvalidContraction)?;
        let first = contraction
            .component_of
            .get(edge.first.0)
            .copied()
            .flatten()
            .ok_or(Error::InvalidContraction)?;
        let second = contraction
            .component_of
            .get(edge.second.0)
            .copied()
            .flatten()
            .ok_or(Error::InvalidContraction)?;
        bound = bound
            .max(
                edge.length
                    .numerator()
                    .checked_abs()
                    .ok_or(Error::Overflow)?,
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
            .map_err(|_| Error::InvalidContraction)?;
    let mut quotient_workspace = projection::Graph::from_source_with_inherited_labels(
        &quotient_graph,
        LengthMode::ExactRational,
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
        .ok_or(Error::Overflow)?;
    metrics.contracted_edges = metrics
        .contracted_edges
        .checked_add(
            u64::try_from(contraction.contracted_edges.len()).map_err(|_| Error::Overflow)?,
        )
        .ok_or(Error::Overflow)?;
    metrics.quotient_edges = metrics
        .quotient_edges
        .checked_add(u64::try_from(contraction.retained_edges.len()).map_err(|_| Error::Overflow)?)
        .ok_or(Error::Overflow)?;
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
                .ok_or(Error::InvalidContraction)
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
                .ok_or(Error::InvalidContraction)
        })
        .collect()
}

fn contracted_vertex(
    contraction: &projection::ShortEdgeContraction,
    vertex: FlowNodeId,
) -> Result<FlowNodeId, Error> {
    contraction
        .component_of
        .get(vertex.0)
        .copied()
        .flatten()
        .map(FlowNodeId)
        .ok_or(Error::InvalidContraction)
}

fn petal_decomposition(
    workspace: &mut projection::Graph,
    mut cluster: BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    delta: ExactRatio,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<(BTreeSet<FlowNodeId>, Vec<Piece>, FlowNodeId), Error> {
    let half = ratio(1, 2)?;
    let r0 = delta.checked_mul(half).map_err(|_| Error::Overflow)?;
    let mut remaining = cluster.clone();
    let projection = hierarchy_projection(workspace, &cluster, metrics, projection_audit)?;
    let paths = hierarchy_shortest_paths(&projection, &cluster, center, metrics)?;
    let target_distance = *paths.distances.get(&target).ok_or(Error::Disconnected)?;
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
        .map_err(|_| Error::Overflow)?;
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
        .map_err(|_| Error::Overflow)?;
    let projection = hierarchy_projection(workspace, &cluster, metrics, projection_audit)?;
    let fixed_paths = hierarchy_shortest_paths(&projection, &cluster, center, metrics)?;
    drop(projection);
    loop {
        let mut outside = None;
        for vertex in &remaining {
            let distance = *fixed_paths
                .distances
                .get(vertex)
                .ok_or(Error::Disconnected)?;
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
        return Err(Error::InvalidAugmentedGraph);
    }
    Ok((remaining, pieces, stigma_target))
}

#[allow(clippy::too_many_arguments)]
fn hierarchy_first_target(
    workspace: &mut projection::Graph,
    cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    target_distance: ExactRatio,
    r0: ExactRatio,
    projection: &projection::Snapshot,
    paths: &HierarchyShortestPaths,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<FlowNodeId, Error> {
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
        .map_err(|_| Error::Overflow)?;
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
            .ok_or(Error::Overflow)?;
        extension = extension
            .checked_sub(segment)
            .map_err(|_| Error::Overflow)?;
        if !extension.is_positive() {
            return Ok(virtual_target);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn create_hierarchy_petal(
    workspace: &mut projection::Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<Piece, Error> {
    let projection = hierarchy_projection(workspace, fixed_cluster, metrics, projection_audit)?;
    let local_cluster = projection.local_nodes(fixed_cluster)?;
    let local_remaining = projection.local_nodes(remaining)?;
    let local_center = projection.local_node(center)?;
    let local_target = projection.local_node(target)?;
    let petal = WeightedPetal::construct_for_hierarchy(
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
        PathPoint::Vertex(vertex) => {
            let position = petal
                .at_radius
                .path_from_center
                .iter()
                .position(|candidate| *candidate == vertex)
                .ok_or(Error::InvalidAugmentedGraph)?;
            let dense = position
                .checked_sub(1)
                .and_then(|index| petal.at_radius.path_edges.get(index))
                .ok_or(Error::InvalidAugmentedGraph)?;
            let stable = *projection
                .dense_to_augmented()
                .get(dense.0)
                .ok_or(Error::InvalidAugmentedGraph)?;
            (projection.augmented_node(vertex)?, stable)
        }
        PathPoint::EdgeInterior {
            edge,
            from,
            offset_from,
            ..
        } => {
            let stable = *projection
                .dense_to_augmented()
                .get(edge.0)
                .ok_or(Error::InvalidAugmentedGraph)?;
            let root_source = workspace
                .edges
                .get(stable)
                .filter(|edge| edge.active)
                .ok_or(Error::InvalidAugmentedGraph)?
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
        return Err(Error::InvalidAugmentedGraph);
    }
    for vertex in &petal_vertices {
        remaining.remove(vertex);
    }
    let predecessor = connection_predecessor(workspace, connection_edge, petal_center)?;
    if !remaining.contains(&predecessor) {
        return Err(Error::InvalidAugmentedGraph);
    }
    metrics.petals = metrics.petals.checked_add(1).ok_or(Error::Overflow)?;
    Ok(Piece {
        cluster: petal_vertices,
        center: petal_center,
        target,
        connection_edge,
    })
}

fn add_petal_metrics(hierarchy: &mut Metrics, petal: &PetalMetrics) -> Result<(), Error> {
    hierarchy.shortest_path_runs = hierarchy
        .shortest_path_runs
        .checked_add(petal.shortest_path_runs)
        .ok_or(Error::Overflow)?;
    hierarchy.edge_relaxations = hierarchy
        .edge_relaxations
        .checked_add(petal.edge_relaxations)
        .ok_or(Error::Overflow)?;
    hierarchy.shortest_heap_pushes = hierarchy
        .shortest_heap_pushes
        .checked_add(petal.shortest_heap_pushes)
        .ok_or(Error::Overflow)?;
    hierarchy.shortest_heap_pops = hierarchy
        .shortest_heap_pops
        .checked_add(petal.shortest_heap_pops)
        .ok_or(Error::Overflow)?;
    hierarchy.shortest_edge_scans = hierarchy
        .shortest_edge_scans
        .checked_add(petal.shortest_edge_scans)
        .ok_or(Error::Overflow)?;
    hierarchy.directed_region_runs = hierarchy
        .directed_region_runs
        .checked_add(petal.directed_region_runs)
        .ok_or(Error::Overflow)?;
    hierarchy.directed_heap_pushes = hierarchy
        .directed_heap_pushes
        .checked_add(petal.directed_heap_pushes)
        .ok_or(Error::Overflow)?;
    hierarchy.directed_heap_pops = hierarchy
        .directed_heap_pops
        .checked_add(petal.directed_heap_pops)
        .ok_or(Error::Overflow)?;
    hierarchy.directed_edge_scans = hierarchy
        .directed_edge_scans
        .checked_add(petal.directed_edge_scans)
        .ok_or(Error::Overflow)?;
    hierarchy.membership_sources = hierarchy
        .membership_sources
        .checked_add(petal.membership_sources)
        .ok_or(Error::Overflow)?;
    hierarchy.event_heap_pushes = hierarchy
        .event_heap_pushes
        .checked_add(petal.event_heap_pushes)
        .ok_or(Error::Overflow)?;
    hierarchy.event_heap_pops = hierarchy
        .event_heap_pops
        .checked_add(petal.event_heap_pops)
        .ok_or(Error::Overflow)?;
    hierarchy.heap_comparisons = hierarchy
        .heap_comparisons
        .checked_add(petal.heap_comparisons)
        .ok_or(Error::Overflow)?;
    add_monotone_metrics(hierarchy, petal)?;
    hierarchy.event_vertex_activations = hierarchy
        .event_vertex_activations
        .checked_add(petal.event_vertex_activations)
        .ok_or(Error::Overflow)?;
    hierarchy.event_edge_touches = hierarchy
        .event_edge_touches
        .checked_add(petal.event_edge_touches)
        .ok_or(Error::Overflow)?;
    hierarchy.volume_queries = hierarchy
        .volume_queries
        .checked_add(petal.volume_queries)
        .ok_or(Error::Overflow)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn ensure_vertex_at_distance(
    workspace: &mut projection::Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    distance: ExactRatio,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<FlowNodeId, Error> {
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
    workspace: &mut projection::Graph,
    fixed_cluster: &mut BTreeSet<FlowNodeId>,
    remaining: &mut BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    distance: ExactRatio,
    projection: &projection::Snapshot,
    paths: &HierarchyShortestPaths,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<FlowNodeId, Error> {
    let path = recover_hierarchy_path(center, target, paths)?;
    let mut traversed = ratio(0, 1)?;
    if distance == traversed {
        return Ok(center);
    }
    for (index, dense_edge) in path.edges.iter().enumerate() {
        let edge = projection
            .graph()
            .edge(*dense_edge)
            .ok_or(Error::InvalidAugmentedGraph)?;
        let next_distance = traversed
            .checked_add(edge.length)
            .map_err(|_| Error::Overflow)?;
        if distance == next_distance {
            return path
                .vertices
                .get(index + 1)
                .copied()
                .ok_or(Error::InvalidAugmentedGraph);
        }
        if ratio_less(traversed, distance)? && ratio_less(distance, next_distance)? {
            let from = path.vertices[index];
            let offset = distance
                .checked_sub(traversed)
                .map_err(|_| Error::Overflow)?;
            let stable = *projection
                .dense_to_augmented()
                .get(dense_edge.0)
                .ok_or(Error::InvalidAugmentedGraph)?;
            let root_source = workspace
                .edges
                .get(stable)
                .filter(|edge| edge.active)
                .ok_or(Error::InvalidAugmentedGraph)?
                .root_source;
            let (vertex, _, _) = workspace.split_edge(stable, from, offset)?;
            fixed_cluster.insert(vertex);
            remaining.insert(vertex);
            projection_audit.record_portal_split(root_source, metrics)?;
            return Ok(vertex);
        }
        traversed = next_distance;
    }
    Err(Error::InvalidRadius)
}

fn connection_predecessor(
    workspace: &projection::Graph,
    edge: usize,
    petal_center: FlowNodeId,
) -> Result<FlowNodeId, Error> {
    let edge = workspace
        .edges
        .get(edge)
        .filter(|candidate| candidate.active)
        .ok_or(Error::InvalidAugmentedGraph)?;
    if edge.first == petal_center {
        Ok(edge.second)
    } else if edge.second == petal_center {
        Ok(edge.first)
    } else {
        Err(Error::InvalidAugmentedGraph)
    }
}

pub(super) fn halve_highway(
    workspace: &mut projection::Graph,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<(), Error> {
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
                .ok_or(Error::InvalidAugmentedGraph)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let changes_length = stable_path.iter().try_fold(false, |changes, stable| {
        let edge = workspace
            .edges
            .get(*stable)
            .filter(|edge| edge.active)
            .ok_or(Error::InvalidAugmentedGraph)?;
        Ok::<_, Error>(changes || !edge.halved)
    })?;
    if changes_length {
        workspace.invalidate_projection_cache();
    }
    for stable in stable_path {
        let edge = workspace
            .edges
            .get_mut(stable)
            .filter(|edge| edge.active)
            .ok_or(Error::InvalidAugmentedGraph)?;
        if edge.halved {
            metrics.highway_edges_reused = metrics
                .highway_edges_reused
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            continue;
        }
        edge.length = edge
            .length
            .checked_mul(ratio(1, 2)?)
            .map_err(|_| Error::Overflow)?;
        edge.halved = true;
        metrics.highway_edges_halved = metrics
            .highway_edges_halved
            .checked_add(1)
            .ok_or(Error::Overflow)?;
    }
    Ok(())
}

fn hierarchy_shortest_paths(
    projection: &projection::Snapshot,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    metrics: &mut Metrics,
) -> Result<HierarchyShortestPaths, Error> {
    let local_cluster = projection.local_nodes(cluster)?;
    let local_center = projection.local_node(center)?;
    let mut petal_metrics = PetalMetrics::default();
    let local_paths = fast_shortest_paths(
        projection.graph(),
        &local_cluster,
        local_center,
        &mut petal_metrics,
    )?;
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(petal_metrics.shortest_path_runs)
        .ok_or(Error::Overflow)?;
    metrics.edge_relaxations = metrics
        .edge_relaxations
        .checked_add(petal_metrics.edge_relaxations)
        .ok_or(Error::Overflow)?;
    metrics.shortest_heap_pushes = metrics
        .shortest_heap_pushes
        .checked_add(petal_metrics.shortest_heap_pushes)
        .ok_or(Error::Overflow)?;
    metrics.shortest_heap_pops = metrics
        .shortest_heap_pops
        .checked_add(petal_metrics.shortest_heap_pops)
        .ok_or(Error::Overflow)?;
    metrics.shortest_edge_scans = metrics
        .shortest_edge_scans
        .checked_add(petal_metrics.shortest_edge_scans)
        .ok_or(Error::Overflow)?;
    metrics.heap_comparisons = metrics
        .heap_comparisons
        .checked_add(petal_metrics.heap_comparisons)
        .ok_or(Error::Overflow)?;
    add_monotone_metrics(metrics, &petal_metrics)?;
    let mut distances = BTreeMap::new();
    let mut predecessors = BTreeMap::new();
    for augmented in cluster {
        let local = projection.local_node(*augmented)?;
        distances.insert(
            *augmented,
            local_paths.distances[local.0].ok_or(Error::Disconnected)?,
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

fn add_monotone_metrics(hierarchy: &mut Metrics, petal: &PetalMetrics) -> Result<(), Error> {
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
    workspace: &projection::Graph,
    cluster: &BTreeSet<FlowNodeId>,
    metrics: &mut Metrics,
    projection_audit: &mut projection::Audit,
) -> Result<Rc<projection::Snapshot>, Error> {
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
    metrics: &mut Metrics,
    class: WorkspaceScanClass,
    edge_count: usize,
    multiplier: u64,
) -> Result<(), Error> {
    let scans = u64::try_from(edge_count)
        .map_err(|_| Error::Overflow)?
        .checked_mul(multiplier)
        .ok_or(Error::Overflow)?;
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
) -> Result<ExactRatio, Error> {
    let mut radius = ratio(0, 1)?;
    for vertex in cluster {
        let distance = *paths.distances.get(vertex).ok_or(Error::Disconnected)?;
        if ratio_less(radius, distance)? {
            radius = distance;
        }
    }
    Ok(radius)
}

#[allow(clippy::too_many_arguments)]
fn build_radius_certificate(
    projection: &projection::Snapshot,
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
) -> Result<RadiusCertificate, Error> {
    let distances = cluster
        .iter()
        .map(|vertex| {
            paths
                .distances
                .get(vertex)
                .copied()
                .map(|distance| (*vertex, distance))
                .ok_or(Error::Disconnected)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut edges = Vec::new();
    for index in 0..projection.graph().edge_count() {
        let edge = projection
            .graph()
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidAugmentedGraph)?;
        edges.push(RadiusEdge {
            first: projection.augmented_node(edge.first)?,
            second: projection.augmented_node(edge.second)?,
            length: edge.length,
            root_source: projection.root_source(SourceEdgeId(index))?,
        });
    }
    let certificate = RadiusCertificate {
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
    projection: &projection::Snapshot,
    cluster: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    paths: &HierarchyShortestPaths,
) -> Result<BTreeSet<usize>, Error> {
    let mut tree = BTreeSet::new();
    for vertex in cluster {
        if *vertex == center {
            continue;
        }
        let (_, dense) = paths
            .predecessors
            .get(vertex)
            .copied()
            .ok_or(Error::Disconnected)?;
        tree.insert(
            *projection
                .dense_to_augmented()
                .get(dense.0)
                .ok_or(Error::InvalidAugmentedGraph)?,
        );
    }
    if tree.len() + 1 != cluster.len() {
        return Err(Error::InvalidAugmentedGraph);
    }
    Ok(tree)
}

fn hierarchy_base_threshold(node_count: usize) -> Result<ExactRatio, Error> {
    ratio(
        i128::try_from(hierarchy_base_vertex_limit(node_count)?).map_err(|_| Error::Overflow)?,
        1,
    )
}

fn minimum_cluster_edge_length(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
) -> Result<ExactRatio, Error> {
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

fn hierarchy_base_vertex_limit(node_count: usize) -> Result<usize, Error> {
    let log_n = usize::BITS - node_count.saturating_sub(1).leading_zeros();
    let log_log_n = ceil_log_log(node_count);
    usize::try_from(log_n)
        .ok()
        .and_then(|value| value.checked_mul(log_log_n))
        .and_then(|value| value.checked_mul(10))
        .ok_or(Error::Overflow)
}

fn audit_original_tree_stretch(
    graph: &SourceDynamicGraph,
    tree: &BTreeSet<SourceEdgeId>,
) -> Result<(ExactRatio, ExactRatio), Error> {
    if tree.len() + 1 != graph.node_count() {
        return Err(Error::InvalidAugmentedGraph);
    }
    let mut adjacency = vec![Vec::<(usize, SourceEdgeId)>::new(); graph.node_count()];
    let mut connectivity = DisjointSet::new(graph.node_count());
    for edge_id in tree {
        let edge = graph.edge(*edge_id).ok_or(Error::InvalidAugmentedGraph)?;
        if !connectivity.union(edge.first.0, edge.second.0) {
            return Err(Error::InvalidAugmentedGraph);
        }
        adjacency[edge.first.0].push((edge.second.0, *edge_id));
        adjacency[edge.second.0].push((edge.first.0, *edge_id));
    }
    if !all_connected(&mut connectivity, graph.node_count()) {
        return Err(Error::InvalidAugmentedGraph);
    }
    let distance_index = OriginalTreeDistanceIndex::build(graph, &adjacency)?;
    let mut weighted_stretch = ratio(0, 1)?;
    let mut total_weight = ratio(0, 1)?;
    for index in 0..graph.edge_count() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidAugmentedGraph)?;
        let distance = distance_index.distance(edge.first, edge.second)?;
        let stretch = distance
            .checked_mul(edge.length.reciprocal().map_err(|_| Error::Overflow)?)
            .map_err(|_| Error::Overflow)?;
        let source_stretch = stretch
            .checked_add(ratio(1, 1)?)
            .map_err(|_| Error::Overflow)?;
        weighted_stretch = weighted_stretch
            .checked_add(
                edge.weight
                    .checked_mul(source_stretch)
                    .map_err(|_| Error::Overflow)?,
            )
            .map_err(|_| Error::Overflow)?;
        total_weight = total_weight
            .checked_add(edge.weight)
            .map_err(|_| Error::Overflow)?;
    }
    Ok((weighted_stretch, total_weight))
}

fn tree_audit_work_units(graph: &SourceDynamicGraph) -> Result<u64, Error> {
    let nodes = u64::try_from(graph.node_count()).map_err(|_| Error::Overflow)?;
    let edges = u64::try_from(graph.edge_count()).map_err(|_| Error::Overflow)?;
    let levels = u64::from(usize::BITS - graph.node_count().saturating_sub(1).leading_zeros())
        .checked_add(1)
        .ok_or(Error::Overflow)?;
    nodes
        .checked_mul(levels)
        .and_then(|value| value.checked_add(nodes.saturating_sub(1).checked_mul(3)?))
        .and_then(|value| value.checked_add(edges.checked_mul(levels.checked_add(1)?)?))
        .ok_or(Error::Overflow)
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
    ) -> Result<Self, Error> {
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
                    return Err(Error::InvalidAugmentedGraph);
                }
                let edge = graph.edge(*edge_id).ok_or(Error::InvalidAugmentedGraph)?;
                parent[*next] = node;
                depth[*next] = depth[node].checked_add(1).ok_or(Error::Overflow)?;
                distance_from_root[*next] = distance_from_root[node]
                    .checked_add(edge.length)
                    .map_err(|_| Error::Overflow)?;
                stack.push(*next);
            }
        }
        if parent.contains(&usize::MAX) {
            return Err(Error::InvalidAugmentedGraph);
        }
        let levels: usize =
            usize::try_from(usize::BITS - node_count.saturating_sub(1).leading_zeros())
                .map_err(|_| Error::Overflow)?
                .checked_add(1)
                .ok_or(Error::Overflow)?;
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

    fn distance(&self, first: FlowNodeId, second: FlowNodeId) -> Result<ExactRatio, Error> {
        let ancestor = self.lowest_common_ancestor(first.0, second.0)?;
        self.distance_from_root[first.0]
            .checked_add(self.distance_from_root[second.0])
            .and_then(|value| {
                self.distance_from_root[ancestor]
                    .checked_mul_integer(2)
                    .and_then(|shared| value.checked_sub(shared))
            })
            .map_err(|_| Error::Overflow)
    }

    fn lowest_common_ancestor(&self, mut first: usize, mut second: usize) -> Result<usize, Error> {
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
            .ok_or(Error::InvalidAugmentedGraph)
    }
}
