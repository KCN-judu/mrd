//! Checked rational interior-flow state used to audit potential-reduction
//! experiments before an almost-linear backend is introduced.
//!
//! The FOCS 2023 framework evaluates a fixed-point potential containing a
//! logarithm and fractional powers. Those operations are not exact rationals.
//! This module therefore does not claim to implement that theorem. It verifies
//! a rational reciprocal-slack surrogate, strict feasibility, bounded inputs,
//! and every observed potential decrease with exact arithmetic.

use thiserror::Error;

use crate::{
    CirculationNetwork, CostedFlowRoundingResult, ExactRatio, FractionalCirculation,
    MinCostCirculationError,
};

/// Auditable totals for rational potential-reduction updates.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteriorPointMetrics {
    pub iterations: u64,
    pub changed_coordinates: u64,
}

/// A bounded-domain, strictly feasible rational flow state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalInteriorPointState {
    flow: FractionalCirculation,
    objective_lower_bound: ExactRatio,
    barrier_weight: ExactRatio,
    maximum_abs_input: i128,
    potential: ExactRatio,
    metrics: InteriorPointMetrics,
}

impl RationalInteriorPointState {
    /// Creates a checked strictly interior state.
    ///
    /// `objective_lower_bound` is a known lower objective bound. The state requires a
    /// strictly positive gap so reciprocal-slack potential experiments remain
    /// in the open feasible domain.
    ///
    /// # Errors
    ///
    /// Returns an error when the network or flow violates the bounded/open
    /// domain, the target is not a strict lower bound, or arithmetic overflows.
    pub fn new(
        network: &CirculationNetwork,
        flow: FractionalCirculation,
        objective_lower_bound: ExactRatio,
        barrier_weight: ExactRatio,
        maximum_abs_input: i128,
    ) -> Result<Self, InteriorPointError> {
        network.verify_input_domain(maximum_abs_input)?;
        network.verify_fractional_solution(&flow)?;
        let potential = rational_potential(network, &flow, objective_lower_bound, barrier_weight)?;
        Ok(Self {
            flow,
            objective_lower_bound,
            barrier_weight,
            maximum_abs_input,
            potential,
            metrics: InteriorPointMetrics::default(),
        })
    }

    #[must_use]
    pub const fn flow(&self) -> &FractionalCirculation {
        &self.flow
    }

    #[must_use]
    pub const fn potential(&self) -> ExactRatio {
        self.potential
    }

    #[must_use]
    pub const fn metrics(&self) -> InteriorPointMetrics {
        self.metrics
    }

    #[must_use]
    pub const fn maximum_abs_input(&self) -> i128 {
        self.maximum_abs_input
    }

    /// Applies a rational circulation update only when it remains strictly
    /// feasible and decreases the exact surrogate potential.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-circulation direction, a nonpositive step,
    /// a boundary crossing, a non-decreasing potential, or arithmetic
    /// overflow.
    pub fn apply_decreasing_update(
        &mut self,
        network: &CirculationNetwork,
        direction: &[ExactRatio],
        step: ExactRatio,
    ) -> Result<(), InteriorPointError> {
        network.verify_input_domain(self.maximum_abs_input)?;
        network.verify_fractional_circulation(direction)?;
        let zero = ratio(0)?;
        if !step.at_least(zero).map_err(map_ratio)? || step == zero {
            return Err(InteriorPointError::NonpositiveStep);
        }
        let arc_flows = self
            .flow
            .arc_flows
            .iter()
            .zip(direction)
            .map(|(flow, delta)| {
                flow.checked_add(step.checked_mul(*delta).map_err(map_ratio)?)
                    .map_err(map_ratio)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let candidate = FractionalCirculation {
            cost: network.fractional_cost(&arc_flows)?,
            arc_flows,
        };
        network.verify_fractional_solution(&candidate)?;
        let potential = rational_potential(
            network,
            &candidate,
            self.objective_lower_bound,
            self.barrier_weight,
        )?;
        if !self.potential.at_least(potential).map_err(map_ratio)? || self.potential == potential {
            return Err(InteriorPointError::PotentialDidNotDecrease);
        }
        let changed = direction.iter().filter(|value| **value != zero).count();
        self.metrics.iterations = self
            .metrics
            .iterations
            .checked_add(1)
            .ok_or(InteriorPointError::Overflow)?;
        self.metrics.changed_coordinates = self
            .metrics
            .changed_coordinates
            .checked_add(u64::try_from(changed).map_err(|_| InteriorPointError::Overflow)?)
            .ok_or(InteriorPointError::Overflow)?;
        self.flow = candidate;
        self.potential = potential;
        Ok(())
    }

    /// Deterministically recovers an integral feasible circulation with no
    /// greater cost using the P9 fractional-rounding Oracle.
    ///
    /// # Errors
    ///
    /// Returns an error when the retained rational flow cannot be rounded
    /// exactly or arithmetic overflows.
    pub fn recover_integral(
        &self,
        network: &CirculationNetwork,
    ) -> Result<CostedFlowRoundingResult, InteriorPointError> {
        Ok(network.round_fractional_costed(&self.flow)?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum InteriorPointError {
    #[error(transparent)]
    Network(#[from] MinCostCirculationError),
    #[error("exact rational arithmetic overflowed or is undefined")]
    Overflow,
    #[error("the step must be strictly positive")]
    NonpositiveStep,
    #[error("the candidate did not strictly decrease the rational surrogate potential")]
    PotentialDidNotDecrease,
}

fn map_ratio(_: crate::StableMinRatioError) -> InteriorPointError {
    InteriorPointError::Overflow
}

fn ratio(value: i128) -> Result<ExactRatio, InteriorPointError> {
    ExactRatio::new(value, 1).map_err(map_ratio)
}

fn rational_potential(
    network: &CirculationNetwork,
    flow: &FractionalCirculation,
    objective_lower_bound: ExactRatio,
    barrier_weight: ExactRatio,
) -> Result<ExactRatio, InteriorPointError> {
    let zero = ratio(0)?;
    if !flow
        .cost
        .at_least(objective_lower_bound)
        .map_err(map_ratio)?
        || flow.cost == objective_lower_bound
    {
        return Err(InteriorPointError::PotentialDidNotDecrease);
    }
    if !barrier_weight.at_least(zero).map_err(map_ratio)? || barrier_weight == zero {
        return Err(InteriorPointError::NonpositiveStep);
    }
    let barrier = network
        .fractional_slacks(&flow.arc_flows)?
        .into_iter()
        .try_fold(zero, |sum, (lower, upper)| {
            if lower == zero || upper == zero {
                return Err(InteriorPointError::PotentialDidNotDecrease);
            }
            let reciprocal_sum = lower
                .reciprocal()
                .and_then(|left| upper.reciprocal().and_then(|right| left.checked_add(right)))
                .map_err(map_ratio)?;
            sum.checked_add(
                barrier_weight
                    .checked_mul(reciprocal_sum)
                    .map_err(map_ratio)?,
            )
            .map_err(map_ratio)
        })?;
    flow.cost
        .checked_sub(objective_lower_bound)
        .and_then(|gap| gap.checked_add(barrier))
        .map_err(map_ratio)
}

#[cfg(test)]
mod tests {
    use super::{InteriorPointError, RationalInteriorPointState};
    use crate::{CirculationNetwork, ExactRatio, FlowNodeId, FractionalCirculation};

    #[test]
    fn records_exact_decreasing_rational_updates() {
        let mut network = CirculationNetwork::new(2);
        network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 2, -10)
            .unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let half = ExactRatio::new(1, 2).unwrap();
        let flow = FractionalCirculation {
            arc_flows: vec![half; 2],
            cost: ExactRatio::new(-5, 1).unwrap(),
        };
        let mut state = RationalInteriorPointState::new(
            &network,
            flow,
            ExactRatio::new(-20, 1).unwrap(),
            ExactRatio::new(1, 100).unwrap(),
            20,
        )
        .unwrap();
        let before = state.potential();
        state
            .apply_decreasing_update(
                &network,
                &[
                    ExactRatio::new(1, 1).unwrap(),
                    ExactRatio::new(1, 1).unwrap(),
                ],
                ExactRatio::new(1, 10).unwrap(),
            )
            .unwrap();
        assert!(before.at_least(state.potential()).unwrap());
        assert_eq!(state.metrics().iterations, 1);
        assert_eq!(state.metrics().changed_coordinates, 2);
        network.verify_fractional_solution(state.flow()).unwrap();
    }

    #[test]
    fn rejects_non_decreasing_or_out_of_domain_candidates() {
        let mut network = CirculationNetwork::new(2);
        network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 2, -10)
            .unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let half = ExactRatio::new(1, 2).unwrap();
        let flow = FractionalCirculation {
            arc_flows: vec![half; 2],
            cost: ExactRatio::new(-5, 1).unwrap(),
        };
        assert!(
            RationalInteriorPointState::new(
                &network,
                flow.clone(),
                ExactRatio::new(-20, 1).unwrap(),
                ExactRatio::new(1, 100).unwrap(),
                5,
            )
            .is_err()
        );
        let mut state = RationalInteriorPointState::new(
            &network,
            flow,
            ExactRatio::new(-20, 1).unwrap(),
            ExactRatio::new(1, 100).unwrap(),
            20,
        )
        .unwrap();
        assert_eq!(
            state.apply_decreasing_update(
                &network,
                &[
                    ExactRatio::new(-1, 1).unwrap(),
                    ExactRatio::new(-1, 1).unwrap()
                ],
                ExactRatio::new(1, 10).unwrap(),
            ),
            Err(InteriorPointError::PotentialDidNotDecrease)
        );
    }
}
