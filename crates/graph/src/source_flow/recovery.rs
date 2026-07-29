//! Deterministic exact recovery for the source-flow boundary.
//!
//! This deliberately self-contained reduction turns a feasible fractional
//! circulation into an integral one through stable fractional-cycle
//! cancellations. It has no asymptotic runtime claim; P9.5 uses it to keep
//! recovery independent from the permanent verification implementations.

use std::collections::VecDeque;

use thiserror::Error;

use crate::{
    CirculationArcId, CirculationNetwork, CostedFlowRoundingResult, ExactRatio, FlowRoundingStep,
    FractionalCirculation, MinCostCirculationError, MinCostSolution, StableMinRatioError,
};

/// Recovers an integral feasible circulation without selecting a reference
/// flow backend.
///
/// Every cancellation fixes at least one fractional coordinate, preserves
/// demand balance, and chooses the non-increasing-cost direction. The result
/// is intentionally a traceable semantic primitive; no runtime bound is
/// asserted here.
///
/// # Errors
///
/// Returns an error for invalid input, arithmetic overflow, or a malformed
/// fractional-cycle witness.
pub fn round(
    network: &CirculationNetwork,
    initial: &FractionalCirculation,
) -> Result<CostedFlowRoundingResult, Error> {
    network.verify_fractional_solution(initial)?;
    let mut current = initial.clone();
    let mut steps = Vec::new();

    while current.arc_flows.iter().any(|flow| !flow.is_integral()) {
        let cycle = fractional_cycle(network, &current.arc_flows)?.ok_or(Error::NoCycle)?;
        network.validate_signed_circulation(&cycle)?;
        let signed_cost = cycle.iter().try_fold(0_i128, |sum, (arc, direction)| {
            let (_, cost) = network.arc_capacity_cost(*arc).ok_or(Error::NoCycle)?;
            let contribution = cost
                .checked_mul(i128::from(*direction))
                .ok_or(MinCostCirculationError::Overflow)?;
            sum.checked_add(contribution)
                .ok_or(MinCostCirculationError::Overflow)
                .map_err(Error::from)
        })?;
        let orientation = if signed_cost <= 0 { 1_i8 } else { -1_i8 };
        let oriented = cycle
            .iter()
            .map(|(arc, direction)| {
                Ok((
                    *arc,
                    direction
                        .checked_mul(orientation)
                        .ok_or(Error::InvalidDirection)?,
                ))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let augmentation = oriented
            .iter()
            .try_fold(None::<ExactRatio>, |best, (arc, direction)| {
                let flow = *current.arc_flows.get(arc.0).ok_or(Error::NoCycle)?;
                let available = availability(flow, *direction)?;
                Ok::<Option<ExactRatio>, Error>(match best {
                    None => Some(available),
                    Some(current_best) if current_best.at_least(available)? => Some(available),
                    Some(current_best) => Some(current_best),
                })
            })?
            .ok_or(Error::NoCycle)?;
        let cost_before = current.cost;
        for (arc, direction) in &oriented {
            let delta = augmentation.checked_mul_integer(i128::from(*direction))?;
            let slot = current.arc_flows.get_mut(arc.0).ok_or(Error::NoCycle)?;
            *slot = slot.checked_add(delta)?;
        }
        current.cost = network.fractional_cost(&current.arc_flows)?;
        if current.cost.at_least(cost_before)? && current.cost != cost_before {
            return Err(Error::CostIncreased);
        }
        network.verify_fractional_solution(&current)?;
        steps.push(FlowRoundingStep {
            cycle: oriented,
            augmentation,
            cost_before,
            cost_after: current.cost,
        });
    }

    let solution = integral_solution(network, &current)?;
    Ok(CostedFlowRoundingResult { solution, steps })
}

fn fractional_cycle(
    network: &CirculationNetwork,
    flow: &[ExactRatio],
) -> Result<Option<Vec<(CirculationArcId, i8)>>, Error> {
    let mut adjacency = vec![Vec::<TreeEdge>::new(); network.demands().len()];
    for (index, value) in flow.iter().copied().enumerate() {
        if value.is_integral() {
            continue;
        }
        let arc = CirculationArcId(index);
        let (from, to) = network.arc_endpoints(arc).ok_or(Error::NoCycle)?;
        if from == to {
            return Ok(Some(vec![(arc, 1)]));
        }
        if let Some(mut path) = forest_path(&adjacency, from.0, to.0)? {
            path.push((arc, -1));
            return Ok(Some(path));
        }
        adjacency[from.0].push(TreeEdge {
            to: to.0,
            arc,
            direction: 1,
        });
        adjacency[to.0].push(TreeEdge {
            to: from.0,
            arc,
            direction: -1,
        });
    }
    Ok(None)
}

fn forest_path(
    adjacency: &[Vec<TreeEdge>],
    start: usize,
    end: usize,
) -> Result<Option<Vec<(CirculationArcId, i8)>>, Error> {
    let mut predecessor = vec![None; adjacency.len()];
    let mut queue = VecDeque::from([start]);
    predecessor[start] = Some(Predecessor::Root);
    while let Some(node) = queue.pop_front() {
        if node == end {
            break;
        }
        for edge in &adjacency[node] {
            if predecessor[edge.to].is_none() {
                predecessor[edge.to] = Some(Predecessor::Edge {
                    from: node,
                    arc: edge.arc,
                    direction: edge.direction,
                });
                queue.push_back(edge.to);
            }
        }
    }
    if predecessor[end].is_none() {
        return Ok(None);
    }

    let mut path = Vec::new();
    let mut node = end;
    while node != start {
        let Predecessor::Edge {
            from,
            arc,
            direction,
        } = predecessor[node].ok_or(Error::NoCycle)?
        else {
            return Err(Error::NoCycle);
        };
        path.push((arc, direction));
        node = from;
    }
    path.reverse();
    Ok(Some(path))
}

fn availability(flow: ExactRatio, direction: i8) -> Result<ExactRatio, Error> {
    if flow.is_integral() {
        return Err(Error::NoCycle);
    }
    let floor = flow.numerator() / flow.denominator();
    match direction {
        1 => Ok(ExactRatio::new(
            floor
                .checked_add(1)
                .ok_or(MinCostCirculationError::Overflow)?,
            1,
        )?
        .checked_sub(flow)?),
        -1 => Ok(flow.checked_sub(ExactRatio::new(floor, 1)?)?),
        _ => Err(Error::InvalidDirection),
    }
}

fn integral_solution(
    network: &CirculationNetwork,
    flow: &FractionalCirculation,
) -> Result<MinCostSolution, Error> {
    let arc_flows = flow
        .arc_flows
        .iter()
        .copied()
        .map(|value| {
            if !value.is_integral() {
                return Err(Error::NoCycle);
            }
            Ok(value.numerator() / value.denominator())
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let cost = network.fractional_cost(&flow.arc_flows)?;
    if !cost.is_integral() {
        return Err(Error::NonIntegralCost);
    }
    let integral = FractionalCirculation {
        arc_flows: arc_flows
            .iter()
            .copied()
            .map(|value| ExactRatio::new(value, 1))
            .collect::<Result<Vec<_>, _>>()?,
        cost,
    };
    network.verify_fractional_solution(&integral)?;
    Ok(MinCostSolution {
        arc_flows,
        cost: cost.numerator() / cost.denominator(),
    })
}

#[derive(Clone, Copy)]
struct TreeEdge {
    to: usize,
    arc: CirculationArcId,
    direction: i8,
}

#[derive(Clone, Copy)]
enum Predecessor {
    Root,
    Edge {
        from: usize,
        arc: CirculationArcId,
        direction: i8,
    },
}

/// Recovery failed before a valid integral circulation was constructed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] MinCostCirculationError),
    #[error(transparent)]
    Ratio(#[from] StableMinRatioError),
    #[error("fractional circulation did not expose a cancellation cycle")]
    NoCycle,
    #[error("fractional-cycle direction is invalid")]
    InvalidDirection,
    #[error("a cancellation increased the exact objective")]
    CostIncreased,
    #[error("integral coordinates produced a nonintegral objective")]
    NonIntegralCost,
}

#[cfg(test)]
mod tests {
    use super::round;
    use crate::{CirculationNetwork, ExactRatio, FlowNodeId, FractionalCirculation};

    #[test]
    fn rounds_a_fractional_cycle_with_a_complete_trace() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let quarter = ExactRatio::new(1, 4).unwrap();
        let rounded = round(
            &network,
            &FractionalCirculation {
                arc_flows: vec![quarter; 2],
                cost: quarter,
            },
        )
        .unwrap();
        assert_eq!(rounded.solution.arc_flows, vec![0, 0]);
        assert_eq!(rounded.solution.cost, 0);
        assert_eq!(rounded.steps.len(), 1);
    }

    #[test]
    fn matches_the_reference_recovery_trace_on_a_shared_fractional_cycle() {
        let mut network = CirculationNetwork::new(4);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 2).unwrap();
        network
            .add_arc(FlowNodeId(1), FlowNodeId(2), 2, -1)
            .unwrap();
        network.add_arc(FlowNodeId(2), FlowNodeId(0), 2, 1).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(3), 2, 0).unwrap();
        network.add_arc(FlowNodeId(3), FlowNodeId(2), 2, 1).unwrap();
        let half = ExactRatio::new(1, 2).unwrap();
        let initial = FractionalCirculation {
            arc_flows: vec![half, half, ExactRatio::new(1, 1).unwrap(), half, half],
            cost: ExactRatio::new(2, 1).unwrap(),
        };
        network.verify_fractional_solution(&initial).unwrap();

        let recovered = round(&network, &initial).unwrap();
        let reference = network.round_fractional_costed(&initial).unwrap();
        assert_eq!(recovered, reference);
    }
}
