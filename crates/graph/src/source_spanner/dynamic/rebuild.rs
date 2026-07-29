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

impl Snapshot {
    /// Replays the stable-ID certificate against the current active graph.
    ///
    /// # Errors
    ///
    /// Returns an error when a selected or embedded stable edge is inactive,
    /// an active source edge lacks a path, or the mapped relative certificate
    /// violates the exact static embedding contract.
    pub fn verify(&self, input: &BatchState) -> Result<(), Error> {
        let (graph, stable_ids) = input.active_graph().map_err(Error::Batch)?;
        if self.embeddings.len() != stable_ids.len()
            || self.embeddings.keys().copied().collect::<BTreeSet<_>>()
                != stable_ids.iter().copied().collect()
        {
            return Err(Error::InvalidSnapshot);
        }
        let relative = stable_ids
            .iter()
            .enumerate()
            .map(|(index, stable)| (*stable, EdgeId(index)))
            .collect::<BTreeMap<_, _>>();
        let selected = self
            .selected
            .iter()
            .map(|stable| relative.get(stable).copied().ok_or(Error::InvalidSnapshot))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let paths = stable_ids
            .iter()
            .map(|stable| {
                self.embeddings
                    .get(stable)
                    .ok_or(Error::InvalidSnapshot)?
                    .iter()
                    .map(|edge| relative.get(edge).copied().ok_or(Error::InvalidSnapshot))
                    .collect()
            })
            .collect::<Result<Vec<Vec<_>>, _>>()?;
        Embedding::new(&graph, &graph, Some(&selected), paths).map_err(Error::Model)?;
        Ok(())
    }
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

/// Exact cumulative finite replay measurements, not a Theorem 8.2 bound.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Accounting {
    pub initialization_selected: u64,
    pub batches: u64,
    pub source_updates: u64,
    pub encoded_size: u64,
    pub deletions: u64,
    pub splits: u64,
    pub selected_added: u64,
    pub selected_removed: u64,
    pub reembedded: u64,
}

impl Accounting {
    /// Starts exact accounting from a verified initialization snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the selected edge count cannot fit the source
    /// accounting representation.
    pub fn initialize(state: &State) -> Result<Self, Error> {
        Ok(Self {
            initialization_selected: u64::try_from(state.snapshot.selected.len())
                .map_err(|_| Error::Overflow)?,
            ..Self::default()
        })
    }

    /// Returns accounting extended by one immutable source update transition.
    ///
    /// # Errors
    ///
    /// Returns an error when exact finite counters overflow.
    pub fn record(self, transition: &Transition) -> Result<Self, Error> {
        Ok(Self {
            initialization_selected: self.initialization_selected,
            batches: add(self.batches, 1)?,
            source_updates: add(self.source_updates, transition.batch.encoding.updates)?,
            encoded_size: add(self.encoded_size, transition.batch.encoding.encoded_size)?,
            deletions: add(self.deletions, transition.batch.encoding.deletions)?,
            splits: add(self.splits, transition.batch.encoding.splits)?,
            selected_added: add(
                self.selected_added,
                u64::try_from(transition.added.len()).map_err(|_| Error::Overflow)?,
            )?,
            selected_removed: add(
                self.selected_removed,
                u64::try_from(transition.removed.len()).map_err(|_| Error::Overflow)?,
            )?,
            reembedded: add(
                self.reembedded,
                u64::try_from(transition.reembedded.len()).map_err(|_| Error::Overflow)?,
            )?,
        })
    }
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
    let result = Snapshot {
        selected,
        embeddings: stable_embeddings,
    };
    result.verify(input)?;
    Ok(result)
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

fn add(left: u64, right: u64) -> Result<u64, Error> {
    left.checked_add(right).ok_or(Error::Overflow)
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
    #[error("stable dynamic snapshot is not an active embedding certificate")]
    InvalidSnapshot,
    #[error("identity embedding is invalid: {0}")]
    Model(#[source] ModelError),
    #[error("finite dynamic spanner accounting overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{Accounting, Parameters, State, reembedded};
    use crate::{
        ExactRatio, FlowNodeId,
        source_spanner::{
            dynamic::batch::{Batch, Operation, State as BatchState},
            dynamic::oracle::greedy,
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
        transition
            .next
            .snapshot
            .verify(&transition.next.input)
            .unwrap();
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
        transition
            .next
            .snapshot
            .verify(&transition.next.input)
            .unwrap();
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

    #[test]
    fn records_initialization_and_update_recourse_without_a_runtime_claim() {
        let initial = State::new(BatchState::new(&cycle()).unwrap(), parameters()).unwrap();
        let transition = initial
            .apply(&Batch {
                operations: vec![Operation::Delete(EdgeId(2))],
            })
            .unwrap();
        let accounting = Accounting::initialize(&initial)
            .unwrap()
            .record(&transition)
            .unwrap();

        assert_eq!(accounting.initialization_selected, 4);
        assert_eq!(accounting.batches, 1);
        assert_eq!(accounting.source_updates, 1);
        assert_eq!(accounting.encoded_size, 1);
        assert_eq!(accounting.deletions, 1);
        assert_eq!(accounting.splits, 0);
        assert_eq!(accounting.selected_removed, 1);
        assert_eq!(accounting.reembedded, 0);
    }

    #[test]
    fn differentially_replays_active_source_edges_against_the_greedy_oracle() {
        let initial = State::new(BatchState::new(&cycle()).unwrap(), parameters()).unwrap();
        let oracle_initial = greedy(&initial.input, parameters().maximum_hops).unwrap();
        let transition = initial
            .apply(&Batch {
                operations: vec![Operation::Delete(EdgeId(2))],
            })
            .unwrap();
        let oracle_next = greedy(&transition.next.input, parameters().maximum_hops).unwrap();

        initial.snapshot.verify(&initial.input).unwrap();
        oracle_initial.verify(&initial.input).unwrap();
        transition
            .next
            .snapshot
            .verify(&transition.next.input)
            .unwrap();
        oracle_next.verify(&transition.next.input).unwrap();
        assert_ne!(initial.snapshot.selected, oracle_initial.selected);
        assert_eq!(
            transition
                .next
                .snapshot
                .embeddings
                .keys()
                .copied()
                .collect::<BTreeSet<_>>(),
            oracle_next.embeddings.keys().copied().collect()
        );
        assert!(!transition.next.snapshot.selected.contains(&EdgeId(2)));
        assert!(!oracle_next.selected.contains(&EdgeId(2)));
    }
}
