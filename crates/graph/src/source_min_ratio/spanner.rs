//! Source-maintained finite sparsified-core candidate declarations.
//!
//! This module implements the finite semantic part of Algorithm 1's
//! fundamental-spanner population. It contracts a checked singleton forest,
//! builds the finite Section 9.1 core/spanner snapshot, and turns every
//! rejected core edge into its explicitly maintained spanner embedding cycle.
//! It makes no dynamic recourse, Theorem 5.1, or runtime claim.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::{CirculationNetwork, ExactRatio, FlowNodeId, SourceEdgeId};

use super::{
    candidate::{CandidateId, Context, Error as CandidateError, Fundamental, Kind, Registry},
    chain::{Chain, Error as ChainError, Shifts},
    cycle::{Cycle, Direction, EmbeddingEdge, Segment},
    input::{Error as InputError, Input, Materialization},
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
    forest: LsfStructuralCertificate,
    source_chain: SourceChain,
    chain: Chain,
    shifts: Shifts,
    candidates: Vec<Fundamental>,
    parameters: Parameters,
}

impl Snapshot {
    /// Builds the finite core/spanner snapshot and its rejected-core candidate
    /// declarations.
    ///
    /// # Errors
    ///
    /// Returns an error outside the already certified finite source domain, on
    /// nonintegral lengths, or when the retained core embedding cannot be
    /// mapped to one explicit compact source cycle.
    pub fn build(
        input: Input,
        network: &CirculationNetwork,
        parameters: Parameters,
    ) -> Result<Self, Error> {
        let materialization = input.materialize(network)?;
        if parameters.root.0 >= materialization.graph.node_count()
            || parameters.maximum_absolute_exponent == 0
            || !parameters.phi.is_positive()
            || parameters.maximum_hops == 0
            || parameters.maximum_vertex_congestion == 0
            || parameters.maximum_rounds == 0
        {
            return Err(Error::InvalidParameters);
        }
        let maximum_integral_length = maximum_integral_length(&materialization)?;
        let forest = singleton_forest(&materialization)?;
        let source_chain = SourceChain::build(
            &materialization.graph,
            &forest,
            SourceChainParameters {
                root: parameters.root,
                maximum_integral_length,
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

    /// Returns the exact projection that owns this source snapshot.
    #[must_use]
    pub const fn input(&self) -> &Input {
        &self.input
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

fn singleton_forest(materialization: &Materialization) -> Result<LsfStructuralCertificate, Error> {
    let one = ExactRatio::new(1, 1).map_err(|_| Error::Overflow)?;
    Ok(LsfStructuralCertificate {
        forest_edges: BTreeSet::new(),
        roots: (0..materialization.graph.node_count())
            .map(FlowNodeId)
            .collect(),
        pieces: (0..materialization.graph.node_count())
            .map(|node| LsfPiece {
                vertices: BTreeSet::from([FlowNodeId(node)]),
                forest_edges: BTreeSet::new(),
            })
            .collect(),
        stretch_overestimates: vec![one; materialization.graph.edge_count()],
        piece_volume_limit: 1,
    })
}

fn maximum_integral_length(materialization: &Materialization) -> Result<i128, Error> {
    let mut maximum = 0_i128;
    for index in 0..materialization.graph.edge_count() {
        let edge = materialization
            .graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::MissingSourceEdge(SourceEdgeId(index)))?;
        if edge.length.denominator() != 1 || !edge.length.is_positive() {
            return Err(Error::NonintegralLength(SourceEdgeId(index)));
        }
        maximum = maximum.max(edge.length.numerator());
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
    #[error("source edge {0:?} has a nonintegral finite-core length")]
    NonintegralLength(SourceEdgeId),
    #[error("finite source projection has no active edges")]
    EmptyGraph,
    #[error("rejected core edge {0:?} has an invalid retained spanner embedding")]
    InvalidEmbedding(SourceEdgeId),
    #[error("finite sparsified-core snapshot differs from a fresh rebuild")]
    MismatchedSnapshot,
    #[error("finite sparsified-core arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::{Parameters, Snapshot};
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

    #[test]
    fn emits_rejected_core_cycles_from_a_checked_sparse_embedding() {
        let network = network();
        let input = Input::new(
            &network,
            &vec![ExactRatio::new(-1, 1).unwrap(); network.arc_count()],
            &vec![ExactRatio::new(1, 1).unwrap(); network.arc_count()],
            &vec![ExactRatio::new(1, 1).unwrap(); network.arc_count()],
        )
        .unwrap();
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
}
