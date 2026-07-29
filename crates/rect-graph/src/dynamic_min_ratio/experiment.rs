use crate::{
    CirculationNetwork, ExactRatio, MinRatioEdgeId, StableMinRatioError, StableMinRatioLedger,
    StableUpdate,
};

use super::{AuditMetrics, Cycle, Error, Query, UnsupportedOperation, oracle};

/// Replay counters for the baseline shifted single-branch chain.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChainMetrics {
    pub shift_count: u64,
    pub rebuild_count: u64,
}

/// Deterministic state machine for Definition 5.9 and Definition 5.10.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeChain {
    branch_counts: Vec<usize>,
    shifts: Vec<usize>,
    metrics: ChainMetrics,
}

/// Replay-only composition of a shifted chain and the stable ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Replay {
    chain: TreeChain,
    ledger: StableMinRatioLedger,
}

impl Replay {
    #[must_use]
    pub fn new(chain: TreeChain, ledger: StableMinRatioLedger) -> Self {
        Self { chain, ledger }
    }

    /// # Errors
    /// Returns the underlying stable-ledger validation error.
    pub fn update(&mut self, update: StableUpdate) -> Result<(), StableMinRatioError> {
        self.ledger.update(update)
    }

    /// # Errors
    /// Returns the underlying stable-ledger validation error.
    pub fn query(&mut self, edge: MinRatioEdgeId) -> Result<ExactRatio, StableMinRatioError> {
        self.ledger.query(edge)
    }

    /// # Errors
    /// Returns the underlying stable-ledger validation error.
    pub fn detect(
        &mut self,
        epsilon: ExactRatio,
    ) -> Result<Vec<MinRatioEdgeId>, StableMinRatioError> {
        self.ledger.detect(epsilon)
    }

    /// # Errors
    /// Returns an error when exact ratio arithmetic overflows.
    pub fn minimum_ratio_cycle(&self) -> Result<Option<Query>, Error> {
        self.minimum_ratio_cycle_with_work().map(|(query, _)| query)
    }

    /// # Errors
    /// Returns an error when exact ratio arithmetic overflows.
    pub fn minimum_ratio_cycle_with_work(&self) -> Result<(Option<Query>, u64), Error> {
        oracle::minimum_ratio_cycle(self.ledger.edges())
    }

    /// # Errors
    /// Returns an error for an invalid tree-chain level.
    pub fn shift(&mut self, level: usize) -> Result<(), Error> {
        self.chain.shift(level)
    }

    /// # Errors
    /// Returns an error for an invalid tree-chain level.
    pub fn rebuild(&mut self, level: usize) -> Result<(), Error> {
        self.chain.rebuild(level)
    }

    #[must_use]
    pub const fn chain(&self) -> &TreeChain {
        &self.chain
    }
}

impl TreeChain {
    /// # Errors
    /// Returns an error when any level has zero branches.
    pub fn new(branch_counts: Vec<usize>) -> Result<Self, Error> {
        if branch_counts.contains(&0) {
            return Err(Error::InvalidChain);
        }
        Ok(Self {
            shifts: vec![0; branch_counts.len()],
            branch_counts,
            metrics: ChainMetrics::default(),
        })
    }

    /// # Errors
    /// Returns an error for an invalid level.
    pub fn shift(&mut self, level: usize) -> Result<(), Error> {
        let count = *self.branch_counts.get(level).ok_or(Error::InvalidLevel)?;
        self.shifts[level] = (self.shifts[level] + 1) % count;
        self.shifts[level + 1..].fill(0);
        self.metrics.shift_count += 1;
        Ok(())
    }

    /// # Errors
    /// Returns an error for an invalid level.
    pub fn rebuild(&mut self, level: usize) -> Result<(), Error> {
        if level >= self.shifts.len() {
            return Err(Error::InvalidLevel);
        }
        self.shifts[level..].fill(0);
        self.metrics.rebuild_count += 1;
        Ok(())
    }

    #[must_use]
    pub fn shifts(&self) -> &[usize] {
        &self.shifts
    }

    #[must_use]
    pub const fn metrics(&self) -> ChainMetrics {
        self.metrics
    }
}

/// Checked deterministic replay component for the current experimental layers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Audit {
    replay: Replay,
    metrics: AuditMetrics,
}

impl Audit {
    #[must_use]
    pub fn new(replay: Replay) -> Self {
        Self {
            replay,
            metrics: AuditMetrics::default(),
        }
    }

    /// # Errors
    /// Returns an error when the compact encoding is not a circulation.
    pub fn verify_cycle(
        &mut self,
        cycle: &Cycle,
        network: &CirculationNetwork,
    ) -> Result<(), Error> {
        cycle.decode(network)?;
        self.metrics.compact_cycle_checks += 1;
        Ok(())
    }

    /// # Errors
    /// Always returns an explicit unsupported-operation error.
    pub fn reject_unsupported(&mut self, _: UnsupportedOperation) -> Result<(), Error> {
        self.metrics.rejected_operations += 1;
        Err(Error::UnsupportedOperation)
    }

    /// # Errors
    /// Returns an error when exact arithmetic overflows.
    pub fn query_best_cycle(&mut self) -> Result<Option<Query>, Error> {
        let (query, candidates) = self.replay.minimum_ratio_cycle_with_work()?;
        self.metrics.exact_cycle_queries = self
            .metrics
            .exact_cycle_queries
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        self.metrics.enumerated_cycle_candidates = self
            .metrics
            .enumerated_cycle_candidates
            .checked_add(candidates)
            .ok_or(Error::Overflow)?;
        Ok(query)
    }

    #[must_use]
    pub const fn metrics(&self) -> AuditMetrics {
        self.metrics
    }

    #[must_use]
    pub const fn replay(&self) -> &Replay {
        &self.replay
    }
}
