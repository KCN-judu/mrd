//! Canonical finite-domain witnesses for Algorithm 4's positive-level components.
//!
//! This is an exact finite substitute for the source's `ConstructExpander` call.
//! It selects the first circulant degree compatible with every requested source
//! weight and exhaustively certifies a fixed positive expansion floor. It does
//! not claim the general CGLNPS20 construction or its runtime.

use std::collections::BTreeSet;

use crate::{ExactRatio, FlowNodeId};

use super::{
    super::model::{Edge, Graph},
    certificate::{degrees, expansion, map_ratio},
    domain::{Error, ExhaustiveDomain},
};

const EXPANSION_FLOOR_NUMERATOR: i128 = 1;
const EXPANSION_FLOOR_DENOMINATOR: i128 = 4;

/// Deterministic finite witness and its exhaustive expansion certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Witness {
    pub graph: Graph,
    pub degrees: Vec<u64>,
    pub expansion: ExactRatio,
    pub cuts_checked: u64,
}

/// Builds the first canonical circulant graph satisfying the source degree
/// sandwich and the finite expansion certificate.
///
/// # Errors
///
/// Returns an error when the requested degree profile has no supported
/// canonical witness in the explicit exhaustive domain.
pub fn build(weights: &[ExactRatio], domain: ExhaustiveDomain) -> Result<Witness, Error> {
    let node_count = weights.len();
    if !domain.contains(node_count) {
        return Err(Error::OutsideCertifiedDomain);
    }
    let floor = ExactRatio::new(EXPANSION_FLOOR_NUMERATOR, EXPANSION_FLOOR_DENOMINATOR)
        .map_err(map_ratio)?;
    for degree in 1..node_count {
        let Ok(graph) = graph(node_count, degree) else {
            continue;
        };
        let degrees = degrees(&graph)?;
        if !degree_sandwich(&degrees, weights)? {
            continue;
        }
        let (expansion, cuts_checked) = expansion(&graph)?;
        if expansion.at_least(&floor).map_err(map_ratio)? {
            return Ok(Witness {
                graph,
                degrees,
                expansion,
                cuts_checked,
            });
        }
    }
    Err(Error::DegreeSandwichViolation)
}

impl Witness {
    /// Recomputes all finite graph, degree, and cut evidence.
    ///
    /// # Errors
    ///
    /// Returns an error when the stored witness cannot be reconstructed from
    /// its canonical degree or violates its certificate.
    pub fn verify(&self, weights: &[ExactRatio], domain: ExhaustiveDomain) -> Result<(), Error> {
        if &build(weights, domain)? != self {
            return Err(Error::InvalidCertificate);
        }
        Ok(())
    }
}

fn degree_sandwich(degrees: &[u64], weights: &[ExactRatio]) -> Result<bool, Error> {
    if degrees.len() != weights.len() {
        return Ok(false);
    }
    for (degree, weight) in degrees.iter().zip(weights) {
        let degree = ExactRatio::new(i128::from(*degree), 1).map_err(map_ratio)?;
        if !weight.is_positive()
            || !degree.at_least(weight).map_err(map_ratio)?
            || !weight
                .checked_mul_integer(18)
                .map_err(map_ratio)?
                .at_least(&degree)
                .map_err(map_ratio)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn graph(node_count: usize, degree: usize) -> Result<Graph, Error> {
    let mut pairs = BTreeSet::new();
    let mut distances = Vec::new();
    if degree % 2 == 1 {
        if node_count % 2 == 1 {
            return Err(Error::DegreeSandwichViolation);
        }
        distances.push(node_count / 2);
    }
    distances.extend(1..=(degree / 2));
    if distances.iter().any(|distance| *distance >= node_count) {
        return Err(Error::DegreeSandwichViolation);
    }
    for vertex in 0..node_count {
        for distance in &distances {
            let other = (vertex + distance) % node_count;
            pairs.insert((vertex.min(other), vertex.max(other)));
        }
    }
    Graph::new(
        node_count,
        pairs
            .into_iter()
            .map(|(first, second)| Edge {
                first: FlowNodeId(first),
                second: FlowNodeId(second),
            })
            .collect(),
    )
    .map_err(Error::Model)
}

#[cfg(test)]
mod tests {
    use super::build;
    use crate::{ExactRatio, source_spanner::experiment::domain::ExhaustiveDomain};

    #[test]
    fn chooses_a_sparse_certified_cycle_when_the_degree_sandwich_allows_it() {
        let witness = build(
            &vec![ExactRatio::new(1, 1).unwrap(); 5],
            ExhaustiveDomain { maximum_nodes: 8 },
        )
        .unwrap();
        assert_eq!(witness.graph.edge_count(), 5);
        assert_eq!(witness.degrees, vec![2; 5]);
        assert!(
            witness
                .expansion
                .at_least(&ExactRatio::new(1, 4).unwrap())
                .unwrap()
        );
        witness
            .verify(
                &vec![ExactRatio::new(1, 1).unwrap(); 5],
                ExhaustiveDomain { maximum_nodes: 8 },
            )
            .unwrap();
    }

    #[test]
    fn chooses_complete_only_when_the_requested_degree_requires_it() {
        let witness = build(
            &vec![ExactRatio::new(4, 1).unwrap(); 5],
            ExhaustiveDomain { maximum_nodes: 8 },
        )
        .unwrap();
        assert_eq!(witness.graph.edge_count(), 10);
        assert_eq!(witness.degrees, vec![4; 5]);
    }
}
