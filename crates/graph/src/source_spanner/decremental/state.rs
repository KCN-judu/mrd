//! Pure deletion-state transitions with a recomputable monotone pruned set.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::FlowNodeId;

use super::super::model::{EdgeId, Graph};

/// An immutable decremental graph state and its complete request trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    graph: Graph,
    deleted: BTreeSet<EdgeId>,
    pruned: BTreeSet<FlowNodeId>,
    trace: Vec<Deletion>,
}

/// One deletion request and the complete observable transition it caused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deletion {
    pub sequence: u64,
    pub requested: EdgeId,
    pub outcome: Outcome,
    pub pruned_before: BTreeSet<FlowNodeId>,
    pub pruned_after: BTreeSet<FlowNodeId>,
}

/// The deterministic result of one requested deletion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Deleted,
    AlreadyDeleted,
    UnknownEdge,
}

impl State {
    /// Starts from the supplied graph, pruning vertices that are already isolated.
    ///
    /// # Errors
    ///
    /// Returns an error when degree accounting overflows.
    pub fn new(graph: Graph) -> Result<Self, Error> {
        let deleted = BTreeSet::new();
        let pruned = isolated(&graph, &deleted)?;
        Ok(Self {
            graph,
            deleted,
            pruned,
            trace: Vec::new(),
        })
    }

    /// Applies one request and returns the next immutable decremental state.
    ///
    /// # Errors
    ///
    /// Returns an error when the sequence number or degree accounting overflows.
    pub fn delete(&self, requested: EdgeId) -> Result<Self, Error> {
        let outcome = if self.graph.edge(requested).is_none() {
            Outcome::UnknownEdge
        } else if self.deleted.contains(&requested) {
            Outcome::AlreadyDeleted
        } else {
            Outcome::Deleted
        };
        let mut deleted = self.deleted.clone();
        if outcome == Outcome::Deleted {
            deleted.insert(requested);
        }
        let pruned_after = isolated(&self.graph, &deleted)?;
        let mut trace = self.trace.clone();
        trace.push(Deletion {
            sequence: u64::try_from(trace.len()).map_err(|_| Error::Overflow)?,
            requested,
            outcome,
            pruned_before: self.pruned.clone(),
            pruned_after: pruned_after.clone(),
        });
        Ok(Self {
            graph: self.graph.clone(),
            deleted,
            pruned: pruned_after,
            trace,
        })
    }

    #[must_use]
    pub const fn graph(&self) -> &Graph {
        &self.graph
    }

    #[must_use]
    pub const fn deleted(&self) -> &BTreeSet<EdgeId> {
        &self.deleted
    }

    #[must_use]
    pub fn active_edges(&self) -> BTreeSet<EdgeId> {
        (0..self.graph.edge_count())
            .map(EdgeId)
            .filter(|edge| !self.deleted.contains(edge))
            .collect()
    }

    #[must_use]
    pub const fn pruned(&self) -> &BTreeSet<FlowNodeId> {
        &self.pruned
    }

    #[must_use]
    pub fn trace(&self) -> &[Deletion] {
        &self.trace
    }

    #[must_use]
    pub fn edge_is_active(&self, edge: EdgeId) -> bool {
        self.graph.edge(edge).is_some() && !self.deleted.contains(&edge)
    }

    /// Replays every request and checks the complete stored transition trace.
    ///
    /// # Errors
    ///
    /// Returns an error when a sequence number, outcome, deleted set, or
    /// monotone pruned set differs from the deterministic replay.
    pub fn verify(&self) -> Result<(), Error> {
        let mut deleted = BTreeSet::new();
        let mut pruned = isolated(&self.graph, &deleted)?;
        for (index, event) in self.trace.iter().enumerate() {
            if event.sequence != u64::try_from(index).map_err(|_| Error::Overflow)?
                || event.pruned_before != pruned
            {
                return Err(Error::InvalidTrace);
            }
            let expected = if self.graph.edge(event.requested).is_none() {
                Outcome::UnknownEdge
            } else if deleted.contains(&event.requested) {
                Outcome::AlreadyDeleted
            } else {
                Outcome::Deleted
            };
            if event.outcome != expected {
                return Err(Error::InvalidTrace);
            }
            if expected == Outcome::Deleted {
                deleted.insert(event.requested);
            }
            let next_pruned = isolated(&self.graph, &deleted)?;
            if event.pruned_after != next_pruned || !pruned.is_subset(&next_pruned) {
                return Err(Error::InvalidTrace);
            }
            pruned = next_pruned;
        }
        if self.deleted != deleted || self.pruned != pruned {
            return Err(Error::InvalidTrace);
        }
        Ok(())
    }
}

fn isolated(graph: &Graph, deleted: &BTreeSet<EdgeId>) -> Result<BTreeSet<FlowNodeId>, Error> {
    let mut active_degree = vec![0_u64; graph.node_count()];
    for index in 0..graph.edge_count() {
        let edge_id = EdgeId(index);
        if deleted.contains(&edge_id) {
            continue;
        }
        let edge = graph.edge(edge_id).ok_or(Error::InvalidTrace)?;
        active_degree[edge.first.0] = active_degree[edge.first.0]
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        active_degree[edge.second.0] = active_degree[edge.second.0]
            .checked_add(1)
            .ok_or(Error::Overflow)?;
    }
    Ok(active_degree
        .into_iter()
        .enumerate()
        .filter_map(|(node, degree)| (degree == 0).then_some(FlowNodeId(node)))
        .collect())
}

/// A corrupted deletion trace or arithmetic conversion failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("decremental deletion trace is inconsistent")]
    InvalidTrace,
    #[error("decremental trace sequence overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Error, Outcome, State};
    use crate::{
        FlowNodeId,
        source_spanner::model::{Edge, EdgeId, Graph},
    };

    fn path() -> Graph {
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
            ],
        )
        .unwrap()
    }

    #[test]
    fn records_monotone_pruning_and_rejected_requests() {
        let initial = State::new(path()).unwrap();
        assert_eq!(initial.pruned(), &BTreeSet::new());
        let after_first = initial.delete(EdgeId(0)).unwrap();
        assert_eq!(after_first.trace()[0].outcome, Outcome::Deleted);
        assert_eq!(after_first.pruned(), &BTreeSet::from([FlowNodeId(0)]));
        let after_duplicate = after_first.delete(EdgeId(0)).unwrap();
        assert_eq!(after_duplicate.trace()[1].outcome, Outcome::AlreadyDeleted);
        let final_state = after_duplicate.delete(EdgeId(9)).unwrap();
        assert_eq!(final_state.trace()[2].outcome, Outcome::UnknownEdge);
        final_state.verify().unwrap();
    }

    #[test]
    fn rejects_a_tampered_trace() {
        let state = State::new(path()).unwrap().delete(EdgeId(0)).unwrap();
        let mut tampered = state.clone();
        tampered.trace[0].outcome = Outcome::UnknownEdge;
        assert_eq!(tampered.verify(), Err(Error::InvalidTrace));
    }
}
