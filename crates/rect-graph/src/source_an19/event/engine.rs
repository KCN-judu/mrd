use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::super::petal::{
    Error, ExactHeapEntry, FigureSixSelection, MembershipThresholds, PetalMetrics, RecoveredPath,
    ShortestPaths, fast_shortest_paths, hierarchy_or_oracle_paths, portal_is_interior, ratio,
    ratio_less, recover_path, select_weighted_figure_six_fast, select_weighted_figure_six_oracle,
    shortest_paths, validate_weighted_domain, weighted_membership_thresholds_oracle,
};
use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId};

use super::model::{
    ChargeAnalysis, ChargeKind, Count, Problem, Run, RuntimeStatus, SnapshotMetrics,
    StoppingCertificate,
};
use super::{queue, trace};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    #[serde(rename = "exact_oracle")]
    Exact,
    #[serde(rename = "reduced_exact")]
    Reduced,
    ProvedUnavailable,
}

pub trait Engine {
    fn kind(&self) -> Kind;

    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, trace-consistency, or unsupported
    /// backend error.
    fn run(&self, problem: &Problem<'_>) -> Result<Run, Error>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Exact;

#[derive(Clone, Copy, Debug, Default)]
pub struct Reduced;

#[derive(Clone, Copy, Debug, Default)]
pub struct Proved;

impl Engine for Proved {
    fn kind(&self) -> Kind {
        Kind::ProvedUnavailable
    }

    fn run(&self, _problem: &Problem<'_>) -> Result<Run, Error> {
        Err(Error::UnprovedEventEngine)
    }
}

#[derive(Clone, Copy)]
pub(super) struct ArcWitness {
    edge: SourceEdgeId,
    to: FlowNodeId,
    reduced_cost: ExactRatio,
    orientation: trace::Orientation,
    directed_incidence: usize,
}

struct EnginePreparation {
    path: RecoveredPath,
    center_distances: Vec<Option<ExactRatio>>,
    thresholds: MembershipThresholds,
    selection: FigureSixSelection,
    witnesses: Vec<Option<ArcWitness>>,
    queue_observations: Vec<queue::Observation>,
    queue_statistics: queue::Statistics,
    distinct_reduced_costs: BTreeSet<(i128, i128)>,
}

impl Engine for Exact {
    fn kind(&self) -> Kind {
        Kind::Exact
    }

    fn run(&self, problem: &Problem<'_>) -> Result<Run, Error> {
        validate_event_problem(problem)?;
        let mut metrics = PetalMetrics::default();
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

impl Engine for Reduced {
    fn kind(&self) -> Kind {
        Kind::Reduced
    }

    fn run(&self, problem: &Problem<'_>) -> Result<Run, Error> {
        validate_event_problem(problem)?;
        let mut metrics = PetalMetrics::default();
        let paths =
            fast_shortest_paths(problem.graph, problem.cluster, problem.center, &mut metrics)?;
        let path = recover_path(problem.center, problem.target, &paths)?;
        validate_recovered_path(problem, &path, &paths, true, &mut metrics)?;
        let traced = traced_reduced_thresholds(problem, &path, &paths)?;

        // Cross-check the traced implementation against the existing source-shaped
        // monotone-queue path. This is not an Oracle fallback: disagreement is an
        // explicit error and the traced output is never replaced.
        let fast = super::super::petal::fast_weighted_membership_thresholds(
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
            return Err(Error::InvalidEventTrace);
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

fn validate_event_problem(problem: &Problem<'_>) -> Result<(), Error> {
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
        return Err(Error::InvalidEventTrace);
    }
    for (index, metadata) in problem.segments.iter().enumerate() {
        let edge = problem
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidDomain)?;
        let symbolic = ExactRatio::try_from(metadata.symbolic_unsplit_rounded_length)?;
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

fn validate_recovered_path(
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
    let target_distance = cluster_paths.distances[problem.target.0].ok_or(Error::Disconnected)?;
    if ratio_less(target_distance, problem.budget)? {
        return Err(Error::InvalidRadius);
    }
    Ok(())
}

struct TracedThresholds {
    thresholds: MembershipThresholds,
    witnesses: Vec<Option<ArcWitness>>,
    queue_observations: Vec<queue::Observation>,
    queue_statistics: queue::Statistics,
    distinct_reduced_costs: BTreeSet<(i128, i128)>,
}

#[allow(clippy::too_many_lines)]
fn traced_reduced_thresholds(
    problem: &Problem<'_>,
    path: &RecoveredPath,
    paths: &ShortestPaths,
) -> Result<TracedThresholds, Error> {
    let node_count = problem.graph.node_count();
    let target_distance = paths.distances[problem.target.0].ok_or(Error::Disconnected)?;
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
            .ok_or(Error::InvalidDomain)?;
        let from = path.vertices[path_index + 1];
        let toward_center = path.vertices[path_index];
        let next_distance = distance_from_target
            .checked_add(edge.length)
            .map_err(|_| Error::Overflow)?;
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
                        .map_err(|_| Error::Overflow)?,
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
    let mut queue_observations = seeds
        .iter()
        .cloned()
        .map(|item| queue::Observation {
            item,
            pop_sequence: None,
            stale_reason: None,
            insertion: true,
        })
        .collect::<Vec<_>>();
    let mut statistics = queue::Statistics {
        inserted: u64::try_from(seeds.len()).map_err(|_| Error::Overflow)?,
        ..queue::Statistics::default()
    };
    let mut queue = Vec::with_capacity(seeds.len());
    for seed in seeds {
        queue::push(&mut queue, seed, &mut statistics)?;
        statistics.maximum_size = statistics
            .maximum_size
            .max(u64::try_from(queue.len()).map_err(|_| Error::Overflow)?);
    }
    let mut settled = vec![false; node_count];
    let mut witnesses = vec![None; node_count];
    let mut ordered = Vec::new();
    let mut pop_sequence = 0_u64;
    while let Some(item) = queue::pop(&mut queue, &mut statistics)? {
        pop_sequence = pop_sequence.checked_add(1).ok_or(Error::Overflow)?;
        statistics.popped = statistics.popped.checked_add(1).ok_or(Error::Overflow)?;
        let stale_reason = if settled[item.vertex.0] {
            Some(trace::StaleReason::SettledVertex)
        } else if labels[item.vertex.0] != Some(item.distance) {
            Some(trace::StaleReason::SupersededDistance)
        } else {
            None
        };
        queue_observations.push(queue::Observation {
            item: item.clone(),
            pop_sequence: Some(pop_sequence),
            stale_reason,
            insertion: false,
        });
        if stale_reason.is_some() {
            statistics.stale = statistics.stale.checked_add(1).ok_or(Error::Overflow)?;
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
                .map_err(|_| Error::Overflow)?;
            let improves = match labels[arc.to.0] {
                Some(old) => {
                    statistics.comparisons = statistics
                        .comparisons
                        .checked_add(1)
                        .ok_or(Error::Overflow)?;
                    statistics.relaxation_label_comparisons = statistics
                        .relaxation_label_comparisons
                        .checked_add(1)
                        .ok_or(Error::Overflow)?;
                    if candidate == old {
                        statistics.equal_key_ties = statistics
                            .equal_key_ties
                            .checked_add(1)
                            .ok_or(Error::Overflow)?;
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
                        .ok_or(Error::Overflow)?;
                }
                labels[arc.to.0] = Some(candidate);
                insertion_sequence = insertion_sequence.checked_add(1).ok_or(Error::Overflow)?;
                let queued = queue::Item {
                    distance: candidate,
                    vertex: arc.to,
                    insertion_sequence,
                    predecessor: Some(*arc),
                };
                queue::push(&mut queue, queued.clone(), &mut statistics)?;
                queue_observations.push(queue::Observation {
                    item: queued,
                    pop_sequence: None,
                    stale_reason: None,
                    insertion: true,
                });
                statistics.inserted = statistics.inserted.checked_add(1).ok_or(Error::Overflow)?;
                statistics.maximum_size = statistics
                    .maximum_size
                    .max(u64::try_from(queue.len()).map_err(|_| Error::Overflow)?);
            }
        }
    }
    let mut by_vertex = vec![None; node_count];
    for vertex in problem.remaining {
        let threshold = labels[vertex.0].ok_or(Error::Disconnected)?;
        if threshold.is_negative() {
            return Err(Error::InvalidRadius);
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
    seeds: &mut Vec<queue::Item>,
    insertion_sequence: &mut u64,
) -> Result<(), Error> {
    let two = ratio(2, 1)?;
    let from_center = center_distances[from.0].ok_or(Error::Disconnected)?;
    let toward_center_distance = center_distances[toward_center.0].ok_or(Error::Disconnected)?;
    let potential = target_distance
        .checked_mul(two)
        .and_then(|value| value.checked_sub(radius))
        .map_err(|_| Error::Overflow)?;
    let from_threshold = potential
        .checked_add(offset_from.checked_mul(two).map_err(|_| Error::Overflow)?)
        .and_then(|value| value.checked_sub(from_center.checked_mul(two)?))
        .map_err(|_| Error::Overflow)?;
    let toward_threshold = potential
        .checked_add(
            edge_length
                .checked_sub(offset_from)
                .and_then(|value| value.checked_mul(two))
                .map_err(|_| Error::Overflow)?,
        )
        .and_then(|value| value.checked_sub(toward_center_distance.checked_mul(two)?))
        .map_err(|_| Error::Overflow)?;
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
    seeds: &mut Vec<queue::Item>,
    insertion_sequence: &mut u64,
) -> Result<(), Error> {
    let improves = match labels[vertex.0] {
        Some(old) => ratio_less(distance, old)?,
        None => true,
    };
    if improves {
        labels[vertex.0] = Some(distance);
        *insertion_sequence = insertion_sequence.checked_add(1).ok_or(Error::Overflow)?;
        seeds.push(queue::Item {
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
) -> Result<ReducedAdjacency, Error> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    let mut distinct = BTreeSet::new();
    for index in 0..graph.edge_count() {
        let edge_id = SourceEdgeId(index);
        let edge = graph.edge(edge_id).ok_or(Error::InvalidDomain)?;
        if !allowed.contains(&edge.first) || !allowed.contains(&edge.second) {
            continue;
        }
        let first_distance = center_distances[edge.first.0].ok_or(Error::Disconnected)?;
        let second_distance = center_distances[edge.second.0].ok_or(Error::Disconnected)?;
        let forward = edge
            .length
            .checked_add(first_distance)
            .and_then(|value| value.checked_sub(second_distance))
            .and_then(|value| value.checked_mul_integer(2))
            .map_err(|_| Error::Overflow)?;
        let reverse = edge
            .length
            .checked_add(second_distance)
            .and_then(|value| value.checked_sub(first_distance))
            .and_then(|value| value.checked_mul_integer(2))
            .map_err(|_| Error::Overflow)?;
        if forward.is_negative() || reverse.is_negative() {
            return Err(Error::InvalidHighway);
        }
        distinct.insert((forward.numerator(), forward.denominator()));
        distinct.insert((reverse.numerator(), reverse.denominator()));
        adjacency[edge.first.0].push(ArcWitness {
            edge: edge_id,
            to: edge.second,
            reduced_cost: forward,
            orientation: trace::Orientation::FirstToSecond,
            directed_incidence: index.checked_mul(2).ok_or(Error::Overflow)?,
        });
        adjacency[edge.second.0].push(ArcWitness {
            edge: edge_id,
            to: edge.first,
            reduced_cost: reverse,
            orientation: trace::Orientation::SecondToFirst,
            directed_incidence: index
                .checked_mul(2)
                .and_then(|value| value.checked_add(1))
                .ok_or(Error::Overflow)?,
        });
    }
    Ok((adjacency, distinct))
}

fn reduced_cost_set(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    paths: &ShortestPaths,
) -> Result<BTreeSet<(i128, i128)>, Error> {
    Ok(traced_reduced_adjacency(graph, allowed, &paths.distances)?.1)
}

fn oracle_queue_observations(
    problem: &Problem<'_>,
    thresholds: &MembershipThresholds,
) -> Result<(Vec<queue::Observation>, queue::Statistics), Error> {
    let mut items = problem
        .remaining
        .iter()
        .filter_map(|vertex| {
            thresholds.by_vertex[vertex.0].map(|distance| queue::Item {
                distance,
                vertex: *vertex,
                insertion_sequence: u64::try_from(vertex.0).unwrap_or(u64::MAX),
                predecessor: None,
            })
        })
        .collect::<Vec<_>>();
    let mut statistics = queue::Statistics {
        inserted: u64::try_from(items.len()).map_err(|_| Error::Overflow)?,
        maximum_size: u64::try_from(items.len()).map_err(|_| Error::Overflow)?,
        ..queue::Statistics::default()
    };
    let insertions = items
        .iter()
        .cloned()
        .map(|item| queue::Observation {
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
                .ok_or(Error::Overflow)?;
            if !queue::less(&items[cursor], &items[cursor - 1], &mut statistics)? {
                break;
            }
            items.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
    let mut observations = insertions;
    for (index, item) in items.into_iter().enumerate() {
        let pop_sequence = u64::try_from(index + 1).map_err(|_| Error::Overflow)?;
        observations.push(queue::Observation {
            item,
            pop_sequence: Some(pop_sequence),
            stale_reason: None,
            insertion: false,
        });
        statistics.popped = statistics.popped.checked_add(1).ok_or(Error::Overflow)?;
    }
    Ok((observations, statistics))
}

impl Reduced {
    /// Runs both independent exact paths and returns their fully traced outputs
    /// only when their normalized Figure 6 semantics agree.
    ///
    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, or semantic disagreement error.
    pub fn run_differential(problem: &Problem<'_>) -> Result<(Run, Run), Error> {
        let mut oracle = Exact.run(problem)?;
        let mut reduced = Self.run(problem)?;
        if !oracle.semantically_agrees(&reduced) {
            return Err(Error::InvalidEventTrace);
        }
        oracle.runtime_status.differential_verified = true;
        reduced.runtime_status.exact_oracle_verified = true;
        reduced.runtime_status.differential_verified = true;
        Ok((oracle, reduced))
    }
}

fn build_run(
    problem: &Problem<'_>,
    engine: Kind,
    preparation: &EnginePreparation,
) -> Result<Run, Error> {
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
        Kind::Reduced => Some(super::certificate::build_practical_queue_bound(
            problem,
            &preparation.queue_statistics,
        )?),
        Kind::Exact => None,
        Kind::ProvedUnavailable => {
            return Err(Error::UnprovedEventEngine);
        }
    };
    let charge_analyses = analyze_all_charge_maps(&semantic_trace)?;
    let runtime_status = match engine {
        Kind::Exact => RuntimeStatus {
            semantics_implemented: true,
            exact_oracle_verified: true,
            differential_verified: false,
            trace_complete: true,
            local_event_bound_proved: true,
            global_amortization_proved: false,
            priority_queue_bound_proved: false,
            an19_runtime_verified: false,
        },
        Kind::Reduced => RuntimeStatus {
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
        selected_radius: preparation.selection.radius.into(),
        selected_vertices: selected_vertices.iter().map(|vertex| vertex.0).collect(),
        internal_edge_ids,
        boundary_edge_ids,
        path_edge_ids: preparation.path.edges.iter().map(|edge| edge.0).collect(),
        stopping_certificate: StoppingCertificate {
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
    problem: &Problem<'_>,
    preparation: &EnginePreparation,
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
                    .ok_or(Error::Overflow)?;
            }
            trace.push(make_trace_record(
                problem,
                trace::Kind::VertexEntry,
                radius,
                preparation.witnesses[entry.vertex.0],
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
                let from_distance =
                    preparation.center_distances[entry.vertex.0].ok_or(Error::Disconnected)?;
                let to_distance =
                    preparation.center_distances[other.0].ok_or(Error::Disconnected)?;
                let reduced_cost = edge
                    .length
                    .checked_add(from_distance)
                    .and_then(|value| value.checked_sub(to_distance))
                    .and_then(|value| value.checked_mul_integer(2))
                    .map_err(|_| Error::Overflow)?;
                if reduced_cost.is_negative() {
                    return Err(Error::InvalidHighway);
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
                        trace::Kind::VirtualSegmentEvent,
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
    preparation: &EnginePreparation,
    state: trace::State,
    trace: &mut Vec<trace::Record>,
) -> Result<(), Error> {
    if portal_is_interior(&preparation.thresholds, preparation.selection.radius) {
        trace.push(make_trace_record(
            problem,
            trace::Kind::PortalSplit,
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
            trace::Kind::ContractionRelatedEvent,
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
    let segment_index = explicit_segment.or_else(|| witness.map(|value| value.edge.0));
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
            observation.item.distance,
            observation.item.predecessor,
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
    preparation: &EnginePreparation,
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
        let mut symbolic = ExactRatio::try_from(metadata.symbolic_unsplit_rounded_length)?;
        if metadata.highway_halved {
            symbolic = symbolic
                .checked_mul(ratio(1, 2)?)
                .map_err(|_| Error::Overflow)?;
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

#[cfg(test)]
mod tests {
    use super::super::campaign::{Campaign, Family, adversarial_problems, path_problem};
    use super::super::certificate::{QueueProofScope, QueueStrategy};
    use super::super::model::Ratio;
    use super::*;

    fn assert_rejected(run: &Run, problem: &Problem<'_>) {
        assert_eq!(run.verify_against(problem), Err(Error::InvalidEventTrace));
    }

    #[test]
    fn event_engine_path_snapshot_matches_exact_oracle() {
        let owned = path_problem(16, Family::AllEqualReducedKeys, 0).unwrap();
        let problem = owned.as_problem();
        let (oracle, reduced) = Reduced::run_differential(&problem).unwrap();
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
        assert!(oracle.practical_queue_bound.is_none());
        let practical = reduced.practical_queue_bound.unwrap();
        assert!(practical.observed_total_comparisons <= practical.total_comparison_bound);
        assert!(!practical.an19_priority_queue_target_proved);
        oracle.verify_trace().unwrap();
        reduced.verify_trace().unwrap();
        let mut metrics = PetalMetrics::default();
        let paths =
            shortest_paths(problem.graph, problem.cluster, problem.center, &mut metrics).unwrap();
        for event in reduced.semantic_trace.iter().filter(|event| {
            matches!(
                event.event_type,
                trace::Kind::OutsideToBoundaryEdgeTransition
                    | trace::Kind::BoundaryToInternalEdgeTransition
            )
        }) {
            let edge = problem
                .graph
                .edge(SourceEdgeId(event.active_segment_id.unwrap()))
                .unwrap();
            let (from, to) = match event.orientation.unwrap() {
                trace::Orientation::FirstToSecond => (edge.first, edge.second),
                trace::Orientation::SecondToFirst => (edge.second, edge.first),
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
    fn event_engine_trace_heap_preserves_exact_stable_order() {
        let specifications = [(2, 4, 0), (1, 7, 1), (1, 3, 5), (1, 3, 2), (3, 0, 3)];
        let mut heap = Vec::new();
        let mut statistics = queue::Statistics::default();
        for (distance, vertex, insertion_sequence) in specifications {
            queue::push(
                &mut heap,
                queue::Item {
                    distance: ratio(distance, 1).unwrap(),
                    vertex: FlowNodeId(vertex),
                    insertion_sequence,
                    predecessor: None,
                },
                &mut statistics,
            )
            .unwrap();
        }
        let mut popped = Vec::new();
        while let Some(item) = queue::pop(&mut heap, &mut statistics).unwrap() {
            popped.push((
                item.distance.numerator(),
                item.vertex.0,
                item.insertion_sequence,
            ));
        }
        assert_eq!(
            popped,
            vec![(1, 3, 2), (1, 3, 5), (1, 7, 1), (2, 4, 0), (3, 0, 3)]
        );
        assert!(statistics.equal_key_ties > 0);
        assert_eq!(
            statistics.comparisons,
            statistics.heap_push_comparisons + statistics.heap_pop_comparisons
        );
    }

    #[test]
    fn event_engine_bounded_adversarial_campaign_covers_all_families() {
        let campaign = Campaign::run(
            &Family::ALL,
            &[16, 32],
            "test-sha".to_owned(),
            "bounded-test".to_owned(),
        )
        .unwrap();
        assert!(campaign.cases.iter().all(|case| case.oracle_agreement));
        assert!(campaign.cases.iter().all(|case| {
            case.oracle_run.practical_queue_bound.is_none()
                && case
                    .reduced_run
                    .practical_queue_bound
                    .is_some_and(|certificate| {
                        certificate.observed_total_comparisons <= certificate.total_comparison_bound
                            && !certificate.an19_priority_queue_target_proved
                    })
        }));
        assert!(Family::ALL.iter().all(|family| {
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
        let snapshots = adversarial_problems(Family::HighwayHalvingReorder, 16).unwrap();
        assert_eq!(snapshots.len(), 2);
        let reverse_costs = snapshots
            .iter()
            .map(|snapshot| {
                let problem = snapshot.as_problem();
                let mut metrics = PetalMetrics::default();
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
                                && arc.orientation == trace::Orientation::SecondToFirst
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
        let owned = path_problem(12, Family::AllEqualReducedKeys, 0).unwrap();
        assert_eq!(
            Proved.run(&owned.as_problem()),
            Err(Error::UnprovedEventEngine)
        );
    }

    #[test]
    fn event_engine_exact_ratio_record_serializes_without_floating_point() {
        let record = Ratio {
            numerator: -7,
            denominator: 13,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert_eq!(json, r#"{"numerator":-7,"denominator":13}"#);
        assert_eq!(serde_json::from_str::<Ratio>(&json).unwrap(), record);
        assert_eq!(
            ExactRatio::try_from(record).unwrap(),
            ExactRatio::new(-7, 13).unwrap()
        );
    }

    #[test]
    fn event_engine_trace_mutations_are_rejected() {
        let owned = path_problem(20, Family::RepeatedPortalSplitting, 2).unwrap();
        let problem = owned.as_problem();
        let (_, original) = Reduced::run_differential(&problem).unwrap();

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
        assert_eq!(changed.verify_trace(), Err(Error::InvalidEventTrace));

        let mut changed = original.clone();
        changed
            .local_event_bound
            .priority_queue_comparison_bound_included = true;
        assert_eq!(changed.verify_trace(), Err(Error::InvalidEventTrace));
    }

    #[test]
    fn event_engine_practical_queue_certificate_mutations_are_rejected() {
        let owned = path_problem(20, Family::RepeatedPortalSplitting, 2).unwrap();
        let problem = owned.as_problem();
        let (_, original) = Reduced::run_differential(&problem).unwrap();

        let mutations: [fn(&mut Run); 4] = [
            |run: &mut Run| {
                run.practical_queue_bound
                    .as_mut()
                    .unwrap()
                    .total_comparison_bound += 1;
            },
            |run: &mut Run| {
                run.practical_queue_bound.as_mut().unwrap().strategy =
                    QueueStrategy::LinearMinimumScan;
            },
            |run: &mut Run| {
                run.practical_queue_bound.as_mut().unwrap().proof_scope =
                    QueueProofScope::SourceRuntimeTarget;
            },
            |run: &mut Run| {
                run.practical_queue_bound
                    .as_mut()
                    .unwrap()
                    .an19_priority_queue_target_proved = true;
            },
        ];
        for mutate in mutations {
            let mut changed = original.clone();
            mutate(&mut changed);
            assert_eq!(changed.verify_trace(), Err(Error::InvalidEventTrace));
        }
    }
}
