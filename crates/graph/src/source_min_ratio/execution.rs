//! Explicit finite execution and accounting over the checked stability ledger.

use crate::{ExactRatio, MinRatioEdgeId, StableMinRatioError, StableMinRatioLedger, StableUpdate};

/// Exact operation counters; they are observations, not amortized bounds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Accounting {
    /// Accepted checked updates.
    pub updates: u64,
    /// Public coordinate queries.
    pub queries: u64,
    /// Detection calls.
    pub detects: u64,
    /// Explicitly rejected unsupported requests.
    pub rejected: u64,
}

/// Unsupported P9.4d execution features remain explicit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unsupported {
    /// General dynamic sparsification maintenance.
    DynamicSparsification,
    /// Link-cut tree maintenance.
    LinkCut,
}

/// Thin stateful adapter around the independently checked ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Executor {
    ledger: StableMinRatioLedger,
    accounting: Accounting,
}

impl Executor {
    /// Creates an executor over an already validated hidden-stability state.
    #[must_use]
    pub const fn new(ledger: StableMinRatioLedger) -> Self {
        Self {
            ledger,
            accounting: Accounting {
                updates: 0,
                queries: 0,
                detects: 0,
                rejected: 0,
            },
        }
    }

    /// Applies one checked update and records it after success.
    ///
    /// # Errors
    ///
    /// Returns the ledger's validation error without incrementing accounting.
    pub fn update(&mut self, update: StableUpdate) -> Result<(), StableMinRatioError> {
        self.ledger.update(update)?;
        self.accounting.updates += 1;
        Ok(())
    }

    /// Reads one exact coordinate through the checked ledger.
    ///
    /// # Errors
    ///
    /// Returns the ledger's missing-coordinate error without incrementing accounting.
    pub fn query(&mut self, edge: MinRatioEdgeId) -> Result<ExactRatio, StableMinRatioError> {
        let value = self.ledger.query(edge)?;
        self.accounting.queries += 1;
        Ok(value)
    }

    /// Runs the checked detection transition.
    ///
    /// # Errors
    ///
    /// Returns the ledger's invalid-threshold or overflow error without incrementing accounting.
    pub fn detect(
        &mut self,
        epsilon: ExactRatio,
    ) -> Result<Vec<MinRatioEdgeId>, StableMinRatioError> {
        let result = self.ledger.detect(epsilon)?;
        self.accounting.detects += 1;
        Ok(result)
    }

    /// Rejects a source-grade dynamic operation that is not implemented here.
    pub fn reject(&mut self, _: Unsupported) {
        self.accounting.rejected += 1;
    }

    /// Returns observed finite execution counters.
    #[must_use]
    pub const fn accounting(&self) -> Accounting {
        self.accounting
    }
}

#[cfg(test)]
mod tests {
    use super::{Executor, Unsupported};
    use crate::{
        ExactRatio, FlowNodeId, MinRatioEdgeId, StableEdge, StableMinRatioLedger, StableWitness,
    };

    #[test]
    fn accounts_for_public_operations_and_explicit_rejections() {
        let ledger = StableMinRatioLedger::new(
            2,
            vec![
                StableEdge {
                    from: FlowNodeId(0),
                    to: FlowNodeId(1),
                    gradient: -1,
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
        let mut executor = Executor::new(ledger);
        assert_eq!(
            executor.query(MinRatioEdgeId(0)).unwrap(),
            ExactRatio::new(0, 1).unwrap()
        );
        assert!(
            executor
                .detect(ExactRatio::new(1, 1).unwrap())
                .unwrap()
                .is_empty()
        );
        executor.reject(Unsupported::DynamicSparsification);
        executor.reject(Unsupported::LinkCut);
        assert_eq!(executor.accounting().queries, 1);
        assert_eq!(executor.accounting().detects, 1);
        assert_eq!(executor.accounting().rejected, 2);
    }
}
