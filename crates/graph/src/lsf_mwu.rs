use thiserror::Error;

use crate::{
    DynamicRootedForest, ExactRatio, FlowNodeId, ForestEdge, ForestEdgeId, StableMinRatioError,
};

/// Deterministic reweighting evidence for a P8.4 forest collection.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ForestCollectionMetrics {
    pub round_count: u64,
    pub penalty_updates: u64,
}

/// Exact, deterministic small-instance baseline for Lemma 5.5's forest collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForestCollection {
    trees: Vec<Vec<ForestEdgeId>>,
    average_stretches: Vec<ExactRatio>,
    metrics: ForestCollectionMetrics,
}

impl ForestCollection {
    /// Builds `count` deterministically reweighted spanning-tree candidates.
    ///
    /// # Errors
    ///
    /// Returns an error for zero candidates, invalid edges, disconnected input,
    /// or checked arithmetic failure.
    pub fn build(
        node_count: usize,
        edges: &[ForestEdge],
        count: usize,
    ) -> Result<Self, ForestCollectionError> {
        if count == 0 {
            return Err(ForestCollectionError::ZeroCount);
        }
        let mut penalties = vec![1_i128; edges.len()];
        let mut sums = vec![ExactRatio::new(0, 1).map_err(map_ratio)?; edges.len()];
        let mut trees = Vec::with_capacity(count);
        let mut metrics = ForestCollectionMetrics::default();
        for _ in 0..count {
            let tree = weighted_kruskal(node_count, edges, &penalties)?;
            let forest = DynamicRootedForest::new(
                node_count,
                edges.to_vec(),
                tree.iter().copied(),
                [FlowNodeId(0)],
            )
            .map_err(|_| ForestCollectionError::InvalidGraph)?;
            for index in 0..edges.len() {
                let stretch = forest
                    .stretch(ForestEdgeId(index))
                    .map_err(|_| ForestCollectionError::InvalidGraph)?;
                sums[index] = sums[index].checked_add(&stretch).map_err(map_ratio)?;
                if stretch.numerator() > stretch.denominator() {
                    penalties[index] = penalties[index]
                        .checked_mul(2)
                        .ok_or(ForestCollectionError::Overflow)?;
                    metrics.penalty_updates += 1;
                }
            }
            metrics.round_count += 1;
            trees.push(tree);
        }
        let divisor = i128::try_from(count).map_err(|_| ForestCollectionError::Overflow)?;
        let average_stretches = sums
            .into_iter()
            .map(|sum| {
                sum.checked_mul(&ExactRatio::new(1, divisor).map_err(map_ratio)?)
                    .map_err(map_ratio)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            trees,
            average_stretches,
            metrics,
        })
    }

    /// Returns the exact average stretch certificate for an input edge.
    ///
    /// # Errors
    ///
    /// Returns an error when `edge` is outside the input graph.
    pub fn average_stretch(&self, edge: ForestEdgeId) -> Result<ExactRatio, ForestCollectionError> {
        self.average_stretches
            .get(edge.0)
            .cloned()
            .ok_or(ForestCollectionError::EdgeOutOfBounds)
    }

    #[must_use]
    pub fn trees(&self) -> &[Vec<ForestEdgeId>] {
        &self.trees
    }

    #[must_use]
    pub const fn metrics(&self) -> ForestCollectionMetrics {
        self.metrics
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ForestCollectionError {
    #[error("forest collection count must be positive")]
    ZeroCount,
    #[error("input graph is invalid or disconnected")]
    InvalidGraph,
    #[error("edge is outside the input graph")]
    EdgeOutOfBounds,
    #[error("exact arithmetic overflowed")]
    Overflow,
}

fn weighted_kruskal(
    node_count: usize,
    edges: &[ForestEdge],
    penalties: &[i128],
) -> Result<Vec<ForestEdgeId>, ForestCollectionError> {
    if edges.len() != penalties.len()
        || edges.iter().any(|edge| {
            edge.first.0 >= node_count || edge.second.0 >= node_count || edge.length <= 0
        })
    {
        return Err(ForestCollectionError::InvalidGraph);
    }
    let mut order = edges
        .iter()
        .enumerate()
        .map(|(index, edge)| {
            Ok::<_, ForestCollectionError>((
                edge.length
                    .checked_mul(penalties[index])
                    .ok_or(ForestCollectionError::Overflow)?,
                index,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    order.sort_unstable();
    let mut parent = (0..node_count).collect::<Vec<_>>();
    let mut tree = Vec::new();
    for (_, index) in order {
        let first = find(&mut parent, edges[index].first.0);
        let second = find(&mut parent, edges[index].second.0);
        if first != second {
            parent[first] = second;
            tree.push(ForestEdgeId(index));
        }
    }
    if node_count > 0 && tree.len() != node_count - 1 {
        return Err(ForestCollectionError::InvalidGraph);
    }
    Ok(tree)
}

fn find(parent: &mut [usize], node: usize) -> usize {
    if parent[node] != node {
        parent[node] = find(parent, parent[node]);
    }
    parent[node]
}

fn map_ratio(_: StableMinRatioError) -> ForestCollectionError {
    ForestCollectionError::Overflow
}

#[cfg(test)]
mod tests {
    use super::{ForestCollection, ForestCollectionError};
    use crate::{FlowNodeId, ForestEdge, ForestEdgeId};

    fn edges() -> Vec<ForestEdge> {
        vec![
            ForestEdge {
                first: FlowNodeId(0),
                second: FlowNodeId(1),
                length: 1,
            },
            ForestEdge {
                first: FlowNodeId(1),
                second: FlowNodeId(2),
                length: 1,
            },
            ForestEdge {
                first: FlowNodeId(0),
                second: FlowNodeId(2),
                length: 3,
            },
        ]
    }

    #[test]
    fn produces_deterministic_average_stretch_certificates() {
        let edges = edges();
        let first = ForestCollection::build(3, &edges, 3).unwrap();
        let second = ForestCollection::build(3, &edges, 3).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.trees().len(), 3);
        assert!(
            first
                .average_stretch(ForestEdgeId(2))
                .unwrap()
                .is_positive()
        );
        assert_eq!(first.metrics().round_count, 3);
    }

    #[test]
    fn rejects_zero_count_and_disconnected_input() {
        assert_eq!(
            ForestCollection::build(3, &edges(), 0),
            Err(ForestCollectionError::ZeroCount)
        );
        assert_eq!(
            ForestCollection::build(
                3,
                &[ForestEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: 1
                }],
                1
            ),
            Err(ForestCollectionError::InvalidGraph)
        );
    }
}
