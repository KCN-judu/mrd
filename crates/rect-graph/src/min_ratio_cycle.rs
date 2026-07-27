use thiserror::Error;

use crate::FlowNodeId;

/// Stable identifier of an edge in the checked min-ratio-cycle ledger.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MinRatioEdgeId(pub usize);

/// Reduced exact signed rational number with a positive denominator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactRatio {
    numerator: i128,
    denominator: i128,
}

impl ExactRatio {
    /// Constructs a reduced exact ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero denominator or when normalization overflows.
    pub fn new(numerator: i128, denominator: i128) -> Result<Self, StableMinRatioError> {
        if denominator == 0 {
            return Err(StableMinRatioError::ZeroDenominator);
        }
        let (numerator, denominator) = if denominator < 0 {
            (
                numerator
                    .checked_neg()
                    .ok_or(StableMinRatioError::Overflow)?,
                denominator
                    .checked_neg()
                    .ok_or(StableMinRatioError::Overflow)?,
            )
        } else {
            (numerator, denominator)
        };
        let divisor = gcd(numerator.unsigned_abs(), denominator.unsigned_abs());
        let divisor = i128::try_from(divisor).map_err(|_| StableMinRatioError::Overflow)?;
        Ok(Self {
            numerator: numerator / divisor,
            denominator: denominator / divisor,
        })
    }

    #[must_use]
    pub const fn numerator(self) -> i128 {
        self.numerator
    }

    #[must_use]
    pub const fn denominator(self) -> i128 {
        self.denominator
    }

    /// Compares two ratios with checked cross multiplication.
    ///
    /// # Errors
    ///
    /// Returns an error when the exact comparison overflows.
    pub fn at_least(self, other: Self) -> Result<bool, StableMinRatioError> {
        Ok(self
            .numerator
            .checked_mul(other.denominator)
            .ok_or(StableMinRatioError::Overflow)?
            >= other
                .numerator
                .checked_mul(self.denominator)
                .ok_or(StableMinRatioError::Overflow)?)
    }

    /// Adds two exact ratios.
    ///
    /// # Errors
    ///
    /// Returns an error when exact arithmetic overflows.
    pub fn checked_add(self, other: Self) -> Result<Self, StableMinRatioError> {
        Self::new(
            self.numerator
                .checked_mul(other.denominator)
                .and_then(|left| {
                    other
                        .numerator
                        .checked_mul(self.denominator)
                        .and_then(|right| left.checked_add(right))
                })
                .ok_or(StableMinRatioError::Overflow)?,
            self.denominator
                .checked_mul(other.denominator)
                .ok_or(StableMinRatioError::Overflow)?,
        )
    }

    /// Subtracts two exact ratios.
    ///
    /// # Errors
    ///
    /// Returns an error when exact arithmetic overflows.
    pub fn checked_sub(self, other: Self) -> Result<Self, StableMinRatioError> {
        self.checked_add(Self::new(
            other
                .numerator
                .checked_neg()
                .ok_or(StableMinRatioError::Overflow)?,
            other.denominator,
        )?)
    }

    /// Multiplies two exact ratios.
    ///
    /// # Errors
    ///
    /// Returns an error when exact arithmetic overflows.
    pub fn checked_mul(self, other: Self) -> Result<Self, StableMinRatioError> {
        Self::new(
            self.numerator
                .checked_mul(other.numerator)
                .ok_or(StableMinRatioError::Overflow)?,
            self.denominator
                .checked_mul(other.denominator)
                .ok_or(StableMinRatioError::Overflow)?,
        )
    }

    /// Returns the reciprocal of a nonzero exact ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or when normalization overflows.
    pub fn reciprocal(self) -> Result<Self, StableMinRatioError> {
        Self::new(self.denominator, self.numerator)
    }

    /// Multiplies an exact ratio by an integer.
    ///
    /// # Errors
    ///
    /// Returns an error when exact arithmetic overflows.
    pub fn checked_mul_integer(self, value: i128) -> Result<Self, StableMinRatioError> {
        Self::new(
            self.numerator
                .checked_mul(value)
                .ok_or(StableMinRatioError::Overflow)?,
            self.denominator,
        )
    }

    #[must_use]
    pub const fn is_integral(self) -> bool {
        self.numerator % self.denominator == 0
    }
}

/// One implicitly directed edge with a signed flow coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableEdge {
    pub from: FlowNodeId,
    pub to: FlowNodeId,
    pub gradient: i128,
    pub length: i128,
}

/// Auditor-visible witness for Definition 4.3 and Definition 4.4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableWitness {
    pub circulation: Vec<i128>,
    pub upper_bounds: Vec<i128>,
}

/// One explicit coordinate update and its candidate cycle update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableUpdate {
    pub changed_edges: Vec<(MinRatioEdgeId, i128, i128)>,
    pub direction: Vec<i128>,
    pub eta: i128,
    pub witness: StableWitness,
}

/// Append-only replay trace for Definition 4.5 operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StableOperation {
    Update {
        stage: usize,
        changed_edges: Vec<MinRatioEdgeId>,
    },
    Query {
        edge: MinRatioEdgeId,
    },
    Detect {
        epsilon: ExactRatio,
        edges: Vec<MinRatioEdgeId>,
    },
}

/// Checked exact ledger for Definition 4.2--4.5, without a dynamic algorithm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StableMinRatioLedger {
    node_count: usize,
    edges: Vec<StableEdge>,
    alpha: ExactRatio,
    kappa: ExactRatio,
    stability_floors: Vec<i128>,
    flows: Vec<ExactRatio>,
    directions: Vec<Vec<i128>>,
    last_detect_stage: Vec<usize>,
    operations: Vec<StableOperation>,
}

impl StableMinRatioLedger {
    /// Initializes a checked hidden-stability state.
    ///
    /// The witness is accepted only after validating Definition 4.3 and all
    /// applicable Definition 4.4 inequalities. It is retained only as an
    /// audit input, not exposed as a query result.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid graph coordinates, ratios, or witness.
    pub fn new(
        node_count: usize,
        edges: Vec<StableEdge>,
        alpha: ExactRatio,
        kappa: ExactRatio,
        witness: StableWitness,
    ) -> Result<Self, StableMinRatioError> {
        if alpha.numerator <= 0 || kappa.numerator <= 0 || ratio_greater(kappa, ExactRatio::one()?)?
        {
            return Err(StableMinRatioError::InvalidApproximation);
        }
        validate_edges(node_count, &edges)?;
        validate_witness(&edges, node_count, alpha, &witness)?;
        let edge_count = edges.len();
        Ok(Self {
            node_count,
            edges,
            alpha,
            kappa,
            stability_floors: witness.upper_bounds,
            flows: vec![ExactRatio::zero()?; edge_count],
            directions: Vec::new(),
            last_detect_stage: vec![0; edge_count],
            operations: Vec::new(),
        })
    }

    /// Applies and records one checked Definition 4.5 update.
    ///
    /// # Errors
    ///
    /// Returns an error when an edge change, witness, direction, or quality
    /// bound violates the P8.1 checked contract.
    pub fn update(&mut self, update: StableUpdate) -> Result<(), StableMinRatioError> {
        if update.eta <= 0 || update.direction.len() != self.edges.len() {
            return Err(StableMinRatioError::InvalidUpdate);
        }
        let mut candidate = self.edges.clone();
        let mut explicit = vec![false; candidate.len()];
        let mut changed_ids = Vec::with_capacity(update.changed_edges.len());
        for (id, gradient, length) in &update.changed_edges {
            let edge = candidate
                .get_mut(id.0)
                .ok_or(StableMinRatioError::EdgeOutOfBounds { edge: id.0 })?;
            if explicit[id.0] || *length <= 0 {
                return Err(StableMinRatioError::InvalidUpdate);
            }
            edge.gradient = *gradient;
            edge.length = *length;
            explicit[id.0] = true;
            changed_ids.push(*id);
        }
        validate_witness(&candidate, self.node_count, self.alpha, &update.witness)?;
        for (index, bound) in update.witness.upper_bounds.iter().enumerate() {
            if !explicit[index]
                && *bound
                    > self.stability_floors[index]
                        .checked_mul(2)
                        .ok_or(StableMinRatioError::Overflow)?
            {
                return Err(StableMinRatioError::StabilityViolation {
                    edge: MinRatioEdgeId(index),
                });
            }
        }
        validate_quality(
            &candidate,
            self.node_count,
            self.alpha,
            self.kappa,
            &update.direction,
        )?;
        let objective = dot(&candidate, &update.direction)?;
        let beta = ExactRatio::new(objective, update.eta)?;
        for (flow, direction) in self.flows.iter_mut().zip(&update.direction) {
            *flow = flow.checked_add(beta.checked_mul_integer(-*direction)?)?;
        }
        for (index, changed) in explicit.into_iter().enumerate() {
            if changed {
                self.stability_floors[index] = update.witness.upper_bounds[index];
            } else {
                self.stability_floors[index] =
                    self.stability_floors[index].min(update.witness.upper_bounds[index]);
            }
        }
        self.edges = candidate;
        self.directions.push(update.direction);
        self.operations.push(StableOperation::Update {
            stage: self.directions.len(),
            changed_edges: changed_ids,
        });
        Ok(())
    }

    /// Returns the exact accumulated flow coordinate and appends a replay log.
    ///
    /// # Errors
    ///
    /// Returns an error when `edge` is absent.
    pub fn query(&mut self, edge: MinRatioEdgeId) -> Result<ExactRatio, StableMinRatioError> {
        let value = *self
            .flows
            .get(edge.0)
            .ok_or(StableMinRatioError::EdgeOutOfBounds { edge: edge.0 })?;
        self.operations.push(StableOperation::Query { edge });
        Ok(value)
    }

    /// Detects coordinates whose weighted accumulated change crosses `epsilon`.
    ///
    /// # Errors
    ///
    /// Returns an error when `epsilon` is not positive.
    pub fn detect(
        &mut self,
        epsilon: ExactRatio,
    ) -> Result<Vec<MinRatioEdgeId>, StableMinRatioError> {
        if epsilon.numerator <= 0 {
            return Err(StableMinRatioError::InvalidUpdate);
        }
        let stage = self.directions.len();
        let mut result = Vec::new();
        for edge in 0..self.edges.len() {
            let total = self.directions[self.last_detect_stage[edge]..stage]
                .iter()
                .try_fold(0_i128, |sum, direction| {
                    sum.checked_add(magnitude(direction[edge])?)
                        .ok_or(StableMinRatioError::Overflow)
                })?;
            let weighted = self.edges[edge]
                .length
                .checked_mul(total)
                .ok_or(StableMinRatioError::Overflow)?;
            if weighted
                .checked_mul(epsilon.denominator)
                .ok_or(StableMinRatioError::Overflow)?
                >= epsilon.numerator
            {
                result.push(MinRatioEdgeId(edge));
                self.last_detect_stage[edge] = stage;
            }
        }
        self.operations.push(StableOperation::Detect {
            epsilon,
            edges: result.clone(),
        });
        Ok(result)
    }

    #[must_use]
    pub fn operations(&self) -> &[StableOperation] {
        &self.operations
    }
}

impl ExactRatio {
    fn zero() -> Result<Self, StableMinRatioError> {
        Self::new(0, 1)
    }

    fn one() -> Result<Self, StableMinRatioError> {
        Self::new(1, 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum StableMinRatioError {
    #[error("ratio denominator must be nonzero")]
    ZeroDenominator,
    #[error("exact arithmetic overflowed")]
    Overflow,
    #[error("edge {edge} is outside the current graph")]
    EdgeOutOfBounds { edge: usize },
    #[error("an edge endpoint is outside the current graph")]
    NodeOutOfBounds,
    #[error("lengths, witnesses, or approximation parameters are invalid")]
    InvalidApproximation,
    #[error("the witness is not a valid stable circulation")]
    InvalidWitness,
    #[error("the update direction is not a valid approximate circulation")]
    InvalidUpdate,
    #[error("edge {edge:?} exceeds its factor-two stable witness bound")]
    StabilityViolation { edge: MinRatioEdgeId },
}

fn validate_edges(node_count: usize, edges: &[StableEdge]) -> Result<(), StableMinRatioError> {
    if edges
        .iter()
        .any(|edge| edge.from.0 >= node_count || edge.to.0 >= node_count || edge.length <= 0)
    {
        return Err(StableMinRatioError::NodeOutOfBounds);
    }
    Ok(())
}

fn validate_witness(
    edges: &[StableEdge],
    node_count: usize,
    alpha: ExactRatio,
    witness: &StableWitness,
) -> Result<(), StableMinRatioError> {
    if witness.circulation.len() != edges.len()
        || witness.upper_bounds.len() != edges.len()
        || witness.upper_bounds.iter().any(|bound| *bound < 0)
        || !is_circulation(edges, node_count, &witness.circulation)?
    {
        return Err(StableMinRatioError::InvalidWitness);
    }
    let mut norm = 0_i128;
    for (edge, (circulation, bound)) in edges
        .iter()
        .zip(witness.circulation.iter().zip(&witness.upper_bounds))
    {
        if edge
            .length
            .checked_mul(magnitude(*circulation)?)
            .ok_or(StableMinRatioError::Overflow)?
            > *bound
        {
            return Err(StableMinRatioError::InvalidWitness);
        }
        norm = norm
            .checked_add(*bound)
            .ok_or(StableMinRatioError::Overflow)?;
    }
    if norm == 0
        || dot(edges, &witness.circulation)?
            .checked_mul(alpha.denominator)
            .ok_or(StableMinRatioError::Overflow)?
            > -alpha
                .numerator
                .checked_mul(norm)
                .ok_or(StableMinRatioError::Overflow)?
    {
        return Err(StableMinRatioError::InvalidWitness);
    }
    Ok(())
}

fn validate_quality(
    edges: &[StableEdge],
    node_count: usize,
    alpha: ExactRatio,
    kappa: ExactRatio,
    direction: &[i128],
) -> Result<(), StableMinRatioError> {
    if !is_circulation(edges, node_count, direction)? {
        return Err(StableMinRatioError::InvalidUpdate);
    }
    let norm = edges
        .iter()
        .zip(direction)
        .try_fold(0_i128, |sum, (edge, value)| {
            sum.checked_add(
                edge.length
                    .checked_mul(magnitude(*value)?)
                    .ok_or(StableMinRatioError::Overflow)?,
            )
            .ok_or(StableMinRatioError::Overflow)
        })?;
    if norm == 0 {
        return Err(StableMinRatioError::InvalidUpdate);
    }
    let required = alpha
        .numerator
        .checked_mul(kappa.numerator)
        .and_then(|value| value.checked_mul(norm))
        .ok_or(StableMinRatioError::Overflow)?;
    let denominator = alpha
        .denominator
        .checked_mul(kappa.denominator)
        .ok_or(StableMinRatioError::Overflow)?;
    if dot(edges, direction)?
        .checked_mul(denominator)
        .ok_or(StableMinRatioError::Overflow)?
        > -required
    {
        return Err(StableMinRatioError::InvalidUpdate);
    }
    Ok(())
}

fn is_circulation(
    edges: &[StableEdge],
    node_count: usize,
    values: &[i128],
) -> Result<bool, StableMinRatioError> {
    if values.len() != edges.len() {
        return Ok(false);
    }
    let mut balance = vec![0_i128; node_count];
    for (edge, value) in edges.iter().zip(values) {
        balance[edge.from.0] = balance[edge.from.0]
            .checked_add(*value)
            .ok_or(StableMinRatioError::Overflow)?;
        balance[edge.to.0] = balance[edge.to.0]
            .checked_sub(*value)
            .ok_or(StableMinRatioError::Overflow)?;
    }
    Ok(balance.iter().all(|value| *value == 0))
}

fn dot(edges: &[StableEdge], values: &[i128]) -> Result<i128, StableMinRatioError> {
    edges
        .iter()
        .zip(values)
        .try_fold(0_i128, |sum, (edge, value)| {
            sum.checked_add(
                edge.gradient
                    .checked_mul(*value)
                    .ok_or(StableMinRatioError::Overflow)?,
            )
            .ok_or(StableMinRatioError::Overflow)
        })
}

fn magnitude(value: i128) -> Result<i128, StableMinRatioError> {
    value.checked_abs().ok_or(StableMinRatioError::Overflow)
}

fn ratio_greater(left: ExactRatio, right: ExactRatio) -> Result<bool, StableMinRatioError> {
    Ok(left
        .numerator
        .checked_mul(right.denominator)
        .ok_or(StableMinRatioError::Overflow)?
        > right
            .numerator
            .checked_mul(left.denominator)
            .ok_or(StableMinRatioError::Overflow)?)
}

const fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    if left == 0 { 1 } else { left }
}

#[cfg(test)]
mod tests {
    use super::{
        ExactRatio, MinRatioEdgeId, StableEdge, StableMinRatioError, StableMinRatioLedger,
        StableOperation, StableUpdate, StableWitness,
    };
    use crate::FlowNodeId;

    fn ledger() -> StableMinRatioLedger {
        StableMinRatioLedger::new(
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
        .unwrap()
    }

    #[test]
    fn records_exact_update_query_and_detect_replay() {
        let mut state = ledger();
        state
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
            state.query(MinRatioEdgeId(0)).unwrap(),
            ExactRatio::new(1, 1).unwrap()
        );
        assert_eq!(
            state.detect(ExactRatio::new(1, 1).unwrap()).unwrap(),
            vec![MinRatioEdgeId(0), MinRatioEdgeId(1)]
        );
        assert!(
            state
                .detect(ExactRatio::new(1, 1).unwrap())
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            state.operations()[0],
            StableOperation::Update { stage: 1, .. }
        ));
        assert!(matches!(
            state.operations()[1],
            StableOperation::Query { .. }
        ));
    }

    #[test]
    fn rejects_unannounced_factor_two_witness_drift() {
        let mut state = ledger();
        assert_eq!(
            state.update(StableUpdate {
                changed_edges: Vec::new(),
                direction: vec![1, 1],
                eta: 1,
                witness: StableWitness {
                    circulation: vec![1, 1],
                    upper_bounds: vec![3, 1]
                },
            }),
            Err(StableMinRatioError::StabilityViolation {
                edge: MinRatioEdgeId(0)
            })
        );
    }

    #[test]
    fn stability_is_checked_against_every_unannounced_prior_bound() {
        let mut state = StableMinRatioLedger::new(
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
                upper_bounds: vec![2, 2],
            },
        )
        .unwrap();
        state
            .update(StableUpdate {
                changed_edges: Vec::new(),
                direction: vec![1, 1],
                eta: 1,
                witness: StableWitness {
                    circulation: vec![1, 1],
                    upper_bounds: vec![1, 1],
                },
            })
            .unwrap();
        assert_eq!(
            state.update(StableUpdate {
                changed_edges: Vec::new(),
                direction: vec![1, 1],
                eta: 1,
                witness: StableWitness {
                    circulation: vec![1, 1],
                    upper_bounds: vec![3, 1]
                },
            }),
            Err(StableMinRatioError::StabilityViolation {
                edge: MinRatioEdgeId(0)
            })
        );
    }

    #[test]
    fn permits_an_explicit_witness_bound_reset() {
        let mut state = ledger();
        state
            .update(StableUpdate {
                changed_edges: vec![(MinRatioEdgeId(0), -2, 3)],
                direction: vec![1, 1],
                eta: 1,
                witness: StableWitness {
                    circulation: vec![1, 1],
                    upper_bounds: vec![3, 1],
                },
            })
            .unwrap();
    }

    #[test]
    fn rejects_non_circulating_direction() {
        let mut state = ledger();
        assert_eq!(
            state.update(StableUpdate {
                changed_edges: Vec::new(),
                direction: vec![1, 0],
                eta: 1,
                witness: StableWitness {
                    circulation: vec![1, 1],
                    upper_bounds: vec![1, 1]
                },
            }),
            Err(StableMinRatioError::InvalidUpdate)
        );
    }
}
