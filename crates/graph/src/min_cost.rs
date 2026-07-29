use thiserror::Error;

use crate::{ExactRatio, FlowNodeId, StableMinRatioError};

pub mod experiment;
pub mod oracle;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CirculationArcId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
struct Arc {
    from: usize,
    to: usize,
    capacity: i128,
    cost: i128,
}

/// One integral min-cost-flow arc with explicit lower and upper bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LowerBoundArc {
    pub from: FlowNodeId,
    pub to: FlowNodeId,
    pub lower: i128,
    pub upper: i128,
    pub cost: i128,
}

/// Integral min-cost circulation before lower-bound normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LowerBoundCirculationNetwork {
    node_count: usize,
    demands: Vec<i128>,
    arcs: Vec<LowerBoundArc>,
}

/// Exact `x_e = f_e - u^-_e` normalization and recovery mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LowerBoundNormalization {
    original: LowerBoundCirculationNetwork,
    pub normalized: CirculationNetwork,
    normalized_arc_for_original: Vec<Option<CirculationArcId>>,
    pub objective_offset: i128,
}

/// One realized Lemma 4.11 isolation perturbation with integral scaled costs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationPerturbation {
    original: CirculationNetwork,
    pub scaled_network: CirculationNetwork,
    pub ranks: Vec<i128>,
    pub rank_support_upper: i128,
    pub scaled_cost_denominator: i128,
    pub scaled_near_optimal_tolerance: ExactRatio,
    pub maximum_abs_input: i128,
}

/// Exact recovery evidence for one realized isolation perturbation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IsolationRecoveryCertificate {
    pub solution: MinCostSolution,
    pub rank_support_upper: i128,
    pub scaled_cost_denominator: i128,
    pub scaled_near_optimal_tolerance: ExactRatio,
    pub source_success_probability_numerator: u8,
    pub source_success_probability_denominator: u8,
    pub exact_oracle_verified: bool,
}

impl CirculationNetwork {
    /// Realizes the Lemma 4.11 perturbation from caller-supplied independent
    /// uniform ranks in `1..=2mU`, scaling all costs by `4m^2U^2`.
    ///
    /// The theorem's probability bound applies only when the supplied ranks
    /// were sampled independently and uniformly from the documented support.
    /// This deterministic constructor validates the realization, not the
    /// external entropy source.
    ///
    /// # Errors
    ///
    /// Returns an error for a source-domain violation, invalid rank vector, or
    /// checked scaling overflow.
    pub fn isolation_perturbation(
        &self,
        maximum_abs_input: i128,
        ranks: Vec<i128>,
    ) -> Result<IsolationPerturbation, MinCostCirculationError> {
        self.verify_input_domain(maximum_abs_input)?;
        let m = i128::try_from(self.arc_count()).map_err(|_| MinCostCirculationError::Overflow)?;
        if m == 0 || ranks.len() != self.arc_count() {
            return Err(MinCostCirculationError::InvalidPerturbation);
        }
        let support_upper = m
            .checked_mul(maximum_abs_input)
            .and_then(|value| value.checked_mul(2))
            .ok_or(MinCostCirculationError::Overflow)?;
        if ranks.iter().any(|rank| *rank < 1 || *rank > support_upper) {
            return Err(MinCostCirculationError::InvalidPerturbation);
        }
        let u_squared = maximum_abs_input
            .checked_mul(maximum_abs_input)
            .ok_or(MinCostCirculationError::Overflow)?;
        let scaled_cost_denominator = m
            .checked_mul(m)
            .and_then(|value| value.checked_mul(u_squared))
            .and_then(|value| value.checked_mul(4))
            .ok_or(MinCostCirculationError::Overflow)?;
        let mut scaled_network = Self::new(self.node_count);
        for (node, demand) in self.demands.iter().copied().enumerate() {
            scaled_network.set_demand(FlowNodeId(node), demand)?;
        }
        let mut maximum_scaled = maximum_abs_input;
        for (arc, rank) in self.arcs.iter().zip(&ranks) {
            let scaled_cost = arc
                .cost
                .checked_mul(scaled_cost_denominator)
                .and_then(|value| value.checked_add(*rank))
                .ok_or(MinCostCirculationError::Overflow)?;
            maximum_scaled = maximum_scaled.max(
                scaled_cost
                    .checked_abs()
                    .ok_or(MinCostCirculationError::Overflow)?,
            );
            scaled_network.add_arc(
                FlowNodeId(arc.from),
                FlowNodeId(arc.to),
                arc.capacity,
                scaled_cost,
            )?;
        }
        let tolerance_denominator = m
            .checked_mul(maximum_abs_input)
            .and_then(|value| value.checked_mul(3))
            .ok_or(MinCostCirculationError::Overflow)?;
        Ok(IsolationPerturbation {
            original: self.clone(),
            scaled_network,
            ranks,
            rank_support_upper: support_upper,
            scaled_cost_denominator,
            scaled_near_optimal_tolerance: ExactRatio::new(1, tolerance_denominator)
                .map_err(map_ratio_error)?,
            maximum_abs_input: maximum_scaled,
        })
    }
}

impl IsolationPerturbation {
    /// Rounds a scaled perturbed flow coordinatewise and verifies the recovered
    /// original solution with the permanent exact P7 Oracle.
    ///
    /// # Errors
    ///
    /// Returns an error unless the flow is feasible, its exact scaled objective
    /// is within `1/(3mU)` of the supplied scaled optimum, every coordinate can
    /// be rounded, and the recovered original flow is exactly optimal.
    pub fn recover_near_optimal(
        &self,
        flow: &FractionalCirculation,
        scaled_optimal_cost: ExactRatio,
    ) -> Result<IsolationRecoveryCertificate, MinCostCirculationError> {
        self.scaled_network.verify_fractional_solution(flow)?;
        if !scaled_optimal_cost.is_integral() {
            return Err(MinCostCirculationError::InvalidPerturbation);
        }
        let zero = ExactRatio::new(0, 1).map_err(map_ratio_error)?;
        let gap = flow
            .cost
            .checked_sub(scaled_optimal_cost)
            .map_err(map_ratio_error)?;
        if !gap.at_least(zero).map_err(map_ratio_error)?
            || (!self
                .scaled_near_optimal_tolerance
                .at_least(gap)
                .map_err(map_ratio_error)?)
        {
            return Err(MinCostCirculationError::InvalidPerturbation);
        }
        let arc_flows = flow
            .arc_flows
            .iter()
            .copied()
            .map(round_ratio_nearest)
            .collect::<Result<Vec<_>, _>>()?;
        let solution = MinCostSolution {
            cost: solution_cost(&self.original, &arc_flows)?,
            arc_flows,
        };
        self.original.verify_solution(&solution)?;
        Ok(IsolationRecoveryCertificate {
            solution,
            rank_support_upper: self.rank_support_upper,
            scaled_cost_denominator: self.scaled_cost_denominator,
            scaled_near_optimal_tolerance: self.scaled_near_optimal_tolerance,
            source_success_probability_numerator: 1,
            source_success_probability_denominator: 2,
            exact_oracle_verified: true,
        })
    }
}

impl LowerBoundCirculationNetwork {
    #[must_use]
    pub fn new(node_count: usize) -> Self {
        Self {
            node_count,
            demands: vec![0; node_count],
            arcs: Vec::new(),
        }
    }

    /// # Errors
    ///
    /// Returns an error when `node` is outside the network.
    pub fn set_demand(
        &mut self,
        node: FlowNodeId,
        demand: i128,
    ) -> Result<(), MinCostCirculationError> {
        let slot =
            self.demands
                .get_mut(node.0)
                .ok_or(MinCostCirculationError::NodeOutOfBounds {
                    node: node.0,
                    node_count: self.node_count,
                })?;
        *slot = demand;
        Ok(())
    }

    /// Adds an arc with explicit integral lower and upper bounds.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid endpoint or `lower > upper`.
    pub fn add_arc(
        &mut self,
        from: FlowNodeId,
        to: FlowNodeId,
        lower: i128,
        upper: i128,
        cost: i128,
    ) -> Result<usize, MinCostCirculationError> {
        if from.0 >= self.node_count || to.0 >= self.node_count {
            return Err(MinCostCirculationError::NodeOutOfBounds {
                node: from.0.max(to.0),
                node_count: self.node_count,
            });
        }
        if lower > upper {
            return Err(MinCostCirculationError::InvalidLowerBound);
        }
        let id = self.arcs.len();
        self.arcs.push(LowerBoundArc {
            from,
            to,
            lower,
            upper,
            cost,
        });
        Ok(id)
    }

    #[must_use]
    pub fn arcs(&self) -> &[LowerBoundArc] {
        &self.arcs
    }

    /// Normalizes every arc by `x_e = f_e - lower_e`, shifting demands and
    /// the objective exactly. Fixed-flow arcs are eliminated.
    ///
    /// # Errors
    ///
    /// Returns an error when an input exceeds `maximum_abs_input`, demands are
    /// unbalanced, or checked capacity/demand/objective arithmetic overflows.
    pub fn normalize_lower_bounds(
        &self,
        maximum_abs_input: i128,
    ) -> Result<LowerBoundNormalization, MinCostCirculationError> {
        if maximum_abs_input <= 0 {
            return Err(MinCostCirculationError::InvalidLowerBound);
        }
        for value in self.demands.iter().copied().chain(
            self.arcs
                .iter()
                .flat_map(|arc| [arc.lower, arc.upper, arc.cost]),
        ) {
            if value
                .checked_abs()
                .ok_or(MinCostCirculationError::Overflow)?
                > maximum_abs_input
            {
                return Err(MinCostCirculationError::InvalidLowerBound);
            }
        }
        let mut shifted_demands = self.demands.clone();
        let mut objective_offset = 0_i128;
        for arc in &self.arcs {
            shifted_demands[arc.from.0] = shifted_demands[arc.from.0]
                .checked_add(arc.lower)
                .ok_or(MinCostCirculationError::Overflow)?;
            shifted_demands[arc.to.0] = shifted_demands[arc.to.0]
                .checked_sub(arc.lower)
                .ok_or(MinCostCirculationError::Overflow)?;
            objective_offset = objective_offset
                .checked_add(
                    arc.cost
                        .checked_mul(arc.lower)
                        .ok_or(MinCostCirculationError::Overflow)?,
                )
                .ok_or(MinCostCirculationError::Overflow)?;
        }
        if shifted_demands.iter().try_fold(0_i128, |sum, value| {
            sum.checked_add(*value)
                .ok_or(MinCostCirculationError::Overflow)
        })? != 0
        {
            return Err(MinCostCirculationError::UnbalancedDemand);
        }
        let mut normalized = CirculationNetwork::new(self.node_count);
        for (node, demand) in shifted_demands.into_iter().enumerate() {
            normalized.set_demand(FlowNodeId(node), demand)?;
        }
        let mut normalized_arc_for_original = Vec::with_capacity(self.arcs.len());
        for arc in &self.arcs {
            let capacity = arc
                .upper
                .checked_sub(arc.lower)
                .ok_or(MinCostCirculationError::Overflow)?;
            let normalized_id = if capacity == 0 {
                None
            } else {
                Some(normalized.add_arc(arc.from, arc.to, capacity, arc.cost)?)
            };
            normalized_arc_for_original.push(normalized_id);
        }
        Ok(LowerBoundNormalization {
            original: self.clone(),
            normalized,
            normalized_arc_for_original,
            objective_offset,
        })
    }

    /// Verifies an integral lower-bounded solution and objective exactly.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid dimensions, bounds, conservation, cost, or
    /// checked arithmetic overflow.
    pub fn verify_solution(
        &self,
        solution: &MinCostSolution,
    ) -> Result<(), MinCostCirculationError> {
        if solution.arc_flows.len() != self.arcs.len() {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        let mut balance = vec![0_i128; self.node_count];
        let mut cost = 0_i128;
        for (arc, flow) in self.arcs.iter().zip(&solution.arc_flows) {
            if *flow < arc.lower || *flow > arc.upper {
                return Err(MinCostCirculationError::InvalidSolution);
            }
            balance[arc.from.0] = balance[arc.from.0]
                .checked_sub(*flow)
                .ok_or(MinCostCirculationError::Overflow)?;
            balance[arc.to.0] = balance[arc.to.0]
                .checked_add(*flow)
                .ok_or(MinCostCirculationError::Overflow)?;
            cost = cost
                .checked_add(
                    arc.cost
                        .checked_mul(*flow)
                        .ok_or(MinCostCirculationError::Overflow)?,
                )
                .ok_or(MinCostCirculationError::Overflow)?;
        }
        if balance != self.demands || cost != solution.cost {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        Ok(())
    }
}

impl LowerBoundNormalization {
    #[must_use]
    pub const fn original(&self) -> &LowerBoundCirculationNetwork {
        &self.original
    }

    /// Recovers and verifies an original lower-bounded solution.
    ///
    /// # Errors
    ///
    /// Returns an error when the normalized solution is invalid or any exact
    /// flow/objective addition overflows.
    pub fn recover_original(
        &self,
        normalized_solution: &MinCostSolution,
    ) -> Result<MinCostSolution, MinCostCirculationError> {
        self.normalized.verify_solution(normalized_solution)?;
        let mut arc_flows = Vec::with_capacity(self.original.arcs.len());
        for (arc, normalized_id) in self
            .original
            .arcs
            .iter()
            .zip(&self.normalized_arc_for_original)
        {
            let shifted = normalized_id.map_or(0, |id| normalized_solution.arc_flows[id.0]);
            arc_flows.push(
                arc.lower
                    .checked_add(shifted)
                    .ok_or(MinCostCirculationError::Overflow)?,
            );
        }
        let solution = MinCostSolution {
            cost: normalized_solution
                .cost
                .checked_add(self.objective_offset)
                .ok_or(MinCostCirculationError::Overflow)?,
            arc_flows,
        };
        self.original.verify_solution(&solution)?;
        Ok(solution)
    }
}

/// Exact integer min-cost circulation input. Positive node demand means net
/// inflow required; negative demand means net outflow required.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CirculationNetwork {
    node_count: usize,
    demands: Vec<i128>,
    arcs: Vec<Arc>,
}

impl CirculationNetwork {
    #[must_use]
    pub const fn arc_count(&self) -> usize {
        self.arcs.len()
    }

    #[must_use]
    pub fn demands(&self) -> &[i128] {
        &self.demands
    }

    #[must_use]
    pub fn arc_capacity_cost(&self, arc: CirculationArcId) -> Option<(i128, i128)> {
        self.arcs.get(arc.0).map(|arc| (arc.capacity, arc.cost))
    }

    /// Validates that signed arc occurrences form a nonempty circulation.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid arc, direction, arithmetic overflow, or
    /// nonconserving signed edge sequence.
    pub fn validate_signed_circulation(
        &self,
        arcs: &[(CirculationArcId, i8)],
    ) -> Result<(), MinCostCirculationError> {
        if arcs.is_empty() {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        let mut balance = vec![0_i128; self.node_count];
        for (id, direction) in arcs {
            let arc = self
                .arcs
                .get(id.0)
                .ok_or(MinCostCirculationError::InvalidSolution)?;
            let (from, to) = match direction {
                1 => (arc.from, arc.to),
                -1 => (arc.to, arc.from),
                _ => return Err(MinCostCirculationError::InvalidSolution),
            };
            balance[from] = balance[from]
                .checked_sub(1)
                .ok_or(MinCostCirculationError::Overflow)?;
            balance[to] = balance[to]
                .checked_add(1)
                .ok_or(MinCostCirculationError::Overflow)?;
        }
        if balance.iter().any(|value| *value != 0) {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        Ok(())
    }
    /// Validates exact feasibility, objective value, and residual optimality.
    ///
    /// # Errors
    ///
    /// Returns an error when the flow is malformed, infeasible, has an
    /// incorrect objective, or admits a negative residual cycle.
    pub fn verify_solution(
        &self,
        solution: &MinCostSolution,
    ) -> Result<(), MinCostCirculationError> {
        if solution.arc_flows.len() != self.arcs.len() {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        let mut balance = vec![0_i128; self.node_count];
        let mut cost = 0_i128;
        for (arc, flow) in self.arcs.iter().zip(&solution.arc_flows) {
            if *flow < 0 || *flow > arc.capacity {
                return Err(MinCostCirculationError::InvalidSolution);
            }
            balance[arc.from] = balance[arc.from]
                .checked_sub(*flow)
                .ok_or(MinCostCirculationError::Overflow)?;
            balance[arc.to] = balance[arc.to]
                .checked_add(*flow)
                .ok_or(MinCostCirculationError::Overflow)?;
            cost = cost
                .checked_add(
                    arc.cost
                        .checked_mul(*flow)
                        .ok_or(MinCostCirculationError::Overflow)?,
                )
                .ok_or(MinCostCirculationError::Overflow)?;
        }
        if balance != self.demands || cost != solution.cost {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        let gradients = self.arcs.iter().map(|arc| arc.cost).collect::<Vec<_>>();
        let lengths = vec![1; self.arcs.len()];
        if oracle::minimum_residual_cycle(self, solution, &gradients, &lengths)?
            .is_some_and(|cycle| cycle.gradient_sum < 0)
        {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        Ok(())
    }

    #[must_use]
    pub fn new(node_count: usize) -> Self {
        Self {
            node_count,
            demands: vec![0; node_count],
            arcs: Vec::new(),
        }
    }

    /// Constructs the O(m+n)-edge initial-point augmentation from CKLPPS22
    /// Appendix B.1 for the normalized zero-lower-bound model.
    ///
    /// Original arcs start at their capacity midpoint. One root vertex and at
    /// most one artificial root arc per original vertex correct the resulting
    /// imbalance. Artificial arcs have capacity twice their initial flow and
    /// cost `4mU^2`, so they cannot occur in an optimum whenever the original
    /// instance is feasible.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid bound, zero-capacity arc, unbalanced
    /// demands, nonintegral artificial capacity, or checked arithmetic overflow.
    pub fn initial_point_augmentation(
        &self,
        maximum_abs_input: i128,
    ) -> Result<InitialPointAugmentation, MinCostCirculationError> {
        self.verify_input_domain(maximum_abs_input)?;
        if self.arcs.iter().any(|arc| arc.capacity <= 0) {
            return Err(MinCostCirculationError::InvalidFractionalSolution);
        }
        if self.demands.iter().try_fold(0_i128, |sum, value| {
            sum.checked_add(*value)
                .ok_or(MinCostCirculationError::Overflow)
        })? != 0
        {
            return Err(MinCostCirculationError::UnbalancedDemand);
        }
        let m = i128::try_from(self.arcs.len()).map_err(|_| MinCostCirculationError::Overflow)?;
        let u_squared = maximum_abs_input
            .checked_mul(maximum_abs_input)
            .ok_or(MinCostCirculationError::Overflow)?;
        let artificial_cost = m
            .checked_mul(u_squared)
            .and_then(|value| value.checked_mul(4))
            .ok_or(MinCostCirculationError::Overflow)?;
        let root = self.node_count;
        let mut augmented = Self::new(
            root.checked_add(1)
                .ok_or(MinCostCirculationError::Overflow)?,
        );
        for (node, demand) in self.demands.iter().copied().enumerate() {
            augmented.set_demand(FlowNodeId(node), demand)?;
        }
        let mut initial_flows = Vec::with_capacity(self.arcs.len() + self.node_count);
        let mut balance = vec![ExactRatio::new(0, 1).map_err(map_ratio_error)?; self.node_count];
        for arc in &self.arcs {
            augmented.add_arc(
                FlowNodeId(arc.from),
                FlowNodeId(arc.to),
                arc.capacity,
                arc.cost,
            )?;
            let flow = ExactRatio::new(arc.capacity, 2).map_err(map_ratio_error)?;
            balance[arc.from] = balance[arc.from]
                .checked_sub(flow)
                .map_err(map_ratio_error)?;
            balance[arc.to] = balance[arc.to].checked_add(flow).map_err(map_ratio_error)?;
            initial_flows.push(flow);
        }
        augmented.set_demand(FlowNodeId(root), 0)?;
        let mut artificial_arc_ids = Vec::new();
        for (node, current) in balance.into_iter().enumerate() {
            let target = ExactRatio::new(self.demands[node], 1).map_err(map_ratio_error)?;
            let correction = target.checked_sub(current).map_err(map_ratio_error)?;
            if correction.is_zero() {
                continue;
            }
            let magnitude = correction.abs().map_err(map_ratio_error)?;
            let capacity_ratio = magnitude.checked_mul_integer(2).map_err(map_ratio_error)?;
            if !capacity_ratio.is_integral() {
                return Err(MinCostCirculationError::InvalidFractionalSolution);
            }
            let capacity = capacity_ratio.numerator() / capacity_ratio.denominator();
            let (from, to) = if correction.is_positive() {
                (FlowNodeId(root), FlowNodeId(node))
            } else {
                (FlowNodeId(node), FlowNodeId(root))
            };
            let id = augmented.add_arc(from, to, capacity, artificial_cost)?;
            artificial_arc_ids.push(id);
            initial_flows.push(magnitude);
        }
        let cost = augmented.fractional_cost(&initial_flows)?;
        let initial_flow = FractionalCirculation {
            arc_flows: initial_flows,
            cost,
        };
        augmented.verify_fractional_solution(&initial_flow)?;
        let maximum_abs_augmented = maximum_abs_input.max(artificial_cost).max(
            initial_flow
                .arc_flows
                .iter()
                .map(|flow| flow.numerator().unsigned_abs())
                .max()
                .and_then(|value| i128::try_from(value).ok())
                .unwrap_or(0),
        );
        Ok(InitialPointAugmentation {
            original_network: self.clone(),
            network: augmented,
            initial_flow,
            artificial_arc_ids,
            maximum_abs_input: maximum_abs_augmented,
        })
    }

    /// # Errors
    ///
    /// Returns an error when `node` is outside the network.
    pub fn set_demand(
        &mut self,
        node: FlowNodeId,
        demand: i128,
    ) -> Result<(), MinCostCirculationError> {
        let slot =
            self.demands
                .get_mut(node.0)
                .ok_or(MinCostCirculationError::NodeOutOfBounds {
                    node: node.0,
                    node_count: self.node_count,
                })?;
        *slot = demand;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error when `node` is outside the network.
    pub fn add_arc(
        &mut self,
        from: FlowNodeId,
        to: FlowNodeId,
        capacity: i128,
        cost: i128,
    ) -> Result<CirculationArcId, MinCostCirculationError> {
        if from.0 >= self.node_count || to.0 >= self.node_count {
            return Err(MinCostCirculationError::NodeOutOfBounds {
                node: from.0.max(to.0),
                node_count: self.node_count,
            });
        }
        if capacity < 0 {
            return Err(MinCostCirculationError::NegativeCapacity);
        }
        let id = CirculationArcId(self.arcs.len());
        self.arcs.push(Arc {
            from: from.0,
            to: to.0,
            capacity,
            cost,
        });
        Ok(id)
    }

    /// Validates a rational feasible circulation and its exact objective.
    ///
    /// # Errors
    ///
    /// Returns an error when the vector has the wrong dimension, violates an
    /// integral capacity or demand, has a wrong cost, or arithmetic overflows.
    pub fn verify_fractional_solution(
        &self,
        solution: &FractionalCirculation,
    ) -> Result<(), MinCostCirculationError> {
        validate_fractional_solution(self, solution)
    }

    /// Returns the exact rational objective of a flow coordinate vector.
    ///
    /// # Errors
    ///
    /// Returns an error when dimensions differ or arithmetic overflows.
    pub fn fractional_cost(
        &self,
        arc_flows: &[ExactRatio],
    ) -> Result<ExactRatio, MinCostCirculationError> {
        fractional_cost(self, arc_flows)
    }

    /// Validates that rational edge coordinates form a circulation.
    ///
    /// This checks only zero net update demand; capacity constraints belong to
    /// a full [`FractionalCirculation`] validation.
    ///
    /// # Errors
    ///
    /// Returns an error for a malformed vector, nonzero divergence, or exact
    /// arithmetic overflow.
    pub fn verify_fractional_circulation(
        &self,
        arc_flows: &[ExactRatio],
    ) -> Result<(), MinCostCirculationError> {
        if arc_flows.len() != self.arcs.len() {
            return Err(MinCostCirculationError::InvalidFractionalSolution);
        }
        let zero = ratio_from_integer(0)?;
        let mut balance = vec![zero; self.node_count];
        for (arc, flow) in self.arcs.iter().zip(arc_flows) {
            balance[arc.from] = balance[arc.from]
                .checked_sub(*flow)
                .map_err(map_ratio_error)?;
            balance[arc.to] = balance[arc.to]
                .checked_add(*flow)
                .map_err(map_ratio_error)?;
        }
        if balance.iter().any(|value| *value != zero) {
            return Err(MinCostCirculationError::InvalidFractionalSolution);
        }
        Ok(())
    }

    /// Checks that every integral input coordinate lies in the supplied
    /// positive bounded domain.
    ///
    /// # Errors
    ///
    /// Returns an error when the bound is not positive, a coordinate exceeds
    /// it, or absolute-value arithmetic overflows.
    pub fn verify_input_domain(&self, maximum_abs: i128) -> Result<(), MinCostCirculationError> {
        if maximum_abs <= 0 {
            return Err(MinCostCirculationError::InvalidFractionalSolution);
        }
        for value in self
            .demands
            .iter()
            .copied()
            .chain(self.arcs.iter().flat_map(|arc| [arc.capacity, arc.cost]))
        {
            if value
                .checked_abs()
                .ok_or(MinCostCirculationError::Overflow)?
                > maximum_abs
            {
                return Err(MinCostCirculationError::InvalidFractionalSolution);
            }
        }
        Ok(())
    }

    /// Returns exact lower and upper slack for every rational coordinate.
    ///
    /// # Errors
    ///
    /// Returns an error for dimension mismatch, an out-of-capacity flow, or
    /// exact arithmetic overflow.
    pub fn fractional_slacks(
        &self,
        arc_flows: &[ExactRatio],
    ) -> Result<Vec<(ExactRatio, ExactRatio)>, MinCostCirculationError> {
        if arc_flows.len() != self.arcs.len() {
            return Err(MinCostCirculationError::InvalidFractionalSolution);
        }
        let zero = ratio_from_integer(0)?;
        self.arcs
            .iter()
            .zip(arc_flows)
            .map(|(arc, flow)| {
                let capacity = ratio_from_integer(arc.capacity)?;
                if !flow.at_least(zero).map_err(map_ratio_error)?
                    || !capacity.at_least(*flow).map_err(map_ratio_error)?
                {
                    return Err(MinCostCirculationError::InvalidFractionalSolution);
                }
                Ok((*flow, capacity.checked_sub(*flow).map_err(map_ratio_error)?))
            })
            .collect()
    }

    /// Deterministically rounds a rational feasible circulation to an
    /// integral feasible circulation of no greater cost.
    ///
    /// The implementation is the exact cycle-cancelling reduction for costed
    /// flow rounding: every nonintegral edge belongs to an undirected
    /// fractional cycle; pushing to the nearest integral edge in the cheaper
    /// direction preserves feasibility and never increases cost. A simple
    /// breadth-first search is used as an Oracle, so this method makes no
    /// near-linear running-time claim.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid fractional solution, invalid cycle
    /// witness, or exact arithmetic overflow.
    pub fn round_fractional_costed(
        &self,
        initial: &FractionalCirculation,
    ) -> Result<CostedFlowRoundingResult, MinCostCirculationError> {
        validate_fractional_solution(self, initial)?;
        let mut current = initial.clone();
        let mut steps = Vec::new();
        while current.arc_flows.iter().any(|flow| !flow.is_integral()) {
            let cycle = fractional_cycle(self, &current.arc_flows)?
                .ok_or(MinCostCirculationError::InvalidFractionalSolution)?;
            let signed_cost = cycle.iter().try_fold(0_i128, |sum, (arc, direction)| {
                let cost = self
                    .arcs
                    .get(arc.0)
                    .ok_or(MinCostCirculationError::InvalidFractionalSolution)?
                    .cost;
                sum.checked_add(
                    cost.checked_mul(i128::from(*direction))
                        .ok_or(MinCostCirculationError::Overflow)?,
                )
                .ok_or(MinCostCirculationError::Overflow)
            })?;
            let direction = if signed_cost <= 0 { 1_i8 } else { -1_i8 };
            let oriented = cycle
                .iter()
                .map(|(arc, sign)| {
                    Ok((
                        *arc,
                        sign.checked_mul(direction)
                            .ok_or(MinCostCirculationError::InvalidFractionalSolution)?,
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let augmentation = oriented
                .iter()
                .try_fold(None::<ExactRatio>, |best, (arc, sign)| {
                    let flow = *current
                        .arc_flows
                        .get(arc.0)
                        .ok_or(MinCostCirculationError::InvalidFractionalSolution)?;
                    let available = fractional_availability(flow, *sign)?;
                    match best {
                        None => Ok(Some(available)),
                        Some(current_best)
                            if current_best.at_least(available).map_err(map_ratio_error)? =>
                        {
                            Ok(Some(available))
                        }
                        Some(current_best) => Ok(Some(current_best)),
                    }
                })?
                .ok_or(MinCostCirculationError::InvalidFractionalSolution)?;
            let cost_before = current.cost;
            for (arc, sign) in &oriented {
                let delta = augmentation
                    .checked_mul_integer(i128::from(*sign))
                    .map_err(map_ratio_error)?;
                let slot = current
                    .arc_flows
                    .get_mut(arc.0)
                    .ok_or(MinCostCirculationError::InvalidFractionalSolution)?;
                *slot = slot.checked_add(delta).map_err(map_ratio_error)?;
            }
            current.cost = fractional_cost(self, &current.arc_flows)?;
            if current
                .cost
                .at_least(cost_before)
                .map_err(map_ratio_error)?
                && current.cost != cost_before
            {
                return Err(MinCostCirculationError::InvalidFractionalSolution);
            }
            validate_fractional_solution(self, &current)?;
            steps.push(FlowRoundingStep {
                cycle: oriented,
                augmentation,
                cost_before,
                cost_after: current.cost,
            });
        }
        let arc_flows = current
            .arc_flows
            .iter()
            .map(|flow| flow.numerator() / flow.denominator())
            .collect::<Vec<_>>();
        let solution = MinCostSolution {
            cost: current.cost.numerator() / current.cost.denominator(),
            arc_flows,
        };
        validate_feasible_solution(self, &solution)?;
        Ok(CostedFlowRoundingResult { solution, steps })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinCostSolution {
    pub arc_flows: Vec<i128>,
    pub cost: i128,
}

/// Exact rational feasible circulation over an integral-capacity network.
///
/// This representation is deliberately separate from [`MinCostSolution`]: it
/// captures the fractional starting point required by costed flow rounding and
/// does not imply an interior-point implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FractionalCirculation {
    pub arc_flows: Vec<ExactRatio>,
    pub cost: ExactRatio,
}

/// Source-compatible initial-point augmentation and its strict fractional flow.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InitialPointAugmentation {
    original_network: CirculationNetwork,
    pub network: CirculationNetwork,
    pub initial_flow: FractionalCirculation,
    pub artificial_arc_ids: Vec<CirculationArcId>,
    pub maximum_abs_input: i128,
}

impl InitialPointAugmentation {
    #[must_use]
    pub const fn original_network(&self) -> &CirculationNetwork {
        &self.original_network
    }

    /// Recovers an original optimum from a verified augmented optimum, or
    /// concludes that the original instance is infeasible when an artificial
    /// root arc carries flow.
    ///
    /// # Errors
    ///
    /// Returns an error when the augmented witness is not optimal, an
    /// artificial arc is used, or the truncated original witness is invalid.
    pub fn recover_original(
        &self,
        augmented_solution: &MinCostSolution,
    ) -> Result<MinCostSolution, MinCostCirculationError> {
        self.network.verify_solution(augmented_solution)?;
        if self
            .artificial_arc_ids
            .iter()
            .any(|arc| augmented_solution.arc_flows[arc.0] != 0)
        {
            return Err(MinCostCirculationError::Infeasible);
        }
        let arc_count = self.original_network.arc_count();
        let arc_flows = augmented_solution.arc_flows[..arc_count].to_vec();
        let solution = MinCostSolution {
            cost: solution_cost(&self.original_network, &arc_flows)?,
            arc_flows,
        };
        self.original_network.verify_solution(&solution)?;
        Ok(solution)
    }
}

/// One deterministic fractional-cycle cancellation used by
/// [`CirculationNetwork::round_fractional_costed`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowRoundingStep {
    /// Signed original arc occurrences in traversal order.
    pub cycle: Vec<(CirculationArcId, i8)>,
    /// Exact amount pushed in the recorded signed direction.
    pub augmentation: ExactRatio,
    pub cost_before: ExactRatio,
    pub cost_after: ExactRatio,
}

/// The integral result and full exact trace of deterministic costed flow
/// rounding. It implements the cycle-cancelling reduction in Kang--Payor
/// (2015, Section 3.2); its implementation is intentionally a simple Oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CostedFlowRoundingResult {
    pub solution: MinCostSolution,
    pub steps: Vec<FlowRoundingStep>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MinCostCirculationError {
    #[error("node {node} is outside network with {node_count} nodes")]
    NodeOutOfBounds { node: usize, node_count: usize },
    #[error("capacity must be nonnegative")]
    NegativeCapacity,
    #[error("lower/upper capacity bounds or their source bound are invalid")]
    InvalidLowerBound,
    #[error("isolation perturbation ranks, tolerance, or recovery witness are invalid")]
    InvalidPerturbation,
    #[error("demands do not sum to zero")]
    UnbalancedDemand,
    #[error("no feasible circulation exists")]
    Infeasible,
    #[error("exact integer arithmetic overflowed")]
    Overflow,
    #[error("fractional circulation is malformed or violates feasibility")]
    InvalidFractionalSolution,
    #[error("gradient/length dimensions differ or a length is not positive")]
    InvalidRatioInput,
    #[error(
        "flow vector is infeasible, has an incorrect cost, or admits a negative residual cycle"
    )]
    InvalidSolution,
}

fn map_ratio_error(error: StableMinRatioError) -> MinCostCirculationError {
    if error == StableMinRatioError::Overflow {
        MinCostCirculationError::Overflow
    } else {
        MinCostCirculationError::InvalidFractionalSolution
    }
}

fn ratio_from_integer(value: i128) -> Result<ExactRatio, MinCostCirculationError> {
    ExactRatio::new(value, 1).map_err(map_ratio_error)
}

fn round_ratio_nearest(value: ExactRatio) -> Result<i128, MinCostCirculationError> {
    let denominator = value.denominator();
    let floor = value.numerator().div_euclid(denominator);
    let remainder = value.numerator().rem_euclid(denominator);
    let doubled = remainder
        .checked_mul(2)
        .ok_or(MinCostCirculationError::Overflow)?;
    if doubled < denominator {
        Ok(floor)
    } else {
        floor
            .checked_add(1)
            .ok_or(MinCostCirculationError::Overflow)
    }
}

fn fractional_cost(
    network: &CirculationNetwork,
    flow: &[ExactRatio],
) -> Result<ExactRatio, MinCostCirculationError> {
    if flow.len() != network.arcs.len() {
        return Err(MinCostCirculationError::InvalidFractionalSolution);
    }
    flow.iter()
        .zip(&network.arcs)
        .try_fold(ratio_from_integer(0)?, |sum, (value, arc)| {
            sum.checked_add(
                value
                    .checked_mul_integer(arc.cost)
                    .map_err(map_ratio_error)?,
            )
            .map_err(map_ratio_error)
        })
}

fn validate_fractional_solution(
    network: &CirculationNetwork,
    solution: &FractionalCirculation,
) -> Result<(), MinCostCirculationError> {
    if solution.arc_flows.len() != network.arcs.len() {
        return Err(MinCostCirculationError::InvalidFractionalSolution);
    }
    let zero = ratio_from_integer(0)?;
    let mut balance = vec![zero; network.node_count];
    for (arc, flow) in network.arcs.iter().zip(&solution.arc_flows) {
        let capacity = ratio_from_integer(arc.capacity)?;
        if !flow.at_least(zero).map_err(map_ratio_error)?
            || !capacity.at_least(*flow).map_err(map_ratio_error)?
        {
            return Err(MinCostCirculationError::InvalidFractionalSolution);
        }
        balance[arc.from] = balance[arc.from]
            .checked_sub(*flow)
            .map_err(map_ratio_error)?;
        balance[arc.to] = balance[arc.to]
            .checked_add(*flow)
            .map_err(map_ratio_error)?;
    }
    for (actual, expected) in balance.iter().zip(&network.demands) {
        if *actual != ratio_from_integer(*expected)? {
            return Err(MinCostCirculationError::InvalidFractionalSolution);
        }
    }
    if fractional_cost(network, &solution.arc_flows)? != solution.cost {
        return Err(MinCostCirculationError::InvalidFractionalSolution);
    }
    Ok(())
}

fn fractional_availability(
    flow: ExactRatio,
    direction: i8,
) -> Result<ExactRatio, MinCostCirculationError> {
    if flow.is_integral() {
        return Err(MinCostCirculationError::InvalidFractionalSolution);
    }
    let floor = flow.numerator() / flow.denominator();
    match direction {
        1 => ratio_from_integer(
            floor
                .checked_add(1)
                .ok_or(MinCostCirculationError::Overflow)?,
        )?
        .checked_sub(flow)
        .map_err(map_ratio_error),
        -1 => flow
            .checked_sub(ratio_from_integer(floor)?)
            .map_err(map_ratio_error),
        _ => Err(MinCostCirculationError::InvalidFractionalSolution),
    }
}

fn fractional_cycle(
    network: &CirculationNetwork,
    flow: &[ExactRatio],
) -> Result<Option<Vec<(CirculationArcId, i8)>>, MinCostCirculationError> {
    let mut adjacency = vec![Vec::<(usize, CirculationArcId, i8)>::new(); network.node_count];
    for (index, arc) in network.arcs.iter().enumerate() {
        if flow
            .get(index)
            .ok_or(MinCostCirculationError::InvalidFractionalSolution)?
            .is_integral()
        {
            continue;
        }
        let id = CirculationArcId(index);
        if arc.from == arc.to {
            return Ok(Some(vec![(id, 1)]));
        }
        let mut queue = std::collections::VecDeque::from([arc.from]);
        let mut predecessor = vec![None; network.node_count];
        predecessor[arc.from] = Some((arc.from, id, 0));
        while let Some(node) = queue.pop_front() {
            if node == arc.to {
                break;
            }
            for (next, previous_arc, direction) in &adjacency[node] {
                if predecessor[*next].is_none() {
                    predecessor[*next] = Some((node, *previous_arc, *direction));
                    queue.push_back(*next);
                }
            }
        }
        if predecessor[arc.to].is_some() {
            let mut path = Vec::new();
            let mut node = arc.to;
            while node != arc.from {
                let (previous, previous_arc, direction) =
                    predecessor[node].ok_or(MinCostCirculationError::InvalidFractionalSolution)?;
                path.push((previous_arc, direction));
                node = previous;
            }
            path.reverse();
            path.push((id, -1));
            return Ok(Some(path));
        }
        adjacency[arc.from].push((arc.to, id, 1));
        adjacency[arc.to].push((arc.from, id, -1));
    }
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
fn enumerate_cycles(
    network: &CirculationNetwork,
    gradients: &[i128],
    lengths: &[i128],
    start: usize,
    node: usize,
    seen: &mut [bool],
    path: &mut Vec<(CirculationArcId, i8)>,
    gradient: i128,
    length: i128,
    best: &mut Option<oracle::Cycle>,
) -> Result<(), MinCostCirculationError> {
    for (index, arc) in network
        .arcs
        .iter()
        .enumerate()
        .filter(|(_, arc)| arc.from == node)
    {
        let next = arc.to;
        let g = gradient
            .checked_add(gradients[index])
            .ok_or(MinCostCirculationError::Overflow)?;
        let l = length
            .checked_add(lengths[index])
            .ok_or(MinCostCirculationError::Overflow)?;
        if next == start {
            let mut arcs = path.clone();
            arcs.push((CirculationArcId(index), 1));
            let candidate = oracle::Cycle {
                arcs,
                gradient_sum: g,
                length_sum: l,
            };
            if candidate_is_lower(g, l, best.as_ref())? {
                *best = Some(candidate);
            }
        } else if !seen[next] {
            seen[next] = true;
            path.push((CirculationArcId(index), 1));
            enumerate_cycles(
                network, gradients, lengths, start, next, seen, path, g, l, best,
            )?;
            path.pop();
            seen[next] = false;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn enumerate_residual_cycles(
    network: &CirculationNetwork,
    gradients: &[i128],
    lengths: &[i128],
    residual_edges: &[Residual],
    start: usize,
    node: usize,
    seen: &mut [bool],
    path: &mut Vec<(CirculationArcId, i8)>,
    gradient: i128,
    length: i128,
    best: &mut Option<oracle::Cycle>,
) -> Result<(), MinCostCirculationError> {
    for edge in residual_edges
        .iter()
        .copied()
        .filter(|edge| residual_from(network, *edge) == node)
    {
        let next = residual_to(network, edge);
        let signed_gradient = if edge.reverse {
            gradients[edge.arc]
                .checked_neg()
                .ok_or(MinCostCirculationError::Overflow)?
        } else {
            gradients[edge.arc]
        };
        let g = gradient
            .checked_add(signed_gradient)
            .ok_or(MinCostCirculationError::Overflow)?;
        let l = length
            .checked_add(lengths[edge.arc])
            .ok_or(MinCostCirculationError::Overflow)?;
        let direction = if edge.reverse { -1 } else { 1 };
        if next == start {
            let mut arcs = path.clone();
            arcs.push((CirculationArcId(edge.arc), direction));
            let candidate = oracle::Cycle {
                arcs,
                gradient_sum: g,
                length_sum: l,
            };
            if candidate_is_lower(g, l, best.as_ref())? {
                *best = Some(candidate);
            }
        } else if !seen[next] {
            seen[next] = true;
            path.push((CirculationArcId(edge.arc), direction));
            enumerate_residual_cycles(
                network,
                gradients,
                lengths,
                residual_edges,
                start,
                next,
                seen,
                path,
                g,
                l,
                best,
            )?;
            path.pop();
            seen[next] = false;
        }
    }
    Ok(())
}

fn candidate_is_lower(
    gradient: i128,
    length: i128,
    best: Option<&oracle::Cycle>,
) -> Result<bool, MinCostCirculationError> {
    let Some(old) = best else {
        return Ok(true);
    };
    let left = gradient
        .checked_mul(old.length_sum)
        .ok_or(MinCostCirculationError::Overflow)?;
    let right = old
        .gradient_sum
        .checked_mul(length)
        .ok_or(MinCostCirculationError::Overflow)?;
    Ok(left < right)
}

#[derive(Clone, Copy)]
struct Residual {
    arc: usize,
    reverse: bool,
}
fn residual_from(network: &CirculationNetwork, edge: Residual) -> usize {
    if edge.reverse {
        network.arcs[edge.arc].to
    } else {
        network.arcs[edge.arc].from
    }
}
fn residual_to(network: &CirculationNetwork, edge: Residual) -> usize {
    if edge.reverse {
        network.arcs[edge.arc].from
    } else {
        network.arcs[edge.arc].to
    }
}
fn residual_capacity(network: &CirculationNetwork, flow: &[i128], edge: Residual) -> i128 {
    if edge.reverse {
        flow[edge.arc]
    } else {
        network.arcs[edge.arc].capacity - flow[edge.arc]
    }
}
fn solution_cost(
    network: &CirculationNetwork,
    flow: &[i128],
) -> Result<i128, MinCostCirculationError> {
    network
        .arcs
        .iter()
        .zip(flow)
        .try_fold(0_i128, |sum, (arc, value)| {
            sum.checked_add(
                arc.cost
                    .checked_mul(*value)
                    .ok_or(MinCostCirculationError::Overflow)?,
            )
            .ok_or(MinCostCirculationError::Overflow)
        })
}

fn validate_feasible_solution(
    network: &CirculationNetwork,
    solution: &MinCostSolution,
) -> Result<(), MinCostCirculationError> {
    if solution.arc_flows.len() != network.arcs.len() {
        return Err(MinCostCirculationError::InvalidSolution);
    }
    let mut balance = vec![0_i128; network.node_count];
    for (arc, flow) in network.arcs.iter().zip(&solution.arc_flows) {
        if *flow < 0 || *flow > arc.capacity {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        balance[arc.from] = balance[arc.from]
            .checked_sub(*flow)
            .ok_or(MinCostCirculationError::Overflow)?;
        balance[arc.to] = balance[arc.to]
            .checked_add(*flow)
            .ok_or(MinCostCirculationError::Overflow)?;
    }
    if balance != network.demands || solution.cost != solution_cost(network, &solution.arc_flows)? {
        return Err(MinCostCirculationError::InvalidSolution);
    }
    Ok(())
}

fn apply_residual(
    flow: &mut [i128],
    edge: Residual,
    amount: i128,
) -> Result<(), MinCostCirculationError> {
    let slot = &mut flow[edge.arc];
    if edge.reverse {
        *slot = slot
            .checked_sub(amount)
            .ok_or(MinCostCirculationError::Overflow)?;
    } else {
        *slot = slot
            .checked_add(amount)
            .ok_or(MinCostCirculationError::Overflow)?;
    }
    Ok(())
}
fn edges(network: &CirculationNetwork, flow: &[i128]) -> Vec<Residual> {
    network
        .arcs
        .iter()
        .enumerate()
        .flat_map(|(arc, _)| {
            [
                Residual {
                    arc,
                    reverse: false,
                },
                Residual { arc, reverse: true },
            ]
        })
        .filter(|edge| residual_capacity(network, flow, *edge) > 0)
        .collect()
}
fn feasible_path(
    network: &CirculationNetwork,
    flow: &[i128],
    source: usize,
) -> Vec<Option<Residual>> {
    let mut p = vec![None; network.node_count];
    let mut seen = vec![false; network.node_count];
    let mut queue = std::collections::VecDeque::from([source]);
    seen[source] = true;
    while let Some(node) = queue.pop_front() {
        for e in edges(network, flow) {
            let u = residual_from(network, e);
            let v = residual_to(network, e);
            if u == node && !seen[v] {
                seen[v] = true;
                p[v] = Some(e);
                queue.push_back(v);
            }
        }
    }
    p
}
#[cfg(test)]
mod tests {
    use super::{
        CirculationNetwork, FractionalCirculation, LowerBoundCirculationNetwork,
        MinCostCirculationError, MinCostSolution, experiment, oracle,
    };
    use crate::{ExactRatio, FlowNodeId};

    #[test]
    fn routes_demand_at_minimum_cost() {
        let mut network = CirculationNetwork::new(3);
        network.set_demand(FlowNodeId(0), -3).unwrap();
        network.set_demand(FlowNodeId(2), 3).unwrap();
        let direct = network.add_arc(FlowNodeId(0), FlowNodeId(2), 3, 5).unwrap();
        let first = network.add_arc(FlowNodeId(0), FlowNodeId(1), 3, 1).unwrap();
        let second = network.add_arc(FlowNodeId(1), FlowNodeId(2), 3, 1).unwrap();
        let solution = experiment::solve(&network).unwrap();
        assert_eq!(solution.arc_flows[direct.0], 0);
        assert_eq!(solution.arc_flows[first.0], 3);
        assert_eq!(solution.arc_flows[second.0], 3);
        assert_eq!(solution.cost, 6);
        network.verify_solution(&solution).unwrap();
    }

    #[test]
    fn cancels_negative_cost_circulation() {
        let mut network = CirculationNetwork::new(2);
        let forward = network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 2, -3)
            .unwrap();
        let backward = network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 1).unwrap();
        let solution = experiment::solve(&network).unwrap();
        assert_eq!(solution.arc_flows[forward.0], 2);
        assert_eq!(solution.arc_flows[backward.0], 2);
        assert_eq!(solution.cost, -4);
    }

    #[test]
    fn routes_feasibility_before_cancelling_a_reachable_negative_cycle() {
        let mut network = CirculationNetwork::new(3);
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(2), 1).unwrap();
        network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 2, -4)
            .unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(2), 1, 0).unwrap();
        let solution = experiment::solve(&network).unwrap();
        assert_eq!(solution.cost, -7);
    }

    #[test]
    fn rejects_infeasible_demand() {
        let mut network = CirculationNetwork::new(2);
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(1), 1).unwrap();
        assert_eq!(
            experiment::solve(&network),
            Err(MinCostCirculationError::Infeasible)
        );
    }

    #[test]
    fn selects_the_lowest_exact_simple_cycle_ratio() {
        let mut network = CirculationNetwork::new(3);
        let a = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let b = network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(2), 1, 0).unwrap();
        network.add_arc(FlowNodeId(2), FlowNodeId(0), 1, 0).unwrap();
        let result = oracle::minimum_cycle(&network, &[-2, 0, -1, 0], &[1, 1, 1, 1])
            .unwrap()
            .unwrap();
        assert_eq!(result.gradient_sum, -2);
        assert_eq!(result.length_sum, 2);
        assert_eq!(result.arcs, vec![(a, 1), (b, 1)]);
    }

    #[test]
    fn selects_and_cancels_a_negative_self_loop() {
        let mut network = CirculationNetwork::new(1);
        let loop_arc = network
            .add_arc(FlowNodeId(0), FlowNodeId(0), 2, -3)
            .unwrap();
        let ratio = oracle::minimum_cycle(&network, &[-3], &[1])
            .unwrap()
            .unwrap();
        assert_eq!(ratio.arcs, vec![(loop_arc, 1)]);
        let solution = experiment::solve(&network).unwrap();
        assert_eq!(solution.arc_flows, vec![2]);
        assert_eq!(solution.cost, -6);
        network.verify_solution(&solution).unwrap();
    }

    #[test]
    fn residual_ratio_cycle_includes_reverse_arcs() {
        let mut network = CirculationNetwork::new(2);
        let forward = network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 5).unwrap();
        let backward = network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 1).unwrap();
        let flow = MinCostSolution {
            arc_flows: vec![2, 2],
            cost: 12,
        };
        let cycle = oracle::minimum_residual_cycle(&network, &flow, &[5, 1], &[1, 1])
            .unwrap()
            .unwrap();
        assert_eq!(cycle.gradient_sum, -6);
        assert_eq!(cycle.length_sum, 2);
        assert_eq!(cycle.arcs, vec![(backward, -1), (forward, -1)]);
    }

    #[test]
    fn refinement_records_strict_objective_decreases_and_recovers_optimum() {
        let mut network = CirculationNetwork::new(2);
        let forward = network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 3, -4)
            .unwrap();
        let backward = network.add_arc(FlowNodeId(1), FlowNodeId(0), 3, 1).unwrap();
        let result = experiment::refine(
            &network,
            &MinCostSolution {
                arc_flows: vec![0, 0],
                cost: 0,
            },
        )
        .unwrap();
        assert_eq!(result.solution.arc_flows[forward.0], 3);
        assert_eq!(result.solution.arc_flows[backward.0], 3);
        assert_eq!(result.solution.cost, -9);
        assert_eq!(result.steps.len(), 1);
        assert_eq!(result.steps[0].cost_before, 0);
        assert_eq!(result.steps[0].cost_after, -9);
        network.verify_solution(&result.solution).unwrap();
    }

    #[test]
    fn rejects_incorrectly_recovered_solution() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 2).unwrap();
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(1), 1).unwrap();
        assert_eq!(
            network.verify_solution(&MinCostSolution {
                arc_flows: vec![1],
                cost: 3,
            }),
            Err(MinCostCirculationError::InvalidSolution)
        );
    }

    #[test]
    fn agrees_with_bounded_flow_enumeration_for_all_tiny_cost_assignments() {
        for first_cost in -2..=2 {
            for second_cost in -2..=2 {
                for direct_cost in -2..=2 {
                    let mut network = CirculationNetwork::new(3);
                    network.set_demand(FlowNodeId(0), -2).unwrap();
                    network.set_demand(FlowNodeId(2), 2).unwrap();
                    network
                        .add_arc(FlowNodeId(0), FlowNodeId(1), 2, first_cost)
                        .unwrap();
                    network
                        .add_arc(FlowNodeId(1), FlowNodeId(2), 2, second_cost)
                        .unwrap();
                    network
                        .add_arc(FlowNodeId(0), FlowNodeId(2), 2, direct_cost)
                        .unwrap();
                    let expected = brute_force_cost(&network).unwrap();
                    let actual = experiment::solve(&network).unwrap();
                    assert_eq!(actual.cost, expected);
                    network.verify_solution(&actual).unwrap();
                }
            }
        }
    }

    #[test]
    fn costed_rounding_cancels_a_fractional_cycle_without_increasing_cost() {
        let mut network = CirculationNetwork::new(3);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 5).unwrap();
        network
            .add_arc(FlowNodeId(1), FlowNodeId(2), 1, -2)
            .unwrap();
        network.add_arc(FlowNodeId(2), FlowNodeId(0), 1, 0).unwrap();
        let half = ExactRatio::new(1, 2).unwrap();
        let initial = FractionalCirculation {
            arc_flows: vec![half; 3],
            cost: ExactRatio::new(3, 2).unwrap(),
        };
        network.verify_fractional_solution(&initial).unwrap();
        let rounded = network.round_fractional_costed(&initial).unwrap();
        assert_eq!(rounded.solution.arc_flows, vec![0, 0, 0]);
        assert_eq!(rounded.solution.cost, 0);
        assert_eq!(rounded.steps.len(), 1);
        assert!(
            rounded.steps[0]
                .cost_before
                .at_least(rounded.steps[0].cost_after)
                .unwrap()
        );
        network.verify_solution(&rounded.solution).unwrap();
    }

    #[test]
    fn costed_rounding_rational_differential_is_feasible_and_nonincreasing() {
        for numerator in 1..=3 {
            for forward_cost in -2..=2 {
                for backward_cost in -2..=2 {
                    let mut network = CirculationNetwork::new(2);
                    network
                        .add_arc(FlowNodeId(0), FlowNodeId(1), 1, forward_cost)
                        .unwrap();
                    network
                        .add_arc(FlowNodeId(1), FlowNodeId(0), 1, backward_cost)
                        .unwrap();
                    let value = ExactRatio::new(numerator, 4).unwrap();
                    let initial = FractionalCirculation {
                        arc_flows: vec![value; 2],
                        cost: value
                            .checked_mul_integer(forward_cost + backward_cost)
                            .unwrap(),
                    };
                    let rounded = network.round_fractional_costed(&initial).unwrap();
                    network.verify_solution(&rounded.solution).unwrap();
                    let rounded_cost = ExactRatio::new(rounded.solution.cost, 1).unwrap();
                    assert!(initial.cost.at_least(rounded_cost).unwrap());
                    assert!(
                        rounded.solution.arc_flows == vec![0, 0]
                            || rounded.solution.arc_flows == vec![1, 1]
                    );
                }
            }
        }
    }

    #[test]
    fn rejects_fractional_flow_that_breaks_conservation() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let invalid = FractionalCirculation {
            arc_flows: vec![ExactRatio::new(1, 2).unwrap()],
            cost: ExactRatio::new(0, 1).unwrap(),
        };
        assert_eq!(
            network.round_fractional_costed(&invalid),
            Err(MinCostCirculationError::InvalidFractionalSolution)
        );
    }

    fn brute_force_cost(network: &CirculationNetwork) -> Option<i128> {
        let mut best = None;
        for first in 0..=2 {
            for second in 0..=2 {
                for direct in 0..=2 {
                    let flow = [first, second, direct];
                    let mut balance = vec![0; network.node_count];
                    let mut cost = 0;
                    for (arc, amount) in network.arcs.iter().zip(flow) {
                        balance[arc.from] -= amount;
                        balance[arc.to] += amount;
                        cost += arc.cost * amount;
                    }
                    if balance == network.demands {
                        best = Some(best.map_or(cost, |old: i128| old.min(cost)));
                    }
                }
            }
        }
        best
    }

    #[test]
    fn source_initial_augmentation_routes_arbitrary_demands_strictly() {
        let mut network = CirculationNetwork::new(2);
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(1), 1).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network
            .add_arc(FlowNodeId(1), FlowNodeId(0), 2, -1)
            .unwrap();
        let augmentation = network.initial_point_augmentation(2).unwrap();
        assert_eq!(augmentation.network.arc_count(), 4);
        assert_eq!(augmentation.artificial_arc_ids.len(), 2);
        assert_eq!(
            augmentation.initial_flow.arc_flows[0],
            ExactRatio::new(1, 1).unwrap()
        );
        assert_eq!(
            augmentation.initial_flow.arc_flows[1],
            ExactRatio::new(1, 1).unwrap()
        );
        augmentation
            .network
            .verify_fractional_solution(&augmentation.initial_flow)
            .unwrap();
        assert!(augmentation.maximum_abs_input >= 32);
        let augmented_optimum = experiment::solve(&augmentation.network).unwrap();
        let original_optimum = augmentation.recover_original(&augmented_optimum).unwrap();
        assert_eq!(original_optimum.cost, 1);
        assert_eq!(original_optimum.arc_flows, vec![1, 0]);
    }

    #[test]
    fn lower_bound_normalization_shifts_demands_costs_and_fixed_arcs() {
        let mut original = LowerBoundCirculationNetwork::new(2);
        original.set_demand(FlowNodeId(0), -2).unwrap();
        original.set_demand(FlowNodeId(1), 2).unwrap();
        original
            .add_arc(FlowNodeId(0), FlowNodeId(1), 1, 3, 2)
            .unwrap();
        original
            .add_arc(FlowNodeId(1), FlowNodeId(0), -1, 2, 1)
            .unwrap();
        original
            .add_arc(FlowNodeId(0), FlowNodeId(0), 2, 2, 3)
            .unwrap();
        let normalization = original.normalize_lower_bounds(3).unwrap();
        assert_eq!(normalization.normalized.demands(), &[0, 0]);
        assert_eq!(normalization.normalized.arc_count(), 2);
        assert_eq!(normalization.objective_offset, 7);
        let normalized = experiment::solve(&normalization.normalized).unwrap();
        let recovered = normalization.recover_original(&normalized).unwrap();
        assert_eq!(recovered.arc_flows, vec![1, -1, 2]);
        assert_eq!(recovered.cost, 7);
        original.verify_solution(&recovered).unwrap();
    }

    #[test]
    fn isolation_perturbation_recovers_exact_optimum_from_near_flow() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        let perturbation = network.isolation_perturbation(1, vec![1, 2]).unwrap();
        assert_eq!(perturbation.scaled_cost_denominator, 16);
        assert_eq!(
            perturbation.scaled_near_optimal_tolerance,
            ExactRatio::new(1, 6).unwrap()
        );
        let hundredth = ExactRatio::new(1, 100).unwrap();
        let near = FractionalCirculation {
            arc_flows: vec![hundredth; 2],
            cost: ExactRatio::new(3, 100).unwrap(),
        };
        let recovered = perturbation
            .recover_near_optimal(&near, ExactRatio::new(0, 1).unwrap())
            .unwrap();
        assert_eq!(recovered.solution.arc_flows, vec![0, 0]);
        assert_eq!(recovered.solution.cost, 0);
        assert_eq!(recovered.rank_support_upper, 4);
        assert_eq!(recovered.source_success_probability_numerator, 1);
        assert_eq!(recovered.source_success_probability_denominator, 2);
        assert!(recovered.exact_oracle_verified);
        assert_eq!(
            network.isolation_perturbation(1, vec![0, 2]),
            Err(MinCostCirculationError::InvalidPerturbation)
        );
    }
}
