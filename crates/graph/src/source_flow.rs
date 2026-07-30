//! Experimental source-shaped flow backend boundary.
//!
//! This module intentionally has no reference max-flow or enumerating
//! min-cost-cycle dependency. It cannot be enabled until P9.5 supplies the
//! complete certified iteration path and compressed-network integration.

use thiserror::Error;

use crate::{
    CertifiedIpmError, CertifiedIpmSnapshot, CertifiedLowerBoundInitialPoint, CirculationNetwork,
    CostedFlowRoundingResult, InitialPointAugmentation, IpmTerminationCertificate,
    MinCostCirculationError, MinCostSolution,
};

pub mod coordinates;
pub mod iteration;
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
        // The additive-half certificate and exact cost equality above establish
        // optimality; recovery only needs a no-Oracle feasibility check.
        network.verify_feasible_solution(&rounding.solution)?;
        Ok(RecoveredFlow {
            termination,
            rounding,
        })
    }

    /// Recovers the original zero-lower-bound network from an augmented terminal
    /// snapshot without invoking a reference recovery path.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal recovery fails, an artificial arc carries
    /// flow, or the recovered original solution is invalid.
    pub fn recover_augmented_terminated(
        self,
        snapshot: &CertifiedIpmSnapshot,
        augmentation: &InitialPointAugmentation,
    ) -> Result<RecoveredAugmentedFlow, Error> {
        let terminal = self.recover_terminated(snapshot, &augmentation.network)?;
        let original = augmentation
            .recover_original_feasible(&terminal.rounding.solution)
            .map_err(Error::Augmentation)?;
        Ok(RecoveredAugmentedFlow { terminal, original })
    }

    /// Recovers an original lower-bounded flow from a terminal augmented
    /// snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when augmented recovery, lower-bound restoration, or
    /// either exact validation fails.
    pub fn recover_lower_bounded_terminated(
        self,
        snapshot: &CertifiedIpmSnapshot,
        initial: &CertifiedLowerBoundInitialPoint,
    ) -> Result<RecoveredLowerBoundFlow, Error> {
        let augmented =
            self.recover_augmented_terminated(snapshot, &initial.initial_point.augmentation)?;
        let original = initial
            .normalization
            .recover_original_feasible(&augmented.original)
            .map_err(Error::Normalization)?;
        Ok(RecoveredLowerBoundFlow {
            terminal: augmented.terminal,
            normalized: augmented.original,
            original,
        })
    }

    /// Starts a checked sequence of externally supplied certified updates.
    ///
    /// The source minimum-ratio layer does not yet select these directions, so
    /// this method intentionally executes supplied steps only. It cannot be
    /// used to claim a complete source-flow solver.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial snapshot cannot initialize its exact
    /// Detect accounting ledger.
    pub fn begin_iterations(
        self,
        snapshot: CertifiedIpmSnapshot,
    ) -> Result<iteration::Session, Error> {
        iteration::Session::new(snapshot).map_err(Error::Iteration)
    }

    /// Starts a bounded run that requests one certified source projection per
    /// current IPM snapshot.
    ///
    /// This composes only the exact iteration contract. It does not select a
    /// reference backend, infer coordinates from fixed-point intervals, or
    /// make this experimental backend complete.
    ///
    /// # Errors
    ///
    /// Returns an error when the initial snapshot cannot initialize exact
    /// Detect accounting.
    pub fn begin_source_iterations<F: iteration::Factory>(
        self,
        snapshot: CertifiedIpmSnapshot,
        factory: F,
        maximum_iterations: u64,
    ) -> Result<iteration::Driver<F>, Error> {
        Ok(iteration::Driver::new(
            self.begin_iterations(snapshot)?,
            factory,
            maximum_iterations,
        ))
    }
}

/// An exact integral recovery paired with its terminal certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredFlow {
    pub termination: IpmTerminationCertificate,
    pub rounding: CostedFlowRoundingResult,
}

/// Exact recovery from an augmented circulation to its original network.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredAugmentedFlow {
    pub terminal: RecoveredFlow,
    pub original: MinCostSolution,
}

/// Exact recovery through augmentation and lower-bound normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveredLowerBoundFlow {
    pub terminal: RecoveredFlow,
    pub normalized: MinCostSolution,
    pub original: MinCostSolution,
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
    /// Recovery could not remove the certified initial-point augmentation.
    #[error("augmented source-flow recovery failed: {0}")]
    Augmentation(#[source] MinCostCirculationError),
    /// Recovery could not restore lower bounds after augmentation recovery.
    #[error("lower-bound source-flow recovery failed: {0}")]
    Normalization(#[source] MinCostCirculationError),
    /// A supplied certified iteration step failed validation.
    #[error("source-shaped iteration failed: {0}")]
    Iteration(#[source] iteration::Error),
}

#[cfg(test)]
mod tests {
    use super::{Backend, Error, iteration};
    use crate::{
        CertifiedIpmError, CertifiedIpmSnapshot, CirculationNetwork, ExactRatio, FixedPointConfig,
        FlowNodeId, FractionalCirculation, LowerBoundCirculationNetwork,
    };

    #[test]
    fn never_silently_selects_a_fallback_backend() {
        let backend = Backend;
        assert!(!backend.status().an19_runtime_verified);
        assert_eq!(backend.require_complete(), Err(Error::Incomplete));
    }

    #[test]
    fn begins_a_bounded_source_driver_without_marking_the_backend_complete() {
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
        let factory = |_: &CertifiedIpmSnapshot, _: &CirculationNetwork| {
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        };
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 0)
            .unwrap();

        let completion = driver.run(&network).unwrap();
        assert!(completion.records.is_empty());
        assert_eq!(Backend.require_complete(), Err(Error::Incomplete));
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

    #[test]
    fn rejects_terminal_recovery_for_a_different_certified_network() {
        let mut certified = CirculationNetwork::new(2);
        certified
            .add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1)
            .unwrap();
        certified
            .add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0)
            .unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &certified,
            &FractionalCirculation {
                arc_flows: vec![ExactRatio::new(1, 4).unwrap(); 2],
                cost: ExactRatio::new(1, 4).unwrap(),
            },
            ExactRatio::new(0, 1).unwrap(),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();
        let mut mismatched = CirculationNetwork::new(2);
        mismatched
            .add_arc(FlowNodeId(0), FlowNodeId(1), 2, 0)
            .unwrap();
        mismatched
            .add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0)
            .unwrap();

        assert_eq!(
            Backend.recover_terminated(&snapshot, &mismatched),
            Err(Error::Ipm(CertifiedIpmError::NetworkMismatch))
        );
    }

    #[test]
    fn recovers_an_augmented_terminal_snapshot_to_its_original_network() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let config = FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap();
        let initial = CertifiedIpmSnapshot::initial_point_augmented(
            &network,
            ExactRatio::new(0, 1).unwrap(),
            4,
            config,
        )
        .unwrap();
        let quarter = ExactRatio::new(1, 4).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &initial.augmentation.network,
            &FractionalCirculation {
                arc_flows: vec![quarter; 2],
                cost: quarter,
            },
            ExactRatio::new(0, 1).unwrap(),
            initial.augmentation.maximum_abs_input,
            config,
        )
        .unwrap();

        let recovered = Backend
            .recover_augmented_terminated(&snapshot, &initial.augmentation)
            .unwrap();
        assert_eq!(recovered.original.arc_flows, vec![0, 0]);
        assert_eq!(recovered.original.cost, 0);
    }

    #[test]
    fn recovers_lower_bounds_after_augmented_terminal_recovery() {
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
        let config = FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap();
        let initial = CertifiedIpmSnapshot::initial_point_lower_bounded(
            &network,
            ExactRatio::new(7, 1).unwrap(),
            3,
            config,
        )
        .unwrap();
        let initial_flow = &initial.initial_point.augmentation.initial_flow;
        let scale = ExactRatio::new(
            initial_flow.cost.denominator(),
            initial_flow.cost.numerator().checked_mul(8).unwrap(),
        )
        .unwrap();
        let arc_flows = initial_flow
            .arc_flows
            .iter()
            .copied()
            .map(|flow| flow.checked_mul(scale).unwrap())
            .collect();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &initial.initial_point.augmentation.network,
            &FractionalCirculation {
                arc_flows,
                cost: initial_flow.cost.checked_mul(scale).unwrap(),
            },
            ExactRatio::new(0, 1).unwrap(),
            initial.initial_point.augmentation.maximum_abs_input,
            config,
        )
        .unwrap();

        let recovered = Backend
            .recover_lower_bounded_terminated(&snapshot, &initial)
            .unwrap();
        assert_eq!(recovered.normalized.arc_flows, vec![0, 0]);
        assert_eq!(recovered.original.arc_flows, vec![1, -1, 2]);
        assert_eq!(recovered.original.cost, 7);
    }

    #[test]
    fn records_a_supplied_certified_step_without_selecting_a_direction() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &network,
            &FractionalCirculation {
                arc_flows: vec![ExactRatio::new(1, 1).unwrap(); 2],
                cost: ExactRatio::new(1, 1).unwrap(),
            },
            ExactRatio::new(0, 1).unwrap(),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();
        let mut session = Backend.begin_iterations(snapshot).unwrap();
        let outcome = session
            .apply(
                &network,
                &iteration::Step {
                    approximate_gradients: vec![
                        ExactRatio::new(40, 1).unwrap(),
                        ExactRatio::new(0, 1).unwrap(),
                    ],
                    approximate_lengths: vec![ExactRatio::new(2, 1).unwrap(); 2],
                    kappa: ExactRatio::new(1, 2).unwrap(),
                    direction: vec![ExactRatio::new(-1, 1).unwrap(); 2],
                },
            )
            .unwrap();
        assert_eq!(outcome.eta, ExactRatio::new(1, 8_000).unwrap());
        assert_eq!(session.snapshot().update_metrics().iterations, 1);
        assert!(
            session
                .detect(ExactRatio::new(1, 1_000).unwrap())
                .unwrap()
                .is_empty()
        );
    }
}
