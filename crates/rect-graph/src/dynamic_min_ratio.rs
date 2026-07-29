use thiserror::Error;

use crate::{
    CirculationArcId, CirculationNetwork, ExactRatio, MinCostCirculationError, MinRatioEdgeId,
};

pub mod experiment;
pub mod oracle;

/// One compactly encoded component of a signed circulation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Segment {
    OffTreeEdge(CirculationArcId, i8),
    TreePath(Vec<(CirculationArcId, i8)>),
}

/// Compact signed cycle representation, decoded by the P7 static Oracle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cycle {
    pub segments: Vec<Segment>,
}

impl Cycle {
    /// Decodes and validates a nonempty signed circulation.
    ///
    /// # Errors
    ///
    /// Returns an error when the decoded sequence is not a circulation.
    pub fn decode(
        &self,
        network: &CirculationNetwork,
    ) -> Result<Vec<(CirculationArcId, i8)>, Error> {
        let arcs = self
            .segments
            .iter()
            .flat_map(|segment| match segment {
                Segment::OffTreeEdge(id, direction) => vec![(*id, *direction)],
                Segment::TreePath(path) => path.clone(),
            })
            .collect::<Vec<_>>();
        network
            .validate_signed_circulation(&arcs)
            .map_err(Error::InvalidCycle)?;
        Ok(arcs)
    }
}

/// Explicitly unsupported operations outside the P8 certificate domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedOperation {
    EdgeInsertion,
    DirectedEdge,
    ArbitraryTopologyUpdate,
}

/// Exact replay/audit work counters for the P8.6 integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuditMetrics {
    pub compact_cycle_checks: u64,
    pub rejected_operations: u64,
    pub exact_cycle_queries: u64,
    pub enumerated_cycle_candidates: u64,
}

/// Exact signed simple-cycle result for the current stable-ledger coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Query {
    pub cycle: Vec<(MinRatioEdgeId, i8)>,
    pub gradient_sum: i128,
    pub length_sum: i128,
    pub ratio: ExactRatio,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("tree chain branch count is invalid")]
    InvalidChain,
    #[error("tree chain level is invalid")]
    InvalidLevel,
    #[error("compact cycle is invalid: {0}")]
    InvalidCycle(MinCostCirculationError),
    #[error("operation is outside the checked P8 dynamic domain")]
    UnsupportedOperation,
    #[error("exact dynamic-query arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::experiment::{Audit, Replay, TreeChain};
    use super::{Cycle, Error, Segment, UnsupportedOperation};
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
        let cycle = Cycle {
            segments: vec![
                Segment::OffTreeEdge(forward, 1),
                Segment::TreePath(vec![(backward, 1)]),
            ],
        };
        assert_eq!(
            cycle.decode(&network).unwrap(),
            vec![(forward, 1), (backward, 1)]
        );
    }

    #[test]
    fn shifts_and_rebuilds_have_deterministic_replay() {
        let mut chain = TreeChain::new(vec![2, 3]).unwrap();
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
        let mut replay = Replay::new(TreeChain::new(vec![2]).unwrap(), ledger);
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

    #[test]
    fn integration_audit_counts_static_checks_and_rejects_scope_expansion() {
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
        let replay = Replay::new(TreeChain::new(vec![1]).unwrap(), ledger);
        let mut audit = Audit::new(replay);
        let mut network = CirculationNetwork::new(2);
        let forward = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let backward = network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        audit
            .verify_cycle(
                &Cycle {
                    segments: vec![
                        Segment::OffTreeEdge(forward, 1),
                        Segment::OffTreeEdge(backward, 1),
                    ],
                },
                &network,
            )
            .unwrap();
        let query = audit.query_best_cycle().unwrap().unwrap();
        assert_eq!(query.gradient_sum, -2);
        assert_eq!(query.length_sum, 2);
        assert_eq!(query.ratio, ExactRatio::new(-1, 1).unwrap());
        assert_eq!(
            audit.reject_unsupported(UnsupportedOperation::EdgeInsertion),
            Err(Error::UnsupportedOperation)
        );
        assert_eq!(audit.metrics().compact_cycle_checks, 1);
        assert_eq!(audit.metrics().rejected_operations, 1);
        assert_eq!(audit.metrics().exact_cycle_queries, 1);
        assert!(audit.metrics().enumerated_cycle_candidates >= 1);
    }
}
