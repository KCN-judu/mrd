//! Independent bounded simple-path certificates for decremental queries.

use std::cmp::Ordering;

use thiserror::Error;

use crate::FlowNodeId;

use super::{
    super::{model::Error as ModelError, oracle::simple_paths},
    query::{Error as QueryError, Outcome, Path, Response},
    state::{Error as StateError, State},
};

/// A production response with an independent enumerating verification witness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certificate {
    pub response: Response,
}

/// Builds a certificate for a stable breadth-first response.
///
/// # Errors
///
/// Returns an error when a state or endpoint is invalid, or when the production
/// result differs from the bounded simple-path Oracle.
pub fn certify(response: Response, state: &State) -> Result<Certificate, Error> {
    let certificate = Certificate { response };
    certificate.verify(state)?;
    Ok(certificate)
}

impl Certificate {
    /// Recomputes the semantic response with the isolated simple-path Oracle.
    ///
    /// # Errors
    ///
    /// Returns an error when the response differs from the independent Oracle.
    pub fn verify(&self, state: &State) -> Result<(), Error> {
        state.verify().map_err(Error::State)?;
        let expected = oracle_outcome(
            state,
            self.response.start,
            self.response.target,
            self.response.maximum_hops,
        )?;
        if self.response.outcome != expected {
            return Err(Error::Disagreement);
        }
        if let Outcome::Path(path) = &self.response.outcome {
            path.verify(
                state,
                self.response.start,
                self.response.target,
                self.response.maximum_hops,
            )
            .map_err(Error::Query)?;
        }
        Ok(())
    }
}

fn oracle_outcome(
    state: &State,
    start: FlowNodeId,
    target: FlowNodeId,
    maximum_hops: usize,
) -> Result<Outcome, Error> {
    endpoints(state, start, target)?;
    if start == target {
        return Ok(Outcome::Path(Path {
            vertices: vec![start],
            edges: Vec::new(),
        }));
    }
    let all_paths = simple_paths(
        state.graph(),
        start,
        target,
        Some(&state.active_edges()),
        state.graph().node_count().saturating_sub(1).max(1),
    )
    .map_err(Error::Oracle)?;
    let Some(edges) = all_paths.into_iter().min_by(compare_paths) else {
        return Ok(Outcome::Disconnected);
    };
    if edges.len() > maximum_hops {
        return Ok(Outcome::HopBoundExceeded {
            shortest_hops: edges.len(),
        });
    }
    Ok(Outcome::Path(materialize(state, start, &edges)?))
}

fn compare_paths(
    left: &Vec<super::super::model::EdgeId>,
    right: &Vec<super::super::model::EdgeId>,
) -> Ordering {
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

fn materialize(
    state: &State,
    start: FlowNodeId,
    edges: &[super::super::model::EdgeId],
) -> Result<Path, Error> {
    let mut vertices = vec![start];
    let mut current = start;
    for edge_id in edges {
        let edge = state.graph().edge(*edge_id).ok_or(Error::Disagreement)?;
        current = if edge.first == current {
            edge.second
        } else if edge.second == current {
            edge.first
        } else {
            return Err(Error::Disagreement);
        };
        vertices.push(current);
    }
    Ok(Path {
        vertices,
        edges: edges.to_vec(),
    })
}

fn endpoints(state: &State, start: FlowNodeId, target: FlowNodeId) -> Result<(), Error> {
    if start.0 >= state.graph().node_count() || target.0 >= state.graph().node_count() {
        return Err(Error::Query(QueryError::InvalidEndpoint));
    }
    if state.pruned().contains(&start) || state.pruned().contains(&target) {
        return Err(Error::Query(QueryError::PrunedEndpoint));
    }
    Ok(())
}

/// An independent verification cannot establish the supplied response.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("decremental certificate state is invalid: {0}")]
    State(#[source] StateError),
    #[error("decremental certificate query is invalid: {0}")]
    Query(#[source] QueryError),
    #[error("decremental certificate Oracle failed: {0}")]
    Oracle(#[source] ModelError),
    #[error("decremental production response disagrees with the path Oracle")]
    Disagreement,
}

#[cfg(test)]
mod tests {
    use super::{Error, certify};
    use crate::{
        FlowNodeId,
        source_spanner::{
            decremental::{
                query::{Outcome, shortest},
                state::State,
            },
            model::{Edge, EdgeId, Graph},
        },
    };

    fn triangle() -> Graph {
        Graph::new(
            3,
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
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn certifies_production_paths_and_hop_bounds() {
        let state = State::new(triangle()).unwrap().delete(EdgeId(2)).unwrap();
        let response = shortest(&state, FlowNodeId(0), FlowNodeId(2), 2).unwrap();
        certify(response, &state).unwrap();
        let bounded = shortest(&state, FlowNodeId(0), FlowNodeId(2), 1).unwrap();
        assert_eq!(
            bounded.outcome,
            Outcome::HopBoundExceeded { shortest_hops: 2 }
        );
        certify(bounded, &state).unwrap();
    }

    #[test]
    fn rejects_a_mutated_production_response() {
        let state = State::new(triangle()).unwrap();
        let mut response = shortest(&state, FlowNodeId(0), FlowNodeId(2), 1).unwrap();
        response.outcome = Outcome::Disconnected;
        assert_eq!(certify(response, &state), Err(Error::Disagreement));
    }
}
