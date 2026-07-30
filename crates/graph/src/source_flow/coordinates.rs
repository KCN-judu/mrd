//! Exact-rational source-coordinate reconstruction.
//!
//! This module reads only the exact fractional circulation, exact objective
//! target, and immutable network data retained by a certified snapshot. It
//! deliberately does not read the snapshot's fixed-point length or gradient
//! intervals. The caller must still certify every result against Theorem 4.3.

use thiserror::Error;

use crate::{
    CertifiedIpmError, CertifiedIpmSnapshot, CirculationArcId, CirculationNetwork, ExactRatio,
    MinCostCirculationError, StableMinRatioError,
    source_min_ratio::input::{Error as InputError, Input},
};

/// Reconstructs rational Definition 4.2 approximations from exact source data.
///
/// The length approximation is the reciprocal-slack sum and the gradient
/// approximation retains the exact objective term `20m c_e / (c^T f - F*)`.
/// The omitted barrier contribution is intentional: it is accepted only when
/// the independent Theorem 4.3 certificate proves the required error bound.
/// No fixed-point snapshot interval contributes to the returned coordinates.
///
/// # Errors
///
/// Returns an error when the snapshot does not belong to the network, its exact
/// objective gap is nonpositive, exact arithmetic fails, or the resulting
/// source input is outside its checked domain.
pub fn reciprocal_slack_input(
    snapshot: &CertifiedIpmSnapshot,
    network: &CirculationNetwork,
) -> Result<Input, Error> {
    snapshot.verify_network(network)?;
    let gap = snapshot.flow().cost.checked_sub(snapshot.optimal_cost())?;
    if !gap.is_positive() {
        return Err(Error::NonpositiveObjectiveGap);
    }
    let edge_count = i128::try_from(network.arc_count()).map_err(|_| Error::Overflow)?;
    let scale = edge_count.checked_mul(20).ok_or(Error::Overflow)?;
    let slacks = network.fractional_slacks(&snapshot.flow().arc_flows)?;
    let mut gradients = Vec::with_capacity(slacks.len());
    let mut lengths = Vec::with_capacity(slacks.len());

    for (index, (lower, upper)) in slacks.into_iter().enumerate() {
        if !lower.is_positive() || !upper.is_positive() {
            return Err(Error::NonpositiveSlack { edge: index });
        }
        let (_, cost) = network
            .arc_capacity_cost(CirculationArcId(index))
            .ok_or(Error::MissingArc { edge: index })?;
        let length = lower.reciprocal()?.checked_add(upper.reciprocal()?)?;
        let gradient = ExactRatio::new(cost, 1)?
            .checked_mul_integer(scale)?
            .checked_mul(gap.reciprocal()?)?;
        gradients.push(gradient);
        lengths.push(length);
    }
    Ok(Input::new(network, &gradients, &lengths, &lengths)?)
}

/// Exact rational source-coordinate reconstruction failed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Ipm(#[from] CertifiedIpmError),
    #[error(transparent)]
    Network(#[from] MinCostCirculationError),
    #[error(transparent)]
    Ratio(#[from] StableMinRatioError),
    #[error(transparent)]
    Input(#[from] InputError),
    #[error("the source-coordinate objective gap is not strictly positive")]
    NonpositiveObjectiveGap,
    #[error("the source-coordinate slack for arc {edge} is not strictly positive")]
    NonpositiveSlack { edge: usize },
    #[error("the source-coordinate network is missing arc {edge}")]
    MissingArc { edge: usize },
    #[error("source-coordinate arithmetic exceeds the exact supported domain")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::reciprocal_slack_input;
    use crate::{
        CertifiedIpmSnapshot, CirculationNetwork, ExactRatio, FixedPointConfig, FlowNodeId,
        FractionalCirculation,
    };

    fn ratio(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    #[test]
    fn reconstructs_coordinates_without_reading_snapshot_intervals() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let half = ExactRatio::new(1, 2).unwrap();
        let flow = FractionalCirculation {
            arc_flows: vec![half; 2],
            cost: half,
        };
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &flow,
            ratio(0),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();

        let input = reciprocal_slack_input(&snapshot, &network).unwrap();
        assert_eq!(input.arcs().len(), 2);
        assert_eq!(input.arcs()[0].gradient, ratio(80));
        assert_eq!(input.arcs()[1].gradient, ratio(0));
        assert_eq!(input.arcs()[0].length, ExactRatio::new(8, 3).unwrap());
        assert_eq!(input.arcs()[1].length, ExactRatio::new(8, 3).unwrap());
    }
}
