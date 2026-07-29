//! Deterministic complete-graph witnesses for the finite expander domain.

use crate::ExactRatio;

use super::{
    super::model::{Edge, Graph},
    certificate::{degrees, expansion, map_ratio},
    domain::{Error, ExhaustiveDomain},
};

/// Deterministic witness graph and its exhaustive expansion certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Witness {
    pub graph: Graph,
    pub degrees: Vec<u64>,
    pub expansion: ExactRatio,
    pub cuts_checked: u64,
}

/// Builds a complete-graph witness on an explicitly certified finite domain.
///
/// The complete graph has degree `n - 1`; this constructor accepts exactly
/// when each requested weight is bracketed by that degree and eighteen times
/// that weight. It is not a replacement for CGLNPS20's general construction.
///
/// # Errors
///
/// Returns an error outside the finite domain, for a nonpositive weight, when
/// the degree sandwich fails, or when exact cut accounting overflows.
pub fn build(weights: &[ExactRatio], domain: ExhaustiveDomain) -> Result<Witness, Error> {
    let node_count = weights.len();
    if !domain.contains(node_count) {
        return Err(Error::OutsideCertifiedDomain);
    }
    let degree = u64::try_from(node_count - 1).map_err(|_| Error::Overflow)?;
    let degree_ratio = ExactRatio::new(i128::from(degree), 1).map_err(map_ratio)?;
    for weight in weights {
        if !weight.is_positive()
            || !degree_ratio.at_least(*weight).map_err(map_ratio)?
            || !weight
                .checked_mul_integer(18)
                .map_err(map_ratio)?
                .at_least(degree_ratio)
                .map_err(map_ratio)?
        {
            return Err(Error::DegreeSandwichViolation);
        }
    }
    let graph = Graph::new(
        node_count,
        (0..node_count)
            .flat_map(|first| {
                ((first + 1)..node_count).map(move |second| Edge {
                    first: crate::FlowNodeId(first),
                    second: crate::FlowNodeId(second),
                })
            })
            .collect(),
    )
    .map_err(Error::Model)?;
    let (expansion, cuts_checked) = expansion(&graph)?;
    Ok(Witness {
        graph,
        degrees: vec![degree; node_count],
        expansion,
        cuts_checked,
    })
}

impl Witness {
    /// Recomputes degree and every nontrivial cut certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when any stored measurement differs from a fresh exact
    /// exhaustive calculation.
    pub fn verify(&self) -> Result<(), Error> {
        let (expansion, cuts_checked) = expansion(&self.graph)?;
        let degrees = degrees(&self.graph)?;
        if expansion != self.expansion
            || cuts_checked != self.cuts_checked
            || degrees != self.degrees
        {
            return Err(Error::InvalidCertificate);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::build;
    use crate::{
        ExactRatio,
        source_spanner::experiment::domain::{Error, ExhaustiveDomain},
    };

    fn weight(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    #[test]
    fn certifies_a_complete_witness_expander() {
        let witness = build(
            &[weight(1), weight(1), weight(1), weight(1)],
            ExhaustiveDomain { maximum_nodes: 8 },
        )
        .unwrap();
        assert_eq!(witness.degrees, vec![3; 4]);
        assert_eq!(witness.expansion, ExactRatio::new(2, 3).unwrap());
        assert_eq!(witness.cuts_checked, 14);
        witness.verify().unwrap();
    }

    #[test]
    fn rejects_an_unbracketed_degree_or_large_domain() {
        assert_eq!(
            build(&[weight(1); 20], ExhaustiveDomain { maximum_nodes: 20 }),
            Err(Error::DegreeSandwichViolation)
        );
        assert_eq!(
            build(&[weight(1); 4], ExhaustiveDomain { maximum_nodes: 3 }),
            Err(Error::OutsideCertifiedDomain)
        );
        assert_eq!(
            build(&[weight(1); 4], ExhaustiveDomain { maximum_nodes: 21 }),
            Err(Error::OutsideCertifiedDomain)
        );
    }
}
