//! Experimental source-shaped flow backend boundary.
//!
//! This module intentionally has no reference max-flow or enumerating
//! min-cost-cycle dependency. It cannot be enabled until P9.5 supplies the
//! complete certified iteration path and compressed-network integration.

use thiserror::Error;

use crate::{
    CertifiedIpmError, CertifiedIpmSnapshot, CirculationNetwork, CostedFlowRoundingResult,
    IpmTerminationCertificate, MinCostCirculationError,
};

pub mod recovery;

/// Explicit status for the experimental source-shaped backend.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Status {
    /// This backend never carries the almost-linear runtime claim at present.
    pub an19_runtime_verified: bool,
}

/// Production entry point reserved for the complete P9.5 integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Backend;

impl Backend {
    /// Reports the conservative runtime status of this backend boundary.
    #[must_use]
    pub const fn status(self) -> Status {
        Status {
            an19_runtime_verified: false,
        }
    }

    /// Rejects use until P9.5 connects certified IPM iterations and recovery.
    ///
    /// # Errors
    ///
    /// Always returns an explicit unavailable error; no fallback is selected.
    pub const fn require_complete(self) -> Result<(), Error> {
        Err(Error::Incomplete)
    }

    /// Certifies the additive-half boundary without invoking any recovery Oracle.
    ///
    /// # Errors
    ///
    /// Returns the certified-IPM error when the supplied snapshot has not
    /// reached the additive-half termination boundary.
    pub fn certify_termination(
        self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
    ) -> Result<IpmTerminationCertificate, Error> {
        snapshot
            .certify_additive_half_termination(network)
            .map_err(Error::Ipm)
    }

    /// Recovers an exact integral optimum from a certified terminal snapshot.
    ///
    /// This is a deterministic source-flow recovery boundary, not a complete
    /// source-flow solver. It uses the local exact fractional-cycle reduction
    /// in [`recovery`] and verifies the recovered integral solution against
    /// the terminal snapshot's retained optimum.
    ///
    /// # Errors
    ///
    /// Returns an error when termination, exact recovery, or final optimality
    /// validation fails.
    pub fn recover_terminated(
        self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
    ) -> Result<RecoveredFlow, Error> {
        let termination = self.certify_termination(snapshot, network)?;
        let rounding = recovery::round(network, snapshot.flow()).map_err(Error::Recovery)?;
        let optimal = snapshot.optimal_cost();
        if !optimal.is_integral() || optimal.numerator() != rounding.solution.cost {
            return Err(Error::RecoveryNotOptimal);
        }
        network.verify_solution(&rounding.solution)?;
        Ok(RecoveredFlow {
            termination,
            rounding,
        })
    }
}

/// An exact integral recovery paired with its terminal certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredFlow {
    pub termination: IpmTerminationCertificate,
    pub rounding: CostedFlowRoundingResult,
}

/// The source-shaped backend is not complete enough to execute.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// Certified iteration and recovery integration has not been implemented.
    #[error("source-shaped flow backend is not yet complete")]
    Incomplete,
    /// The certified IPM snapshot cannot establish its termination boundary.
    #[error("certified IPM termination failed: {0}")]
    Ipm(#[from] CertifiedIpmError),
    /// Local exact recovery failed before an integral solution was available.
    #[error("source-shaped recovery failed: {0}")]
    Recovery(#[source] recovery::Error),
    /// The recovered integral cost differs from the terminal snapshot optimum.
    #[error("source-shaped recovery did not return the terminal optimum")]
    RecoveryNotOptimal,
    /// The recovered result is not a valid minimum-cost circulation.
    #[error("recovered circulation validation failed: {0}")]
    Network(#[from] MinCostCirculationError),
}

#[cfg(test)]
mod tests {
    use super::{Backend, Error};
    use crate::{
        CertifiedIpmSnapshot, CirculationNetwork, ExactRatio, FixedPointConfig, FlowNodeId,
        FractionalCirculation,
    };

    #[test]
    fn never_silently_selects_a_fallback_backend() {
        let backend = Backend;
        assert!(!backend.status().an19_runtime_verified);
        assert_eq!(backend.require_complete(), Err(Error::Incomplete));
    }

    #[test]
    fn recovers_a_certified_terminal_snapshot_without_a_fallback_backend() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let quarter = ExactRatio::new(1, 4).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &FractionalCirculation {
                arc_flows: vec![quarter; 2],
                cost: quarter,
            },
            ExactRatio::new(0, 1).unwrap(),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();

        let recovered = Backend.recover_terminated(&snapshot, &network).unwrap();
        assert_eq!(recovered.rounding.solution.arc_flows, vec![0, 0]);
        assert_eq!(recovered.rounding.solution.cost, 0);
    }
}
