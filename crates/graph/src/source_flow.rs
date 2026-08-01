//! Experimental source-shaped flow backend boundary.
//!
//! This module intentionally has no reference max-flow or enumerating
//! min-cost-cycle dependency. It cannot be enabled until P9.5 supplies the
//! complete certified iteration path and compressed-network integration.

use thiserror::Error;

use crate::{
    CertifiedIpmError, CertifiedIpmInitialPoint, CertifiedIpmSnapshot,
    CertifiedLowerBoundInitialPoint, CirculationNetwork, CostedFlowRoundingResult, ExactRatio,
    FixedPointConfig, InitialPointAugmentation, IpmTerminationCertificate, MinCostCirculationError,
    MinCostSolution,
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
        let (termination, rounding) = self.recover_terminal_rounding(snapshot, network)?;
        let optimal = snapshot.optimal_cost();
        if !optimal.is_integral() || optimal.numerator_i128().ok() != Some(rounding.solution.cost) {
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

    /// Recovers a feasible integral flow certified to meet an inclusive target.
    ///
    /// This is the positive side of CKLPPS22's target-decision contract: a
    /// completed run may return a flow whose original integral cost is at most
    /// the supplied target. It deliberately does not classify a failed run as
    /// evidence that no such flow exists.
    ///
    /// Unlike [`Self::recover_terminated`], the snapshot's retained objective
    /// may be an upper target rather than the recovered flow's exact optimum.
    /// The strict recovery method remains available for paths that know the
    /// exact optimum.
    ///
    /// # Errors
    ///
    /// Returns an error when termination or recovery fails, or the integral
    /// rounded cost exceeds `target`.
    pub fn recover_terminated_at_most(
        self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
        target: i128,
    ) -> Result<RecoveredFlow, Error> {
        let (termination, rounding) = self.recover_terminal_rounding(snapshot, network)?;
        if rounding.solution.cost > target {
            return Err(Error::TargetNotMet {
                target,
                actual: rounding.solution.cost,
            });
        }
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

    /// Recovers an augmented terminal flow that satisfies an inclusive original
    /// cost target without asserting that the target is the exact optimum.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal recovery fails, an artificial arc carries
    /// flow, or the recovered original cost exceeds `target`.
    pub fn recover_augmented_terminated_at_most(
        self,
        snapshot: &CertifiedIpmSnapshot,
        augmentation: &InitialPointAugmentation,
        target: i128,
    ) -> Result<RecoveredAugmentedFlow, Error> {
        let terminal = self.recover_terminated_at_most(snapshot, &augmentation.network, target)?;
        let original = augmentation
            .recover_original_feasible(&terminal.rounding.solution)
            .map_err(Error::Augmentation)?;
        if original.cost > target {
            return Err(Error::TargetNotMet {
                target,
                actual: original.cost,
            });
        }
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

    /// Constructs the Appendix B.1 initial point for one caller-supplied
    /// integral target and starts its source-selected driver.
    ///
    /// The target is a checked input, not an optimum query: this boundary does
    /// not infer it from a reference solver, a lower bound, or a terminating
    /// run. A completed run may return an integral original flow with cost at
    /// most this target. An incorrect target may otherwise fail initial
    /// certification, source iteration, or terminal recovery explicitly; those
    /// failures do not classify a target for binary search.
    ///
    /// # Errors
    ///
    /// Returns an error when Appendix B.1 cannot construct a strict certified
    /// initial point for the supplied target, the potential budget cannot be
    /// certified, or the source driver cannot initialize its Detect ledger.
    pub fn begin_with_target<F: iteration::Factory>(
        self,
        network: &CirculationNetwork,
        target: i128,
        maximum_abs_input: i128,
        fixed_point_config: FixedPointConfig,
        kappa: ExactRatio,
        factory: F,
    ) -> Result<TargetDriver<F>, Error> {
        let exact_target = ExactRatio::new(target, 1).map_err(|_| Error::InvalidTarget)?;
        let initial = CertifiedIpmSnapshot::initial_point_augmented(
            network,
            exact_target,
            maximum_abs_input,
            fixed_point_config,
        )
        .map_err(Error::Ipm)?;
        let budget = iteration::PotentialBudget::new(
            &initial.snapshot,
            &initial.augmentation.network,
            kappa,
        )
        .map_err(Error::Iteration)?;
        let driver = self.begin_source_iterations(initial.snapshot.clone(), factory, 0)?;
        Ok(TargetDriver {
            target,
            initial,
            budget,
            driver,
        })
    }

    fn recover_terminal_rounding(
        self,
        snapshot: &CertifiedIpmSnapshot,
        network: &CirculationNetwork,
    ) -> Result<(IpmTerminationCertificate, CostedFlowRoundingResult), Error> {
        let termination = self.certify_termination(snapshot, network)?;
        let rounding = recovery::round(network, snapshot.flow()).map_err(Error::Recovery)?;
        Ok((termination, rounding))
    }
}

/// A source driver bound to one Appendix B.1 initial point and integral target.
///
/// The driver owns no target-search policy. It can use only the augmented
/// network and snapshot certified at construction time.
#[derive(Debug)]
pub struct TargetDriver<F> {
    target: i128,
    initial: CertifiedIpmInitialPoint,
    budget: iteration::PotentialBudget,
    driver: iteration::Driver<F>,
}

impl<F> TargetDriver<F> {
    /// Returns the caller-supplied inclusive integral target.
    #[must_use]
    pub const fn target(&self) -> i128 {
        self.target
    }

    /// Returns the immutable certified initial-point augmentation.
    #[must_use]
    pub const fn initial(&self) -> &CertifiedIpmInitialPoint {
        &self.initial
    }

    /// Returns the conditional source-progress budget for this exact initial state.
    #[must_use]
    pub const fn budget(&self) -> &iteration::PotentialBudget {
        &self.budget
    }

    /// Returns the source-selected driver bound to the augmented network.
    #[must_use]
    pub const fn driver(&self) -> &iteration::Driver<F> {
        &self.driver
    }
}

impl<F: iteration::Factory> TargetDriver<F> {
    /// Runs the target-bound source driver to additive-half termination and
    /// recovers the original circulation through its initial augmentation.
    ///
    /// # Errors
    ///
    /// Returns an error when a source projection fails, the budget does not
    /// reach termination, recovery fails, or the recovered original cost
    /// exceeds the caller-supplied target.
    pub fn run(&mut self) -> Result<TargetRun, Error> {
        let completion = self
            .driver
            .run_with_potential_budget(&self.initial.augmentation.network, &self.budget)
            .map_err(Error::Iteration)?;
        let recovered = Backend.recover_augmented_terminated_at_most(
            self.driver.session().snapshot(),
            &self.initial.augmentation,
            self.target,
        )?;
        Ok(TargetRun {
            target: self.target,
            completion,
            recovered,
        })
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

/// One completed source run bound to a caller-provided integral target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetRun {
    pub target: i128,
    pub completion: iteration::Completion,
    pub recovered: RecoveredAugmentedFlow,
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
    /// The supplied integer target cannot be represented as an exact ratio.
    #[error("caller-supplied inclusive target is invalid")]
    InvalidTarget,
    /// Terminal recovery did not meet the caller-supplied inclusive target.
    #[error("recovered original cost {actual} exceeds supplied target {target}")]
    TargetNotMet { target: i128, actual: i128 },
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
                arc_flows: vec![quarter.clone(); 2],
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

    fn exact_target_network() -> CirculationNetwork {
        let mut network = CirculationNetwork::new(2);
        network.set_demand(FlowNodeId(0), -1).unwrap();
        network.set_demand(FlowNodeId(1), 1).unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        network
    }

    #[test]
    fn binds_an_augmented_source_driver_to_a_caller_supplied_target() {
        let network = exact_target_network();
        let expected_augmented = network.initial_point_augmentation(2).unwrap().network;
        let factory = move |snapshot: &CertifiedIpmSnapshot, active: &CirculationNetwork| {
            assert_eq!(active, &expected_augmented);
            assert_eq!(snapshot.optimal_cost(), ExactRatio::new(1, 1).unwrap());
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        };
        let mut driver = Backend
            .begin_with_target(
                &network,
                1,
                2,
                FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
                ExactRatio::new(1, 2).unwrap(),
                factory,
            )
            .unwrap();

        assert_eq!(driver.target(), 1);
        assert_eq!(
            driver.initial().snapshot.optimal_cost(),
            ExactRatio::new(1, 1).unwrap()
        );
        assert!(driver.budget().maximum_updates() > 0);
        assert_eq!(
            driver.run(),
            Err(Error::Iteration(iteration::Error::NoSourceCandidate))
        );
        assert_eq!(Backend.require_complete(), Err(Error::Incomplete));
    }

    #[test]
    fn rejects_a_target_that_does_not_leave_the_augmented_initial_point_strict() {
        let network = exact_target_network();
        let augmentation = network.initial_point_augmentation(2).unwrap();
        assert!(augmentation.initial_flow.cost.is_integral());
        let invalid_target = augmentation.initial_flow.cost.numerator_i128().unwrap();
        let factory = |_: &CertifiedIpmSnapshot, _: &CirculationNetwork| {
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        };

        assert!(matches!(
            Backend.begin_with_target(
                &network,
                invalid_target,
                2,
                FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
                ExactRatio::new(1, 2).unwrap(),
                factory,
            ),
            Err(Error::Ipm(CertifiedIpmError::InvalidSourceDomain))
        ));
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
                arc_flows: vec![quarter.clone(); 2],
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
                arc_flows: vec![quarter.clone(); 2],
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
    fn target_recovery_accepts_an_original_cost_below_the_target() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let config = FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap();
        let augmentation = network.initial_point_augmentation(4).unwrap();
        let quarter = ExactRatio::new(1, 4).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &augmentation.network,
            &FractionalCirculation {
                arc_flows: vec![quarter.clone(); 2],
                cost: quarter,
            },
            ExactRatio::new(0, 1).unwrap(),
            augmentation.maximum_abs_input,
            config,
        )
        .unwrap();

        let strict = Backend
            .recover_augmented_terminated(&snapshot, &augmentation)
            .unwrap();
        assert_eq!(strict.original.cost, 0);
        let recovered = Backend
            .recover_augmented_terminated_at_most(&snapshot, &augmentation, 1)
            .unwrap();
        assert_eq!(recovered.original.cost, 0);
        let at_target = Backend
            .recover_augmented_terminated_at_most(&snapshot, &augmentation, 0)
            .unwrap();
        assert_eq!(at_target.original.cost, 0);
        assert_eq!(
            Backend.recover_augmented_terminated_at_most(&snapshot, &augmentation, -1),
            Err(Error::TargetNotMet {
                target: -1,
                actual: 0
            })
        );
        assert_eq!(Backend.require_complete(), Err(Error::Incomplete));
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
        let scale = ExactRatio::from_bigints(
            initial_flow.cost.denominator().clone(),
            initial_flow.cost.numerator() * 8,
        )
        .unwrap();
        let arc_flows = initial_flow
            .arc_flows
            .iter()
            .cloned()
            .map(|flow| flow.checked_mul(&scale).unwrap())
            .collect();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &initial.initial_point.augmentation.network,
            &FractionalCirculation {
                arc_flows,
                cost: initial_flow.cost.checked_mul(&scale).unwrap(),
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
