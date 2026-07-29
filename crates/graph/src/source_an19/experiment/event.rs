use std::collections::BTreeSet;

use crate::source_an19::{
    event::{
        backend::{Backend, Kind},
        execution,
        model::{Problem, Run},
        queue, trace,
    },
    oracle,
    petal::{
        Error, ExactHeapEntry, MembershipThresholds, PetalMetrics, RecoveredPath, ShortestPaths,
        fast_shortest_paths, ratio, ratio_less, recover_path, select_weighted_figure_six_fast,
    },
};
use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId};

#[derive(Clone, Copy, Debug, Default)]
pub struct Engine;

impl Backend for Engine {
    fn kind(&self) -> Kind {
        Kind::Experiment
    }

    fn run(&self, problem: &Problem<'_>) -> Result<Run, Error> {
        execution::validate_problem(problem)?;
        let mut metrics = PetalMetrics::default();
        let paths =
            fast_shortest_paths(problem.graph, problem.cluster, problem.center, &mut metrics)?;
        let path = recover_path(problem.center, problem.target, &paths)?;
        execution::validate_path(problem, &path, &paths, true, &mut metrics)?;
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
        execution::build_run(
            problem,
            self.kind(),
            &execution::Preparation {
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

struct TracedThresholds {
    thresholds: MembershipThresholds,
    witnesses: Vec<Option<execution::ArcWitness>>,
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

type ReducedAdjacency = (Vec<Vec<execution::ArcWitness>>, BTreeSet<(i128, i128)>);

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
        adjacency[edge.first.0].push(execution::ArcWitness {
            edge: edge_id,
            to: edge.second,
            reduced_cost: forward,
            orientation: trace::Orientation::FirstToSecond,
            directed_incidence: index.checked_mul(2).ok_or(Error::Overflow)?,
        });
        adjacency[edge.second.0].push(execution::ArcWitness {
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

impl Engine {
    /// Runs both independent exact paths and returns their fully traced outputs
    /// only when their normalized Figure 6 semantics agree.
    ///
    /// # Errors
    ///
    /// Returns an exact domain, arithmetic, or semantic disagreement error.
    pub fn run_differential(problem: &Problem<'_>) -> Result<(Run, Run), Error> {
        let mut oracle = oracle::event::Engine.run(problem)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_an19::event::backend::Unavailable;
    use crate::source_an19::event::campaign::{
        Campaign, Family, adversarial_problems, path_problem,
    };
    use crate::source_an19::event::certificate::{QueueProofScope, QueueStrategy};
    use crate::source_an19::event::model::Ratio;
    use crate::source_an19::petal::shortest_paths;

    fn assert_rejected(run: &Run, problem: &Problem<'_>) {
        assert_eq!(run.verify_against(problem), Err(Error::InvalidEventTrace));
    }

    #[test]
    fn event_engine_path_snapshot_matches_exact_oracle() {
        let owned = path_problem(16, Family::AllEqualReducedKeys, 0).unwrap();
        let problem = owned.as_problem();
        let (oracle, reduced) = Engine::run_differential(&problem).unwrap();
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
            Unavailable.run(&owned.as_problem()),
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
        let (_, original) = Engine::run_differential(&problem).unwrap();

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
        let (_, original) = Engine::run_differential(&problem).unwrap();

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
