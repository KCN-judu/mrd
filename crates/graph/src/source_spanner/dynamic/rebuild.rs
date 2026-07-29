//! Finite Theorem 8.2 update replay using the checked Algorithm 4 subset.

use std::collections::{BTreeMap, BTreeSet};

use crate::ExactRatio;

use super::{
    super::{
        algorithm4::{
            finalize,
            first_embedding::{self, Parameters as FirstParameters},
            witness,
        },
        experiment::{decomposition::single_level, domain::ExhaustiveDomain},
        model::{EdgeId, Embedding, Error as ModelError, Graph},
    },
    batch::{Batch, Error as BatchError, State as BatchState, Transition as BatchTransition},
};

/// Finite checked parameters for a source-shaped decremental spanner replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub phi: ExactRatio,
    pub domain: ExhaustiveDomain,
    pub maximum_hops: usize,
    pub maximum_vertex_congestion: u64,
    pub maximum_rounds: usize,
}

/// The current selected spanner edges and stable-ID embeddings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub selected: BTreeSet<EdgeId>,
    pub embeddings: BTreeMap<EdgeId, Vec<EdgeId>>,
}

/// The current immutable source-shaped decremental spanner state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    pub input: BatchState,
    pub snapshot: Snapshot,
    pub parameters: Parameters,
}

/// One update's batch encoding, selected-edge recourse, and re-embedding set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    pub next: State,
    pub batch: BatchTransition,
    pub added: BTreeSet<EdgeId>,
    pub removed: BTreeSet<EdgeId>,
    pub reembedded: BTreeSet<EdgeId>,
}

impl State {
    /// Initializes the finite source-shaped dynamic spanner replay.
    ///
    /// # Errors
    ///
    /// Returns an error when the initial graph lies outside the certified
    /// Algorithm 4 subset or cannot be fully embedded.
    pub fn new(input: BatchState, parameters: Parameters) -> Result<Self, Error> {
        let snapshot = snapshot(&input, parameters)?;
        Ok(Self {
            input,
            snapshot,
            parameters,
        })
    }

    /// Applies a source batch and derives exact selected-edge and re-embedding recourse.
    ///
    /// # Errors
    ///
    /// Returns an error when the batch or finite Algorithm 4 replay is invalid.
    pub fn apply(&self, batch: &Batch) -> Result<Transition, Error> {
        let applied = self.input.apply(batch).map_err(Error::Batch)?;
        let next_snapshot = snapshot(&applied.next, self.parameters)?;
        let added = next_snapshot
            .selected
            .difference(&self.snapshot.selected)
            .copied()
            .collect();
        let removed = self
            .snapshot
            .selected
            .difference(&next_snapshot.selected)
            .copied()
            .collect();
        let reembedded = reembedded(&self.snapshot.embeddings, &next_snapshot.embeddings);
        Ok(Transition {
            next: State {
                input: applied.next.clone(),
                snapshot: next_snapshot,
                parameters: self.parameters,
            },
            batch: applied,
            added,
            removed,
            reembedded,
        })
    }
}

fn snapshot(input: &BatchState, parameters: Parameters) -> Result<Snapshot, Error> {
    let (graph, stable_ids) = input.active_graph().map_err(Error::Batch)?;
    let decomposition =
        single_level(&graph, parameters.phi, parameters.domain).map_err(Error::Decomposition)?;
    let witness =
        witness::build(&graph, &decomposition, parameters.domain).map_err(Error::Witness)?;
    let identity = identity(&graph)?;
    let first = first_embedding::embed(
        &graph,
        &graph,
        &identity,
        &witness,
        FirstParameters {
            maximum_hops: parameters.maximum_hops,
            maximum_vertex_congestion: parameters.maximum_vertex_congestion,
            maximum_rounds: parameters.maximum_rounds,
        },
    )
    .map_err(Error::FirstEmbedding)?;
    if !first.unembedded.is_empty() {
        return Err(Error::UnembeddedWitness);
    }
    let output =
        finalize::finish(&graph, &graph, &identity, &witness, &first).map_err(Error::Finalize)?;
    let selected = output
        .image
        .iter()
        .map(|edge| stable_ids.get(edge.0).copied().ok_or(Error::StableId))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut stable_embeddings = BTreeMap::new();
    for (relative, stable) in stable_ids.iter().enumerate() {
        let path = output
            .input_to_image
            .path(EdgeId(relative))
            .ok_or(Error::StableId)?
            .iter()
            .map(|edge| stable_ids.get(edge.0).copied().ok_or(Error::StableId))
            .collect::<Result<Vec<_>, _>>()?;
        stable_embeddings.insert(*stable, path);
    }
    Ok(Snapshot {
        selected,
        embeddings: stable_embeddings,
    })
}

fn identity(graph: &Graph) -> Result<Embedding, Error> {
    Embedding::new(
        graph,
        graph,
        None,
        (0..graph.edge_count())
            .map(|edge| vec![EdgeId(edge)])
            .collect(),
    )
    .map_err(Error::Model)
}

/// Derives Algorithm 4's re-embedding set from each surviving source edge's
/// old and new stable-ID image path.
fn reembedded(
    before: &BTreeMap<EdgeId, Vec<EdgeId>>,
    after: &BTreeMap<EdgeId, Vec<EdgeId>>,
) -> BTreeSet<EdgeId> {
    after
        .iter()
        .flat_map(|(source, current)| {
            let previous = before.get(source);
            current
                .iter()
                .filter(move |edge| !previous.is_some_and(|path| path.contains(edge)))
                .copied()
        })
        .collect()
}

/// A source-shaped finite decremental spanner replay cannot proceed.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("source spanner batch is invalid: {0}")]
    Batch(#[source] BatchError),
    #[error("finite expander decomposition is invalid: {0}")]
    Decomposition(#[source] super::super::experiment::domain::Error),
    #[error("finite witness construction is invalid: {0}")]
    Witness(#[source] super::super::algorithm4::witness::Error),
    #[error("finite first embedding is invalid: {0}")]
    FirstEmbedding(#[source] first_embedding::Error),
    #[error("finite Algorithm 4 finalization is invalid: {0}")]
    Finalize(#[source] finalize::Error),
    #[error("finite Algorithm 4 left witness edges unembedded")]
    UnembeddedWitness,
    #[error("active graph lost a stable edge identifier")]
    StableId,
    #[error("identity embedding is invalid: {0}")]
    Model(#[source] ModelError),
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{Parameters, State, reembedded};
    use crate::{
        ExactRatio, FlowNodeId,
        source_spanner::{
            dynamic::batch::{Batch, Operation, State as BatchState},
            experiment::domain::ExhaustiveDomain,
            model::{Edge, EdgeId, Graph},
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
                    first: FlowNodeId(3),
                    second: FlowNodeId(0),
                },
            ],
        )
        .unwrap()
    }

    fn parameters() -> Parameters {
        Parameters {
            phi: ExactRatio::new(1, 4).unwrap(),
            domain: ExhaustiveDomain { maximum_nodes: 8 },
            maximum_hops: 4,
            maximum_vertex_congestion: 100,
            maximum_rounds: 1,
        }
    }

    #[test]
    fn rebuilds_after_a_deletion_with_stable_selected_recourse() {
        let initial = State::new(BatchState::new(&cycle()).unwrap(), parameters()).unwrap();
        let transition = initial
            .apply(&Batch {
                operations: vec![Operation::Delete(EdgeId(2))],
            })
            .unwrap();

        assert!(initial.snapshot.selected.contains(&EdgeId(2)));
        assert!(!transition.next.snapshot.selected.contains(&EdgeId(2)));
        assert!(transition.removed.contains(&EdgeId(2)));
        assert_eq!(transition.batch.deleted_edges, BTreeSet::from([EdgeId(2)]));
        assert!(transition.reembedded.is_empty());
    }

    #[test]
    fn preserves_stable_projection_and_batch_encoding_across_a_smaller_side_split() {
        let initial = State::new(BatchState::new(&cycle()).unwrap(), parameters()).unwrap();
        let transition = initial
            .apply(&Batch {
                operations: vec![Operation::SplitVertex {
                    vertex: FlowNodeId(0),
                    moved_edges: vec![EdgeId(0)],
                }],
            })
            .unwrap();

        assert_eq!(transition.batch.encoding.updates, 1);
        assert_eq!(transition.batch.encoding.encoded_size, 1);
        assert_eq!(transition.batch.new_vertices, vec![FlowNodeId(4)]);
        assert_eq!(
            transition.next.snapshot.selected,
            BTreeSet::from([EdgeId(0), EdgeId(1), EdgeId(2), EdgeId(3)])
        );
        assert!(transition.reembedded.is_empty());
    }

    #[test]
    fn computes_reembedding_by_source_path_not_by_matching_edge_identifier() {
        let before = BTreeMap::from([(EdgeId(0), vec![EdgeId(0)]), (EdgeId(1), vec![EdgeId(1)])]);
        let after = BTreeMap::from([
            (EdgeId(0), vec![EdgeId(2), EdgeId(0)]),
            (EdgeId(1), vec![EdgeId(1)]),
        ]);

        assert_eq!(reembedded(&before, &after), BTreeSet::from([EdgeId(2)]));
    }
}
