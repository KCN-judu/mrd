//! Pure deletion and smaller-side vertex-split batch replay.

use std::collections::BTreeSet;

use crate::FlowNodeId;

use super::super::model::{Edge, EdgeId, Error as ModelError, Graph};

/// One deletion-only Theorem 8.2 input operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operation {
    Delete(EdgeId),
    SplitVertex {
        vertex: FlowNodeId,
        moved_edges: Vec<EdgeId>,
    },
}

/// An atomically validated source update batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Batch {
    pub operations: Vec<Operation>,
}

/// Exact source-style batch measurements, not an asymptotic bound.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Encoding {
    pub updates: u64,
    pub encoded_size: u64,
    pub deletions: u64,
    pub splits: u64,
}

/// An immutable replayable deletion/split state with stable original edge IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    initial_nodes: usize,
    initial_edges: Vec<Edge>,
    history: Vec<Batch>,
}

/// The verified result of one source update batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    pub next: State,
    pub encoding: Encoding,
    pub new_vertices: Vec<FlowNodeId>,
    pub deleted_edges: BTreeSet<EdgeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Replay {
    nodes: usize,
    edges: Vec<Edge>,
    active: Vec<bool>,
    new_vertices: Vec<FlowNodeId>,
    deleted_edges: BTreeSet<EdgeId>,
}

impl State {
    /// Starts a replayable source update state from a simple graph.
    ///
    /// # Errors
    ///
    /// Returns an error when the graph cannot provide one of its stable edges.
    pub fn new(graph: &Graph) -> Result<Self, Error> {
        let initial_edges = (0..graph.edge_count())
            .map(EdgeId)
            .map(|id| graph.edge(id).ok_or(Error::InvalidBatch))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            initial_nodes: graph.node_count(),
            initial_edges,
            history: Vec::new(),
        })
    }

    /// Applies a nonempty batch atomically and returns the explicit transition.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid IDs, duplicate operations, a non-smaller
    /// split side, or a resulting nonsimple graph.
    pub fn apply(&self, batch: &Batch) -> Result<Transition, Error> {
        if batch.operations.is_empty() {
            return Err(Error::InvalidBatch);
        }
        let before = self.replay()?;
        let mut history = self.history.clone();
        history.push(batch.clone());
        let next = Self {
            initial_nodes: self.initial_nodes,
            initial_edges: self.initial_edges.clone(),
            history,
        };
        let after = next.replay()?;
        let encoding = encoding(batch)?;
        let new_vertices = after.new_vertices[before.new_vertices.len()..].to_vec();
        let deleted_edges = after
            .deleted_edges
            .difference(&before.deleted_edges)
            .copied()
            .collect();
        Ok(Transition {
            next,
            encoding,
            new_vertices,
            deleted_edges,
        })
    }

    /// Rebuilds the active simple graph and stable edge-ID projection.
    ///
    /// # Errors
    ///
    /// Returns an error when the replayed active graph is invalid.
    pub fn active_graph(&self) -> Result<(Graph, Vec<EdgeId>), Error> {
        let replay = self.replay()?;
        let ids = replay
            .active
            .iter()
            .enumerate()
            .filter_map(|(index, active)| active.then_some(EdgeId(index)))
            .collect::<Vec<_>>();
        let edges = ids.iter().map(|id| replay.edges[id.0]).collect();
        Ok((Graph::new(replay.nodes, edges).map_err(Error::Model)?, ids))
    }

    /// Replays the complete persistent history and checks its invariants.
    ///
    /// # Errors
    ///
    /// Returns an error when a historical batch violates the source domain.
    pub fn verify(&self) -> Result<(), Error> {
        self.replay().map(|_| ())
    }

    fn replay(&self) -> Result<Replay, Error> {
        let mut replay = Replay {
            nodes: self.initial_nodes,
            edges: self.initial_edges.clone(),
            active: vec![true; self.initial_edges.len()],
            new_vertices: Vec::new(),
            deleted_edges: BTreeSet::new(),
        };
        Graph::new(replay.nodes, replay.edges.clone()).map_err(Error::Model)?;
        for batch in &self.history {
            if batch.operations.is_empty() {
                return Err(Error::InvalidBatch);
            }
            let mut touched = BTreeSet::new();
            for operation in &batch.operations {
                match operation {
                    Operation::Delete(edge) => {
                        if edge.0 >= replay.edges.len()
                            || !replay.active[edge.0]
                            || !touched.insert(*edge)
                        {
                            return Err(Error::InvalidBatch);
                        }
                        replay.active[edge.0] = false;
                        replay.deleted_edges.insert(*edge);
                    }
                    Operation::SplitVertex {
                        vertex,
                        moved_edges,
                    } => {
                        if vertex.0 >= replay.nodes || moved_edges.is_empty() {
                            return Err(Error::InvalidSplit);
                        }
                        let incident = replay
                            .edges
                            .iter()
                            .enumerate()
                            .filter(|(index, edge)| {
                                replay.active[*index]
                                    && (edge.first == *vertex || edge.second == *vertex)
                            })
                            .map(|(index, _)| EdgeId(index))
                            .collect::<BTreeSet<_>>();
                        let moved = moved_edges.iter().copied().collect::<BTreeSet<_>>();
                        if moved.len() != moved_edges.len()
                            || !moved.is_subset(&incident)
                            || moved.len() > incident.len().saturating_sub(moved.len())
                        {
                            return Err(Error::InvalidSplit);
                        }
                        let split = FlowNodeId(replay.nodes);
                        replay.nodes = replay.nodes.checked_add(1).ok_or(Error::Overflow)?;
                        for edge in moved {
                            let item = replay.edges.get_mut(edge.0).ok_or(Error::InvalidSplit)?;
                            if item.first == *vertex {
                                item.first = split;
                            } else if item.second == *vertex {
                                item.second = split;
                            } else {
                                return Err(Error::InvalidSplit);
                            }
                        }
                        replay.new_vertices.push(split);
                    }
                }
            }
            let active_edges = replay
                .edges
                .iter()
                .enumerate()
                .filter_map(|(index, edge)| replay.active[index].then_some(*edge))
                .collect();
            Graph::new(replay.nodes, active_edges).map_err(Error::Model)?;
        }
        Ok(replay)
    }
}

fn encoding(batch: &Batch) -> Result<Encoding, Error> {
    batch
        .operations
        .iter()
        .try_fold(Encoding::default(), |mut total, operation| {
            total.updates = total.updates.checked_add(1).ok_or(Error::Overflow)?;
            match operation {
                Operation::Delete(_) => {
                    total.deletions = total.deletions.checked_add(1).ok_or(Error::Overflow)?;
                    total.encoded_size =
                        total.encoded_size.checked_add(1).ok_or(Error::Overflow)?;
                }
                Operation::SplitVertex { moved_edges, .. } => {
                    total.splits = total.splits.checked_add(1).ok_or(Error::Overflow)?;
                    total.encoded_size = total
                        .encoded_size
                        .checked_add(
                            u64::try_from(moved_edges.len().max(1)).map_err(|_| Error::Overflow)?,
                        )
                        .ok_or(Error::Overflow)?;
                }
            }
            Ok(total)
        })
}

/// A batch cannot be replayed in the source-shaped decremental domain.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("source spanner batch is empty or has duplicate/inactive operations")]
    InvalidBatch,
    #[error("source spanner vertex split is not an active smaller incident side")]
    InvalidSplit,
    #[error("source spanner dynamic graph is invalid: {0}")]
    Model(#[source] ModelError),
    #[error("source spanner batch accounting overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::{Batch, Error, Operation, State};
    use crate::{
        FlowNodeId,
        source_spanner::model::{Edge, EdgeId, Graph},
    };

    fn star() -> Graph {
        Graph::new(
            4,
            vec![
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                },
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                },
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(3),
                },
            ],
        )
        .unwrap()
    }

    #[test]
    fn replays_delete_and_smaller_side_split_with_stable_ids() {
        let initial = State::new(&star()).unwrap();
        let first = initial
            .apply(&Batch {
                operations: vec![Operation::Delete(EdgeId(2))],
            })
            .unwrap();
        let second = first
            .next
            .apply(&Batch {
                operations: vec![Operation::SplitVertex {
                    vertex: FlowNodeId(0),
                    moved_edges: vec![EdgeId(0)],
                }],
            })
            .unwrap();
        assert_eq!(second.encoding.encoded_size, 1);
        assert_eq!(second.new_vertices, vec![FlowNodeId(4)]);
        let (_, ids) = second.next.active_graph().unwrap();
        assert_eq!(ids, vec![EdgeId(0), EdgeId(1)]);
        second.next.verify().unwrap();
    }

    #[test]
    fn rejects_a_non_smaller_split_side() {
        let state = State::new(&star()).unwrap();
        assert_eq!(
            state.apply(&Batch {
                operations: vec![Operation::SplitVertex {
                    vertex: FlowNodeId(0),
                    moved_edges: vec![EdgeId(0), EdgeId(1)],
                }],
            }),
            Err(Error::InvalidSplit)
        );
    }
}
