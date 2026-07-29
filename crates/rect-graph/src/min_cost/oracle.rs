use super::{
    CirculationArcId, CirculationNetwork, MinCostCirculationError, MinCostSolution, edges,
    enumerate_cycles, enumerate_residual_cycles, validate_feasible_solution,
};

/// Exact simple-cycle minimum-ratio result for the static Oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cycle {
    pub arcs: Vec<(CirculationArcId, i8)>,
    pub gradient_sum: i128,
    pub length_sum: i128,
}

/// Exhaustively enumerates simple directed cycles in the input graph.
///
/// # Errors
///
/// Returns an error for invalid dimensions, nonpositive lengths, or exact
/// arithmetic overflow.
pub fn minimum_cycle(
    network: &CirculationNetwork,
    gradients: &[i128],
    lengths: &[i128],
) -> Result<Option<Cycle>, MinCostCirculationError> {
    if gradients.len() != network.arcs.len()
        || lengths.len() != network.arcs.len()
        || lengths.iter().any(|value| *value <= 0)
    {
        return Err(MinCostCirculationError::InvalidRatioInput);
    }
    let mut best = None;
    for start in 0..network.node_count {
        let mut seen = vec![false; network.node_count];
        seen[start] = true;
        enumerate_cycles(
            network,
            gradients,
            lengths,
            start,
            start,
            &mut seen,
            &mut Vec::new(),
            0,
            0,
            &mut best,
        )?;
    }
    Ok(best)
}

/// Exhaustively enumerates simple cycles in the residual graph.
///
/// # Errors
///
/// Returns an error when the solution is infeasible, inputs are invalid, or
/// exact arithmetic overflows.
pub fn minimum_residual_cycle(
    network: &CirculationNetwork,
    solution: &MinCostSolution,
    gradients: &[i128],
    lengths: &[i128],
) -> Result<Option<Cycle>, MinCostCirculationError> {
    validate_feasible_solution(network, solution)?;
    if gradients.len() != network.arcs.len()
        || lengths.len() != network.arcs.len()
        || lengths.iter().any(|value| *value <= 0)
    {
        return Err(MinCostCirculationError::InvalidRatioInput);
    }
    let residual_edges = edges(network, &solution.arc_flows);
    let mut best = None;
    for start in 0..network.node_count {
        let mut seen = vec![false; network.node_count];
        seen[start] = true;
        enumerate_residual_cycles(
            network,
            gradients,
            lengths,
            &residual_edges,
            start,
            start,
            &mut seen,
            &mut Vec::new(),
            0,
            0,
            &mut best,
        )?;
    }
    Ok(best)
}
