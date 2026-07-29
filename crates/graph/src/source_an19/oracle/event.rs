use std::collections::BTreeSet;

use crate::source_an19::{
    event::{
        backend::{Backend, Kind},
        execution,
        model::{Problem, Run},
        queue,
    },
    petal::{
        Error, MembershipThresholds, PetalMetrics, ShortestPaths, recover_path,
        select_weighted_figure_six_oracle, shortest_paths, weighted_membership_thresholds_oracle,
    },
};
use crate::{FlowNodeId, SourceDynamicGraph, SourceEdgeId};

#[derive(Clone, Copy, Debug, Default)]
pub struct Engine;

impl Backend for Engine {
    fn kind(&self) -> Kind {
        Kind::Oracle
    }

    fn run(&self, problem: &Problem<'_>) -> Result<Run, Error> {
        execution::validate_problem(problem)?;
        let mut metrics = PetalMetrics::default();
        let paths = shortest_paths(problem.graph, problem.cluster, problem.center, &mut metrics)?;
        let path = recover_path(problem.center, problem.target, &paths)?;
        execution::validate_path(problem, &path, &paths, false, &mut metrics)?;
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
        execution::build_run(
            problem,
            self.kind(),
            &execution::Preparation {
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

fn reduced_cost_set(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    paths: &ShortestPaths,
) -> Result<BTreeSet<(i128, i128)>, Error> {
    let mut distinct = BTreeSet::new();
    for index in 0..graph.edge_count() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InvalidDomain)?;
        if !allowed.contains(&edge.first) || !allowed.contains(&edge.second) {
            continue;
        }
        let first = paths.distances[edge.first.0].ok_or(Error::Disconnected)?;
        let second = paths.distances[edge.second.0].ok_or(Error::Disconnected)?;
        for (from, to) in [(first, second), (second, first)] {
            let reduced = edge
                .length
                .checked_add(from)
                .and_then(|value| value.checked_sub(to))
                .and_then(|value| value.checked_mul_integer(2))
                .map_err(|_| Error::Overflow)?;
            if reduced.is_negative() {
                return Err(Error::InvalidHighway);
            }
            distinct.insert((reduced.numerator(), reduced.denominator()));
        }
    }
    Ok(distinct)
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
