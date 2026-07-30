//! Source-maintained terminal-tree candidate declarations.
//!
//! This module materializes one exact AN19-shaped static tree for one immutable
//! IPM projection. It turns every source edge outside that tree into its unique
//! terminal fundamental-cycle declaration. It deliberately does not construct
//! core/spanner embeddings, enumerate graph cycles, or assert a runtime bound.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{CirculationNetwork, FlowNodeId, SourceEdgeId};

use super::{
    candidate::{CandidateId, Context, Error as CandidateError, Fundamental, Kind, Registry},
    chain::{Chain, Error as ChainError, Selection, Shifts},
    cycle::{Cycle, Direction, Error as CycleError, Segment},
    input::{Error as InputError, Input, Materialization, StructuralGraph},
    model::{Branch, BranchId, Level, LevelId, Tree as SourceTree},
};

use crate::source_an19::experiment::hierarchy::Lsst;

/// Immutable terminal-tree projection for one exact source/IPM snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tree {
    input: Input,
    materialization: Materialization,
    structural: StructuralGraph,
    hierarchy: Lsst,
    chain: Chain,
    shifts: Shifts,
    candidates: Vec<Fundamental>,
    root: FlowNodeId,
}

/// One immutable terminal source-snapshot transition and its exact candidate
/// recourse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    /// Rebuilt exact terminal state for the new coordinate projection.
    pub next: Tree,
    /// Stable candidate IDs newly declared by the rebuilt terminal tree.
    pub inserted: BTreeSet<CandidateId>,
    /// Retained IDs that must be re-scored in the new coordinate snapshot.
    pub refreshed: BTreeSet<CandidateId>,
    /// Candidate IDs no longer declared by the rebuilt terminal tree.
    pub retired: BTreeSet<CandidateId>,
    /// Retained IDs whose decoded terminal path changed.
    pub reembedded: BTreeSet<CandidateId>,
    previous_candidates: Vec<Fundamental>,
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
        let structural = input.structural_graph()?;
        let hierarchy = Lsst::construct(&structural.graph, root)?;
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
            structural,
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
        self.hierarchy.verify(&self.structural.graph)?;
        let rebuilt = Self::build(self.input.clone(), network, self.root)?;
        if &rebuilt == self {
            Ok(())
        } else {
            Err(Error::MismatchedSnapshot)
        }
    }

    /// Rebuilds one supported exact terminal projection and derives stable-ID
    /// candidate recourse without mutating either snapshot or registry.
    ///
    /// # Errors
    ///
    /// Returns an error when the rebuilt projection changes the source or
    /// circulation identities, or when an exact terminal path cannot be
    /// decoded for comparison.
    pub fn transition(
        &self,
        input: Input,
        network: &CirculationNetwork,
    ) -> Result<Transition, Error> {
        let next = Self::build(input, network, self.root)?;
        if !self.input.has_same_source_identity(next.input()) {
            return Err(Error::SourceIdentityChanged);
        }
        let before = candidates_by_id(&self.candidates)?;
        let after = candidates_by_id(&next.candidates)?;
        let inserted = after
            .keys()
            .filter(|id| !before.contains_key(*id))
            .copied()
            .collect::<BTreeSet<_>>();
        let refreshed = after
            .keys()
            .filter(|id| before.contains_key(*id))
            .copied()
            .collect::<BTreeSet<_>>();
        let retired = before
            .keys()
            .filter(|id| !after.contains_key(*id))
            .copied()
            .collect::<BTreeSet<_>>();
        let mut reembedded = BTreeSet::new();
        for id in &refreshed {
            let prior = before.get(id).ok_or(Error::InvalidTransition)?;
            let replacement = after.get(id).ok_or(Error::InvalidTransition)?;
            if decoded_candidate(self, prior, network)?
                != decoded_candidate(&next, replacement, network)?
            {
                reembedded.insert(*id);
            }
        }
        Ok(Transition {
            next,
            inserted,
            refreshed,
            retired,
            reembedded,
            previous_candidates: self.candidates.clone(),
        })
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
    /// This does not add core/spanner candidates. Cross-snapshot terminal
    /// maintenance is represented by [`Transition`].
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

impl Transition {
    /// Applies this source-declared terminal recourse to the exact candidate
    /// registry that produced it.
    ///
    /// Every retained declaration is replaced, even when its compact form is
    /// unchanged, because its exact gradient or length coordinate may differ.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry is not exactly the prior terminal
    /// population or the next snapshot no longer has exact provenance.
    pub fn apply(
        &self,
        registry: &mut Registry,
        network: &CirculationNetwork,
    ) -> Result<(), Error> {
        let active = registry
            .candidates()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        if active != self.previous_candidates {
            return Err(Error::MismatchedRegistry);
        }
        let context = self.next.context(network)?;
        for id in &self.retired {
            registry.retire(*id)?;
        }
        for candidate in &self.next.candidates {
            if self.refreshed.contains(&candidate.id) {
                registry.replace(&context, candidate.clone())?;
            } else if self.inserted.contains(&candidate.id) {
                registry.insert(&context, candidate.clone())?;
            } else {
                return Err(Error::InvalidTransition);
            }
        }
        Ok(())
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

fn candidates_by_id(
    candidates: &[Fundamental],
) -> Result<BTreeMap<CandidateId, Fundamental>, Error> {
    let mut result = BTreeMap::new();
    for candidate in candidates {
        if result.insert(candidate.id, candidate.clone()).is_some() {
            return Err(Error::DuplicateCandidate(candidate.id));
        }
    }
    Ok(result)
}

fn decoded_candidate(
    snapshot: &Tree,
    candidate: &Fundamental,
    network: &CirculationNetwork,
) -> Result<Vec<(crate::CirculationArcId, i8)>, Error> {
    Ok(candidate.cycle.decode(
        &snapshot.materialization.graph,
        &snapshot.chain,
        &snapshot.shifts,
        &snapshot.materialization.bindings,
        network,
    )?)
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
    #[error("terminal compact-cycle decoding failed: {0}")]
    Cycle(#[from] CycleError),
    #[error("terminal source tree has no selected branch")]
    MissingSelection,
    #[error("terminal source tree refers to missing source edge {0:?}")]
    MissingSourceEdge(SourceEdgeId),
    #[error("rebuilt terminal source snapshot differs from stored evidence")]
    MismatchedSnapshot,
    #[error("terminal source snapshot changed a stable source identity")]
    SourceIdentityChanged,
    #[error("terminal source snapshot contains duplicate candidate {0:?}")]
    DuplicateCandidate(CandidateId),
    #[error("terminal candidate registry does not match the prior source snapshot")]
    MismatchedRegistry,
    #[error("terminal source snapshot transition is internally inconsistent")]
    InvalidTransition,
}

#[cfg(test)]
mod tests {
    use super::{Error, Tree};
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

    fn input(network: &CirculationNetwork, gradient: i128) -> Input {
        input_with_lengths(network, gradient, &vec![ratio(1); network.arc_count()])
    }

    fn input_with_lengths(
        network: &CirculationNetwork,
        gradient: i128,
        lengths: &[ExactRatio],
    ) -> Input {
        Input::new(
            network,
            &vec![ratio(gradient); network.arc_count()],
            lengths,
            &vec![ratio(1); network.arc_count()],
        )
        .unwrap()
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

    #[test]
    fn refreshes_the_full_terminal_population_when_only_coordinates_change() {
        let network = network();
        let tree = Tree::build(input(&network, -1), &network, FlowNodeId(0)).unwrap();
        let mut registry = tree.registry(&network).unwrap();
        let transition = tree.transition(input(&network, 1), &network).unwrap();

        assert!(transition.inserted.is_empty());
        assert!(transition.retired.is_empty());
        assert!(transition.reembedded.is_empty());
        assert_eq!(transition.refreshed.len(), tree.candidates().len());
        transition.apply(&mut registry, &network).unwrap();
        assert_eq!(registry.accounting().replaced, 1);
        assert_eq!(
            registry.candidates(),
            transition.next.candidates().iter().collect::<Vec<_>>()
        );
        transition.next.verify(&network).unwrap();
    }

    #[test]
    fn rejects_applying_terminal_recourse_to_a_nonmatching_registry() {
        let network = network();
        let tree = Tree::build(input(&network, -1), &network, FlowNodeId(0)).unwrap();
        let mut registry = tree.registry(&network).unwrap();
        registry.retire(tree.candidates()[0].id).unwrap();
        let transition = tree.transition(input(&network, 1), &network).unwrap();

        assert_eq!(
            transition.apply(&mut registry, &network),
            Err(Error::MismatchedRegistry)
        );
    }

    #[test]
    fn applies_terminal_insertions_and_retires_after_a_tree_change() {
        let network = network();
        let tree = Tree::build(input(&network, -1), &network, FlowNodeId(0)).unwrap();
        let mut registry = tree.registry(&network).unwrap();
        let transition = tree
            .transition(
                input_with_lengths(&network, -1, &[ratio(8), ratio(1), ratio(1)]),
                &network,
            )
            .unwrap();

        assert!(
            !transition.inserted.is_empty()
                || !transition.retired.is_empty()
                || !transition.reembedded.is_empty()
        );
        transition.apply(&mut registry, &network).unwrap();
        assert_eq!(
            registry.candidates(),
            transition.next.candidates().iter().collect::<Vec<_>>()
        );
        transition.next.verify(&network).unwrap();
    }
}
