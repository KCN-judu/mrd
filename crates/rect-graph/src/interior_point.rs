//! Checked rational interior-flow state used to audit potential-reduction
//! experiments before an almost-linear backend is introduced.
//!
//! The FOCS 2023 framework evaluates a fixed-point potential containing a
//! logarithm and fractional powers. Those operations are not exact rationals.
//! This module therefore does not claim to implement that theorem. It verifies
//! a rational reciprocal-slack surrogate, strict feasibility, bounded inputs,
//! and every observed potential decrease with exact arithmetic.

use num_bigint::BigInt;
use thiserror::Error;

use crate::{
    CertifiedFixedPoint, CirculationArcId, CirculationNetwork, CostedFlowRoundingResult,
    DyadicInterval, ExactRatio, FixedPointConfig, FixedPointError, FixedPointMetrics,
    FractionalCirculation, MinCostCirculationError,
};

/// Certified Equation (9) and Definition 4.2 quantities at one feasible flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedIpmSnapshot {
    fixed_point_config: FixedPointConfig,
    alpha: DyadicInterval,
    objective_gap: DyadicInterval,
    potential: DyadicInterval,
    lengths: Vec<DyadicInterval>,
    gradients: Vec<DyadicInterval>,
    arithmetic_metrics: FixedPointMetrics,
}

/// Proof that supplied approximate lengths and gradients meet Theorem 4.3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpmApproximationCertificate {
    pub edge_count: usize,
    pub factor_two_length_checks: u64,
    pub scaled_gradient_checks: u64,
}

impl CertifiedIpmSnapshot {
    /// Evaluates CKLPPS22 Equation (9) and Definition 4.2 with certified
    /// fixed-point intervals.
    ///
    /// The current circulation model has lower capacity zero. `optimal_cost`
    /// is the known integral `F*`, and `maximum_abs_input` is the checked source
    /// bound `U`. The source parameter is generated as
    /// `alpha = 1 / (1000 log(mU))`.
    ///
    /// # Errors
    ///
    /// Returns an error unless the flow is strictly interior, its objective is
    /// strictly above an integral `F*`, `mU > 1`, all inputs satisfy the bound,
    /// and every fixed-point enclosure stays within the configured word limit.
    pub fn evaluate(
        network: &CirculationNetwork,
        flow: &FractionalCirculation,
        optimal_cost: ExactRatio,
        maximum_abs_input: i128,
        fixed_point_config: FixedPointConfig,
    ) -> Result<Self, CertifiedIpmError> {
        network.verify_input_domain(maximum_abs_input)?;
        network.verify_fractional_solution(flow)?;
        if !optimal_cost.is_integral() {
            return Err(CertifiedIpmError::InvalidSourceDomain);
        }
        let objective_gap = flow
            .cost
            .checked_sub(optimal_cost)
            .map_err(|_| CertifiedIpmError::ExactOverflow)?;
        let zero = exact_ratio(0)?;
        if !objective_gap
            .at_least(zero)
            .map_err(|_| CertifiedIpmError::ExactOverflow)?
            || objective_gap == zero
        {
            return Err(CertifiedIpmError::InvalidSourceDomain);
        }
        let edge_count = network.arc_count();
        let edge_count_i128 =
            i128::try_from(edge_count).map_err(|_| CertifiedIpmError::InvalidSourceDomain)?;
        let m_u = edge_count_i128
            .checked_mul(maximum_abs_input)
            .ok_or(CertifiedIpmError::ExactOverflow)?;
        if m_u <= 1 {
            return Err(CertifiedIpmError::InvalidSourceDomain);
        }
        let mut arithmetic = CertifiedFixedPoint::new(fixed_point_config)?;

        let m_u_interval = arithmetic.enclose_ratio(m_u, 1)?;
        let log_m_u = arithmetic.logarithm(&m_u_interval)?;
        let thousand_log = arithmetic.multiply_interval_integer(&log_m_u, 1_000)?;
        let one = arithmetic.enclose_ratio(1, 1)?;
        let alpha = arithmetic.divide_intervals(&one, &thousand_log)?;
        if !alpha.is_strictly_positive() {
            return Err(CertifiedIpmError::UncertifiedApproximation);
        }

        let gap_interval = enclose_exact(&mut arithmetic, objective_gap)?;
        let log_gap = arithmetic.logarithm(&gap_interval)?;
        let potential_factor = edge_count_i128
            .checked_mul(20)
            .ok_or(CertifiedIpmError::ExactOverflow)?;
        let mut potential = arithmetic.multiply_interval_integer(&log_gap, potential_factor)?;
        let exponent = arithmetic.add_intervals(&one, &alpha)?;
        let slacks = network.fractional_slacks(&flow.arc_flows)?;
        let mut lengths = Vec::with_capacity(edge_count);
        let mut gradients = Vec::with_capacity(edge_count);

        for (index, (lower_slack, upper_slack)) in slacks.into_iter().enumerate() {
            let lower = enclose_exact(&mut arithmetic, lower_slack)?;
            let upper = enclose_exact(&mut arithmetic, upper_slack)?;
            if !lower.is_strictly_positive() || !upper.is_strictly_positive() {
                return Err(CertifiedIpmError::NotStrictlyInterior);
            }
            let lower_barrier = arithmetic.negative_power(&lower, &alpha)?;
            let upper_barrier = arithmetic.negative_power(&upper, &alpha)?;
            potential = arithmetic.add_intervals(&potential, &lower_barrier)?;
            potential = arithmetic.add_intervals(&potential, &upper_barrier)?;

            let lower_length = arithmetic.negative_power(&lower, &exponent)?;
            let upper_length = arithmetic.negative_power(&upper, &exponent)?;
            let length = arithmetic.add_intervals(&upper_length, &lower_length)?;
            if !length.is_strictly_positive() {
                return Err(CertifiedIpmError::UncertifiedApproximation);
            }

            let (_, cost) = network
                .arc_capacity_cost(CirculationArcId(index))
                .ok_or(CertifiedIpmError::InvalidSourceDomain)?;
            let cost_interval = arithmetic.enclose_ratio(cost, 1)?;
            let objective_numerator =
                arithmetic.multiply_interval_integer(&cost_interval, potential_factor)?;
            let objective_gradient =
                arithmetic.divide_intervals(&objective_numerator, &gap_interval)?;
            let upper_gradient = arithmetic.multiply_intervals(&alpha, &upper_length)?;
            let lower_gradient = arithmetic.multiply_intervals(&alpha, &lower_length)?;
            let barrier_gradient =
                arithmetic.subtract_intervals(&upper_gradient, &lower_gradient)?;
            let gradient = arithmetic.add_intervals(&objective_gradient, &barrier_gradient)?;

            lengths.push(length);
            gradients.push(gradient);
        }

        Ok(Self {
            fixed_point_config,
            alpha,
            objective_gap: gap_interval,
            potential,
            lengths,
            gradients,
            arithmetic_metrics: arithmetic.metrics(),
        })
    }

    #[must_use]
    pub const fn fixed_point_config(&self) -> FixedPointConfig {
        self.fixed_point_config
    }

    #[must_use]
    pub const fn alpha(&self) -> &DyadicInterval {
        &self.alpha
    }

    #[must_use]
    pub const fn objective_gap(&self) -> &DyadicInterval {
        &self.objective_gap
    }

    #[must_use]
    pub const fn potential(&self) -> &DyadicInterval {
        &self.potential
    }

    #[must_use]
    pub fn lengths(&self) -> &[DyadicInterval] {
        &self.lengths
    }

    #[must_use]
    pub fn gradients(&self) -> &[DyadicInterval] {
        &self.gradients
    }

    #[must_use]
    pub const fn arithmetic_metrics(&self) -> FixedPointMetrics {
        self.arithmetic_metrics
    }

    /// Certifies the factor-two length and scaled-gradient-error hypotheses in
    /// CKLPPS22 Theorem 4.3 for exact rational approximations.
    ///
    /// # Errors
    ///
    /// Returns the first edge whose length enclosure cannot prove
    /// `ell/2 <= ell_tilde <= 2ell` or whose gradient enclosure cannot prove
    /// `|(g_tilde-g)/ell| <= kappa/8`.
    pub fn certify_approximations(
        &self,
        approximate_gradients: &[ExactRatio],
        approximate_lengths: &[ExactRatio],
        kappa: ExactRatio,
        arithmetic: &mut CertifiedFixedPoint,
    ) -> Result<IpmApproximationCertificate, CertifiedIpmError> {
        if arithmetic.config() != self.fixed_point_config {
            return Err(CertifiedIpmError::ArithmeticConfigMismatch);
        }
        if approximate_gradients.len() != self.gradients.len()
            || approximate_lengths.len() != self.lengths.len()
        {
            return Err(CertifiedIpmError::DimensionMismatch);
        }
        let zero = exact_ratio(0)?;
        let one = exact_ratio(1)?;
        if !kappa
            .at_least(zero)
            .map_err(|_| CertifiedIpmError::ExactOverflow)?
            || kappa == zero
            || !one
                .at_least(kappa)
                .map_err(|_| CertifiedIpmError::ExactOverflow)?
        {
            return Err(CertifiedIpmError::InvalidSourceDomain);
        }
        let kappa_numerator = BigInt::from(kappa.numerator());
        let kappa_denominator = BigInt::from(kappa.denominator());

        for (edge, ((gradient, length), (approx_gradient, approx_length))) in self
            .gradients
            .iter()
            .zip(&self.lengths)
            .zip(approximate_gradients.iter().zip(approximate_lengths))
            .enumerate()
        {
            let approximate_length = enclose_exact(arithmetic, *approx_length)?;
            if !approximate_length.is_strictly_positive()
                || approximate_length.lower_scaled() * 2 < *length.upper_scaled()
                || approximate_length.upper_scaled() > &(length.lower_scaled() * 2)
            {
                return Err(CertifiedIpmError::LengthApproximation { edge });
            }

            let approximate_gradient = enclose_exact(arithmetic, *approx_gradient)?;
            let error = arithmetic.subtract_intervals(&approximate_gradient, gradient)?;
            let scaled_error = error.absolute_upper_scaled();
            let left = scaled_error * 8 * &kappa_denominator;
            let right = &kappa_numerator * length.lower_scaled();
            if left > right {
                return Err(CertifiedIpmError::GradientApproximation { edge });
            }
        }

        let edge_count = self.lengths.len();
        let checks = u64::try_from(edge_count).map_err(|_| CertifiedIpmError::ExactOverflow)?;
        Ok(IpmApproximationCertificate {
            edge_count,
            factor_two_length_checks: checks,
            scaled_gradient_checks: checks,
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CertifiedIpmError {
    #[error(transparent)]
    Network(#[from] MinCostCirculationError),
    #[error(transparent)]
    FixedPoint(#[from] FixedPointError),
    #[error("exact source arithmetic overflowed")]
    ExactOverflow,
    #[error("the source IPM domain or parameter is invalid")]
    InvalidSourceDomain,
    #[error("the flow is not strictly inside every capacity bound")]
    NotStrictlyInterior,
    #[error("the approximation vector dimensions do not match the graph")]
    DimensionMismatch,
    #[error("edge {edge} does not have a certified factor-two length approximation")]
    LengthApproximation { edge: usize },
    #[error("edge {edge} exceeds the certified scaled-gradient error")]
    GradientApproximation { edge: usize },
    #[error("the fixed-point intervals are too wide to certify the source hypothesis")]
    UncertifiedApproximation,
    #[error("the approximation auditor uses a different fixed-point configuration")]
    ArithmeticConfigMismatch,
}

fn exact_ratio(value: i128) -> Result<ExactRatio, CertifiedIpmError> {
    ExactRatio::new(value, 1).map_err(|_| CertifiedIpmError::ExactOverflow)
}

fn enclose_exact(
    arithmetic: &mut CertifiedFixedPoint,
    value: ExactRatio,
) -> Result<DyadicInterval, CertifiedIpmError> {
    Ok(arithmetic.enclose_ratio(value.numerator(), value.denominator())?)
}

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
    use super::{
        CertifiedIpmError, CertifiedIpmSnapshot, InteriorPointError, RationalInteriorPointState,
    };
    use crate::{
        CertifiedFixedPoint, CirculationNetwork, ExactRatio, FixedPointConfig, FlowNodeId,
        FractionalCirculation,
    };

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

    fn certified_arithmetic() -> CertifiedFixedPoint {
        CertifiedFixedPoint::new(FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap())
            .unwrap()
    }

    #[test]
    fn certifies_equation_nine_and_definition_four_two() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let flow = FractionalCirculation {
            arc_flows: vec![ExactRatio::new(1, 1).unwrap(); 2],
            cost: ExactRatio::new(1, 1).unwrap(),
        };
        let mut arithmetic = certified_arithmetic();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &flow,
            ExactRatio::new(0, 1).unwrap(),
            4,
            arithmetic.config(),
        )
        .unwrap();

        assert!(snapshot.alpha().is_strictly_positive());
        assert!(snapshot.objective_gap().contains_ratio(1, 1).unwrap());
        assert!(snapshot.potential().contains_ratio(4, 1).unwrap());
        assert_eq!(snapshot.lengths().len(), 2);
        assert!(
            snapshot
                .lengths()
                .iter()
                .all(|length| length.contains_ratio(2, 1).unwrap())
        );
        assert!(snapshot.gradients()[0].contains_ratio(40, 1).unwrap());
        assert!(snapshot.gradients()[1].contains_ratio(0, 1).unwrap());
        assert!(snapshot.arithmetic_metrics().arithmetic_operations > 0);

        let certificate = snapshot
            .certify_approximations(
                &[
                    ExactRatio::new(40, 1).unwrap(),
                    ExactRatio::new(0, 1).unwrap(),
                ],
                &[
                    ExactRatio::new(2, 1).unwrap(),
                    ExactRatio::new(2, 1).unwrap(),
                ],
                ExactRatio::new(1, 2).unwrap(),
                &mut arithmetic,
            )
            .unwrap();
        assert_eq!(certificate.edge_count, 2);
        assert_eq!(certificate.factor_two_length_checks, 2);
        assert_eq!(certificate.scaled_gradient_checks, 2);
    }

    #[test]
    fn rejects_uncertified_ipm_domains_and_approximations() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let boundary_flow = FractionalCirculation {
            arc_flows: vec![ExactRatio::new(0, 1).unwrap(); 2],
            cost: ExactRatio::new(0, 1).unwrap(),
        };
        let mut arithmetic = certified_arithmetic();
        assert_eq!(
            CertifiedIpmSnapshot::evaluate(
                &network,
                &boundary_flow,
                ExactRatio::new(-1, 1).unwrap(),
                4,
                arithmetic.config(),
            ),
            Err(CertifiedIpmError::NotStrictlyInterior)
        );

        let flow = FractionalCirculation {
            arc_flows: vec![ExactRatio::new(1, 1).unwrap(); 2],
            cost: ExactRatio::new(1, 1).unwrap(),
        };
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &flow,
            ExactRatio::new(0, 1).unwrap(),
            4,
            arithmetic.config(),
        )
        .unwrap();
        assert_eq!(
            snapshot.certify_approximations(
                &[
                    ExactRatio::new(40, 1).unwrap(),
                    ExactRatio::new(0, 1).unwrap()
                ],
                &[
                    ExactRatio::new(5, 1).unwrap(),
                    ExactRatio::new(2, 1).unwrap()
                ],
                ExactRatio::new(1, 2).unwrap(),
                &mut arithmetic,
            ),
            Err(CertifiedIpmError::LengthApproximation { edge: 0 })
        );
        assert_eq!(
            snapshot.certify_approximations(
                &[
                    ExactRatio::new(41, 1).unwrap(),
                    ExactRatio::new(0, 1).unwrap()
                ],
                &[
                    ExactRatio::new(2, 1).unwrap(),
                    ExactRatio::new(2, 1).unwrap()
                ],
                ExactRatio::new(1, 2).unwrap(),
                &mut arithmetic,
            ),
            Err(CertifiedIpmError::GradientApproximation { edge: 0 })
        );

        let mut mismatched =
            CertifiedFixedPoint::new(FixedPointConfig::source_bounded(1 << 20, 64, 32, 3).unwrap())
                .unwrap();
        assert_eq!(
            snapshot.certify_approximations(
                &[
                    ExactRatio::new(40, 1).unwrap(),
                    ExactRatio::new(0, 1).unwrap()
                ],
                &[
                    ExactRatio::new(2, 1).unwrap(),
                    ExactRatio::new(2, 1).unwrap()
                ],
                ExactRatio::new(1, 2).unwrap(),
                &mut mismatched,
            ),
            Err(CertifiedIpmError::ArithmeticConfigMismatch)
        );
    }

    #[test]
    fn matches_high_precision_oracle_on_nonunit_slacks() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 3, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 3, 0).unwrap();
        let flow = FractionalCirculation {
            arc_flows: vec![ExactRatio::new(1, 1).unwrap(); 2],
            cost: ExactRatio::new(1, 1).unwrap(),
        };
        let arithmetic = certified_arithmetic();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &flow,
            ExactRatio::new(0, 1).unwrap(),
            4,
            arithmetic.config(),
        )
        .unwrap();

        assert!(
            snapshot
                .alpha()
                .overlaps_ratio_interval(
                    480_898_346_962_987,
                    480_898_346_962_988,
                    1_000_000_000_000_000_000,
                )
                .unwrap()
        );
        assert!(
            snapshot
                .potential()
                .overlaps_ratio_interval(
                    3_999_333_444_432_099_794,
                    3_999_333_444_432_099_795,
                    1_000_000_000_000_000_000,
                )
                .unwrap()
        );
        for length in snapshot.lengths() {
            assert!(
                length
                    .overlaps_ratio_interval(
                        1_499_833_361_108_024_948,
                        1_499_833_361_108_024_949,
                        1_000_000_000_000_000_000,
                    )
                    .unwrap()
            );
        }
        assert!(
            snapshot.gradients()[0]
                .overlaps_ratio_interval(
                    39_999_759_470_690_150_815,
                    39_999_759_470_690_150_816,
                    1_000_000_000_000_000_000,
                )
                .unwrap()
        );
        assert!(
            snapshot.gradients()[1]
                .overlaps_ratio_interval(
                    -240_529_309_849_185,
                    -240_529_309_849_184,
                    1_000_000_000_000_000_000,
                )
                .unwrap()
        );
    }
}
