//! Checked execution of externally supplied IPM directions.
//!
//! This module isolates the pure state transition and Detect accounting from
//! the still-missing direction-selection construction. It is intentionally not
//! a solver: callers must supply an exact direction and both certified
//! approximations for every step.

use thiserror::Error;

use crate::{
    CertifiedIpmError, CertifiedIpmSnapshot, CirculationNetwork, ExactRatio,
    IpmApproximationCertificate, IpmDetectLedger, IpmUpdateMetrics, MinRatioEdgeId,
};

/// One immutable, externally supplied Lemma 4.4 update request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step {
    /// Certified approximate gradients for the current snapshot.
    pub approximate_gradients: Vec<ExactRatio>,
    /// Certified approximate lengths for the current snapshot.
    pub approximate_lengths: Vec<ExactRatio>,
    /// Source update-quality parameter.
    pub kappa: ExactRatio,
    /// Exact circulation direction selected by an external construction.
    pub direction: Vec<ExactRatio>,
}

/// Observable result of one accepted transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Outcome {
    /// Certified source step size.
    pub eta: ExactRatio,
    /// Approximation checks accepted before the transition.
    pub approximation: IpmApproximationCertificate,
    /// Cumulative snapshot update metrics after the transition.
    pub metrics: IpmUpdateMetrics,
}

/// Session-local IPM state and explicit Detect accounting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Session {
    snapshot: CertifiedIpmSnapshot,
    detect: IpmDetectLedger,
}

impl Session {
    /// Creates a session from an already certified snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot's bounded fixed-point configuration
    /// cannot initialize the Detect ledger.
    pub fn new(snapshot: CertifiedIpmSnapshot) -> Result<Self, Error> {
        let detect = IpmDetectLedger::new(&snapshot)?;
        Ok(Self { snapshot, detect })
    }

    /// Returns the current immutable certified snapshot.
    #[must_use]
    pub const fn snapshot(&self) -> &CertifiedIpmSnapshot {
        &self.snapshot
    }

    /// Applies one externally supplied direction after complete local checks.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied step fails any Lemma 4.4 condition
    /// or Detect accounting cannot record the accepted transition.
    pub fn apply(&mut self, network: &CirculationNetwork, step: &Step) -> Result<Outcome, Error> {
        let update = self.snapshot.apply_lemma_44_update(
            network,
            &step.approximate_gradients,
            &step.approximate_lengths,
            step.kappa,
            &step.direction,
        )?;
        self.detect
            .record_update(&self.snapshot, update.eta, &step.direction)?;
        self.snapshot = update.next_snapshot;
        Ok(Outcome {
            eta: update.eta,
            approximation: update.approximation,
            metrics: self.snapshot.update_metrics(),
        })
    }

    /// Runs the exact source Detect threshold over accumulated updates.
    ///
    /// # Errors
    ///
    /// Returns an error when the threshold is invalid or cannot be certified.
    pub fn detect(&mut self, epsilon: ExactRatio) -> Result<Vec<MinRatioEdgeId>, Error> {
        Ok(self.detect.detect(epsilon)?)
    }

    /// Returns the observed Detect counters for this session.
    #[must_use]
    pub const fn detect_metrics(&self) -> IpmUpdateMetrics {
        self.detect.metrics()
    }
}

/// One supplied iteration step could not be certified or accounted for.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Ipm(#[from] CertifiedIpmError),
}
