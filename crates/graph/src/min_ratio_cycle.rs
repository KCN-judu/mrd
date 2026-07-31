use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Signed, ToPrimitive, Zero};
use thiserror::Error;

use crate::FlowNodeId;

/// Stable identifier of an edge in the checked min-ratio-cycle ledger.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MinRatioEdgeId(pub usize);

/// Reduced exact signed rational number with a positive arbitrary-precision
/// denominator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactRatio {
    numerator: BigInt,
    denominator: BigInt,
}

impl ExactRatio {
    /// Constructs a reduced exact ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero denominator.
    pub fn new(numerator: i128, denominator: i128) -> Result<Self, StableMinRatioError> {
        Self::from_parts(BigInt::from(numerator), BigInt::from(denominator))
    }

    /// Constructs a reduced arbitrary-precision ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero denominator.
    pub fn from_bigints(
        numerator: BigInt,
        denominator: BigInt,
    ) -> Result<Self, StableMinRatioError> {
        Self::from_parts(numerator, denominator)
    }

    #[must_use]
    pub const fn numerator(&self) -> &BigInt {
        &self.numerator
    }

    #[must_use]
    pub const fn denominator(&self) -> &BigInt {
        &self.denominator
    }

    /// Returns the numerator when this value is inside the bounded structural
    /// integer domain.
    ///
    /// # Errors
    ///
    /// Returns an error when the exact component cannot fit in `i128`.
    pub fn numerator_i128(&self) -> Result<i128, StableMinRatioError> {
        self.numerator
            .to_i128()
            .ok_or(StableMinRatioError::Overflow)
    }

    /// Returns the denominator when this value is inside the bounded structural
    /// integer domain.
    ///
    /// # Errors
    ///
    /// Returns an error when the exact component cannot fit in `i128`.
    pub fn denominator_i128(&self) -> Result<i128, StableMinRatioError> {
        self.denominator
            .to_i128()
            .ok_or(StableMinRatioError::Overflow)
    }

    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.numerator.is_zero()
    }

    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.numerator.is_positive()
    }

    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.numerator.is_negative()
    }

    /// Returns the exact absolute value.
    ///
    /// # Errors
    ///
    pub fn abs(&self) -> Result<Self, StableMinRatioError> {
        if self.numerator.is_negative() {
            Self::from_parts(-&self.numerator, self.denominator.clone())
        } else {
            Ok(self.clone())
        }
    }

    /// Negates an exact ratio.
    ///
    /// # Errors
    ///
    pub fn checked_neg(&self) -> Result<Self, StableMinRatioError> {
        Self::from_parts(-&self.numerator, self.denominator.clone())
    }

    /// Compares two ratios with checked cross multiplication.
    ///
    /// # Errors
    ///
    pub fn at_least(&self, other: &Self) -> Result<bool, StableMinRatioError> {
        Ok(&self.numerator * &other.denominator >= &other.numerator * &self.denominator)
    }

    /// Adds two exact ratios.
    ///
    /// # Errors
    ///
    pub fn checked_add(&self, other: &Self) -> Result<Self, StableMinRatioError> {
        Self::from_parts(
            &self.numerator * &other.denominator + &other.numerator * &self.denominator,
            &self.denominator * &other.denominator,
        )
    }

    /// Subtracts two exact ratios.
    ///
    /// # Errors
    ///
    pub fn checked_sub(&self, other: &Self) -> Result<Self, StableMinRatioError> {
        self.checked_add(&other.checked_neg()?)
    }

    /// Multiplies two exact ratios.
    ///
    /// # Errors
    ///
    pub fn checked_mul(&self, other: &Self) -> Result<Self, StableMinRatioError> {
        Self::from_parts(
            &self.numerator * &other.numerator,
            &self.denominator * &other.denominator,
        )
    }

    /// Returns the reciprocal of a nonzero exact ratio.
    ///
    /// # Errors
    ///
    /// Returns an error for zero.
    pub fn reciprocal(&self) -> Result<Self, StableMinRatioError> {
        Self::from_parts(self.denominator.clone(), self.numerator.clone())
    }

    /// Multiplies an exact ratio by an integer.
    ///
    /// # Errors
    ///
    pub fn checked_mul_integer(&self, value: i128) -> Result<Self, StableMinRatioError> {
        Self::from_parts(&self.numerator * value, self.denominator.clone())
    }

    #[must_use]
    pub fn is_integral(&self) -> bool {
        self.numerator.is_multiple_of(&self.denominator)
    }

    fn from_parts(
        mut numerator: BigInt,
        mut denominator: BigInt,
    ) -> Result<Self, StableMinRatioError> {
        if denominator.is_zero() {
            return Err(StableMinRatioError::ZeroDenominator);
        }
        if denominator.is_negative() {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = numerator.gcd(&denominator);
        Ok(Self {
            numerator: numerator / &divisor,
            denominator: denominator / divisor,
        })
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
        if !alpha.is_positive()
            || !kappa.is_positive()
            || ratio_greater(&kappa, &ExactRatio::one()?)
        {
            return Err(StableMinRatioError::InvalidApproximation);
        }
        validate_edges(node_count, &edges)?;
        validate_witness(&edges, node_count, alpha.clone(), &witness)?;
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
        validate_witness(
            &candidate,
            self.node_count,
            self.alpha.clone(),
            &update.witness,
        )?;
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
            self.alpha.clone(),
            self.kappa.clone(),
            &update.direction,
        )?;
        let objective = dot(&candidate, &update.direction)?;
        let beta = ExactRatio::new(objective, update.eta)?;
        for (flow, direction) in self.flows.iter_mut().zip(&update.direction) {
            *flow = flow.checked_add(&beta.checked_mul_integer(-*direction)?)?;
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

    /// Returns the currently validated directed edge coordinates.
    ///
    /// The returned slice is intended for independent exact audit Oracles; it
    /// does not expose the hidden witness retained by this ledger.
    #[must_use]
    pub fn edges(&self) -> &[StableEdge] {
        &self.edges
    }

    /// Returns the exact accumulated flow coordinate and appends a replay log.
    ///
    /// # Errors
    ///
    /// Returns an error when `edge` is absent.
    pub fn query(&mut self, edge: MinRatioEdgeId) -> Result<ExactRatio, StableMinRatioError> {
        let value = self
            .flows
            .get(edge.0)
            .ok_or(StableMinRatioError::EdgeOutOfBounds { edge: edge.0 })?
            .clone();
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
        if !epsilon.is_positive() {
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
            if BigInt::from(weighted) * epsilon.denominator() >= *epsilon.numerator() {
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
        || BigInt::from(dot(edges, &witness.circulation)?) * alpha.denominator()
            > -alpha.numerator() * BigInt::from(norm)
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
    let required = alpha.numerator() * kappa.numerator() * norm;
    let denominator = alpha.denominator() * kappa.denominator();
    if BigInt::from(dot(edges, direction)?) * denominator > -required {
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

fn ratio_greater(left: &ExactRatio, right: &ExactRatio) -> bool {
    left.numerator() * right.denominator() > right.numerator() * left.denominator()
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

    #[test]
    fn cancels_shared_exact_terms_before_arithmetic() {
        let base = 1_i128 << 100;
        let first = ExactRatio::new(base - 1, base - 3).unwrap();
        let second = ExactRatio::new(base - 3, base - 5).unwrap();
        assert_eq!(
            first.checked_mul(&second).unwrap(),
            ExactRatio::new(base - 1, base - 5).unwrap()
        );
        assert_eq!(
            first
                .checked_add(&ExactRatio::new(2, base - 3).unwrap())
                .unwrap(),
            ExactRatio::new(base + 1, base - 3).unwrap()
        );
        assert!(
            ExactRatio::new(base - 1, base - 5)
                .unwrap()
                .at_least(&first)
                .unwrap()
        );
    }
}
