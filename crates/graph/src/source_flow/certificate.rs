//! Exact negative-decision certificates for the inclusive-target contract.
//!
//! The positive target direction recovers a feasible integral flow whose cost
//! is at most the supplied target. This module supplies the complementary,
//! independently verifiable certificate for the negative direction: a caller
//! may prove `F_opt > target` by supplying a feasible dual solution of the
//! min-cost circulation LP whose exact objective value strictly exceeds the
//! target. No reference solver constructs the certificate; the verifier only
//! checks exact dual feasibility and the strict objective bound.

use num_traits::Zero;
use thiserror::Error;

use crate::{
    CirculationArcId, CirculationNetwork, ExactRatio, MinCostCirculationError, StableMinRatioError,
};

/// A caller-supplied feasible dual solution certifying a strict lower bound.
///
/// For the primal `min c^T f` subject to `B^T f = d` and `0 <= f <= u`, the
/// dual (CKLPPS22 Equation (59)) is
///
/// ```text
/// max d^T y - s+^T u
/// s.t. B y + s- - s+ = c,  s-, s+ >= 0
/// ```
///
/// with the edge-node incidence used by this repository, `(B y)_e =
/// y[to_e] - y[from_e]`. By weak duality every feasible dual is a lower bound
/// on the integral optimum `F_opt`, so a dual objective strictly greater than
/// `target` certifies `F_opt > target`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DualLowerBoundCertificate {
    /// One vertex potential per network node (`y`).
    pub vertex_potentials: Vec<ExactRatio>,
    /// One nonnegative lower-bound slack per arc (`s-`).
    pub lower_slack: Vec<ExactRatio>,
    /// One nonnegative upper-bound slack per arc (`s+`).
    pub upper_slack: Vec<ExactRatio>,
}

impl DualLowerBoundCertificate {
    /// Builds a feasible dual certificate from one vertex-potential vector.
    ///
    /// For every arc the constructor sets
    /// `s+_e = max(0, y[to] - y[from] - c_e)` and
    /// `s-_e = max(0, c_e - (y[to] - y[from]))`, which is feasible by
    /// construction and maximizes the dual objective for the supplied `y`.
    ///
    /// # Errors
    ///
    /// Returns an error when the potential dimension does not match the
    /// network or exact arithmetic overflows.
    pub fn from_potentials(
        network: &CirculationNetwork,
        vertex_potentials: Vec<ExactRatio>,
    ) -> Result<Self, Error> {
        if vertex_potentials.len() != network.node_count() {
            return Err(Error::DimensionMismatch);
        }
        let zero = ExactRatio::new(0, 1)?;
        let mut lower_slack = Vec::with_capacity(network.arc_count());
        let mut upper_slack = Vec::with_capacity(network.arc_count());
        for index in 0..network.arc_count() {
            let (from, to) = network
                .arc_endpoints(CirculationArcId(index))
                .ok_or(Error::ArcOutOfBounds { arc: index })?;
            let (_, cost) = network
                .arc_capacity_cost(CirculationArcId(index))
                .ok_or(Error::ArcOutOfBounds { arc: index })?;
            let reduced = vertex_potentials[to.0]
                .checked_sub(&vertex_potentials[from.0])?
                .checked_sub(&ExactRatio::new(cost, 1)?)?;
            if reduced.at_least(&zero)? {
                upper_slack.push(reduced.clone());
                lower_slack.push(zero.clone());
            } else {
                upper_slack.push(zero.clone());
                lower_slack.push(reduced.checked_neg()?);
            }
        }
        Ok(Self {
            vertex_potentials,
            lower_slack,
            upper_slack,
        })
    }

    /// Verifies exact dual feasibility and returns the exact dual objective.
    ///
    /// Checks the vector dimensions, the nonnegativity of both slack vectors,
    /// and the exact per-arc equality `y[to] - y[from] + s- - s+ = c`.
    ///
    /// # Errors
    ///
    /// Returns an error on a dimension, sign, or per-arc feasibility violation
    /// or on exact arithmetic overflow.
    pub fn verify(&self, network: &CirculationNetwork) -> Result<ExactRatio, Error> {
        if self.vertex_potentials.len() != network.node_count()
            || self.lower_slack.len() != network.arc_count()
            || self.upper_slack.len() != network.arc_count()
        {
            return Err(Error::DimensionMismatch);
        }
        for index in 0..network.arc_count() {
            let (from, to) = network
                .arc_endpoints(CirculationArcId(index))
                .ok_or(Error::ArcOutOfBounds { arc: index })?;
            let (_, cost) = network
                .arc_capacity_cost(CirculationArcId(index))
                .ok_or(Error::ArcOutOfBounds { arc: index })?;
            if self.lower_slack[index].is_negative() || self.upper_slack[index].is_negative() {
                return Err(Error::NegativeSlack { arc: index });
            }
            let lhs = self.vertex_potentials[to.0]
                .checked_sub(&self.vertex_potentials[from.0])?
                .checked_add(&self.lower_slack[index])?
                .checked_sub(&self.upper_slack[index])?;
            if lhs != ExactRatio::new(cost, 1)? {
                return Err(Error::DualInfeasible { arc: index });
            }
        }
        let mut objective = ExactRatio::new(0, 1)?;
        for (node, demand) in network.demands().iter().copied().enumerate() {
            if demand.is_zero() {
                continue;
            }
            objective = objective
                .checked_add(&self.vertex_potentials[node].checked_mul_integer(demand)?)?;
        }
        for index in 0..network.arc_count() {
            let (capacity, _) = network
                .arc_capacity_cost(CirculationArcId(index))
                .ok_or(Error::ArcOutOfBounds { arc: index })?;
            if capacity == 0 {
                continue;
            }
            objective =
                objective.checked_sub(&self.upper_slack[index].checked_mul_integer(capacity)?)?;
        }
        Ok(objective)
    }
}

/// Exact evidence that a caller-supplied dual certifies `F_opt > target`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InfeasibilityProof {
    /// The target that the optimal cost strictly exceeds.
    pub target: i128,
    /// The exact verified dual objective value.
    pub dual_objective: ExactRatio,
}

/// A dual certificate could not be verified exactly.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("dual-certificate vector dimensions do not match the network")]
    DimensionMismatch,
    #[error("dual-certificate arc {arc} is outside the network")]
    ArcOutOfBounds { arc: usize },
    #[error("dual-certificate slack for arc {arc} is negative")]
    NegativeSlack { arc: usize },
    #[error("dual certificate is not feasible on arc {arc}")]
    DualInfeasible { arc: usize },
    #[error(transparent)]
    Ratio(#[from] StableMinRatioError),
    #[error(transparent)]
    Network(#[from] MinCostCirculationError),
}

#[cfg(test)]
mod tests {
    use super::{DualLowerBoundCertificate, Error};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, MinCostSolution, source_flow::Backend,
    };

    #[test]
    fn dual_certificate_certifies_a_strict_lower_bound_on_the_optimum() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let certificate = DualLowerBoundCertificate::from_potentials(
            &network,
            vec![
                ExactRatio::new(0, 1).unwrap(),
                ExactRatio::new(0, 1).unwrap(),
            ],
        )
        .unwrap();
        let proof = Backend
            .prove_infeasible_below(&network, -1, &certificate)
            .unwrap();
        assert_eq!(proof.target, -1);
        assert_eq!(proof.dual_objective, ExactRatio::new(0, 1).unwrap());
        assert_eq!(
            Backend.require_complete(),
            Err(crate::source_flow::Error::Incomplete)
        );
    }

    #[test]
    fn dual_certificate_rejects_a_target_that_is_not_exceeded() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let certificate = DualLowerBoundCertificate::from_potentials(
            &network,
            vec![
                ExactRatio::new(0, 1).unwrap(),
                ExactRatio::new(0, 1).unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(
            Backend.prove_infeasible_below(&network, 0, &certificate),
            Err(crate::source_flow::Error::CertificateInsufficient {
                target: 0,
                dual_objective: ExactRatio::new(0, 1).unwrap()
            })
        );
    }

    #[test]
    fn dual_certificate_rejects_an_infeasible_slack_assignment() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let certificate = DualLowerBoundCertificate {
            vertex_potentials: vec![ExactRatio::new(0, 1).unwrap(); 2],
            lower_slack: vec![ExactRatio::new(1, 1).unwrap(); 2],
            upper_slack: vec![ExactRatio::new(0, 1).unwrap(); 2],
        };
        assert_eq!(
            certificate.verify(&network),
            Err(Error::DualInfeasible { arc: 1 })
        );
    }

    #[test]
    fn dual_certificate_rejects_negative_slack() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let certificate = DualLowerBoundCertificate {
            vertex_potentials: vec![ExactRatio::new(0, 1).unwrap(); 2],
            lower_slack: vec![
                ExactRatio::new(-1, 1).unwrap(),
                ExactRatio::new(0, 1).unwrap(),
            ],
            upper_slack: vec![ExactRatio::new(0, 1).unwrap(); 2],
        };
        assert_eq!(
            certificate.verify(&network),
            Err(Error::NegativeSlack { arc: 0 })
        );
    }

    #[test]
    fn dual_certificate_uses_demands_in_the_objective() {
        let mut network = CirculationNetwork::new(2);
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(1), 1).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let certificate = DualLowerBoundCertificate::from_potentials(
            &network,
            vec![
                ExactRatio::new(0, 1).unwrap(),
                ExactRatio::new(2, 1).unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(
            certificate.verify(&network).unwrap(),
            ExactRatio::new(0, 1).unwrap()
        );
        let solution = MinCostSolution {
            arc_flows: vec![1, 0],
            cost: 1,
        };
        network.verify_feasible_solution(&solution).unwrap();
        let solution = MinCostSolution {
            arc_flows: vec![0, 1],
            cost: 0,
        };
        assert_eq!(
            network.verify_feasible_solution(&solution),
            Err(crate::MinCostCirculationError::InvalidSolution)
        );
    }

    #[test]
    fn dual_certificate_dimension_mismatch_rejects() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let certificate = DualLowerBoundCertificate {
            vertex_potentials: vec![ExactRatio::new(0, 1).unwrap(); 3],
            lower_slack: vec![ExactRatio::new(0, 1).unwrap(); 2],
            upper_slack: vec![ExactRatio::new(0, 1).unwrap(); 2],
        };
        assert_eq!(certificate.verify(&network), Err(Error::DimensionMismatch));
    }

    #[test]
    fn from_potentials_never_constructs_an_infeasible_certificate() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        for y in [0_i128, 1, -1, 5, -5] {
            let certificate = DualLowerBoundCertificate::from_potentials(
                &network,
                vec![
                    ExactRatio::new(y, 1).unwrap(),
                    ExactRatio::new(-y, 1).unwrap(),
                ],
            )
            .unwrap();
            assert!(certificate.verify(&network).is_ok());
        }
    }

    #[test]
    fn zero_potential_certificate_upper_bounds_the_zero_flow() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let certificate = DualLowerBoundCertificate::from_potentials(
            &network,
            vec![
                ExactRatio::new(0, 1).unwrap(),
                ExactRatio::new(0, 1).unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(
            certificate.verify(&network).unwrap(),
            ExactRatio::new(0, 1).unwrap()
        );
        let solution = MinCostSolution {
            arc_flows: vec![0, 0],
            cost: 0,
        };
        network.verify_feasible_solution(&solution).unwrap();
    }
}
