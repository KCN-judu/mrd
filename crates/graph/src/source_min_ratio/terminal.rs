//! Source-maintained terminal-tree candidate declarations.
//!
//! This module materializes one exact AN19-shaped static tree for one immutable
//! IPM projection. It turns every source edge outside that tree into its unique
//! terminal fundamental-cycle declaration. It deliberately does not construct
//! core/spanner embeddings, enumerate graph cycles, or assert a runtime bound.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::{CirculationNetwork, FlowNodeId, SourceEdgeId};

use super::{
    candidate::{CandidateId, Context, Error as CandidateError, Fundamental, Kind, Registry},
    chain::{Chain, Error as ChainError, Selection, Shifts},
    cycle::{Cycle, Direction, Segment},
    input::{Error as InputError, Input, Materialization},
    model::{Branch, BranchId, Level, LevelId, Tree as SourceTree},
};

use crate::source_an19::experiment::hierarchy::Lsst;

/// Immutable terminal-tree projection for one exact source/IPM snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tree {
    input: Input,
    materialization: Materialization,
    hierarchy: Lsst,
    chain: Chain,
    shifts: Shifts,
    candidates: Vec<Fundamental>,
    root: FlowNodeId,
}

impl Tree {
    /// Builds one checked source-shaped terminal tree and its fundamental
    /// non-tree declarations.
    ///
    /// The AN19 hierarchy supplies the maintained tree edge set. Each source
    /// edge outside that set contributes its unique tree-path-plus-edge compact
    /// cycle. This is direct construction from maintained source state, not a
    /// graph-cycle enumeration.
    ///
    /// # Errors
    ///
    /// Returns an error if the current input cannot materialize, the static
    /// source hierarchy cannot construct a tree, or the tree cannot form a
    /// checked one-level chain.
    pub fn build(
        input: Input,
        network: &CirculationNetwork,
        root: FlowNodeId,
    ) -> Result<Self, Error> {
        let materialization = input.materialize(network)?;
        let hierarchy = Lsst::construct(&materialization.graph, root)?;
        let tree = hierarchy.tree_edges.clone();
        let chain = Chain::new(
            &materialization.graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    SourceTree::new(root, tree.clone()),
                )],
            )],
        )?;
        let shifts = chain.initial_shifts();
        let selection = selected_terminal_branch(&chain, &shifts)?;
        let candidates = declarations(&materialization, &tree, selection)?;
        Ok(Self {
            input,
            materialization,
            hierarchy,
            chain,
            shifts,
            candidates,
            root,
        })
    }

    /// Rebuilds the exact source tree and declarations to validate this snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the network no longer matches the input projection
    /// or any rebuilt source-derived field differs.
    pub fn verify(&self, network: &CirculationNetwork) -> Result<(), Error> {
        self.hierarchy.verify(&self.materialization.graph)?;
        let rebuilt = Self::build(self.input.clone(), network, self.root)?;
        if &rebuilt == self {
            Ok(())
        } else {
            Err(Error::MismatchedSnapshot)
        }
    }

    /// Returns the exact input projection that originated this tree snapshot.
    #[must_use]
    pub const fn input(&self) -> &Input {
        &self.input
    }

    /// Returns the jointly materialized source graph and circulation bindings.
    #[must_use]
    pub const fn materialization(&self) -> &Materialization {
        &self.materialization
    }

    /// Returns the static AN19-shaped source-tree evidence.
    #[must_use]
    pub const fn hierarchy(&self) -> &Lsst {
        &self.hierarchy
    }

    /// Returns the checked one-level source tree chain.
    #[must_use]
    pub const fn chain(&self) -> &Chain {
        &self.chain
    }

    /// Returns the immutable selected branch state for this terminal tree.
    #[must_use]
    pub const fn shifts(&self) -> &Shifts {
        &self.shifts
    }

    /// Returns the source-declared terminal fundamental candidates in stable
    /// source-edge order.
    #[must_use]
    pub fn candidates(&self) -> &[Fundamental] {
        &self.candidates
    }

    /// Creates the checked candidate context for this exact snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the supplied network no longer matches this
    /// materialized input projection.
    pub fn context<'a>(&'a self, network: &'a CirculationNetwork) -> Result<Context<'a>, Error> {
        Ok(Context::new(
            &self.input,
            &self.materialization,
            &self.chain,
            &self.shifts,
            network,
        )?)
    }

    /// Creates the exact heap over this source-maintained terminal population.
    ///
    /// This does not add core/spanner candidates or perform cross-snapshot
    /// maintenance; those remain separate construction work.
    ///
    /// # Errors
    ///
    /// Returns an error when the source projection no longer matches the
    /// network or a declaration cannot be exactly evaluated.
    pub fn registry(&self, network: &CirculationNetwork) -> Result<Registry, Error> {
        let context = self.context(network)?;
        Ok(Registry::new(&context, self.candidates.clone())?)
    }
}

fn selected_terminal_branch(chain: &Chain, shifts: &Shifts) -> Result<Selection, Error> {
    chain
        .select(shifts)?
        .into_iter()
        .next()
        .ok_or(Error::MissingSelection)
}

fn declarations(
    materialization: &Materialization,
    tree: &BTreeSet<SourceEdgeId>,
    selection: Selection,
) -> Result<Vec<Fundamental>, Error> {
    let mut candidates = Vec::new();
    for index in 0..materialization.graph.edge_count() {
        let source = SourceEdgeId(index);
        if tree.contains(&source) {
            continue;
        }
        let edge = materialization
            .graph
            .edge(source)
            .ok_or(Error::MissingSourceEdge(source))?;
        candidates.push(Fundamental {
            id: CandidateId(source.0),
            kind: Kind::FundamentalTree { source },
            cycle: Cycle {
                segments: vec![
                    Segment::TreePath {
                        selection,
                        from: edge.first,
                        to: edge.second,
                    },
                    Segment::OffTree {
                        source,
                        direction: Direction::Reverse,
                    },
                ],
            },
        });
    }
    Ok(candidates)
}

/// A terminal-tree projection or declaration could not be constructed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("terminal source input failed: {0}")]
    Input(#[from] InputError),
    #[error("AN19-shaped terminal tree construction failed: {0}")]
    Hierarchy(#[from] crate::source_an19::petal::Error),
    #[error("terminal source tree chain failed: {0}")]
    Chain(#[from] ChainError),
    #[error("terminal candidate evaluation failed: {0}")]
    Candidate(#[from] CandidateError),
    #[error("terminal source tree has no selected branch")]
    MissingSelection,
    #[error("terminal source tree refers to missing source edge {0:?}")]
    MissingSourceEdge(SourceEdgeId),
    #[error("rebuilt terminal source snapshot differs from stored evidence")]
    MismatchedSnapshot,
}

#[cfg(test)]
mod tests {
    use super::Tree;
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, SourceEdgeId,
        source_min_ratio::{candidate::Kind, input::Input},
    };

    fn ratio(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    fn network() -> CirculationNetwork {
        let mut network = CirculationNetwork::new(3);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 3, 0).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(2), 3, 0).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(2), 3, 0).unwrap();
        network
    }

    #[test]
    fn emits_exact_terminal_cycles_from_the_source_tree_without_enumeration() {
        let network = network();
        let input = Input::new(
            &network,
            &[ratio(1), ratio(4), ratio(16)],
            &[ratio(1), ratio(1), ratio(1)],
            &[ratio(1), ratio(1), ratio(1)],
        )
        .unwrap();
        let tree = Tree::build(input, &network, FlowNodeId(0)).unwrap();
        tree.verify(&network).unwrap();

        assert_eq!(tree.hierarchy().tree_edges.len(), 2);
        assert_eq!(tree.candidates().len(), 1);
        let declaration = &tree.candidates()[0];
        assert!(matches!(
            declaration.kind,
            Kind::FundamentalTree { source }
                if declaration.id.0 == source.0 && !tree.hierarchy().tree_edges.contains(&source)
        ));

        let mut registry = tree.registry(&network).unwrap();
        let choice = registry.best().unwrap().unwrap();
        let decoded = choice
            .cycle
            .decode(
                &tree.materialization().graph,
                tree.chain(),
                tree.shifts(),
                &tree.materialization().bindings,
                &network,
            )
            .unwrap();
        assert_eq!(decoded.len(), 3);
    }

    #[test]
    fn retains_an_empty_population_when_the_source_graph_is_a_tree() {
        let mut network = CirculationNetwork::new(3);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 3, 0).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(2), 3, 0).unwrap();
        let input = Input::new(
            &network,
            &[ratio(1), ratio(4)],
            &[ratio(1), ratio(1)],
            &[ratio(1), ratio(1)],
        )
        .unwrap();
        let tree = Tree::build(input, &network, FlowNodeId(0)).unwrap();

        assert!(tree.candidates().is_empty());
        assert_eq!(tree.registry(&network).unwrap().best().unwrap(), None);
        assert_eq!(
            tree.hierarchy().tree_edges,
            [SourceEdgeId(0), SourceEdgeId(1)].into()
        );
    }
}
