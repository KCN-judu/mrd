use thiserror::Error;

use crate::FlowNodeId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CirculationArcId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
struct Arc {
    from: usize,
    to: usize,
    capacity: i128,
    cost: i128,
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
        if self
            .minimum_ratio_residual_cycle(solution, &gradients, &lengths)?
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

    /// Solves the exact integral min-cost circulation with the deliberately
    /// superlinear residual-cycle refinement Oracle.
    ///
    /// # Errors
    ///
    /// Returns an error when demands are unbalanced, infeasible, or exact
    /// arithmetic overflows.
    pub fn solve(&self) -> Result<MinCostSolution, MinCostCirculationError> {
        if self
            .demands
            .iter()
            .try_fold(0_i128, |sum, value| sum.checked_add(*value))
            .ok_or(MinCostCirculationError::Overflow)?
            != 0
        {
            return Err(MinCostCirculationError::UnbalancedDemand);
        }
        let mut flow = vec![0_i128; self.arcs.len()];
        let mut balance = vec![0_i128; self.node_count];
        loop {
            let Some(source) = balance
                .iter()
                .enumerate()
                .find_map(|(node, value)| (*value > self.demands[node]).then_some(node))
            else {
                break;
            };
            let target = balance
                .iter()
                .enumerate()
                .find_map(|(node, value)| (*value < self.demands[node]).then_some(node))
                .ok_or(MinCostCirculationError::Infeasible)?;
            let predecessor = feasible_path(self, &flow, source);
            if predecessor[target].is_none() {
                return Err(MinCostCirculationError::Infeasible);
            }
            let mut amount = (balance[source] - self.demands[source])
                .min(self.demands[target] - balance[target]);
            let mut node = target;
            while node != source {
                let edge = predecessor[node].ok_or(MinCostCirculationError::Infeasible)?;
                amount = amount.min(residual_capacity(self, &flow, edge));
                node = residual_from(self, edge);
            }
            let mut node = target;
            while node != source {
                let edge = predecessor[node].ok_or(MinCostCirculationError::Infeasible)?;
                apply_residual(&mut flow, edge, amount)?;
                node = residual_from(self, edge);
            }
            balance[source] -= amount;
            balance[target] += amount;
        }
        self.refine_feasible(&MinCostSolution {
            cost: solution_cost(self, &flow)?,
            arc_flows: flow,
        })
        .map(|result| result.solution)
    }

    /// Refines a feasible integral circulation through exact signed residual
    /// minimum-ratio cycles. Unit residual lengths make every selected
    /// negative-ratio cycle a strict cost improvement. This finite baseline
    /// is not the interior-point or dynamic algorithm from the cited source.
    ///
    /// # Errors
    ///
    /// Returns an error when the initial solution is not exactly feasible or
    /// exact arithmetic overflows during refinement.
    pub fn refine_feasible(
        &self,
        initial: &MinCostSolution,
    ) -> Result<IterativeRefinementResult, MinCostCirculationError> {
        validate_feasible_solution(self, initial)?;
        let mut solution = initial.clone();
        let mut steps = Vec::new();
        loop {
            let gradients = self.arcs.iter().map(|arc| arc.cost).collect::<Vec<_>>();
            let lengths = vec![1; self.arcs.len()];
            let Some(cycle) = self.minimum_ratio_residual_cycle(&solution, &gradients, &lengths)?
            else {
                break;
            };
            if cycle.gradient_sum >= 0 {
                break;
            }
            let residual = cycle
                .arcs
                .iter()
                .map(|(arc, direction)| Residual {
                    arc: arc.0,
                    reverse: *direction < 0,
                })
                .collect::<Vec<_>>();
            let amount = residual.iter().try_fold(i128::MAX, |current, edge| {
                Ok::<_, MinCostCirculationError>(current.min(residual_capacity(
                    self,
                    &solution.arc_flows,
                    *edge,
                )))
            })?;
            if amount <= 0 {
                return Err(MinCostCirculationError::InvalidSolution);
            }
            let cost_before = solution.cost;
            for edge in residual {
                apply_residual(&mut solution.arc_flows, edge, amount)?;
            }
            solution.cost = solution_cost(self, &solution.arc_flows)?;
            if solution.cost >= cost_before {
                return Err(MinCostCirculationError::InvalidSolution);
            }
            steps.push(IterativeRefinementStep {
                cycle,
                augmentation: amount,
                cost_before,
                cost_after: solution.cost,
            });
        }
        self.verify_solution(&solution)?;
        Ok(IterativeRefinementResult { solution, steps })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinCostSolution {
    pub arc_flows: Vec<i128>,
    pub cost: i128,
}

/// An exact simple-cycle minimum-ratio result for the static Oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinRatioCycle {
    pub arcs: Vec<(CirculationArcId, i8)>,
    pub gradient_sum: i128,
    pub length_sum: i128,
}

/// One strict objective-decreasing update performed by the baseline Oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IterativeRefinementStep {
    pub cycle: MinRatioCycle,
    pub augmentation: i128,
    pub cost_before: i128,
    pub cost_after: i128,
}

/// Exact recovered circulation and its auditable baseline refinement trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IterativeRefinementResult {
    pub solution: MinCostSolution,
    pub steps: Vec<IterativeRefinementStep>,
}

impl CirculationNetwork {
    /// Exhaustively enumerates simple directed cycles in the input graph. This
    /// is a superlinear baseline Oracle for Definition 4.2, not a dynamic
    /// backend.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid dimensions, nonpositive lengths, or exact
    /// arithmetic overflow.
    pub fn minimum_ratio_cycle(
        &self,
        gradients: &[i128],
        lengths: &[i128],
    ) -> Result<Option<MinRatioCycle>, MinCostCirculationError> {
        if gradients.len() != self.arcs.len()
            || lengths.len() != self.arcs.len()
            || lengths.iter().any(|value| *value <= 0)
        {
            return Err(MinCostCirculationError::InvalidRatioInput);
        }
        let mut best = None;
        for start in 0..self.node_count {
            let mut seen = vec![false; self.node_count];
            seen[start] = true;
            let mut path = Vec::new();
            enumerate_cycles(
                self, gradients, lengths, start, start, &mut seen, &mut path, 0, 0, &mut best,
            )?;
        }
        Ok(best)
    }

    /// Exhaustively enumerates simple cycles in the residual graph of a
    /// feasible solution. Reverse residual arcs negate their gradient while
    /// retaining the corresponding positive length.
    ///
    /// # Errors
    ///
    /// Returns an error when the solution is not feasible, the input vectors
    /// are invalid, or exact arithmetic overflows.
    pub fn minimum_ratio_residual_cycle(
        &self,
        solution: &MinCostSolution,
        gradients: &[i128],
        lengths: &[i128],
    ) -> Result<Option<MinRatioCycle>, MinCostCirculationError> {
        validate_feasible_solution(self, solution)?;
        if gradients.len() != self.arcs.len()
            || lengths.len() != self.arcs.len()
            || lengths.iter().any(|value| *value <= 0)
        {
            return Err(MinCostCirculationError::InvalidRatioInput);
        }
        let residual_edges = edges(self, &solution.arc_flows);
        let mut best = None;
        for start in 0..self.node_count {
            let mut seen = vec![false; self.node_count];
            seen[start] = true;
            let mut path = Vec::new();
            enumerate_residual_cycles(
                self,
                gradients,
                lengths,
                &residual_edges,
                start,
                start,
                &mut seen,
                &mut path,
                0,
                0,
                &mut best,
            )?;
        }
        Ok(best)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MinCostCirculationError {
    #[error("node {node} is outside network with {node_count} nodes")]
    NodeOutOfBounds { node: usize, node_count: usize },
    #[error("capacity must be nonnegative")]
    NegativeCapacity,
    #[error("demands do not sum to zero")]
    UnbalancedDemand,
    #[error("no feasible circulation exists")]
    Infeasible,
    #[error("exact integer arithmetic overflowed")]
    Overflow,
    #[error("gradient/length dimensions differ or a length is not positive")]
    InvalidRatioInput,
    #[error(
        "flow vector is infeasible, has an incorrect cost, or admits a negative residual cycle"
    )]
    InvalidSolution,
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
    best: &mut Option<MinRatioCycle>,
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
            let candidate = MinRatioCycle {
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
    best: &mut Option<MinRatioCycle>,
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
            let candidate = MinRatioCycle {
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
    best: Option<&MinRatioCycle>,
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
    use super::{CirculationNetwork, MinCostCirculationError, MinCostSolution};
    use crate::FlowNodeId;

    #[test]
    fn routes_demand_at_minimum_cost() {
        let mut network = CirculationNetwork::new(3);
        network.set_demand(FlowNodeId(0), -3).unwrap();
        network.set_demand(FlowNodeId(2), 3).unwrap();
        let direct = network.add_arc(FlowNodeId(0), FlowNodeId(2), 3, 5).unwrap();
        let first = network.add_arc(FlowNodeId(0), FlowNodeId(1), 3, 1).unwrap();
        let second = network.add_arc(FlowNodeId(1), FlowNodeId(2), 3, 1).unwrap();
        let solution = network.solve().unwrap();
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
        let solution = network.solve().unwrap();
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
        let solution = network.solve().unwrap();
        assert_eq!(solution.cost, -7);
    }

    #[test]
    fn rejects_infeasible_demand() {
        let mut network = CirculationNetwork::new(2);
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(1), 1).unwrap();
        assert_eq!(network.solve(), Err(MinCostCirculationError::Infeasible));
    }

    #[test]
    fn selects_the_lowest_exact_simple_cycle_ratio() {
        let mut network = CirculationNetwork::new(3);
        let a = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let b = network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(2), 1, 0).unwrap();
        network.add_arc(FlowNodeId(2), FlowNodeId(0), 1, 0).unwrap();
        let result = network
            .minimum_ratio_cycle(&[-2, 0, -1, 0], &[1, 1, 1, 1])
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
        let ratio = network.minimum_ratio_cycle(&[-3], &[1]).unwrap().unwrap();
        assert_eq!(ratio.arcs, vec![(loop_arc, 1)]);
        let solution = network.solve().unwrap();
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
        let cycle = network
            .minimum_ratio_residual_cycle(&flow, &[5, 1], &[1, 1])
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
        let result = network
            .refine_feasible(&MinCostSolution {
                arc_flows: vec![0, 0],
                cost: 0,
            })
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
                    let actual = network.solve().unwrap();
                    assert_eq!(actual.cost, expected);
                    network.verify_solution(&actual).unwrap();
                }
            }
        }
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
}
