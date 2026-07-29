//! Algorithm 4 Task 1: an exact finite-domain witness union.

use crate::{ExactRatio, FlowNodeId};

use super::super::{
    experiment::{
        complete::build as build_complete,
        decomposition::Decomposition,
        domain::{Error as ExperimentError, ExhaustiveDomain},
    },
    model::{EdgeId, Graph},
};

/// The single certified witness component currently supported by Algorithm 4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Component {
    pub level: u32,
    pub weight: ExactRatio,
    pub vertices: Vec<FlowNodeId>,
    pub source_edges: Vec<EdgeId>,
    pub witness_edges: Vec<EdgeId>,
}

/// Algorithm 4's Task 1 witness graph with explicit source provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Union {
    pub graph: Graph,
    pub components: Vec<Component>,
}

/// Constructs the finite-domain witness union from one certified decomposition.
///
/// # Errors
///
/// Returns an error unless the decomposition has exactly one component covering
/// every source vertex and edge, or when the finite witness cannot be certified.
pub fn build(
    source: &Graph,
    decomposition: &Decomposition,
    domain: ExhaustiveDomain,
) -> Result<Union, Error> {
    decomposition
        .verify(source, domain)
        .map_err(Error::Experiment)?;
    let [component] = decomposition.components.as_slice() else {
        return Err(Error::UnsupportedDecomposition);
    };
    if component.vertices != (0..source.node_count()).map(FlowNodeId).collect::<Vec<_>>()
        || component.edges != (0..source.edge_count()).map(EdgeId).collect::<Vec<_>>()
    {
        return Err(Error::UnsupportedDecomposition);
    }
    let weight = decomposition
        .phi
        .checked_mul_integer(1_i128 << decomposition.level)
        .map_err(|_| Error::Overflow)?;
    let witness =
        build_complete(&vec![weight; source.node_count()], domain).map_err(Error::Experiment)?;
    let witness_edges = (0..witness.graph.edge_count()).map(EdgeId).collect();
    Ok(Union {
        graph: witness.graph,
        components: vec![Component {
            level: decomposition.level,
            weight,
            vertices: component.vertices.clone(),
            source_edges: component.edges.clone(),
            witness_edges,
        }],
    })
}

impl Union {
    /// Reconstructs the witness union from its source decomposition.
    ///
    /// # Errors
    ///
    /// Returns an error when any stored provenance, weight, or graph differs.
    pub fn verify(
        &self,
        source: &Graph,
        decomposition: &Decomposition,
        domain: ExhaustiveDomain,
    ) -> Result<(), Error> {
        if &build(source, decomposition, domain)? != self {
            return Err(Error::InvalidCertificate);
        }
        Ok(())
    }
}

/// Task 1 cannot build or verify the requested finite witness union.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("Algorithm 4 Task 1 decomposition is outside the supported single-component domain")]
    UnsupportedDecomposition,
    #[error("Algorithm 4 Task 1 finite witness failed: {0}")]
    Experiment(#[source] ExperimentError),
    #[error("Algorithm 4 Task 1 exact weight overflowed")]
    Overflow,
    #[error("Algorithm 4 Task 1 witness certificate is invalid")]
    InvalidCertificate,
}

#[cfg(test)]
mod tests {
    use super::build;
    use crate::{
        ExactRatio,
        source_spanner::experiment::{
            complete::build as build_complete, decomposition::single_level,
            domain::ExhaustiveDomain,
        },
    };

    #[test]
    fn constructs_a_weighted_single_component_witness_union() {
        let domain = ExhaustiveDomain { maximum_nodes: 8 };
        let source = build_complete(&[ExactRatio::new(1, 1).unwrap(); 4], domain)
            .unwrap()
            .graph;
        let decomposition = single_level(&source, ExactRatio::new(1, 2).unwrap(), domain).unwrap();
        let union = build(&source, &decomposition, domain).unwrap();
        assert_eq!(union.components.len(), 1);
        assert_eq!(union.components[0].weight, ExactRatio::new(1, 1).unwrap());
        assert_eq!(union.components[0].source_edges.len(), source.edge_count());
        union.verify(&source, &decomposition, domain).unwrap();
    }
}
