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

    /// Computes the exact current minimum-ratio signed simple cycle by
    /// exhaustive enumeration. This is an auditable dynamic-query Oracle, not
    /// the source's subpolynomial dynamic data structure.
    ///
    /// # Errors
    ///
    /// Returns an error when exact ratio arithmetic overflows.
    pub fn minimum_ratio_cycle(&self) -> Result<Option<DynamicCycleQuery>, DynamicMinRatioError> {
        self.minimum_ratio_cycle_with_work().map(|(query, _)| query)
    }

    /// Runs the exact query and returns its explicit simple-cycle candidate
    /// count for audit accounting.
    ///
    /// # Errors
    ///
    /// Returns an error when exact ratio arithmetic overflows.
    pub fn minimum_ratio_cycle_with_work(
        &self,
    ) -> Result<(Option<DynamicCycleQuery>, u64), DynamicMinRatioError> {
        minimum_ratio_cycle(self.ledger.edges())
    }

    /// Replays one deterministic shift.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid tree-chain level.
    pub fn shift(&mut self, level: usize) -> Result<(), DynamicMinRatioError> {
        self.chain.shift(level)
    }

    /// Replays one deterministic rebuild.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid tree-chain level.
    pub fn rebuild(&mut self, level: usize) -> Result<(), DynamicMinRatioError> {
        self.chain.rebuild(level)
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

/// Explicitly unsupported operations outside the P8 certificate domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedDynamicOperation {
    EdgeInsertion,
    DirectedEdge,
    ArbitraryTopologyUpdate,
}

/// Exact replay/audit work counters for the P8.6 integration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DynamicAuditMetrics {
    pub compact_cycle_checks: u64,
    pub rejected_operations: u64,
    pub exact_cycle_queries: u64,
    pub enumerated_cycle_candidates: u64,
}

/// Exact signed simple-cycle result for the current stable-ledger coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicCycleQuery {
    pub cycle: Vec<(MinRatioEdgeId, i8)>,
    pub gradient_sum: i128,
    pub length_sum: i128,
    pub ratio: ExactRatio,
}

/// Integrates P8.1 and P8.5 only as a checked deterministic replay component.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicMinRatioAudit {
    replay: DynamicMinRatioReplay,
    metrics: DynamicAuditMetrics,
}

impl DynamicMinRatioAudit {
    #[must_use]
    pub fn new(replay: DynamicMinRatioReplay) -> Self {
        Self {
            replay,
            metrics: DynamicAuditMetrics::default(),
        }
    }

    /// Validates a compact cycle with P7's exact static circulation Oracle.
    ///
    /// # Errors
    ///
    /// Returns an error when the compact encoding is not a circulation.
    pub fn verify_cycle(
        &mut self,
        cycle: &CompactCycle,
        network: &CirculationNetwork,
    ) -> Result<(), DynamicMinRatioError> {
        cycle.decode(network)?;
        self.metrics.compact_cycle_checks += 1;
        Ok(())
    }

    /// Rejects operations not admitted by P8's scoped certificate layers.
    ///
    /// # Errors
    ///
    /// Always returns an explicit unsupported-operation error.
    pub fn reject_unsupported(
        &mut self,
        _: UnsupportedDynamicOperation,
    ) -> Result<(), DynamicMinRatioError> {
        self.metrics.rejected_operations += 1;
        Err(DynamicMinRatioError::UnsupportedOperation)
    }

    /// Runs the exact enumerating query and records its candidate work.
    ///
    /// # Errors
    ///
    /// Returns an error when exact arithmetic overflows.
    pub fn query_best_cycle(&mut self) -> Result<Option<DynamicCycleQuery>, DynamicMinRatioError> {
        let (query, candidates) = self.replay.minimum_ratio_cycle_with_work()?;
        self.metrics.exact_cycle_queries = self
            .metrics
            .exact_cycle_queries
            .checked_add(1)
            .ok_or(DynamicMinRatioError::Overflow)?;
        self.metrics.enumerated_cycle_candidates = self
            .metrics
            .enumerated_cycle_candidates
            .checked_add(candidates)
            .ok_or(DynamicMinRatioError::Overflow)?;
        Ok(query)
    }

    #[must_use]
    pub const fn metrics(&self) -> DynamicAuditMetrics {
        self.metrics
    }

    #[must_use]
    pub const fn replay(&self) -> &DynamicMinRatioReplay {
        &self.replay
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
    #[error("operation is outside the checked P8 dynamic domain")]
    UnsupportedOperation,
    #[error("exact dynamic-query arithmetic overflowed")]
    Overflow,
}

fn minimum_ratio_cycle(
    edges: &[crate::StableEdge],
) -> Result<(Option<DynamicCycleQuery>, u64), DynamicMinRatioError> {
    let node_count = edges
        .iter()
        .flat_map(|edge| [edge.from.0, edge.to.0])
        .max()
        .map_or(0, |node| node + 1);
    let mut adjacency = vec![Vec::<(usize, MinRatioEdgeId, i8)>::new(); node_count];
    for (index, edge) in edges.iter().enumerate() {
        let id = MinRatioEdgeId(index);
        adjacency[edge.from.0].push((edge.to.0, id, 1));
        adjacency[edge.to.0].push((edge.from.0, id, -1));
    }
    let mut best = None;
    let mut candidates = 0_u64;
    for start in 0..node_count {
        let mut seen = vec![false; node_count];
        seen[start] = true;
        enumerate_cycles(
            edges,
            &adjacency,
            start,
            start,
            &mut seen,
            &mut Vec::new(),
            0,
            0,
            &mut best,
            &mut candidates,
        )?;
    }
    Ok((best, candidates))
}

#[allow(clippy::too_many_arguments)]
fn enumerate_cycles(
    edges: &[crate::StableEdge],
    adjacency: &[Vec<(usize, MinRatioEdgeId, i8)>],
    start: usize,
    node: usize,
    seen: &mut [bool],
    path: &mut Vec<(MinRatioEdgeId, i8)>,
    gradient: i128,
    length: i128,
    best: &mut Option<DynamicCycleQuery>,
    candidates: &mut u64,
) -> Result<(), DynamicMinRatioError> {
    for (next, id, direction) in &adjacency[node] {
        if path.iter().any(|(previous, _)| previous == id) {
            continue;
        }
        let edge = edges.get(id.0).ok_or(DynamicMinRatioError::Overflow)?;
        let signed_gradient = edge
            .gradient
            .checked_mul(i128::from(*direction))
            .ok_or(DynamicMinRatioError::Overflow)?;
        let next_gradient = gradient
            .checked_add(signed_gradient)
            .ok_or(DynamicMinRatioError::Overflow)?;
        let next_length = length
            .checked_add(edge.length)
            .ok_or(DynamicMinRatioError::Overflow)?;
        if *next == start {
            *candidates = candidates
                .checked_add(1)
                .ok_or(DynamicMinRatioError::Overflow)?;
            let mut cycle = path.clone();
            cycle.push((*id, *direction));
            let ratio = ExactRatio::new(next_gradient, next_length)
                .map_err(|_| DynamicMinRatioError::Overflow)?;
            let candidate = DynamicCycleQuery {
                cycle,
                gradient_sum: next_gradient,
                length_sum: next_length,
                ratio,
            };
            let replace = match best.as_ref() {
                None => true,
                Some(current) => {
                    current
                        .ratio
                        .at_least(candidate.ratio)
                        .map_err(|_| DynamicMinRatioError::Overflow)?
                        && current.ratio != candidate.ratio
                }
            };
            if replace {
                *best = Some(candidate);
            }
        } else if !seen[*next] {
            seen[*next] = true;
            path.push((*id, *direction));
            enumerate_cycles(
                edges,
                adjacency,
                start,
                *next,
                seen,
                path,
                next_gradient,
                next_length,
                best,
                candidates,
            )?;
            path.pop();
            seen[*next] = false;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CompactCycle, CompactCycleSegment, DynamicMinRatioAudit, DynamicMinRatioError,
        DynamicMinRatioReplay, ShiftedTreeChain, UnsupportedDynamicOperation,
    };
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
        let replay = DynamicMinRatioReplay::new(ShiftedTreeChain::new(vec![1]).unwrap(), ledger);
        let mut audit = DynamicMinRatioAudit::new(replay);
        let mut network = CirculationNetwork::new(2);
        let forward = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let backward = network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        audit
            .verify_cycle(
                &CompactCycle {
                    segments: vec![
                        CompactCycleSegment::OffTreeEdge(forward, 1),
                        CompactCycleSegment::OffTreeEdge(backward, 1),
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
            audit.reject_unsupported(UnsupportedDynamicOperation::EdgeInsertion),
            Err(DynamicMinRatioError::UnsupportedOperation)
        );
        assert_eq!(audit.metrics().compact_cycle_checks, 1);
        assert_eq!(audit.metrics().rejected_operations, 1);
        assert_eq!(audit.metrics().exact_cycle_queries, 1);
        assert!(audit.metrics().enumerated_cycle_candidates >= 1);
    }
}
