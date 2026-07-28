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
    FractionalCirculation, InitialPointAugmentation, IsolationPerturbation,
    IsolationRecoveryCertificate, LowerBoundCirculationNetwork, LowerBoundNormalization,
    MinCostCirculationError, MinRatioEdgeId,
};

/// Certified Equation (9) and Definition 4.2 quantities at one feasible flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedIpmSnapshot {
    fixed_point_config: FixedPointConfig,
    flow: FractionalCirculation,
    optimal_cost: ExactRatio,
    maximum_abs_input: i128,
    alpha: DyadicInterval,
    objective_gap: DyadicInterval,
    potential: DyadicInterval,
    lengths: Vec<DyadicInterval>,
    gradients: Vec<DyadicInterval>,
    arithmetic_metrics: FixedPointMetrics,
    update_metrics: IpmUpdateMetrics,
}

/// Counters required to audit the deterministic IPM interaction contract.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IpmUpdateMetrics {
    pub iterations: u64,
    pub changed_coordinates: u64,
    pub detect_calls: u64,
    pub detected_edges: u64,
}

/// A certified Lemma 4.4 transition and its successor state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedIpmUpdate {
    pub next_snapshot: CertifiedIpmSnapshot,
    pub eta: ExactRatio,
    pub approximation: IpmApproximationCertificate,
}

/// Evidence that the source additive-half termination boundary is certified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IpmTerminationCertificate {
    pub potential_bound: DyadicInterval,
    pub objective_gap: DyadicInterval,
}

/// Augmented source instance together with its certified initial snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedIpmInitialPoint {
    pub augmentation: InitialPointAugmentation,
    pub snapshot: CertifiedIpmSnapshot,
}

/// Lower-bound normalization followed by the certified Appendix B.1 initial point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CertifiedLowerBoundInitialPoint {
    pub normalization: LowerBoundNormalization,
    pub initial_point: CertifiedIpmInitialPoint,
}

/// Certified per-edge accounting for the dynamic `Detect` operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IpmDetectLedger {
    fixed_point_config: FixedPointConfig,
    accumulated_changes: Vec<DyadicInterval>,
    metrics: IpmUpdateMetrics,
}

impl IpmDetectLedger {
    /// Creates an empty ledger with the same precision as a certified snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot's fixed-point configuration cannot be
    /// constructed or its bounded word model rejects the zero interval.
    pub fn new(snapshot: &CertifiedIpmSnapshot) -> Result<Self, CertifiedIpmError> {
        let mut arithmetic = CertifiedFixedPoint::new(snapshot.fixed_point_config)?;
        let zero = arithmetic.enclose_ratio(0, 1)?;
        Ok(Self {
            fixed_point_config: snapshot.fixed_point_config,
            accumulated_changes: vec![zero; snapshot.lengths.len()],
            metrics: IpmUpdateMetrics::default(),
        })
    }

    #[must_use]
    pub fn accumulated_changes(&self) -> &[DyadicInterval] {
        &self.accumulated_changes
    }

    #[must_use]
    pub const fn metrics(&self) -> IpmUpdateMetrics {
        self.metrics
    }

    /// Records one update using the lengths certified at its source snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for a precision mismatch, dimension mismatch,
    /// nonpositive step, exact overflow, or a fixed-point word-bound failure.
    pub fn record_update(
        &mut self,
        snapshot: &CertifiedIpmSnapshot,
        eta: ExactRatio,
        direction: &[ExactRatio],
    ) -> Result<(), CertifiedIpmError> {
        if snapshot.fixed_point_config != self.fixed_point_config {
            return Err(CertifiedIpmError::ArithmeticConfigMismatch);
        }
        if direction.len() != self.accumulated_changes.len()
            || snapshot.lengths.len() != self.accumulated_changes.len()
        {
            return Err(CertifiedIpmError::DimensionMismatch);
        }
        if !eta.is_positive() {
            return Err(CertifiedIpmError::InvalidUpdateDirection);
        }
        let mut arithmetic = CertifiedFixedPoint::new(self.fixed_point_config)?;
        for ((accumulated, length), delta) in self
            .accumulated_changes
            .iter_mut()
            .zip(&snapshot.lengths)
            .zip(direction)
        {
            let magnitude = eta
                .checked_mul(delta.abs().map_err(map_exact)?)
                .map_err(map_exact)?;
            let change = enclose_exact(&mut arithmetic, magnitude)?;
            let weighted = arithmetic.multiply_intervals(length, &change)?;
            *accumulated = arithmetic.add_intervals(accumulated, &weighted)?;
        }
        Ok(())
    }

    /// Reports exactly the edges whose accumulated interval lower bound is at
    /// least `epsilon`, and resets those edge accumulators to zero.
    ///
    /// # Errors
    ///
    /// Returns an error when `epsilon` is not positive or the configured
    /// fixed-point arithmetic cannot certify the threshold.
    pub fn detect(
        &mut self,
        epsilon: ExactRatio,
    ) -> Result<Vec<MinRatioEdgeId>, CertifiedIpmError> {
        if !epsilon.is_positive() {
            return Err(CertifiedIpmError::InvalidDetectThreshold);
        }
        let mut arithmetic = CertifiedFixedPoint::new(self.fixed_point_config)?;
        let threshold = enclose_exact(&mut arithmetic, epsilon)?;
        let zero = arithmetic.enclose_ratio(0, 1)?;
        let mut detected = Vec::new();
        for (index, accumulated) in self.accumulated_changes.iter_mut().enumerate() {
            if accumulated.lower_scaled() >= threshold.upper_scaled() {
                detected.push(MinRatioEdgeId(index));
                *accumulated = zero.clone();
            }
        }
        self.metrics.detect_calls = self
            .metrics
            .detect_calls
            .checked_add(1)
            .ok_or(CertifiedIpmError::ExactOverflow)?;
        self.metrics.detected_edges = self
            .metrics
            .detected_edges
            .checked_add(
                u64::try_from(detected.len()).map_err(|_| CertifiedIpmError::ExactOverflow)?,
            )
            .ok_or(CertifiedIpmError::ExactOverflow)?;
        Ok(detected)
    }
}

/// Proof that supplied approximate lengths and gradients meet Theorem 4.3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IpmApproximationCertificate {
    pub edge_count: usize,
    pub factor_two_length_checks: u64,
    pub scaled_gradient_checks: u64,
}

impl CertifiedIpmSnapshot {
    /// Normalizes nonzero lower bounds, shifts the supplied optimum exactly,
    /// and constructs the certified Appendix B.1 initial point.
    ///
    /// # Errors
    ///
    /// Returns an error when lower-bound normalization, objective shifting,
    /// source augmentation, or fixed-point certification fails.
    pub fn initial_point_lower_bounded(
        network: &LowerBoundCirculationNetwork,
        optimal_cost: ExactRatio,
        maximum_abs_input: i128,
        fixed_point_config: FixedPointConfig,
    ) -> Result<CertifiedLowerBoundInitialPoint, CertifiedIpmError> {
        let normalization = network.normalize_lower_bounds(maximum_abs_input)?;
        let offset = ExactRatio::new(normalization.objective_offset, 1).map_err(map_exact)?;
        let normalized_optimal = optimal_cost.checked_sub(offset).map_err(map_exact)?;
        let initial_point = Self::initial_point_augmented(
            &normalization.normalized,
            normalized_optimal,
            maximum_abs_input,
            fixed_point_config,
        )?;
        Ok(CertifiedLowerBoundInitialPoint {
            normalization,
            initial_point,
        })
    }

    /// Constructs and certifies the Appendix B.1 initial-point augmentation
    /// for the current zero-lower-bound circulation model.
    ///
    /// # Errors
    ///
    /// Returns an error when the source input bound is violated, the augmented
    /// midpoint is malformed, or its conservative initial-potential bound
    /// cannot be certified.
    pub fn initial_point_augmented(
        network: &CirculationNetwork,
        optimal_cost: ExactRatio,
        maximum_abs_input: i128,
        fixed_point_config: FixedPointConfig,
    ) -> Result<CertifiedIpmInitialPoint, CertifiedIpmError> {
        let augmentation = network.initial_point_augmentation(maximum_abs_input)?;
        let snapshot = Self::evaluate(
            &augmentation.network,
            &augmentation.initial_flow,
            optimal_cost,
            augmentation.maximum_abs_input,
            fixed_point_config,
        )?;
        certify_initial_potential_bound(&snapshot, &augmentation.network)?;
        Ok(CertifiedIpmInitialPoint {
            augmentation,
            snapshot,
        })
    }

    /// Builds the midpoint flow for the normalized zero-demand model and
    /// certifies the source `200m log(mU)` initial-potential bound.
    ///
    /// This is deliberately a restricted initializer: it does not construct
    /// the source paper's O(m)-edge augmentation for arbitrary demands or
    /// lower bounds. Every arc must have positive capacity and the network
    /// must have zero demands.
    ///
    /// # Errors
    ///
    /// Returns an error when the normalized domain is unsupported, the
    /// midpoint is not feasible, or the initial potential bound cannot be
    /// certified with the supplied fixed-point configuration.
    pub fn initial_point_zero_demand(
        network: &CirculationNetwork,
        optimal_cost: ExactRatio,
        maximum_abs_input: i128,
        fixed_point_config: FixedPointConfig,
    ) -> Result<Self, CertifiedIpmError> {
        if network.arc_count() == 0 {
            return Err(CertifiedIpmError::UnsupportedInitialPointDomain);
        }
        let mut arc_flows = Vec::with_capacity(network.arc_count());
        for index in 0..network.arc_count() {
            let (capacity, _) = network
                .arc_capacity_cost(CirculationArcId(index))
                .ok_or(CertifiedIpmError::InvalidSourceDomain)?;
            if capacity <= 0 {
                return Err(CertifiedIpmError::UnsupportedInitialPointDomain);
            }
            arc_flows.push(ExactRatio::new(capacity, 2).map_err(map_exact)?);
        }
        let flow = FractionalCirculation {
            cost: network.fractional_cost(&arc_flows)?,
            arc_flows,
        };
        let snapshot = Self::evaluate(
            network,
            &flow,
            optimal_cost,
            maximum_abs_input,
            fixed_point_config,
        )?;
        certify_initial_potential_bound(&snapshot, network)?;
        Ok(snapshot)
    }

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
            flow: flow.clone(),
            optimal_cost,
            maximum_abs_input,
            alpha,
            objective_gap: gap_interval,
            potential,
            lengths,
            gradients,
            arithmetic_metrics: arithmetic.metrics(),
            update_metrics: IpmUpdateMetrics::default(),
        })
    }

    #[must_use]
    pub const fn fixed_point_config(&self) -> FixedPointConfig {
        self.fixed_point_config
    }

    #[must_use]
    pub const fn flow(&self) -> &FractionalCirculation {
        &self.flow
    }

    #[must_use]
    pub const fn optimal_cost(&self) -> ExactRatio {
        self.optimal_cost
    }

    #[must_use]
    pub const fn maximum_abs_input(&self) -> i128 {
        self.maximum_abs_input
    }

    #[must_use]
    pub const fn update_metrics(&self) -> IpmUpdateMetrics {
        self.update_metrics
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

    /// Certifies the additive-half termination boundary from Lemma 4.1.
    ///
    /// The proof uses the certified inequality
    /// `Phi(f) <= 20m log(1/2)`, and also retains the enclosed objective gap
    /// as an independently auditable consequence.
    ///
    /// # Errors
    ///
    /// Returns an error unless the potential and objective gap are both
    /// certified below one half using this snapshot's fixed-point budget.
    pub fn certify_additive_half_termination(
        &self,
        network: &CirculationNetwork,
    ) -> Result<IpmTerminationCertificate, CertifiedIpmError> {
        let mut arithmetic = CertifiedFixedPoint::new(self.fixed_point_config)?;
        let half = arithmetic.enclose_ratio(1, 2)?;
        let log_half = arithmetic.logarithm(&half)?;
        let factor = i128::try_from(network.arc_count())
            .map_err(|_| CertifiedIpmError::ExactOverflow)?
            .checked_mul(20)
            .ok_or(CertifiedIpmError::ExactOverflow)?;
        let potential_bound = arithmetic.multiply_interval_integer(&log_half, factor)?;
        if self.potential.upper_scaled() > potential_bound.lower_scaled()
            || self.objective_gap.upper_scaled() > half.upper_scaled()
        {
            return Err(CertifiedIpmError::NotAtAdditiveHalfBoundary);
        }
        Ok(IpmTerminationCertificate {
            potential_bound,
            objective_gap: self.objective_gap.clone(),
        })
    }

    /// Runs the permanent exact rounding Oracle after additive-half
    /// termination and checks that the recovered integral cost equals `F*`.
    ///
    /// # Errors
    ///
    /// Returns an error when the additive-half boundary is not certified, the
    /// exact rounding Oracle rejects the flow, or its result is not optimal.
    pub fn recover_additive_half(
        &self,
        network: &CirculationNetwork,
    ) -> Result<CostedFlowRoundingResult, CertifiedIpmError> {
        self.certify_additive_half_termination(network)?;
        let result = network.round_fractional_costed(&self.flow)?;
        if ExactRatio::new(result.solution.cost, 1).map_err(map_exact)? != self.optimal_cost {
            return Err(CertifiedIpmError::RecoveryNotOptimal);
        }
        Ok(result)
    }

    /// Applies the Lemma 4.11 nearest-integer recovery contract to this
    /// snapshot of the integral-cost scaled perturbation.
    ///
    /// # Errors
    ///
    /// Returns an error unless this flow is within the source scaled tolerance
    /// of its retained optimum and exact P7 verification accepts the rounded
    /// original solution.
    pub fn recover_isolation_perturbed(
        &self,
        perturbation: &IsolationPerturbation,
    ) -> Result<IsolationRecoveryCertificate, CertifiedIpmError> {
        Ok(perturbation.recover_near_optimal(&self.flow, self.optimal_cost)?)
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

    /// Applies one certified CKLPPS22 Lemma 4.4 update.
    ///
    /// The direction is checked as a circulation, the ratio condition is
    /// checked exactly, and the successor potential is independently
    /// re-enclosed with the same bounded fixed-point configuration. No flow
    /// backend or cycle Oracle is consulted by this transition.
    ///
    /// # Errors
    ///
    /// Returns an error when the direction is malformed, approximations fail
    /// their source bounds, the ratio quality is not certified, the candidate
    /// leaves the strict interior, or the potential drop cannot be certified.
    #[allow(clippy::too_many_lines)]
    pub fn apply_lemma_44_update(
        &self,
        network: &CirculationNetwork,
        approximate_gradients: &[ExactRatio],
        approximate_lengths: &[ExactRatio],
        kappa: ExactRatio,
        direction: &[ExactRatio],
    ) -> Result<CertifiedIpmUpdate, CertifiedIpmError> {
        network.verify_input_domain(self.maximum_abs_input)?;
        network.verify_fractional_circulation(direction)?;
        if direction.len() != self.flow.arc_flows.len() {
            return Err(CertifiedIpmError::DimensionMismatch);
        }
        let mut arithmetic = CertifiedFixedPoint::new(self.fixed_point_config)?;
        let approximation = self.certify_approximations(
            approximate_gradients,
            approximate_lengths,
            kappa,
            &mut arithmetic,
        )?;
        let zero = exact_ratio(0)?;
        let mut dot = zero;
        let mut norm = zero;
        for ((gradient, length), delta) in approximate_gradients
            .iter()
            .zip(approximate_lengths)
            .zip(direction)
        {
            dot = dot
                .checked_add(gradient.checked_mul(*delta).map_err(map_exact)?)
                .map_err(map_exact)?;
            let magnitude = delta.abs().map_err(map_exact)?;
            norm = norm
                .checked_add(length.checked_mul(magnitude).map_err(map_exact)?)
                .map_err(map_exact)?;
        }
        if !dot.is_negative() {
            return Err(CertifiedIpmError::InvalidUpdateDirection);
        }
        let kappa_norm = kappa.checked_mul(norm).map_err(map_exact)?;
        let quality = dot.checked_add(kappa_norm).map_err(map_exact)?;
        if quality != zero && quality.at_least(zero).map_err(map_exact)? {
            return Err(CertifiedIpmError::RatioQualityNotCertified);
        }
        let kappa_squared = kappa.checked_mul(kappa).map_err(map_exact)?;
        let denominator = dot
            .abs()
            .map_err(map_exact)?
            .checked_mul_integer(50)
            .map_err(map_exact)?;
        let eta = ExactRatio::new(
            kappa_squared
                .numerator()
                .checked_mul(denominator.denominator())
                .ok_or(CertifiedIpmError::ExactOverflow)?,
            kappa_squared
                .denominator()
                .checked_mul(denominator.numerator())
                .ok_or(CertifiedIpmError::ExactOverflow)?,
        )
        .map_err(map_exact)?;
        if !eta.is_positive() {
            return Err(CertifiedIpmError::InvalidUpdateDirection);
        }
        let arc_flows = self
            .flow
            .arc_flows
            .iter()
            .zip(direction)
            .map(|(flow, delta)| {
                flow.checked_add(eta.checked_mul(*delta).map_err(map_exact)?)
                    .map_err(map_exact)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let candidate = FractionalCirculation {
            cost: network.fractional_cost(&arc_flows)?,
            arc_flows,
        };
        network.verify_fractional_solution(&candidate)?;
        let mut next = Self::evaluate(
            network,
            &candidate,
            self.optimal_cost,
            self.maximum_abs_input,
            self.fixed_point_config,
        )?;
        let mut decrease_arithmetic = CertifiedFixedPoint::new(self.fixed_point_config)?;
        let decrease = decrease_arithmetic.subtract_intervals(&self.potential, &next.potential)?;
        let required = decrease_arithmetic.enclose_ratio(
            kappa_squared.numerator(),
            kappa_squared
                .denominator()
                .checked_mul(500)
                .ok_or(CertifiedIpmError::ExactOverflow)?,
        )?;
        if decrease.lower_scaled() < required.upper_scaled() {
            return Err(CertifiedIpmError::PotentialDecreaseNotCertified);
        }
        let changed = u64::try_from(direction.iter().filter(|delta| !delta.is_zero()).count())
            .map_err(|_| CertifiedIpmError::ExactOverflow)?;
        next.update_metrics = IpmUpdateMetrics {
            iterations: self
                .update_metrics
                .iterations
                .checked_add(1)
                .ok_or(CertifiedIpmError::ExactOverflow)?,
            changed_coordinates: self
                .update_metrics
                .changed_coordinates
                .checked_add(changed)
                .ok_or(CertifiedIpmError::ExactOverflow)?,
            ..self.update_metrics
        };
        Ok(CertifiedIpmUpdate {
            next_snapshot: next,
            eta,
            approximation,
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
    #[error("the approximate ratio direction has no strictly negative dot product")]
    InvalidUpdateDirection,
    #[error("the approximate ratio direction does not certify the required quality")]
    RatioQualityNotCertified,
    #[error("the successor potential drop is below kappa^2/500")]
    PotentialDecreaseNotCertified,
    #[error("the Detect threshold must be strictly positive")]
    InvalidDetectThreshold,
    #[error("the normalized zero-demand initial-point domain is unsupported")]
    UnsupportedInitialPointDomain,
    #[error("the initial potential is not certified below 200m log(mU)")]
    InitialPotentialNotCertified,
    #[error("the additive-half termination potential or gap boundary is not certified")]
    NotAtAdditiveHalfBoundary,
    #[error("exact recovery did not return the supplied optimal cost")]
    RecoveryNotOptimal,
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

fn map_exact(_: crate::StableMinRatioError) -> CertifiedIpmError {
    CertifiedIpmError::ExactOverflow
}

fn certify_initial_potential_bound(
    snapshot: &CertifiedIpmSnapshot,
    network: &CirculationNetwork,
) -> Result<(), CertifiedIpmError> {
    let mut arithmetic = CertifiedFixedPoint::new(snapshot.fixed_point_config)?;
    let edge_count =
        i128::try_from(network.arc_count()).map_err(|_| CertifiedIpmError::ExactOverflow)?;
    let m_u = edge_count
        .checked_mul(snapshot.maximum_abs_input)
        .ok_or(CertifiedIpmError::ExactOverflow)?;
    let m_u_interval = arithmetic.enclose_ratio(m_u, 1)?;
    let log_m_u = arithmetic.logarithm(&m_u_interval)?;
    let factor = edge_count
        .checked_mul(200)
        .ok_or(CertifiedIpmError::ExactOverflow)?;
    let bound = arithmetic.multiply_interval_integer(&log_m_u, factor)?;
    if snapshot.potential.upper_scaled() > bound.lower_scaled() {
        return Err(CertifiedIpmError::InitialPotentialNotCertified);
    }
    Ok(())
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
        CertifiedIpmError, CertifiedIpmSnapshot, InteriorPointError, IpmDetectLedger,
        RationalInteriorPointState,
    };
    use crate::{
        CertifiedFixedPoint, CirculationNetwork, ExactRatio, FixedPointConfig, FlowNodeId,
        FractionalCirculation, LowerBoundCirculationNetwork,
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

    #[test]
    fn applies_certified_lemma_44_update_without_flow_oracle() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
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
        let update = snapshot
            .apply_lemma_44_update(
                &network,
                &[
                    ExactRatio::new(40, 1).unwrap(),
                    ExactRatio::new(0, 1).unwrap(),
                ],
                &[
                    ExactRatio::new(2, 1).unwrap(),
                    ExactRatio::new(2, 1).unwrap(),
                ],
                ExactRatio::new(1, 2).unwrap(),
                &[
                    ExactRatio::new(-1, 1).unwrap(),
                    ExactRatio::new(-1, 1).unwrap(),
                ],
            )
            .unwrap();
        assert_eq!(update.eta, ExactRatio::new(1, 8_000).unwrap());
        assert_eq!(update.next_snapshot.update_metrics().iterations, 1);
        assert_eq!(update.next_snapshot.update_metrics().changed_coordinates, 2);
        assert!(
            update
                .next_snapshot
                .objective_gap()
                .contains_ratio(7_999, 8_000)
                .unwrap()
        );
        network
            .verify_fractional_solution(update.next_snapshot.flow())
            .unwrap();
    }

    #[test]
    fn detect_ledger_requires_certified_lower_bound_and_resets_edges() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
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
        let mut ledger = IpmDetectLedger::new(&snapshot).unwrap();
        ledger
            .record_update(
                &snapshot,
                ExactRatio::new(1, 8_000).unwrap(),
                &[
                    ExactRatio::new(-1, 1).unwrap(),
                    ExactRatio::new(-1, 1).unwrap(),
                ],
            )
            .unwrap();
        assert!(
            ledger
                .detect(ExactRatio::new(1, 1_000).unwrap())
                .unwrap()
                .is_empty()
        );
        let detected = ledger.detect(ExactRatio::new(1, 10_000).unwrap()).unwrap();
        assert_eq!(detected.len(), 2);
        assert!(
            ledger
                .accumulated_changes()
                .iter()
                .all(|value| value.contains_ratio(0, 1).unwrap())
        );
        assert_eq!(ledger.metrics().detect_calls, 2);
        assert_eq!(ledger.metrics().detected_edges, 2);
    }

    #[test]
    fn certifies_restricted_initial_point_and_additive_half_recovery() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let arithmetic = certified_arithmetic();
        let initial = CertifiedIpmSnapshot::initial_point_zero_demand(
            &network,
            ExactRatio::new(0, 1).unwrap(),
            4,
            arithmetic.config(),
        )
        .unwrap();
        assert_eq!(
            initial.flow().arc_flows,
            vec![ExactRatio::new(1, 1).unwrap(); 2]
        );

        let quarter = ExactRatio::new(1, 4).unwrap();
        let near = FractionalCirculation {
            arc_flows: vec![quarter; 2],
            cost: quarter,
        };
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &near,
            ExactRatio::new(0, 1).unwrap(),
            4,
            arithmetic.config(),
        )
        .unwrap();
        let certificate = snapshot
            .certify_additive_half_termination(&network)
            .unwrap();
        assert!(certificate.objective_gap.contains_ratio(1, 4).unwrap());
        let recovered = snapshot.recover_additive_half(&network).unwrap();
        assert_eq!(recovered.solution.cost, 0);
        assert_eq!(recovered.solution.arc_flows, vec![0, 0]);
    }

    #[test]
    fn certifies_appendix_b_initial_point_for_nonzero_demands() {
        let mut network = CirculationNetwork::new(2);
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(1), 1).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network
            .add_arc(FlowNodeId(1), FlowNodeId(0), 2, -1)
            .unwrap();
        let arithmetic = certified_arithmetic();
        let initial = CertifiedIpmSnapshot::initial_point_augmented(
            &network,
            ExactRatio::new(1, 1).unwrap(),
            2,
            arithmetic.config(),
        )
        .unwrap();
        assert_eq!(initial.augmentation.artificial_arc_ids.len(), 2);
        assert_eq!(
            initial.snapshot.optimal_cost(),
            ExactRatio::new(1, 1).unwrap()
        );
        initial
            .augmentation
            .network
            .verify_fractional_solution(initial.snapshot.flow())
            .unwrap();
    }

    #[test]
    fn normalizes_lower_bounds_before_source_initialization() {
        let mut network = LowerBoundCirculationNetwork::new(2);
        network.set_demand(FlowNodeId(0), -2).unwrap();
        network.set_demand(FlowNodeId(1), 2).unwrap();
        network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 1, 3, 2)
            .unwrap();
        network
            .add_arc(FlowNodeId(1), FlowNodeId(0), -1, 2, 1)
            .unwrap();
        network
            .add_arc(FlowNodeId(0), FlowNodeId(0), 2, 2, 3)
            .unwrap();
        let arithmetic = certified_arithmetic();
        let initial = CertifiedIpmSnapshot::initial_point_lower_bounded(
            &network,
            ExactRatio::new(7, 1).unwrap(),
            3,
            arithmetic.config(),
        )
        .unwrap();
        assert_eq!(initial.normalization.objective_offset, 7);
        assert_eq!(
            initial.initial_point.snapshot.optimal_cost(),
            ExactRatio::new(0, 1).unwrap()
        );
        let normalized = initial.normalization.normalized.solve().unwrap();
        let recovered = initial.normalization.recover_original(&normalized).unwrap();
        assert_eq!(recovered.cost, 7);
        assert_eq!(recovered.arc_flows, vec![1, -1, 2]);
    }

    #[test]
    fn recovers_isolation_perturbed_snapshot_through_exact_oracle() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        let perturbation = network.isolation_perturbation(1, vec![1, 2]).unwrap();
        let hundredth = ExactRatio::new(1, 100).unwrap();
        let near = FractionalCirculation {
            arc_flows: vec![hundredth; 2],
            cost: ExactRatio::new(3, 100).unwrap(),
        };
        let arithmetic = certified_arithmetic();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &perturbation.scaled_network,
            &near,
            ExactRatio::new(0, 1).unwrap(),
            perturbation.maximum_abs_input,
            arithmetic.config(),
        )
        .unwrap();
        let recovered = snapshot.recover_isolation_perturbed(&perturbation).unwrap();
        assert_eq!(recovered.solution.arc_flows, vec![0, 0]);
        assert!(recovered.exact_oracle_verified);
    }
}
