//! Checked finite-domain witness expanders for Theorem 8.4.

use crate::ExactRatio;

use super::model::{Edge, Error as ModelError, Graph};

/// A finite domain in which every nontrivial cut can be checked exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Domain {
    pub maximum_nodes: usize,
}

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
pub fn complete(weights: &[ExactRatio], domain: Domain) -> Result<Witness, Error> {
    let node_count = weights.len();
    if node_count < 2
        || domain.maximum_nodes < 2
        || node_count > domain.maximum_nodes
        || node_count >= u64::BITS as usize
    {
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
    let (expansion, cuts_checked) = exact_expansion(&graph)?;
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
        let (expansion, cuts_checked) = exact_expansion(&self.graph)?;
        let mut degrees = vec![0_u64; self.graph.node_count()];
        for index in 0..self.graph.edge_count() {
            let edge = self
                .graph
                .edge(super::model::EdgeId(index))
                .ok_or(Error::InvalidCertificate)?;
            degrees[edge.first.0] = degrees[edge.first.0]
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            degrees[edge.second.0] = degrees[edge.second.0]
                .checked_add(1)
                .ok_or(Error::Overflow)?;
        }
        if expansion != self.expansion
            || cuts_checked != self.cuts_checked
            || degrees != self.degrees
        {
            return Err(Error::InvalidCertificate);
        }
        Ok(())
    }
}

fn exact_expansion(graph: &Graph) -> Result<(ExactRatio, u64), Error> {
    let nodes = graph.node_count();
    let all = (1_u64 << nodes) - 1;
    let mut minimum: Option<ExactRatio> = None;
    let mut checked = 0_u64;
    for mask in 1..all {
        let complement = all ^ mask;
        if complement == 0 {
            continue;
        }
        let mut cut = 0_u64;
        let mut left_volume = 0_u64;
        let mut right_volume = 0_u64;
        for index in 0..graph.edge_count() {
            let edge = graph
                .edge(super::model::EdgeId(index))
                .ok_or(Error::InvalidCertificate)?;
            let first = mask & (1_u64 << edge.first.0) != 0;
            let second = mask & (1_u64 << edge.second.0) != 0;
            if first {
                left_volume = left_volume.checked_add(1).ok_or(Error::Overflow)?;
            } else {
                right_volume = right_volume.checked_add(1).ok_or(Error::Overflow)?;
            }
            if second {
                left_volume = left_volume.checked_add(1).ok_or(Error::Overflow)?;
            } else {
                right_volume = right_volume.checked_add(1).ok_or(Error::Overflow)?;
            }
            if first != second {
                cut = cut.checked_add(1).ok_or(Error::Overflow)?;
            }
        }
        let denominator = left_volume.min(right_volume);
        if denominator == 0 {
            return Err(Error::InvalidCertificate);
        }
        let value = ExactRatio::new(i128::from(cut), i128::from(denominator)).map_err(map_ratio)?;
        if minimum.is_none_or(|current| {
            current
                .at_least(value)
                .is_ok_and(|greater| greater && current != value)
        }) {
            minimum = Some(value);
        }
        checked = checked.checked_add(1).ok_or(Error::Overflow)?;
    }
    Ok((minimum.ok_or(Error::InvalidCertificate)?, checked))
}

#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("witness input is outside the exhaustive certified domain")]
    OutsideCertifiedDomain,
    #[error("witness degree does not satisfy w <= degree <= 18w")]
    DegreeSandwichViolation,
    #[error("witness expansion certificate is invalid")]
    InvalidCertificate,
    #[error("witness construction arithmetic overflowed")]
    Overflow,
    #[error("witness graph model is invalid: {0}")]
    Model(#[source] ModelError),
}

fn map_ratio(_: crate::StableMinRatioError) -> Error {
    Error::Overflow
}

#[cfg(test)]
mod tests {
    use super::{Domain, Error, complete};
    use crate::ExactRatio;

    fn weight(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    #[test]
    fn certifies_a_complete_witness_expander() {
        let witness = complete(
            &[weight(1), weight(1), weight(1), weight(1)],
            Domain { maximum_nodes: 8 },
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
            complete(&[weight(1); 20], Domain { maximum_nodes: 20 }),
            Err(Error::DegreeSandwichViolation)
        );
        assert_eq!(
            complete(&[weight(1); 4], Domain { maximum_nodes: 3 }),
            Err(Error::OutsideCertifiedDomain)
        );
    }
}
