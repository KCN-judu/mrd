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
    #[must_use]
    pub fn new(node_count: usize) -> Self {
        Self {
            node_count,
            demands: vec![0; node_count],
            arcs: Vec::new(),
        }
    }

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

    /// Solves the exact integral min-cost circulation by successive
    /// Bellman--Ford augmentations followed by negative-cycle cancellation.
    /// This is deliberately a superlinear correctness Oracle.
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
        while let Some(cycle) = negative_cycle(self, &flow)? {
            let amount = cycle.iter().try_fold(i128::MAX, |a, edge| {
                Ok::<_, MinCostCirculationError>(a.min(residual_capacity(self, &flow, *edge)))
            })?;
            for edge in cycle {
                apply_residual(&mut flow, edge, amount)?;
            }
        }
        let cost = self
            .arcs
            .iter()
            .zip(&flow)
            .try_fold(0_i128, |sum, (arc, value)| {
                sum.checked_add(
                    arc.cost
                        .checked_mul(*value)
                        .ok_or(MinCostCirculationError::Overflow)?,
                )
                .ok_or(MinCostCirculationError::Overflow)
            })?;
        Ok(MinCostSolution {
            arc_flows: flow,
            cost,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MinCostSolution {
    pub arc_flows: Vec<i128>,
    pub cost: i128,
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
fn residual_cost(network: &CirculationNetwork, edge: Residual) -> i128 {
    if edge.reverse {
        -network.arcs[edge.arc].cost
    } else {
        network.arcs[edge.arc].cost
    }
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
fn negative_cycle(
    network: &CirculationNetwork,
    flow: &[i128],
) -> Result<Option<Vec<Residual>>, MinCostCirculationError> {
    let mut d = vec![0_i128; network.node_count];
    let mut p = vec![None; network.node_count];
    let mut changed = None;
    for _ in 0..network.node_count {
        changed = None;
        for e in edges(network, flow) {
            let u = residual_from(network, e);
            let v = residual_to(network, e);
            let n = d[u]
                .checked_add(residual_cost(network, e))
                .ok_or(MinCostCirculationError::Overflow)?;
            if n < d[v] {
                d[v] = n;
                p[v] = Some(e);
                changed = Some(v)
            }
        }
    }
    let Some(mut v) = changed else {
        return Ok(None);
    };
    for _ in 0..network.node_count {
        v = residual_from(network, p[v].ok_or(MinCostCirculationError::Infeasible)?)
    }
    let start = v;
    let mut cycle = Vec::new();
    loop {
        let e = p[v].ok_or(MinCostCirculationError::Infeasible)?;
        cycle.push(e);
        v = residual_from(network, e);
        if v == start {
            break;
        }
    }
    Ok(Some(cycle))
}

#[cfg(test)]
mod tests {
    use super::{CirculationNetwork, MinCostCirculationError};
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
}
