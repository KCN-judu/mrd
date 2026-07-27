use thiserror::Error;

use crate::{
    CirculationArcId, CirculationNetwork, ExactRatio, MinCostCirculationError, MinRatioEdgeId,
    StableMinRatioError, StableMinRatioLedger, StableUpdate,
};

/// One compactly encoded component of a signed circulation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompactCycleSegment {
    OffTreeEdge(CirculationArcId, i8),
    TreePath(Vec<(CirculationArcId, i8)>),
}

/// Compact signed cycle representation, decoded by the P7 static Oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactCycle {
    pub segments: Vec<CompactCycleSegment>,
}

impl CompactCycle {
    /// Decodes and validates a nonempty signed circulation.
    ///
    /// # Errors
    ///
    /// Returns an error when the decoded sequence is not a circulation.
    pub fn decode(
        &self,
        network: &CirculationNetwork,
    ) -> Result<Vec<(CirculationArcId, i8)>, DynamicMinRatioError> {
        let arcs = self
            .segments
            .iter()
            .flat_map(|segment| match segment {
                CompactCycleSegment::OffTreeEdge(id, direction) => vec![(*id, *direction)],
                CompactCycleSegment::TreePath(path) => path.clone(),
            })
            .collect::<Vec<_>>();
        network
            .validate_signed_circulation(&arcs)
            .map_err(DynamicMinRatioError::InvalidCycle)?;
        Ok(arcs)
    }
}

/// Replay counters for the baseline shifted single-branch chain.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TreeChainMetrics {
    pub shift_count: u64,
    pub rebuild_count: u64,
}

/// Deterministic state machine for Definition 5.9 and Definition 5.10.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShiftedTreeChain {
    branch_counts: Vec<usize>,
    shifts: Vec<usize>,
    metrics: TreeChainMetrics,
}

/// Replay-only composition of a shifted chain and the P8.1 stable ledger.
/// It deliberately performs no dynamic cycle search or approximation claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicMinRatioReplay {
    chain: ShiftedTreeChain,
    ledger: StableMinRatioLedger,
}

impl DynamicMinRatioReplay {
    /// Combines an already-validated stable ledger with a tree-chain trace.
    #[must_use]
    pub fn new(chain: ShiftedTreeChain, ledger: StableMinRatioLedger) -> Self {
        Self { chain, ledger }
    }

    /// Replays a checked P8.1 update.
    ///
    /// # Errors
    ///
    /// Returns the underlying stable-ledger validation error.
    pub fn update(&mut self, update: StableUpdate) -> Result<(), StableMinRatioError> {
        self.ledger.update(update)
    }

    /// Replays a checked P8.1 coordinate query.
    ///
    /// # Errors
    ///
    /// Returns the underlying stable-ledger validation error.
    pub fn query(&mut self, edge: MinRatioEdgeId) -> Result<ExactRatio, StableMinRatioError> {
        self.ledger.query(edge)
    }

    /// Replays a checked P8.1 detection operation.
    ///
    /// # Errors
    ///
    /// Returns the underlying stable-ledger validation error.
    pub fn detect(
        &mut self,
        epsilon: ExactRatio,
    ) -> Result<Vec<MinRatioEdgeId>, StableMinRatioError> {
        self.ledger.detect(epsilon)
    }

    #[must_use]
    pub const fn chain(&self) -> &ShiftedTreeChain {
        &self.chain
    }
}

impl ShiftedTreeChain {
    /// Initializes a chain with every shift index at zero.
    ///
    /// # Errors
    ///
    /// Returns an error when any level has zero branches.
    pub fn new(branch_counts: Vec<usize>) -> Result<Self, DynamicMinRatioError> {
        if branch_counts.contains(&0) {
            return Err(DynamicMinRatioError::InvalidChain);
        }
        Ok(Self {
            shifts: vec![0; branch_counts.len()],
            branch_counts,
            metrics: TreeChainMetrics::default(),
        })
    }

    /// Shifts one level and reinitializes every descendant shift index.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid level.
    pub fn shift(&mut self, level: usize) -> Result<(), DynamicMinRatioError> {
        let count = *self
            .branch_counts
            .get(level)
            .ok_or(DynamicMinRatioError::InvalidLevel)?;
        self.shifts[level] = (self.shifts[level] + 1) % count;
        self.shifts[level + 1..].fill(0);
        self.metrics.shift_count += 1;
        Ok(())
    }

    /// Rebuilds one level and every descendant.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid level.
    pub fn rebuild(&mut self, level: usize) -> Result<(), DynamicMinRatioError> {
        if level >= self.shifts.len() {
            return Err(DynamicMinRatioError::InvalidLevel);
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
    pub const fn metrics(&self) -> TreeChainMetrics {
        self.metrics
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum DynamicMinRatioError {
    #[error("tree chain branch count is invalid")]
    InvalidChain,
    #[error("tree chain level is invalid")]
    InvalidLevel,
    #[error("compact cycle is invalid: {0}")]
    InvalidCycle(MinCostCirculationError),
}

#[cfg(test)]
mod tests {
    use super::{CompactCycle, CompactCycleSegment, DynamicMinRatioReplay, ShiftedTreeChain};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, StableEdge, StableMinRatioLedger, StableUpdate,
        StableWitness,
    };

    #[test]
    fn compact_cycle_decodes_to_static_oracle_circulation() {
        let mut network = CirculationNetwork::new(2);
        let forward = network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 1, -1)
            .unwrap();
        let backward = network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        let cycle = CompactCycle {
            segments: vec![
                CompactCycleSegment::OffTreeEdge(forward, 1),
                CompactCycleSegment::TreePath(vec![(backward, 1)]),
            ],
        };
        assert_eq!(
            cycle.decode(&network).unwrap(),
            vec![(forward, 1), (backward, 1)]
        );
    }

    #[test]
    fn shifts_and_rebuilds_have_deterministic_replay() {
        let mut chain = ShiftedTreeChain::new(vec![2, 3]).unwrap();
        chain.shift(0).unwrap();
        chain.shift(1).unwrap();
        assert_eq!(chain.shifts(), &[1, 1]);
        chain.shift(0).unwrap();
        assert_eq!(chain.shifts(), &[0, 0]);
        chain.rebuild(1).unwrap();
        assert_eq!(chain.metrics().shift_count, 3);
        assert_eq!(chain.metrics().rebuild_count, 1);
    }

    #[test]
    fn replay_delegates_update_query_and_detect_to_stable_contract() {
        let ledger = StableMinRatioLedger::new(
            2,
            vec![
                StableEdge {
                    from: FlowNodeId(0),
                    to: FlowNodeId(1),
                    gradient: -2,
                    length: 1,
                },
                StableEdge {
                    from: FlowNodeId(1),
                    to: FlowNodeId(0),
                    gradient: 0,
                    length: 1,
                },
            ],
            ExactRatio::new(1, 4).unwrap(),
            ExactRatio::new(1, 2).unwrap(),
            StableWitness {
                circulation: vec![1, 1],
                upper_bounds: vec![1, 1],
            },
        )
        .unwrap();
        let mut replay =
            DynamicMinRatioReplay::new(ShiftedTreeChain::new(vec![2]).unwrap(), ledger);
        replay
            .update(StableUpdate {
                changed_edges: Vec::new(),
                direction: vec![1, 1],
                eta: 2,
                witness: StableWitness {
                    circulation: vec![1, 1],
                    upper_bounds: vec![1, 1],
                },
            })
            .unwrap();
        assert_eq!(
            replay.query(crate::MinRatioEdgeId(0)).unwrap(),
            ExactRatio::new(1, 1).unwrap()
        );
        assert_eq!(
            replay.detect(ExactRatio::new(1, 1).unwrap()).unwrap().len(),
            2
        );
    }
}
