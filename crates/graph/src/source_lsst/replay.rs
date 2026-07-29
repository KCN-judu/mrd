//! Immutable finite update replay for the Section 9.1 tree-chain experiment.

use std::collections::BTreeSet;

use crate::{ExactRatio, FlowNodeId};

use super::{
    LsfPiece, LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId, SourceLsstError,
    SourceUpdateBatch,
    chain::{Chain, Error as ChainError, Parameters as ChainParameters},
};

/// Explicit finite replay parameters; none imply a source runtime bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub chain: ChainParameters,
    pub batches_before_scheduled_rebuild: u64,
}

/// Exact observed work and tree-recourse counters for immutable replay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Accounting {
    pub initial_tree_edges: u64,
    pub snapshots: u64,
    pub source_batches: u64,
    pub source_encoded_update_size: u64,
    pub full_snapshot_rebuilds: u64,
    pub scheduled_rebuilds: u64,
    pub tree_added: u64,
    pub tree_removed: u64,
    pub maximum_tree_edges: u64,
}

/// Immutable current source graph, source history, and finite tree chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    initial: SourceDynamicGraph,
    history: Vec<SourceUpdateBatch>,
    graph: SourceDynamicGraph,
    pub chain: Chain,
    pub accounting: Accounting,
    pub updates_since_scheduled_rebuild: u64,
    pub parameters: Parameters,
}

/// One replayed source batch and exact source-tree recourse sets.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    pub next: State,
    pub added_tree_edges: BTreeSet<SourceEdgeId>,
    pub removed_tree_edges: BTreeSet<SourceEdgeId>,
    pub scheduled_rebuild: bool,
}

impl State {
    /// Initializes the finite update replay at a checked source graph snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for zero rebuild interval or when the initial finite
    /// chain lies outside its explicit source domain.
    pub fn new(initial: SourceDynamicGraph, parameters: Parameters) -> Result<Self, Error> {
        if parameters.batches_before_scheduled_rebuild == 0 {
            return Err(Error::InvalidParameters);
        }
        materialize(initial, Vec::new(), parameters)
    }

    #[must_use]
    pub const fn graph(&self) -> &SourceDynamicGraph {
        &self.graph
    }

    #[must_use]
    pub fn history(&self) -> &[SourceUpdateBatch] {
        &self.history
    }

    /// Applies one source batch by replaying the complete immutable history.
    ///
    /// # Errors
    ///
    /// Returns an error when a source operation is invalid, the updated graph
    /// leaves the finite chain domain, or exact recourse accounting overflows.
    pub fn apply(&self, batch: &SourceUpdateBatch) -> Result<Transition, Error> {
        let mut history = self.history.clone();
        history.push(batch.clone());
        let next = materialize(self.initial.clone(), history, self.parameters)?;
        let added_tree_edges = next
            .chain
            .tree_edges
            .difference(&self.chain.tree_edges)
            .copied()
            .collect();
        let removed_tree_edges = self
            .chain
            .tree_edges
            .difference(&next.chain.tree_edges)
            .copied()
            .collect();
        let scheduled_rebuild =
            next.accounting.scheduled_rebuilds > self.accounting.scheduled_rebuilds;
        Ok(Transition {
            next,
            added_tree_edges,
            removed_tree_edges,
            scheduled_rebuild,
        })
    }

    /// Independently rebuilds this state from its initial graph and history.
    ///
    /// # Errors
    ///
    /// Returns an error when any stored batch or derived exact certificate
    /// disagrees with fresh replay evidence.
    pub fn verify(&self) -> Result<(), Error> {
        if &materialize(self.initial.clone(), self.history.clone(), self.parameters)? != self {
            return Err(Error::InvalidReplay);
        }
        Ok(())
    }
}

fn materialize(
    initial: SourceDynamicGraph,
    history: Vec<SourceUpdateBatch>,
    parameters: Parameters,
) -> Result<State, Error> {
    let mut graph = initial.clone();
    let initial_forest = singleton_forest(&graph)?;
    let mut chain =
        Chain::build(&graph, &initial_forest, parameters.chain).map_err(Error::Chain)?;
    let initial_tree_edges = u64::try_from(chain.tree_edges.len()).map_err(|_| Error::Overflow)?;
    let mut accounting = Accounting {
        initial_tree_edges,
        snapshots: 1,
        maximum_tree_edges: initial_tree_edges,
        ..Accounting::default()
    };
    let mut updates_since_scheduled_rebuild = 0_u64;
    for batch in &history {
        let previous_tree = chain.tree_edges.clone();
        graph.apply_batch(batch).map_err(Error::Source)?;
        let forest = singleton_forest(&graph)?;
        chain = Chain::build(&graph, &forest, parameters.chain).map_err(Error::Chain)?;
        let added = chain.tree_edges.difference(&previous_tree).count();
        let removed = previous_tree.difference(&chain.tree_edges).count();
        accounting.snapshots = accounting.snapshots.checked_add(1).ok_or(Error::Overflow)?;
        accounting.full_snapshot_rebuilds = accounting
            .full_snapshot_rebuilds
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        accounting.tree_added = accounting
            .tree_added
            .checked_add(u64::try_from(added).map_err(|_| Error::Overflow)?)
            .ok_or(Error::Overflow)?;
        accounting.tree_removed = accounting
            .tree_removed
            .checked_add(u64::try_from(removed).map_err(|_| Error::Overflow)?)
            .ok_or(Error::Overflow)?;
        accounting.maximum_tree_edges = accounting
            .maximum_tree_edges
            .max(u64::try_from(chain.tree_edges.len()).map_err(|_| Error::Overflow)?);
        let elapsed = updates_since_scheduled_rebuild
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        if elapsed >= parameters.batches_before_scheduled_rebuild {
            accounting.scheduled_rebuilds = accounting
                .scheduled_rebuilds
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            updates_since_scheduled_rebuild = 0;
        } else {
            updates_since_scheduled_rebuild = elapsed;
        }
    }
    accounting.source_batches = u64::try_from(history.len()).map_err(|_| Error::Overflow)?;
    accounting.source_encoded_update_size = graph.metrics().encoded_update_size;
    Ok(State {
        initial,
        history,
        graph,
        chain,
        accounting,
        updates_since_scheduled_rebuild,
        parameters,
    })
}

fn singleton_forest(graph: &SourceDynamicGraph) -> Result<LsfStructuralCertificate, Error> {
    let roots = (0..graph.node_count())
        .map(FlowNodeId)
        .collect::<BTreeSet<_>>();
    let pieces = roots
        .iter()
        .copied()
        .map(|root| LsfPiece {
            vertices: BTreeSet::from([root]),
            forest_edges: BTreeSet::new(),
        })
        .collect();
    let stretch_overestimates = (0..graph.edge_count())
        .map(|_| ExactRatio::new(1, 1).map_err(|_| Error::Overflow))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LsfStructuralCertificate {
        forest_edges: BTreeSet::new(),
        roots,
        pieces,
        stretch_overestimates,
        piece_volume_limit: 0,
    })
}

/// The finite immutable dynamic tree replay cannot proceed.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("finite dynamic tree replay parameters are invalid")]
    InvalidParameters,
    #[error("source graph update is invalid: {0}")]
    Source(#[source] SourceLsstError),
    #[error("finite tree-chain snapshot is invalid: {0}")]
    Chain(#[source] ChainError),
    #[error("immutable dynamic tree replay disagrees with fresh evidence")]
    InvalidReplay,
    #[error("finite dynamic tree replay accounting overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Parameters, State};
    use crate::{
        ExactRatio, FlowNodeId,
        source_lsf::oracle::Lsst as Oracle,
        source_lsst::{
            SourceDynamicGraph, SourceEdgeId, SourceGraphUpdate, SourceUpdateBatch,
            SourceWeightedEdge, bucket::Parameters as BucketParameters,
            chain::Parameters as ChainParameters,
        },
        source_spanner::{
            dynamic::rebuild::Parameters as RebuildParameters, experiment::domain::ExhaustiveDomain,
        },
    };

    fn edge(first: usize, second: usize) -> SourceWeightedEdge {
        SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(1, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        }
    }

    fn parameters() -> Parameters {
        Parameters {
            chain: ChainParameters {
                root: FlowNodeId(0),
                maximum_integral_length: 8,
                buckets: BucketParameters {
                    maximum_absolute_exponent: 4,
                    spanner: RebuildParameters {
                        phi: ExactRatio::new(1, 4).unwrap(),
                        domain: ExhaustiveDomain { maximum_nodes: 8 },
                        maximum_hops: 4,
                        maximum_vertex_congestion: 100,
                        maximum_rounds: 1,
                    },
                },
            },
            batches_before_scheduled_rebuild: 1,
        }
    }

    fn triangle() -> SourceDynamicGraph {
        SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap()
    }

    fn assert_oracle(state: &State) {
        let oracle = Oracle::solve(state.graph()).unwrap();
        assert!(
            state
                .chain
                .tree_audit
                .weighted_stretch
                .at_least(oracle.weighted_stretch)
                .unwrap()
        );
        assert_eq!(state.chain.tree_audit.total_weight, oracle.total_weight);
    }

    #[test]
    fn replays_a_connected_deletion_with_tree_recourse_and_oracle_evidence() {
        let initial = State::new(triangle(), parameters()).unwrap();
        let transition = initial
            .apply(&SourceUpdateBatch {
                updates: vec![SourceGraphUpdate::Delete(SourceEdgeId(2))],
            })
            .unwrap();

        assert_eq!(
            transition.next.chain.tree_edges,
            BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)])
        );
        assert!(transition.scheduled_rebuild);
        assert_eq!(transition.next.accounting.source_batches, 1);
        assert_eq!(transition.next.accounting.source_encoded_update_size, 1);
        assert_eq!(transition.next.accounting.full_snapshot_rebuilds, 1);
        assert_oracle(&transition.next);
        transition.next.verify().unwrap();
    }

    #[test]
    fn replays_a_smaller_side_split_without_losing_stable_tree_ids() {
        let initial = State::new(triangle(), parameters()).unwrap();
        let transition = initial
            .apply(&SourceUpdateBatch {
                updates: vec![SourceGraphUpdate::SplitVertex {
                    vertex: FlowNodeId(1),
                    moved_edges: vec![SourceEdgeId(1)],
                }],
            })
            .unwrap();

        assert_eq!(transition.next.graph().node_count(), 4);
        assert_eq!(transition.next.graph().metrics().vertex_splits, 1);
        assert_eq!(
            transition.next.chain.tree_edges,
            BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1), SourceEdgeId(2)])
        );
        assert_oracle(&transition.next);
        transition.next.verify().unwrap();
    }

    #[test]
    fn replays_an_insertion_and_keeps_oracle_outside_the_construction_path() {
        let initial = State::new(
            SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2)], 8).unwrap(),
            parameters(),
        )
        .unwrap();
        let transition = initial
            .apply(&SourceUpdateBatch {
                updates: vec![SourceGraphUpdate::Insert(edge(0, 2))],
            })
            .unwrap();

        assert_eq!(transition.next.graph().metrics().edge_insertions, 1);
        assert_eq!(transition.next.history().len(), 1);
        assert_oracle(&transition.next);
        transition.next.verify().unwrap();
    }
}
