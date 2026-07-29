//! One-level finite-domain expander decomposition certificates.

use crate::ExactRatio;

use super::{
    super::model::{EdgeId, Graph},
    certificate::{ceil_log2, connected, degrees, expansion, map_ratio},
    domain::{Error, ExhaustiveDomain},
};

/// A checked one-level, edge-disjoint expander decomposition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Decomposition {
    pub level: u32,
    pub edges: Vec<EdgeId>,
    pub phi: ExactRatio,
    pub minimum_degree: u64,
    pub expansion: ExactRatio,
    pub cuts_checked: u64,
}

/// Certifies the one nonempty level when the input itself satisfies Theorem
/// 8.5's level capacity, degree-floor, and expansion predicates.
///
/// # Errors
///
/// Returns an error for a disconnected or oversized input, a nonpositive
/// `phi`, or when the input needs a genuinely multi-level decomposition.
pub fn single_level(
    graph: &Graph,
    phi: ExactRatio,
    domain: ExhaustiveDomain,
) -> Result<Decomposition, Error> {
    if !domain.contains(graph.node_count()) || !phi.is_positive() || !connected(graph)? {
        return Err(Error::OutsideCertifiedDomain);
    }
    let level = ceil_log2(graph.edge_count().div_ceil(graph.node_count()));
    let capacity = (1_u64 << level)
        .checked_mul(u64::try_from(graph.node_count()).map_err(|_| Error::Overflow)?)
        .ok_or(Error::Overflow)?;
    if u64::try_from(graph.edge_count()).map_err(|_| Error::Overflow)? > capacity {
        return Err(Error::InvalidCertificate);
    }
    let degrees = degrees(graph)?;
    let minimum_degree = *degrees.iter().min().ok_or(Error::InvalidCertificate)?;
    let required = phi
        .checked_mul_integer(1_i128 << level)
        .map_err(map_ratio)?;
    if !ExactRatio::new(i128::from(minimum_degree), 1)
        .map_err(map_ratio)?
        .at_least(required)
        .map_err(map_ratio)?
    {
        return Err(Error::DegreeSandwichViolation);
    }
    let (expansion, cuts_checked) = expansion(graph)?;
    if !expansion.at_least(phi).map_err(map_ratio)? {
        return Err(Error::InvalidCertificate);
    }
    Ok(Decomposition {
        level,
        edges: (0..graph.edge_count()).map(EdgeId).collect(),
        phi,
        minimum_degree,
        expansion,
        cuts_checked,
    })
}

impl Decomposition {
    /// Recomputes the level, partition, degree floor, and expansion evidence.
    ///
    /// # Errors
    ///
    /// Returns an error when a certificate field differs from fresh evidence.
    pub fn verify(&self, graph: &Graph, domain: ExhaustiveDomain) -> Result<(), Error> {
        if &single_level(graph, self.phi, domain)? != self {
            return Err(Error::InvalidCertificate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::single_level;
    use crate::{
        ExactRatio, FlowNodeId,
        source_spanner::{
            experiment::{
                complete::build,
                domain::{Error, ExhaustiveDomain},
            },
            model::{Edge, Graph},
        },
    };

    #[test]
    fn certifies_one_edge_disjoint_expander_level() {
        let witness = build(
            &[ExactRatio::new(1, 1).unwrap(); 4],
            ExhaustiveDomain { maximum_nodes: 8 },
        )
        .unwrap();
        let decomposition = single_level(
            &witness.graph,
            ExactRatio::new(1, 2).unwrap(),
            ExhaustiveDomain { maximum_nodes: 8 },
        )
        .unwrap();
        assert_eq!(decomposition.level, 1);
        assert_eq!(decomposition.edges.len(), witness.graph.edge_count());
        decomposition
            .verify(&witness.graph, ExhaustiveDomain { maximum_nodes: 8 })
            .unwrap();
        let disconnected = Graph::new(
            4,
            vec![
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                },
                Edge {
                    first: FlowNodeId(2),
                    second: FlowNodeId(3),
                },
            ],
        )
        .unwrap();
        assert_eq!(
            single_level(
                &disconnected,
                ExactRatio::new(1, 2).unwrap(),
                ExhaustiveDomain { maximum_nodes: 8 }
            ),
            Err(Error::OutsideCertifiedDomain)
        );
    }
}
