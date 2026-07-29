//! Algorithm 4 Task 2: finite `W -> J` embedding rounds.

use std::collections::BTreeSet;

use crate::FlowNodeId;

use super::{
    super::{
        decremental::{
            query::{Outcome as QueryOutcome, shortest},
            state::{Outcome as DeletionOutcome, State},
        },
        model::{EdgeId, Embedding, Error as ModelError, Graph},
    },
    witness::Union,
};

/// Explicit finite bounds for Algorithm 4 Task 2 replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub maximum_hops: usize,
    pub maximum_vertex_congestion: u64,
    pub maximum_rounds: usize,
}

/// The persistent `W -> J` paths and every finite replay round.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace {
    pub paths: Vec<Option<Vec<EdgeId>>>,
    pub unembedded: BTreeSet<EdgeId>,
    pub rounds: Vec<Round>,
}

/// One `eta_1` round, including its threshold-induced deletions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Round {
    pub sequence: u64,
    pub embedded: Vec<EdgeId>,
    pub overload_vertices: Vec<FlowNodeId>,
    pub deleted: Vec<EdgeId>,
}

/// Runs the finite `W -> J` path loop against a verified direct `J -> H'` embedding.
///
/// # Errors
///
/// Returns an error for invalid inputs, zero bounds, arithmetic overflow, or an
/// invalid direct embedding. A finite replay that cannot embed every witness
/// edge returns a trace with explicit `unembedded` edges instead of falling back.
pub fn embed(
    host: &Graph,
    input: &Graph,
    input_to_host: &Embedding,
    witness: &Union,
    parameters: Parameters,
) -> Result<Trace, Error> {
    if parameters.maximum_hops == 0
        || parameters.maximum_vertex_congestion == 0
        || parameters.maximum_rounds == 0
        || witness.graph.node_count() != input.node_count()
    {
        return Err(Error::InvalidParameters);
    }
    input_to_host
        .verify(input, host, None)
        .map_err(Error::Model)?;
    let mut paths = vec![None; witness.graph.edge_count()];
    let mut rounds = Vec::new();
    for sequence in 0..parameters.maximum_rounds {
        let mut state = State::new(input.clone()).map_err(Error::State)?;
        let mut embedded = Vec::new();
        let mut overload_vertices = Vec::new();
        let mut deleted = Vec::new();
        for index in 0..witness.graph.edge_count() {
            let witness_edge = EdgeId(index);
            if paths[index].is_some() {
                continue;
            }
            let edge = witness
                .graph
                .edge(witness_edge)
                .ok_or(Error::InvalidWitness)?;
            let path = edge_in(input, edge.first, edge.second)
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
            embedded.push(witness_edge);
            while let Some(overloaded) = first_overload(
                host,
                input,
                input_to_host,
                &paths,
                parameters.maximum_vertex_congestion,
            )? {
                overload_vertices.push(overloaded);
                let candidates = source_edges_through(host, input, input_to_host, overloaded);
                let mut changed = false;
                for edge in candidates {
                    let next = state.delete(edge).map_err(Error::State)?;
                    if next
                        .trace()
                        .last()
                        .is_some_and(|event| event.outcome == DeletionOutcome::Deleted)
                    {
                        deleted.push(edge);
                        changed = true;
                    }
                    state = next;
                }
                if !changed {
                    break;
                }
            }
        }
        rounds.push(Round {
            sequence: u64::try_from(sequence).map_err(|_| Error::Overflow)?,
            embedded,
            overload_vertices,
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

fn edge_in(graph: &Graph, first: FlowNodeId, second: FlowNodeId) -> Option<EdgeId> {
    (0..graph.edge_count()).map(EdgeId).find(|edge_id| {
        graph.edge(*edge_id).is_some_and(|edge| {
            (edge.first == first && edge.second == second)
                || (edge.first == second && edge.second == first)
        })
    })
}

fn first_overload(
    host: &Graph,
    input: &Graph,
    input_to_host: &Embedding,
    paths: &[Option<Vec<EdgeId>>],
    maximum: u64,
) -> Result<Option<FlowNodeId>, Error> {
    let mut loads = vec![0_u64; host.node_count()];
    for path in paths.iter().flatten() {
        for input_edge in path {
            let source = input.edge(*input_edge).ok_or(Error::InvalidWitness)?;
            let mut current = source.first;
            loads[current.0] = loads[current.0].checked_add(1).ok_or(Error::Overflow)?;
            for host_edge in input_to_host
                .path(*input_edge)
                .ok_or(Error::InvalidWitness)?
            {
                let edge = host.edge(*host_edge).ok_or(Error::InvalidWitness)?;
                current = if edge.first == current {
                    edge.second
                } else if edge.second == current {
                    edge.first
                } else {
                    return Err(Error::InvalidWitness);
                };
                loads[current.0] = loads[current.0].checked_add(1).ok_or(Error::Overflow)?;
            }
        }
    }
    Ok(loads
        .into_iter()
        .enumerate()
        .find_map(|(node, load)| (load >= maximum).then_some(FlowNodeId(node))))
}

fn source_edges_through(
    host: &Graph,
    input: &Graph,
    input_to_host: &Embedding,
    vertex: FlowNodeId,
) -> Vec<EdgeId> {
    (0..input.edge_count())
        .map(EdgeId)
        .filter(|edge| input_path_touches(host, input, input_to_host, *edge, vertex))
        .collect()
}

fn input_path_touches(
    host: &Graph,
    input: &Graph,
    embedding: &Embedding,
    edge: EdgeId,
    target: FlowNodeId,
) -> bool {
    let Some(source) = input.edge(edge) else {
        return false;
    };
    let mut current = source.first;
    if current == target {
        return true;
    }
    embedding.path(edge).is_some_and(|path| {
        path.iter().any(|host_edge| {
            let Some(edge) = host.edge(*host_edge) else {
                return false;
            };
            current = if edge.first == current {
                edge.second
            } else if edge.second == current {
                edge.first
            } else {
                return false;
            };
            current == target
        })
    })
}

/// Task 2 cannot replay the supplied finite embedding instance.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("Algorithm 4 Task 2 parameters are invalid")]
    InvalidParameters,
    #[error("Algorithm 4 Task 2 witness is invalid")]
    InvalidWitness,
    #[error("Algorithm 4 Task 2 direct embedding is invalid: {0}")]
    Model(#[source] ModelError),
    #[error("Algorithm 4 Task 2 deletion state is invalid: {0}")]
    State(#[source] super::super::decremental::state::Error),
    #[error("Algorithm 4 Task 2 arithmetic overflowed")]
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
            model::{Edge, EdgeId, Embedding, Graph},
        },
    };

    fn cycle() -> Graph {
        Graph::new(
            4,
            vec![
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                },
                Edge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                },
                Edge {
                    first: FlowNodeId(2),
                    second: FlowNodeId(3),
                },
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(3),
                },
            ],
        )
        .unwrap()
    }

    fn identity(graph: &Graph) -> Embedding {
        Embedding::new(
            graph,
            graph,
            None,
            (0..graph.edge_count())
                .map(|edge| vec![EdgeId(edge)])
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn embeds_the_finite_witness_with_direct_and_bounded_paths() {
        let domain = ExhaustiveDomain { maximum_nodes: 8 };
        let input = cycle();
        let decomposition = single_level(&input, ExactRatio::new(1, 2).unwrap(), domain).unwrap();
        let witness = witness::build(&input, &decomposition, domain).unwrap();
        let trace = embed(
            &input,
            &input,
            &identity(&input),
            &witness,
            Parameters {
                maximum_hops: 2,
                maximum_vertex_congestion: 100,
                maximum_rounds: 1,
            },
        )
        .unwrap();
        assert!(trace.unembedded.is_empty());
        assert_eq!(trace.paths[1].as_ref().unwrap().len(), 2);
    }

    #[test]
    fn records_threshold_induced_deletions() {
        let domain = ExhaustiveDomain { maximum_nodes: 8 };
        let input = cycle();
        let decomposition = single_level(&input, ExactRatio::new(1, 2).unwrap(), domain).unwrap();
        let witness = witness::build(&input, &decomposition, domain).unwrap();
        let trace = embed(
            &input,
            &input,
            &identity(&input),
            &witness,
            Parameters {
                maximum_hops: 2,
                maximum_vertex_congestion: 1,
                maximum_rounds: 1,
            },
        )
        .unwrap();
        assert!(!trace.rounds[0].overload_vertices.is_empty());
        assert!(!trace.rounds[0].deleted.is_empty());
    }
}
