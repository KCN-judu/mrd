//! Stable-id breadth-first paths over one immutable decremental state.

use std::collections::{BTreeSet, VecDeque};

use thiserror::Error;

use crate::FlowNodeId;

use super::{
    super::model::EdgeId,
    state::{Error as StateError, State},
};

/// A deterministic bounded path-query response for one decremental snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Response {
    pub start: FlowNodeId,
    pub target: FlowNodeId,
    pub maximum_hops: usize,
    pub outcome: Outcome,
}

/// The production query outcome after a stable-id breadth-first search.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Outcome {
    Path(Path),
    Disconnected,
    HopBoundExceeded { shortest_hops: usize },
}

/// An explicit simple active path suitable for a later independent certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Path {
    pub vertices: Vec<FlowNodeId>,
    pub edges: Vec<EdgeId>,
}

/// Finds the stable-id breadth-first path in the current deletion state.
///
/// # Errors
///
/// Returns an error for an invalid or pruned endpoint, or a corrupted state.
pub fn shortest(
    state: &State,
    start: FlowNodeId,
    target: FlowNodeId,
    maximum_hops: usize,
) -> Result<Response, Error> {
    state.verify().map_err(Error::State)?;
    endpoints(state, start, target)?;
    let path = breadth_first(state, start, target);
    let outcome = match path {
        None => Outcome::Disconnected,
        Some(path) if path.edges.len() > maximum_hops => Outcome::HopBoundExceeded {
            shortest_hops: path.edges.len(),
        },
        Some(path) => Outcome::Path(path),
    };
    Ok(Response {
        start,
        target,
        maximum_hops,
        outcome,
    })
}

impl Path {
    /// Verifies active-edge contiguity, simplicity, endpoints, and hop bound.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed, inactive, nonsimple, or too-long path.
    pub fn verify(
        &self,
        state: &State,
        start: FlowNodeId,
        target: FlowNodeId,
        maximum_hops: usize,
    ) -> Result<(), Error> {
        endpoints(state, start, target)?;
        if self.vertices.len() != self.edges.len() + 1
            || self.vertices.first() != Some(&start)
            || self.vertices.last() != Some(&target)
            || self.edges.len() > maximum_hops
            || self.vertices.iter().collect::<BTreeSet<_>>().len() != self.vertices.len()
        {
            return Err(Error::InvalidPath);
        }
        for (index, edge_id) in self.edges.iter().enumerate() {
            if !state.edge_is_active(*edge_id) {
                return Err(Error::InvalidPath);
            }
            let edge = state.graph().edge(*edge_id).ok_or(Error::InvalidPath)?;
            let first = self.vertices[index];
            let next = self.vertices[index + 1];
            if !((edge.first == first && edge.second == next)
                || (edge.second == first && edge.first == next))
            {
                return Err(Error::InvalidPath);
            }
        }
        Ok(())
    }
}

fn endpoints(state: &State, start: FlowNodeId, target: FlowNodeId) -> Result<(), Error> {
    if start.0 >= state.graph().node_count() || target.0 >= state.graph().node_count() {
        return Err(Error::InvalidEndpoint);
    }
    if state.pruned().contains(&start) || state.pruned().contains(&target) {
        return Err(Error::PrunedEndpoint);
    }
    Ok(())
}

fn breadth_first(state: &State, start: FlowNodeId, target: FlowNodeId) -> Option<Path> {
    let nodes = state.graph().node_count();
    let mut predecessors: Vec<Option<(FlowNodeId, EdgeId)>> = vec![None; nodes];
    let mut discovered = vec![false; nodes];
    let mut queue = VecDeque::from([start]);
    discovered[start.0] = true;
    while let Some(current) = queue.pop_front() {
        if current == target {
            break;
        }
        for index in 0..state.graph().edge_count() {
            let edge_id = EdgeId(index);
            if !state.edge_is_active(edge_id) {
                continue;
            }
            let edge = state.graph().edge(edge_id)?;
            let next = if edge.first == current {
                edge.second
            } else if edge.second == current {
                edge.first
            } else {
                continue;
            };
            if discovered[next.0] {
                continue;
            }
            discovered[next.0] = true;
            predecessors[next.0] = Some((current, edge_id));
            queue.push_back(next);
        }
    }
    if !discovered[target.0] {
        return None;
    }
    let mut vertices = vec![target];
    let mut edges = Vec::new();
    let mut current = target;
    while current != start {
        let (previous, edge) = predecessors[current.0]?;
        vertices.push(previous);
        edges.push(edge);
        current = previous;
    }
    vertices.reverse();
    edges.reverse();
    Some(Path { vertices, edges })
}

/// A production query cannot be evaluated from the supplied state or endpoints.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("decremental query state is invalid: {0}")]
    State(#[source] StateError),
    #[error("decremental query endpoint is out of range")]
    InvalidEndpoint,
    #[error("decremental query endpoint has been pruned")]
    PrunedEndpoint,
    #[error("decremental query path is invalid")]
    InvalidPath,
}

#[cfg(test)]
mod tests {
    use super::{Error, Outcome, shortest};
    use crate::{
        FlowNodeId,
        source_spanner::{
            decremental::state::State,
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
    fn recomputes_stable_paths_after_deletions() {
        let state = State::new(triangle()).unwrap();
        let direct = shortest(&state, FlowNodeId(0), FlowNodeId(2), 1).unwrap();
        assert_eq!(
            direct.outcome,
            Outcome::Path(super::Path {
                vertices: vec![FlowNodeId(0), FlowNodeId(2)],
                edges: vec![EdgeId(2)],
            })
        );
        let without_direct = state.delete(EdgeId(2)).unwrap();
        let via_middle = shortest(&without_direct, FlowNodeId(0), FlowNodeId(2), 2).unwrap();
        assert_eq!(
            via_middle.outcome,
            Outcome::Path(super::Path {
                vertices: vec![FlowNodeId(0), FlowNodeId(1), FlowNodeId(2)],
                edges: vec![EdgeId(0), EdgeId(1)],
            })
        );
        let bounded = shortest(&without_direct, FlowNodeId(0), FlowNodeId(2), 1).unwrap();
        assert_eq!(
            bounded.outcome,
            Outcome::HopBoundExceeded { shortest_hops: 2 }
        );
    }

    #[test]
    fn rejects_pruned_endpoints() {
        let state = State::new(triangle())
            .unwrap()
            .delete(EdgeId(0))
            .unwrap()
            .delete(EdgeId(2))
            .unwrap();
        assert_eq!(
            shortest(&state, FlowNodeId(0), FlowNodeId(2), 2),
            Err(Error::PrunedEndpoint)
        );
    }
}
