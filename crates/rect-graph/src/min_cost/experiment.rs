use super::{
    CirculationNetwork, MinCostCirculationError, MinCostSolution, Residual, apply_residual,
    feasible_path, residual_capacity, residual_from, solution_cost, validate_feasible_solution,
};

/// One strict objective-decreasing update in the refinement experiment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    pub cycle: super::oracle::Cycle,
    pub augmentation: i128,
    pub cost_before: i128,
    pub cost_after: i128,
}

/// Exact recovered circulation and its auditable refinement trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Result {
    pub solution: MinCostSolution,
    pub steps: Vec<Step>,
}

/// Solves the exact integral min-cost circulation with residual-cycle refinement.
///
/// # Errors
///
/// Returns an error when demands are unbalanced, infeasible, or exact
/// arithmetic overflows.
pub fn solve(
    network: &CirculationNetwork,
) -> std::result::Result<MinCostSolution, MinCostCirculationError> {
    if network
        .demands
        .iter()
        .try_fold(0_i128, |sum, value| sum.checked_add(*value))
        .ok_or(MinCostCirculationError::Overflow)?
        != 0
    {
        return Err(MinCostCirculationError::UnbalancedDemand);
    }
    let mut flow = vec![0_i128; network.arcs.len()];
    let mut balance = vec![0_i128; network.node_count];
    loop {
        let Some(source) = balance
            .iter()
            .enumerate()
            .find_map(|(node, value)| (*value > network.demands[node]).then_some(node))
        else {
            break;
        };
        let target = balance
            .iter()
            .enumerate()
            .find_map(|(node, value)| (*value < network.demands[node]).then_some(node))
            .ok_or(MinCostCirculationError::Infeasible)?;
        let predecessor = feasible_path(network, &flow, source);
        if predecessor[target].is_none() {
            return Err(MinCostCirculationError::Infeasible);
        }
        let mut amount = (balance[source] - network.demands[source])
            .min(network.demands[target] - balance[target]);
        let mut node = target;
        while node != source {
            let edge = predecessor[node].ok_or(MinCostCirculationError::Infeasible)?;
            amount = amount.min(residual_capacity(network, &flow, edge));
            node = residual_from(network, edge);
        }
        let mut node = target;
        while node != source {
            let edge = predecessor[node].ok_or(MinCostCirculationError::Infeasible)?;
            apply_residual(&mut flow, edge, amount)?;
            node = residual_from(network, edge);
        }
        balance[source] -= amount;
        balance[target] += amount;
    }
    refine(
        network,
        &MinCostSolution {
            cost: solution_cost(network, &flow)?,
            arc_flows: flow,
        },
    )
    .map(|result| result.solution)
}

/// Refines a feasible integral circulation through exact residual cycles.
///
/// # Errors
///
/// Returns an error when the initial solution is infeasible or exact arithmetic
/// overflows during refinement.
pub fn refine(
    network: &CirculationNetwork,
    initial: &MinCostSolution,
) -> std::result::Result<Result, MinCostCirculationError> {
    validate_feasible_solution(network, initial)?;
    let mut solution = initial.clone();
    let mut steps = Vec::new();
    loop {
        let gradients = network.arcs.iter().map(|arc| arc.cost).collect::<Vec<_>>();
        let lengths = vec![1; network.arcs.len()];
        let Some(cycle) =
            super::oracle::minimum_residual_cycle(network, &solution, &gradients, &lengths)?
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
                network,
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
        solution.cost = solution_cost(network, &solution.arc_flows)?;
        if solution.cost >= cost_before {
            return Err(MinCostCirculationError::InvalidSolution);
        }
        steps.push(Step {
            cycle,
            augmentation: amount,
            cost_before,
            cost_after: solution.cost,
        });
    }
    network.verify_solution(&solution)?;
    Ok(Result { solution, steps })
}
