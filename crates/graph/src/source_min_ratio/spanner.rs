//! Source-maintained finite sparsified-core candidate declarations.
//!
//! This module implements the finite semantic part of Algorithm 1's
//! fundamental-spanner population. It contracts a checked singleton forest,
//! builds the finite Section 9.1 core/spanner snapshot, and turns every
//! rejected core edge into its explicitly maintained spanner embedding cycle.
//! It makes no dynamic recourse, Theorem 5.1, or runtime claim.

use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::{CirculationNetwork, ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId};

use super::{
    candidate::{CandidateId, Context, Error as CandidateError, Fundamental, Kind, Registry},
    chain::{Chain, Error as ChainError, Shifts},
    cycle::{Cycle, Direction, EmbeddingEdge, Segment},
    input::{Error as InputError, Input, Materialization, StructuralGraph},
    model::{Branch, BranchId, Level, LevelId, Tree},
};

use crate::{
    source_lsst::{
        LsfPiece, LsfStructuralCertificate,
        bucket::Parameters as BucketParameters,
        chain::{
            Chain as SourceChain, Error as SourceChainError, Parameters as SourceChainParameters,
        },
    },
    source_spanner::{
        dynamic::rebuild::Parameters as RebuildParameters, experiment::domain::ExhaustiveDomain,
    },
};

/// Explicit finite-domain controls for a sparsified-core snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub root: FlowNodeId,
    pub maximum_absolute_exponent: u32,
    pub phi: ExactRatio,
    pub domain: ExhaustiveDomain,
    pub maximum_hops: usize,
    pub maximum_vertex_congestion: u64,
    pub maximum_rounds: usize,
}

/// Immutable source core/spanner state for one exact IPM projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    input: Input,
    materialization: Materialization,
    structural: StructuralGraph,
    forest: LsfStructuralCertificate,
    source_chain: SourceChain,
    chain: Chain,
    shifts: Shifts,
    candidates: Vec<Fundamental>,
    parameters: Parameters,
}

/// One immutable source-snapshot transition and its exact candidate recourse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Transition {
    /// Rebuilt exact source state for the new IPM coordinate projection.
    pub next: Snapshot,
    /// Candidate IDs newly declared by the rebuilt finite core.
    pub inserted: BTreeSet<CandidateId>,
    /// Retained IDs that must be re-scored in the new coordinate snapshot.
    pub refreshed: BTreeSet<CandidateId>,
    /// Candidate IDs no longer declared by the rebuilt finite core.
    pub retired: BTreeSet<CandidateId>,
    /// Retained IDs whose maintained embedding cycle changed.
    pub reembedded: BTreeSet<CandidateId>,
    previous_candidates: Vec<Fundamental>,
}

impl Snapshot {
    /// Builds the finite core/spanner snapshot and its rejected-core candidate
    /// declarations.
    ///
    /// # Errors
    ///
    /// Returns an error outside the already certified finite source domain or
    /// when the retained core embedding cannot be mapped to one explicit compact
    /// source cycle.
    pub fn build(
        input: Input,
        network: &CirculationNetwork,
        parameters: Parameters,
    ) -> Result<Self, Error> {
        let materialization = input.materialize(network)?;
        let structural = input.structural_graph()?;
        if parameters.root.0 >= structural.graph.node_count()
            || parameters.maximum_absolute_exponent == 0
            || !parameters.phi.is_positive()
            || parameters.maximum_hops == 0
            || parameters.maximum_vertex_congestion == 0
            || parameters.maximum_rounds == 0
        {
            return Err(Error::InvalidParameters);
        }
        let maximum_coordinate = maximum_coordinate(&structural.graph)?;
        let forest = singleton_forest(&structural.graph)?;
        let source_chain = SourceChain::build(
            &structural.graph,
            &forest,
            SourceChainParameters {
                root: parameters.root,
                maximum_coordinate,
                buckets: BucketParameters {
                    maximum_absolute_exponent: parameters.maximum_absolute_exponent,
                    spanner: RebuildParameters {
                        phi: parameters.phi,
                        domain: parameters.domain,
                        maximum_hops: parameters.maximum_hops,
                        maximum_vertex_congestion: parameters.maximum_vertex_congestion,
                        maximum_rounds: parameters.maximum_rounds,
                    },
                },
            },
        )?;
        let chain = Chain::new(
            &materialization.graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    Tree::new(parameters.root, source_chain.tree_edges.clone()),
                )],
            )],
        )?;
        let shifts = chain.initial_shifts();
        let candidates = declarations(&materialization, &source_chain)?;
        Ok(Self {
            input,
            materialization,
            structural,
            forest,
            source_chain,
            chain,
            shifts,
            candidates,
            parameters,
        })
    }

    /// Rebuilds all finite core, spanner, tree, and candidate evidence.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied network or any stored source
    /// provenance no longer agrees with the immutable snapshot.
    pub fn verify(&self, network: &CirculationNetwork) -> Result<(), Error> {
        let rebuilt = Self::build(self.input.clone(), network, self.parameters)?;
        if &rebuilt == self {
            Ok(())
        } else {
            Err(Error::MismatchedSnapshot)
        }
    }

    /// Rebuilds one supported IPM/source projection and derives exact stable-ID
    /// candidate recourse without mutating either snapshot or a candidate heap.
    ///
    /// # Errors
    ///
    /// Returns an error when the rebuilt source projection changes a stable
    /// source identity or leaves the explicit finite domain.
    pub fn transition(
        &self,
        input: Input,
        network: &CirculationNetwork,
    ) -> Result<Transition, Error> {
        let next = Self::build(input, network, self.parameters)?;
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
        let reembedded = refreshed
            .iter()
            .filter(|id| before.get(id) != after.get(id))
            .copied()
            .collect::<BTreeSet<_>>();
        Ok(Transition {
            next,
            inserted,
            refreshed,
            retired,
            reembedded,
            previous_candidates: self.candidates.clone(),
        })
    }

    /// Returns the exact projection that owns this source snapshot.
    #[must_use]
    pub const fn input(&self) -> &Input {
        &self.input
    }

    /// Returns the jointly materialized source graph and circulation bindings.
    #[must_use]
    pub const fn materialization(&self) -> &Materialization {
        &self.materialization
    }

    /// Returns the explicit singleton rooted forest used to form `C(G,F)`.
    #[must_use]
    pub const fn forest(&self) -> &LsfStructuralCertificate {
        &self.forest
    }

    /// Returns the finite Section 9.1 core/spanner/tree certificate.
    #[must_use]
    pub const fn source_chain(&self) -> &SourceChain {
        &self.source_chain
    }

    /// Returns the checked compact-cycle tree-chain context.
    #[must_use]
    pub const fn chain(&self) -> &Chain {
        &self.chain
    }

    /// Returns the immutable selected branch state.
    #[must_use]
    pub const fn shifts(&self) -> &Shifts {
        &self.shifts
    }

    /// Returns rejected-core declarations in stable source-edge order.
    #[must_use]
    pub fn candidates(&self) -> &[Fundamental] {
        &self.candidates
    }

    /// Creates the exact candidate context for this immutable snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the network no longer materializes this snapshot.
    pub fn context<'a>(&'a self, network: &'a CirculationNetwork) -> Result<Context<'a>, Error> {
        Ok(Context::new(
            &self.input,
            &self.materialization,
            &self.chain,
            &self.shifts,
            network,
        )?)
    }

    /// Creates a registry over only the source-declared rejected-core cycles.
    ///
    /// # Errors
    ///
    /// Returns an error when a declaration no longer has exact provenance.
    pub fn registry(&self, network: &CirculationNetwork) -> Result<Registry, Error> {
        let context = self.context(network)?;
        Ok(Registry::new(&context, self.candidates.clone())?)
    }
}

impl Transition {
    /// Applies this source-declared recourse to the exact candidate registry.
    ///
    /// Every retained declaration is replaced, including unchanged embeddings,
    /// because its exact gradient and length coordinates may have changed.
    ///
    /// # Errors
    ///
    /// Returns an error if the supplied registry is not exactly the source
    /// population that produced this transition or if the next snapshot cannot
    /// validate its circulation provenance.
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

fn singleton_forest(graph: &SourceDynamicGraph) -> Result<LsfStructuralCertificate, Error> {
    let one = ExactRatio::new(1, 1).map_err(|_| Error::Overflow)?;
    Ok(LsfStructuralCertificate {
        forest_edges: BTreeSet::new(),
        roots: (0..graph.node_count()).map(FlowNodeId).collect(),
        pieces: (0..graph.node_count())
            .map(|node| LsfPiece {
                vertices: BTreeSet::from([FlowNodeId(node)]),
                forest_edges: BTreeSet::new(),
            })
            .collect(),
        stretch_overestimates: vec![one; graph.edge_count()],
        piece_volume_limit: 1,
    })
}

fn maximum_coordinate(graph: &SourceDynamicGraph) -> Result<i128, Error> {
    let mut maximum = 0_i128;
    for index in 0..graph.edge_count() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::MissingSourceEdge(SourceEdgeId(index)))?;
        if !edge.length.is_positive() {
            return Err(Error::InvalidLength(SourceEdgeId(index)));
        }
        maximum = maximum.max(
            edge.length
                .numerator()
                .checked_abs()
                .ok_or(Error::Overflow)?
                .max(edge.length.denominator()),
        );
    }
    if maximum <= 0 {
        Err(Error::EmptyGraph)
    } else {
        Ok(maximum)
    }
}

fn declarations(
    materialization: &Materialization,
    source_chain: &SourceChain,
) -> Result<Vec<Fundamental>, Error> {
    let mut candidates = Vec::new();
    let offset = materialization.graph.edge_count();
    for bucket in &source_chain.partition.buckets {
        for (rejected, path) in &bucket.embedding.paths {
            if bucket.embedding.selected.contains(rejected) {
                continue;
            }
            let edge = materialization
                .graph
                .edge(*rejected)
                .ok_or(Error::MissingSourceEdge(*rejected))?;
            let edges = embedding_edges(
                &materialization.graph,
                *rejected,
                edge.first,
                edge.second,
                path,
            )?;
            let id = offset.checked_add(rejected.0).ok_or(Error::Overflow)?;
            candidates.push(Fundamental {
                id: CandidateId(id),
                kind: Kind::FundamentalSpanner {
                    rejected: *rejected,
                },
                cycle: Cycle {
                    segments: vec![
                        Segment::SpannerPath { edges },
                        Segment::OffTree {
                            source: *rejected,
                            direction: Direction::Reverse,
                        },
                    ],
                },
            });
        }
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

fn embedding_edges(
    graph: &crate::SourceDynamicGraph,
    rejected: SourceEdgeId,
    start: FlowNodeId,
    target: FlowNodeId,
    path: &[SourceEdgeId],
) -> Result<Vec<EmbeddingEdge>, Error> {
    if path.is_empty() || path.contains(&rejected) {
        return Err(Error::InvalidEmbedding(rejected));
    }
    let mut current = start;
    let mut result = Vec::with_capacity(path.len());
    for source in path {
        let edge = graph
            .edge(*source)
            .ok_or(Error::MissingSourceEdge(*source))?;
        let direction = if edge.first == current {
            current = edge.second;
            Direction::Forward
        } else if edge.second == current {
            current = edge.first;
            Direction::Reverse
        } else {
            return Err(Error::InvalidEmbedding(rejected));
        };
        result.push(EmbeddingEdge {
            source: *source,
            direction,
        });
    }
    if current == target {
        Ok(result)
    } else {
        Err(Error::InvalidEmbedding(rejected))
    }
}

/// A finite sparsified-core snapshot cannot be constructed or revalidated.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("finite sparsified-core parameters are invalid")]
    InvalidParameters,
    #[error("finite sparsified-core input failed: {0}")]
    Input(#[from] InputError),
    #[error("finite singleton-core tree chain failed: {0}")]
    SourceChain(#[from] SourceChainError),
    #[error("finite compact-cycle chain failed: {0}")]
    Chain(#[from] ChainError),
    #[error("finite core candidate evaluation failed: {0}")]
    Candidate(#[from] CandidateError),
    #[error("source edge {0:?} is absent from the materialized projection")]
    MissingSourceEdge(SourceEdgeId),
    #[error("source edge {0:?} has an invalid finite-core length")]
    InvalidLength(SourceEdgeId),
    #[error("finite source projection has no active edges")]
    EmptyGraph,
    #[error("rejected core edge {0:?} has an invalid retained spanner embedding")]
    InvalidEmbedding(SourceEdgeId),
    #[error("finite sparsified-core snapshot differs from a fresh rebuild")]
    MismatchedSnapshot,
    #[error("finite source snapshot changed a stable source identity")]
    SourceIdentityChanged,
    #[error("finite source snapshot contains duplicate candidate {0:?}")]
    DuplicateCandidate(CandidateId),
    #[error("candidate registry does not match the prior source snapshot")]
    MismatchedRegistry,
    #[error("finite source snapshot transition is internally inconsistent")]
    InvalidTransition,
    #[error("finite sparsified-core arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::{Error, Parameters, Snapshot};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId,
        source_min_ratio::{candidate::Kind, input::Input},
        source_spanner::experiment::domain::ExhaustiveDomain,
    };

    fn network() -> CirculationNetwork {
        let mut result = CirculationNetwork::new(5);
        for first in 0..5 {
            for second in (first + 1)..5 {
                result
                    .add_arc(FlowNodeId(first), FlowNodeId(second), 1, 0)
                    .unwrap();
            }
        }
        result
    }

    fn parameters() -> Parameters {
        Parameters {
            root: FlowNodeId(0),
            maximum_absolute_exponent: 4,
            phi: ExactRatio::new(1, 2).unwrap(),
            domain: ExhaustiveDomain { maximum_nodes: 8 },
            maximum_hops: 4,
            maximum_vertex_congestion: 100,
            maximum_rounds: 1,
        }
    }

    fn input(network: &CirculationNetwork, gradient: i128) -> Input {
        input_with_lengths(
            network,
            gradient,
            &vec![ExactRatio::new(1, 1).unwrap(); network.arc_count()],
        )
    }

    fn input_with_lengths(
        network: &CirculationNetwork,
        gradient: i128,
        lengths: &[ExactRatio],
    ) -> Input {
        Input::new(
            network,
            &vec![ExactRatio::new(gradient, 1).unwrap(); network.arc_count()],
            lengths,
            &vec![ExactRatio::new(1, 1).unwrap(); network.arc_count()],
        )
        .unwrap()
    }

    #[test]
    fn emits_rejected_core_cycles_from_a_checked_sparse_embedding() {
        let network = network();
        let input = input(&network, -1);
        let snapshot = Snapshot::build(input, &network, parameters()).unwrap();
        assert_eq!(snapshot.candidates().len(), 5);
        assert!(
            snapshot
                .candidates()
                .iter()
                .all(|candidate| { matches!(candidate.kind, Kind::FundamentalSpanner { .. }) })
        );
        for candidate in snapshot.candidates() {
            let decoded = candidate
                .cycle
                .decode(
                    &snapshot.materialization.graph,
                    &snapshot.chain,
                    &snapshot.shifts,
                    &snapshot.materialization.bindings,
                    &network,
                )
                .unwrap();
            assert!(!decoded.is_empty());
        }
        let mut registry = snapshot.registry(&network).unwrap();
        assert_eq!(registry.candidates().len(), 5);
        assert!(registry.best().unwrap().is_some());
        snapshot.verify(&network).unwrap();
    }

    #[test]
    fn preserves_rational_coordinates_through_the_structural_chain() {
        let network = network();
        let rational_lengths = vec![ExactRatio::new(1, 2).unwrap(); network.arc_count()];
        let snapshot = Snapshot::build(
            input_with_lengths(&network, -1, &rational_lengths),
            &network,
            parameters(),
        )
        .unwrap();

        assert_eq!(
            snapshot
                .materialization()
                .graph
                .edge(crate::SourceEdgeId(0))
                .unwrap()
                .length,
            ExactRatio::new(1, 2).unwrap()
        );
        assert!(
            snapshot
                .registry(&network)
                .unwrap()
                .best()
                .unwrap()
                .is_some()
        );
        snapshot.verify(&network).unwrap();
    }

    #[test]
    fn refreshes_the_full_population_when_only_coordinates_change() {
        let network = network();
        let snapshot = Snapshot::build(input(&network, -1), &network, parameters()).unwrap();
        let mut registry = snapshot.registry(&network).unwrap();
        let transition = snapshot.transition(input(&network, 1), &network).unwrap();

        assert!(transition.inserted.is_empty());
        assert!(transition.retired.is_empty());
        assert!(transition.reembedded.is_empty());
        assert_eq!(transition.refreshed.len(), snapshot.candidates().len());
        transition.apply(&mut registry, &network).unwrap();
        assert_eq!(registry.accounting().replaced, 5);
        assert!(registry.best().unwrap().is_some());
        transition.next.verify(&network).unwrap();
    }

    #[test]
    fn rejects_applying_recourse_to_a_nonmatching_registry() {
        let network = network();
        let snapshot = Snapshot::build(input(&network, -1), &network, parameters()).unwrap();
        let mut registry = snapshot.registry(&network).unwrap();
        registry.retire(snapshot.candidates()[0].id).unwrap();
        let transition = snapshot.transition(input(&network, 1), &network).unwrap();

        assert_eq!(
            transition.apply(&mut registry, &network),
            Err(Error::MismatchedRegistry)
        );
    }

    #[test]
    fn applies_source_declared_insertions_and_retires_after_a_bucket_change() {
        let network = network();
        let snapshot = Snapshot::build(input(&network, -1), &network, parameters()).unwrap();
        let mut registry = snapshot.registry(&network).unwrap();
        let mut lengths = vec![ExactRatio::new(1, 1).unwrap(); network.arc_count()];
        lengths[0] = ExactRatio::new(2, 1).unwrap();
        let transition = snapshot
            .transition(input_with_lengths(&network, -1, &lengths), &network)
            .unwrap();

        assert!(!transition.inserted.is_empty() || !transition.retired.is_empty());
        transition.apply(&mut registry, &network).unwrap();
        assert_eq!(
            registry.candidates(),
            transition.next.candidates().iter().collect::<Vec<_>>()
        );
        transition.next.verify(&network).unwrap();
    }
}
