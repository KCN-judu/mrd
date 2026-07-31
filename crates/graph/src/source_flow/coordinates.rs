//! Exact-rational source-coordinate reconstruction.
//!
//! This module reads only the exact fractional circulation, exact objective
//! target, and immutable network data retained by a certified snapshot. It
//! deliberately does not read the snapshot's fixed-point length or gradient
//! intervals. The caller must still certify every result against Theorem 4.3.

use num_bigint::BigInt;
use num_traits::One;
use thiserror::Error;

use crate::{
    CertifiedFixedPoint, CertifiedIpmError, CertifiedIpmSnapshot, CirculationArcId,
    CirculationNetwork, DyadicInterval, ExactRatio, FixedPointError, MinCostCirculationError,
    StableMinRatioError,
    source_min_ratio::input::{Error as InputError, Input},
};

/// Reconstructs full Definition 4.2 approximations as exact dyadic inputs.
///
/// This is intentionally an independent calculation from the snapshot's
/// retained coordinate intervals. It starts from the exact fractional flow,
/// exact objective target, immutable network, and the checked fixed-point
/// configuration. It then recomputes the source `alpha`, the two
/// `slack^-(1 + alpha)` length terms, and the barrier contribution to the
/// gradient. Each returned coordinate is an exact dyadic rational selected
/// from that fresh certified computation. [`CertifiedIpmSnapshot::certify_approximations`]
/// remains the independent acceptance check.
///
/// # Errors
///
/// Returns an error when the snapshot does not belong to the network, the
/// Definition 4.2 calculation cannot be certified in the configured
/// fixed-point model, or the resulting exact source input is invalid.
pub fn definition_input(
    snapshot: &CertifiedIpmSnapshot,
    network: &CirculationNetwork,
) -> Result<Input, Error> {
    snapshot.verify_network(network)?;
    let gap = snapshot.flow().cost.checked_sub(&snapshot.optimal_cost())?;
    if !gap.is_positive() {
        return Err(Error::NonpositiveObjectiveGap);
    }
    let edge_count = i128::try_from(network.arc_count()).map_err(|_| Error::Overflow)?;
    let maximum_abs_input = snapshot.maximum_abs_input();
    let m_u = edge_count
        .checked_mul(maximum_abs_input)
        .ok_or(Error::Overflow)?;
    if m_u <= 1 {
        return Err(Error::InvalidSourceDomain);
    }

    let mut arithmetic = CertifiedFixedPoint::new(snapshot.fixed_point_config())?;
    let m_u_interval = arithmetic.enclose_ratio(m_u, 1)?;
    let log_m_u = arithmetic.logarithm(&m_u_interval)?;
    let thousand_log = arithmetic.multiply_interval_integer(&log_m_u, 1_000)?;
    let one = arithmetic.enclose_ratio(1, 1)?;
    let alpha = arithmetic.divide_intervals(&one, &thousand_log)?;
    let exponent = arithmetic.add_intervals(&one, &alpha)?;
    let gap_interval = arithmetic.enclose_big_ratio(gap.numerator(), gap.denominator())?;
    let objective_factor = edge_count.checked_mul(20).ok_or(Error::Overflow)?;
    let slacks = network.fractional_slacks(&snapshot.flow().arc_flows)?;
    let mut gradients = Vec::with_capacity(slacks.len());
    let mut lengths = Vec::with_capacity(slacks.len());

    for (index, (lower_slack, upper_slack)) in slacks.into_iter().enumerate() {
        if !lower_slack.is_positive() || !upper_slack.is_positive() {
            return Err(Error::NonpositiveSlack { edge: index });
        }
        let lower =
            arithmetic.enclose_big_ratio(lower_slack.numerator(), lower_slack.denominator())?;
        let upper =
            arithmetic.enclose_big_ratio(upper_slack.numerator(), upper_slack.denominator())?;
        let lower_length = arithmetic.negative_power(&lower, &exponent)?;
        let upper_length = arithmetic.negative_power(&upper, &exponent)?;
        let length = arithmetic.add_intervals(&upper_length, &lower_length)?;
        if !length.is_strictly_positive() {
            return Err(Error::UncertifiedDefinitionCoordinate { edge: index });
        }

        let (_, cost) = network
            .arc_capacity_cost(CirculationArcId(index))
            .ok_or(Error::MissingArc { edge: index })?;
        let cost_interval = arithmetic.enclose_ratio(cost, 1)?;
        let objective_numerator =
            arithmetic.multiply_interval_integer(&cost_interval, objective_factor)?;
        let objective_gradient =
            arithmetic.divide_intervals(&objective_numerator, &gap_interval)?;
        let upper_gradient = arithmetic.multiply_intervals(&alpha, &upper_length)?;
        let lower_gradient = arithmetic.multiply_intervals(&alpha, &lower_length)?;
        let barrier_gradient = arithmetic.subtract_intervals(&upper_gradient, &lower_gradient)?;
        let gradient = arithmetic.add_intervals(&objective_gradient, &barrier_gradient)?;

        gradients.push(dyadic_lower(&gradient)?);
        lengths.push(dyadic_lower(&length)?);
    }
    Ok(Input::new(network, &gradients, &lengths, &lengths)?)
}

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
    let gap = snapshot.flow().cost.checked_sub(&snapshot.optimal_cost())?;
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
        let length = lower.reciprocal()?.checked_add(&upper.reciprocal()?)?;
        let gradient = ExactRatio::new(cost, 1)?
            .checked_mul_integer(scale)?
            .checked_mul(&gap.reciprocal()?)?;
        gradients.push(gradient);
        lengths.push(length);
    }
    Ok(Input::new(network, &gradients, &lengths, &lengths)?)
}

fn dyadic_lower(interval: &DyadicInterval) -> Result<ExactRatio, Error> {
    ExactRatio::from_bigints(
        interval.lower_scaled().clone(),
        BigInt::one() << interval.fractional_bits(),
    )
    .map_err(Error::from)
}

/// Exact rational source-coordinate reconstruction failed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    FixedPoint(#[from] FixedPointError),
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
    #[error("the source-coordinate Definition 4.2 domain is invalid")]
    InvalidSourceDomain,
    #[error("Definition 4.2 coordinate {edge} is not certifiably positive")]
    UncertifiedDefinitionCoordinate { edge: usize },
}

#[cfg(test)]
mod tests {
    use super::{definition_input, reciprocal_slack_input};
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
            arc_flows: vec![half.clone(); 2],
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

    #[test]
    fn reconstructs_full_definition_coordinates_that_recertify() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let quarter = ExactRatio::new(1, 4).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &FractionalCirculation {
                arc_flows: vec![quarter.clone(); 2],
                cost: quarter,
            },
            ratio(0),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();

        let input = definition_input(&snapshot, &network).unwrap();
        let reciprocal = reciprocal_slack_input(&snapshot, &network).unwrap();
        let gradients = input
            .arcs()
            .iter()
            .map(|arc| arc.gradient.clone())
            .collect::<Vec<_>>();
        let lengths = input
            .arcs()
            .iter()
            .map(|arc| arc.length.clone())
            .collect::<Vec<_>>();
        let mut arithmetic =
            crate::CertifiedFixedPoint::new(snapshot.fixed_point_config()).unwrap();
        snapshot
            .certify_approximations(
                &gradients,
                &lengths,
                ExactRatio::new(1, 2).unwrap(),
                &mut arithmetic,
            )
            .unwrap();
        assert_ne!(input.arcs()[0].gradient, reciprocal.arcs()[0].gradient);
        assert!(input.arcs().iter().all(|arc| arc.length.is_positive()));
    }
}
