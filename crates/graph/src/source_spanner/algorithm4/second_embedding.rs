//! Algorithm 4 Task 3: finite `J -> W` embedding rounds.

use std::collections::BTreeSet;

use crate::FlowNodeId;

use super::{
    super::{
        decremental::{
            query::{Outcome as QueryOutcome, shortest},
            state::{Outcome as DeletionOutcome, State},
        },
        model::{EdgeId, Graph},
    },
    witness::Union,
};

/// Explicit finite bounds for Algorithm 4 Task 3 replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub maximum_hops: usize,
    pub maximum_edge_congestion: u64,
    pub maximum_rounds: usize,
}

/// The persistent `J -> W` paths and every finite replay round.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace {
    pub paths: Vec<Option<Vec<EdgeId>>>,
    pub unembedded: BTreeSet<EdgeId>,
    pub rounds: Vec<Round>,
}

/// One `eta_2` round, including edge-congestion-induced witness deletions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Round {
    pub sequence: u64,
    pub embedded: Vec<EdgeId>,
    pub overloaded_edges: Vec<EdgeId>,
    pub deleted: Vec<EdgeId>,
}

/// Runs Algorithm 4's finite Task 3 path loop on the witness graph.
///
/// # Errors
///
/// Returns an error for invalid parameters or arithmetic overflow. A finite
/// replay that cannot embed every input edge returns its explicit unembedded
/// set instead of replacing the source construction with an Oracle.
pub fn embed(input: &Graph, witness: &Union, parameters: Parameters) -> Result<Trace, Error> {
    if parameters.maximum_hops == 0
        || parameters.maximum_edge_congestion == 0
        || parameters.maximum_rounds == 0
        || witness.graph.node_count() != input.node_count()
    {
        return Err(Error::InvalidParameters);
    }
    let mut paths = vec![None; input.edge_count()];
    let mut rounds = Vec::new();
    for sequence in 0..parameters.maximum_rounds {
        let mut state = State::new(witness.graph.clone()).map_err(Error::State)?;
        let mut embedded = Vec::new();
        let mut overloaded_edges = Vec::new();
        let mut deleted = Vec::new();
        let multiplier = u64::try_from(sequence)
            .map_err(|_| Error::Overflow)?
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        let threshold = parameters
            .maximum_edge_congestion
            .checked_mul(multiplier)
            .ok_or(Error::Overflow)?;
        for index in 0..input.edge_count() {
            if paths[index].is_some() {
                continue;
            }
            let edge = input.edge(EdgeId(index)).ok_or(Error::InvalidInput)?;
            let path = direct_edge(&witness.graph, edge.first, edge.second)
                .filter(|edge| state.edge_is_active(*edge))
                .map(|edge| vec![edge])
                .or_else(|| {
                    match shortest(&state, edge.first, edge.second, parameters.maximum_hops) {
                        Ok(response) => match response.outcome {
                            QueryOutcome::Path(path) => Some(path.edges),
                            QueryOutcome::Disconnected | QueryOutcome::HopBoundExceeded { .. } => {
                                None
                            }
                        },
                        Err(_) => None,
                    }
                });
            let Some(path) = path else {
                continue;
            };
            paths[index] = Some(path);
            embedded.push(EdgeId(index));
            while let Some(overloaded) = first_overload(&paths, threshold)? {
                overloaded_edges.push(overloaded);
                let next = state.delete(overloaded).map_err(Error::State)?;
                if next
                    .trace()
                    .last()
                    .is_some_and(|event| event.outcome == DeletionOutcome::Deleted)
                {
                    deleted.push(overloaded);
                } else {
                    break;
                }
                state = next;
            }
        }
        rounds.push(Round {
            sequence: u64::try_from(sequence).map_err(|_| Error::Overflow)?,
            embedded,
            overloaded_edges,
            deleted,
        });
        if paths.iter().all(Option::is_some)
            || rounds.last().is_none_or(|round| round.embedded.is_empty())
        {
            break;
        }
    }
    let unembedded = paths
        .iter()
        .enumerate()
        .filter_map(|(index, path)| path.is_none().then_some(EdgeId(index)))
        .collect();
    Ok(Trace {
        paths,
        unembedded,
        rounds,
    })
}

fn direct_edge(graph: &Graph, first: FlowNodeId, second: FlowNodeId) -> Option<EdgeId> {
    (0..graph.edge_count()).map(EdgeId).find(|edge_id| {
        graph.edge(*edge_id).is_some_and(|edge| {
            (edge.first == first && edge.second == second)
                || (edge.first == second && edge.second == first)
        })
    })
}

fn first_overload(paths: &[Option<Vec<EdgeId>>], threshold: u64) -> Result<Option<EdgeId>, Error> {
    let maximum_edge = paths.iter().flatten().flatten().map(|edge| edge.0).max();
    let edge_count = maximum_edge
        .map(|index| index.checked_add(1).ok_or(Error::Overflow))
        .transpose()?
        .unwrap_or(0);
    let mut load = vec![0_u64; edge_count];
    for path in paths.iter().flatten() {
        for edge in path {
            load[edge.0] = load[edge.0].checked_add(1).ok_or(Error::Overflow)?;
        }
    }
    Ok(load
        .into_iter()
        .enumerate()
        .find_map(|(edge, load)| (load >= threshold).then_some(EdgeId(edge))))
}

/// Task 3 cannot replay the supplied finite witness instance.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("Algorithm 4 Task 3 parameters are invalid")]
    InvalidParameters,
    #[error("Algorithm 4 Task 3 input is invalid")]
    InvalidInput,
    #[error("Algorithm 4 Task 3 deletion state is invalid: {0}")]
    State(#[source] super::super::decremental::state::Error),
    #[error("Algorithm 4 Task 3 arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::{Parameters, embed};
    use crate::{
        ExactRatio, FlowNodeId,
        source_spanner::{
            algorithm4::witness,
            experiment::{decomposition::single_level, domain::ExhaustiveDomain},
            model::{Edge, Graph},
        },
    };

    fn complete() -> Graph {
        Graph::new(
            5,
            (0..5)
                .flat_map(|first| {
                    ((first + 1)..5).map(move |second| Edge {
                        first: FlowNodeId(first),
                        second: FlowNodeId(second),
                    })
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn embeds_dense_input_through_a_sparse_witness() {
        let input = complete();
        let domain = ExhaustiveDomain { maximum_nodes: 8 };
        let decomposition = single_level(&input, ExactRatio::new(1, 2).unwrap(), domain).unwrap();
        let witness = witness::build(&input, &decomposition, domain).unwrap();
        let trace = embed(
            &input,
            &witness,
            Parameters {
                maximum_hops: 4,
                maximum_edge_congestion: 100,
                maximum_rounds: 1,
            },
        )
        .unwrap();
        assert!(trace.unembedded.is_empty());
        assert!(witness.graph.edge_count() < input.edge_count());
        assert!(trace.paths.iter().all(Option::is_some));
    }

    #[test]
    fn records_edge_congestion_deletions() {
        let input = complete();
        let domain = ExhaustiveDomain { maximum_nodes: 8 };
        let decomposition = single_level(&input, ExactRatio::new(1, 2).unwrap(), domain).unwrap();
        let witness = witness::build(&input, &decomposition, domain).unwrap();
        let trace = embed(
            &input,
            &witness,
            Parameters {
                maximum_hops: 4,
                maximum_edge_congestion: 1,
                maximum_rounds: 1,
            },
        )
        .unwrap();
        assert!(!trace.rounds[0].overloaded_edges.is_empty());
        assert!(!trace.rounds[0].deleted.is_empty());
    }
}
