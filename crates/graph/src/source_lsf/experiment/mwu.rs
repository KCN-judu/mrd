//! Deterministic rational multiplicative-weights collection for Lemma 5.5.
//!
//! The paper writes the update as `exp(stretch / rho)`. The source graph and
//! LSF interfaces use exact rational weights, so this module uses
//! `1 + x + x^2` for `x = stretch / rho`. For `0 <= x <= 1/10`, it is at
//! least `1 + x` and at most `1 + 2x`, which is the only exponential estimate
//! used by the Appendix A.2 potential argument. The resulting certificate
//! records the exact finite-instance bound; it does not assert the paper's
//! asymptotic `O(log^7 n)` without a checked Lemma 5.4 envelope.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::{
    ExactRatio, FlowNodeId, LsfPiece, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge,
};

use super::{Core, WeightedExpansion};
use crate::source_an19::experiment::hierarchy::Lsst;

/// Checked finite-instance inputs for the Lemma 5.5 MWU construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameters {
    /// The exact number of source LSFs to construct.
    pub tree_count: usize,
    /// The reduction factor consumed by the Lemma 5.4 forest initializer.
    pub reduction_k: usize,
    /// A checked `W` satisfying the two Lemma 5.4 stretch inequalities for
    /// every generated round.
    pub stretch_envelope: ExactRatio,
}

/// One source LSF and its checked round-local MWU evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Round {
    pub weights: Vec<ExactRatio>,
    pub tree_edges: BTreeSet<SourceEdgeId>,
    pub forest_edges: BTreeSet<SourceEdgeId>,
    pub roots: BTreeSet<FlowNodeId>,
    pub pieces: Vec<LsfPiece>,
    pub stretch_overestimates: Vec<ExactRatio>,
    pub weighted_stretch: ExactRatio,
    pub maximum_stretch: ExactRatio,
}

/// Exact certificate for the complete deterministic MWU collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Certificate {
    pub parameters: Parameters,
    pub rho: ExactRatio,
    pub maximum_round_stretch: ExactRatio,
    pub uniform_average_stretch_bound: ExactRatio,
    pub aggregate_stretches: Vec<ExactRatio>,
    pub final_weights: Vec<ExactRatio>,
    pub rounds: Vec<Round>,
}

/// A deterministic collection of exactly `k` source-shaped low-stretch
/// forests, together with a self-verifying average-stretch certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Collection {
    certificate: Certificate,
}

impl Collection {
    /// Constructs exactly `parameters.tree_count` forests rooted at `root`.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is not a connected static source graph,
    /// a source LSF construction fails, a supplied Lemma 5.4 envelope is
    /// violated, or exact arithmetic overflows.
    pub fn build(
        graph: &SourceDynamicGraph,
        root: FlowNodeId,
        parameters: Parameters,
    ) -> Result<Self, Error> {
        Ok(Self {
            certificate: compute(graph, root, parameters)?,
        })
    }

    /// Recomputes the construction and rejects any altered certificate field.
    ///
    /// This deliberately reruns the source-shaped constructor rather than
    /// trusting stored trees, forest edges, or arithmetic summaries.
    ///
    /// # Errors
    ///
    /// Returns an error when recomputation cannot construct or certify the
    /// source LSFs, or when any stored certificate field differs.
    pub fn verify(&self, graph: &SourceDynamicGraph, root: FlowNodeId) -> Result<(), Error> {
        let expected = compute(graph, root, self.certificate.parameters.clone())?;
        if expected != self.certificate {
            return Err(Error::InvalidCertificate);
        }
        Ok(())
    }

    #[must_use]
    pub const fn certificate(&self) -> &Certificate {
        &self.certificate
    }

    #[must_use]
    pub fn rounds(&self) -> &[Round] {
        &self.certificate.rounds
    }

    /// Returns the exact average stretch certificate for one source edge.
    ///
    /// # Errors
    ///
    /// Returns an error when `edge` is outside the static input snapshot.
    pub fn average_stretch(&self, edge: SourceEdgeId) -> Result<ExactRatio, Error> {
        let total = self
            .certificate
            .aggregate_stretches
            .get(edge.0)
            .ok_or(Error::EdgeOutOfBounds)?
            .clone();
        let count = ExactRatio::new(
            i128::try_from(self.certificate.parameters.tree_count).map_err(|_| Error::Overflow)?,
            1,
        )
        .map_err(map_ratio)?;
        total
            .checked_mul(&count.reciprocal().map_err(map_ratio)?)
            .map_err(map_ratio)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("MWU requires a positive tree count, reduction factor, and stretch envelope")]
    InvalidParameters,
    #[error("MWU requires a connected static source graph and an in-range root")]
    InvalidGraph,
    #[error("MWU does not initialize across inactive source edges")]
    InactiveInputEdge,
    #[error("the source AN19-shaped static tree construction failed: {0}")]
    SourceTreeConstruction(#[source] crate::source_an19::petal::Error),
    #[error("the source weighted-copy expansion failed: {0}")]
    WeightedCopyConstruction(#[source] super::Error),
    #[error("the source Lemma 5.4 forest initializer failed: {0}")]
    SourceForestConstruction(#[source] super::Error),
    #[error("a constructed LSF violates the supplied Lemma 5.4 envelope")]
    StretchEnvelopeViolation,
    #[error("the deterministic MWU certificate does not match a fresh construction")]
    InvalidCertificate,
    #[error("edge is outside the static source graph")]
    EdgeOutOfBounds,
    #[error("exact MWU arithmetic overflowed")]
    Overflow,
}

#[derive(Clone)]
struct Bounds {
    rho: ExactRatio,
    maximum_round_stretch: ExactRatio,
    uniform_average_stretch_bound: ExactRatio,
    tree_count_ratio: ExactRatio,
    target_piece_count: usize,
}

fn compute(
    graph: &SourceDynamicGraph,
    root: FlowNodeId,
    parameters: Parameters,
) -> Result<Certificate, Error> {
    validate_input(graph, root, parameters.clone())?;
    let edge_count = graph.edge_count();
    let bounds = derive_bounds(graph, parameters.clone())?;
    let one = ExactRatio::new(1, 1).map_err(map_ratio)?;
    let zero = ExactRatio::new(0, 1).map_err(map_ratio)?;
    let mut weights = vec![one; edge_count];
    let mut aggregate_stretches = vec![zero; edge_count];
    let mut rounds = Vec::with_capacity(parameters.tree_count);

    for _ in 0..parameters.tree_count {
        let round = construct_round(graph, root, parameters.clone(), bounds.clone(), &weights)?;
        for (total, stretch) in aggregate_stretches
            .iter_mut()
            .zip(&round.stretch_overestimates)
        {
            *total = total.checked_add(stretch).map_err(map_ratio)?;
        }
        weights = next_weights(&weights, &round.stretch_overestimates, bounds.rho.clone())?;
        rounds.push(round);
    }

    verify_average_bound(&aggregate_stretches, bounds.clone())?;
    Ok(Certificate {
        parameters,
        rho: bounds.rho,
        maximum_round_stretch: bounds.maximum_round_stretch,
        uniform_average_stretch_bound: bounds.uniform_average_stretch_bound,
        aggregate_stretches,
        final_weights: weights,
        rounds,
    })
}

fn validate_input(
    graph: &SourceDynamicGraph,
    root: FlowNodeId,
    parameters: Parameters,
) -> Result<(), Error> {
    if parameters.tree_count == 0
        || parameters.reduction_k == 0
        || !parameters.stretch_envelope.is_positive()
        || graph.node_count() < 2
        || root.0 >= graph.node_count()
    {
        return Err(Error::InvalidParameters);
    }
    if graph.edge_count() == 0 {
        return Err(Error::InvalidGraph);
    }
    for index in 0..graph.edge_count() {
        if graph.edge(SourceEdgeId(index)).is_none() {
            return Err(Error::InactiveInputEdge);
        }
    }
    Ok(())
}

fn derive_bounds(graph: &SourceDynamicGraph, parameters: Parameters) -> Result<Bounds, Error> {
    let node_log = integer_ratio(ceil_log2(graph.node_count()).max(1))?;
    let node_log_squared = node_log.checked_mul(&node_log).map_err(map_ratio)?;
    let tree_count_ratio = integer_ratio(parameters.tree_count)?;
    let rho = parameters
        .stretch_envelope
        .checked_mul_integer(10)
        .and_then(|value| value.checked_mul(&tree_count_ratio))
        .and_then(|value| value.checked_mul(&node_log_squared))
        .map_err(map_ratio)?;
    let maximum_round_stretch = parameters
        .stretch_envelope
        .checked_mul(&tree_count_ratio)
        .and_then(|value| value.checked_mul(&node_log_squared))
        .map_err(map_ratio)?;
    let edge_log = integer_i128(ceil_log2(graph.edge_count()).max(1))?;
    let twenty_nineteen = ExactRatio::new(20, 19).map_err(map_ratio)?;
    let uniform_average_stretch_bound = parameters
        .stretch_envelope
        .checked_mul_integer(10)
        .and_then(|value| value.checked_mul(&node_log_squared))
        .and_then(|value| value.checked_mul_integer(edge_log))
        .and_then(|value| value.checked_add(&parameters.stretch_envelope.checked_mul_integer(2)?))
        .and_then(|value| value.checked_mul(&twenty_nineteen))
        .map_err(map_ratio)?;
    let target_piece_count = graph
        .edge_count()
        .checked_add(parameters.reduction_k - 1)
        .ok_or(Error::Overflow)?
        / parameters.reduction_k;
    Ok(Bounds {
        rho,
        maximum_round_stretch,
        uniform_average_stretch_bound,
        tree_count_ratio,
        target_piece_count,
    })
}

fn construct_round(
    graph: &SourceDynamicGraph,
    root: FlowNodeId,
    parameters: Parameters,
    bounds: Bounds,
    weights: &[ExactRatio],
) -> Result<Round, Error> {
    let weighted_graph = reweighted_graph(graph, weights)?;
    let expansion =
        WeightedExpansion::build(&weighted_graph).map_err(Error::WeightedCopyConstruction)?;
    let hierarchy =
        Lsst::construct(&expansion.graph, root).map_err(Error::SourceTreeConstruction)?;
    let tree_edges = hierarchy
        .tree_edges
        .iter()
        .map(|copy| {
            expansion
                .copy_to_original
                .get(copy.0)
                .copied()
                .ok_or(Error::InvalidCertificate)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if tree_edges.len() != hierarchy.tree_edges.len()
        || tree_edges.len().checked_add(1) != Some(graph.node_count())
    {
        return Err(Error::InvalidCertificate);
    }
    let initialization = Core::new_with_spielman_teng(
        weighted_graph,
        tree_edges.iter().copied(),
        root,
        bounds.target_piece_count.max(1),
        parameters.reduction_k,
        bounds.maximum_round_stretch.clone(),
    )
    .map_err(Error::SourceForestConstruction)?;
    let stretches = initialization
        .core
        .global_stretch()
        .stretch_overestimates
        .clone();
    if stretches.len() != weights.len() {
        return Err(Error::InvalidCertificate);
    }
    let (weighted_stretch, maximum_stretch) = stretch_summary(weights, &stretches)?;
    let weighted_limit = parameters
        .stretch_envelope
        .checked_mul(&sum(weights)?)
        .map_err(map_ratio)?;
    if !weighted_limit
        .at_least(&weighted_stretch)
        .map_err(map_ratio)?
        || !bounds
            .maximum_round_stretch
            .at_least(&maximum_stretch)
            .map_err(map_ratio)?
    {
        return Err(Error::StretchEnvelopeViolation);
    }
    Ok(Round {
        weights: weights.to_vec(),
        tree_edges,
        forest_edges: initialization.core.forest_edges().clone(),
        roots: initialization.core.roots().clone(),
        pieces: initialization.spielman_teng.pieces,
        stretch_overestimates: stretches,
        weighted_stretch,
        maximum_stretch,
    })
}

fn verify_average_bound(aggregate_stretches: &[ExactRatio], bounds: Bounds) -> Result<(), Error> {
    let divisor = bounds.tree_count_ratio.reciprocal().map_err(map_ratio)?;
    for total in aggregate_stretches {
        let average = total.checked_mul(&divisor).map_err(map_ratio)?;
        if !bounds
            .uniform_average_stretch_bound
            .at_least(&average)
            .map_err(map_ratio)?
        {
            return Err(Error::InvalidCertificate);
        }
    }
    Ok(())
}

fn reweighted_graph(
    graph: &SourceDynamicGraph,
    weights: &[ExactRatio],
) -> Result<SourceDynamicGraph, Error> {
    if weights.len() != graph.edge_count() {
        return Err(Error::InvalidGraph);
    }
    let mut maximum = graph.maximum_abs_coordinate();
    let mut edges = Vec::with_capacity(graph.edge_count());
    for (index, weight) in weights.iter().cloned().enumerate() {
        let edge = graph
            .edge(SourceEdgeId(index))
            .ok_or(Error::InactiveInputEdge)?;
        let numerator = weight
            .numerator_i128()
            .map_err(|_| Error::Overflow)?
            .checked_abs()
            .ok_or(Error::Overflow)?;
        let denominator = weight.denominator_i128().map_err(|_| Error::Overflow)?;
        maximum = maximum.max(numerator).max(denominator);
        edges.push(SourceWeightedEdge {
            first: edge.first,
            second: edge.second,
            length: edge.length.clone(),
            weight,
        });
    }
    SourceDynamicGraph::new(graph.node_count(), edges, maximum).map_err(|_| Error::InvalidGraph)
}

fn stretch_summary(
    weights: &[ExactRatio],
    stretches: &[ExactRatio],
) -> Result<(ExactRatio, ExactRatio), Error> {
    if weights.len() != stretches.len() {
        return Err(Error::InvalidGraph);
    }
    let zero = ExactRatio::new(0, 1).map_err(map_ratio)?;
    let mut weighted = zero.clone();
    let mut maximum = zero;
    for (weight, stretch) in weights.iter().zip(stretches) {
        if !stretch.is_positive() {
            return Err(Error::InvalidCertificate);
        }
        weighted = weighted
            .checked_add(&weight.checked_mul(stretch).map_err(map_ratio)?)
            .map_err(map_ratio)?;
        if stretch.at_least(&maximum).map_err(map_ratio)? {
            maximum = stretch.clone();
        }
    }
    Ok((weighted, maximum))
}

fn next_weights(
    weights: &[ExactRatio],
    stretches: &[ExactRatio],
    rho: ExactRatio,
) -> Result<Vec<ExactRatio>, Error> {
    if weights.len() != stretches.len() || !rho.is_positive() {
        return Err(Error::InvalidGraph);
    }
    let one = ExactRatio::new(1, 1).map_err(map_ratio)?;
    let inverse_rho = rho.reciprocal().map_err(map_ratio)?;
    weights
        .iter()
        .zip(stretches)
        .map(|(weight, stretch)| {
            let x = stretch.checked_mul(&inverse_rho).map_err(map_ratio)?;
            let factor = one
                .checked_add(&x)
                .and_then(|value| value.checked_add(&x.checked_mul(&x)?))
                .map_err(map_ratio)?;
            weight.checked_mul(&factor).map_err(map_ratio)
        })
        .collect()
}

fn sum(values: &[ExactRatio]) -> Result<ExactRatio, Error> {
    values
        .iter()
        .try_fold(ExactRatio::new(0, 1).map_err(map_ratio)?, |total, value| {
            total.checked_add(value).map_err(map_ratio)
        })
}

fn ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::try_from(usize::BITS - (value - 1).leading_zeros()).unwrap_or(usize::MAX)
    }
}

fn integer_i128(value: usize) -> Result<i128, Error> {
    i128::try_from(value).map_err(|_| Error::Overflow)
}

fn integer_ratio(value: usize) -> Result<ExactRatio, Error> {
    ExactRatio::new(integer_i128(value)?, 1).map_err(map_ratio)
}

fn map_ratio(_: crate::StableMinRatioError) -> Error {
    Error::Overflow
}

#[cfg(test)]
mod tests {
    use super::{Collection, Error, Parameters};
    use crate::{
        ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceGraphUpdate,
        SourceUpdateBatch, SourceWeightedEdge,
    };

    fn ratio(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    fn graph() -> SourceDynamicGraph {
        SourceDynamicGraph::new(
            5,
            (0..4)
                .map(|node| SourceWeightedEdge {
                    first: FlowNodeId(node),
                    second: FlowNodeId(node + 1),
                    length: ratio(1),
                    weight: ratio(1),
                })
                .collect(),
            16,
        )
        .unwrap()
    }

    fn parameters() -> Parameters {
        Parameters {
            tree_count: 3,
            reduction_k: 1,
            stretch_envelope: ratio(100),
        }
    }

    #[test]
    fn constructs_exactly_k_deterministic_source_lsfs() {
        let graph = graph();
        let first = Collection::build(&graph, FlowNodeId(0), parameters()).unwrap();
        let second = Collection::build(&graph, FlowNodeId(0), parameters()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.rounds().len(), 3);
        first.verify(&graph, FlowNodeId(0)).unwrap();
        for round in first.rounds() {
            assert_eq!(round.tree_edges.len(), graph.node_count() - 1);
            assert!(round.forest_edges.is_subset(&round.tree_edges));
            assert!(!round.roots.is_empty());
            assert!(!round.pieces.is_empty());
        }
        for index in 0..graph.edge_count() {
            assert!(
                first
                    .certificate()
                    .uniform_average_stretch_bound
                    .at_least(&first.average_stretch(SourceEdgeId(index)).unwrap())
                    .unwrap()
            );
        }
    }

    #[test]
    fn rejects_an_envelope_that_does_not_certify_the_source_lsf() {
        let graph = graph();
        assert_eq!(
            Collection::build(
                &graph,
                FlowNodeId(0),
                Parameters {
                    stretch_envelope: ratio(1),
                    ..parameters()
                }
            ),
            Err(Error::StretchEnvelopeViolation)
        );
    }

    #[test]
    fn rejects_inactive_snapshots_and_certificate_mutation() {
        let mut inactive_graph = graph();
        inactive_graph
            .apply_batch(&SourceUpdateBatch {
                updates: vec![SourceGraphUpdate::Delete(SourceEdgeId(2))],
            })
            .unwrap();
        assert_eq!(
            Collection::build(&inactive_graph, FlowNodeId(0), parameters()),
            Err(Error::InactiveInputEdge)
        );

        let graph = graph();
        let mut collection = Collection::build(&graph, FlowNodeId(0), parameters()).unwrap();
        collection.certificate.final_weights[0] = ratio(1);
        assert_eq!(
            collection.verify(&graph, FlowNodeId(0)),
            Err(Error::InvalidCertificate)
        );
    }
}
