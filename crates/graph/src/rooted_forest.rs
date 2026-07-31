use std::collections::{BTreeSet, VecDeque};

use thiserror::Error;

use crate::{ExactRatio, FlowNodeId};

/// Stable edge identifier for the dynamic rooted-forest primitive.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ForestEdgeId(pub usize);

/// Undirected positive-length graph edge used by the rooted forest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForestEdge {
    pub first: FlowNodeId,
    pub second: FlowNodeId,
    pub length: i128,
}

/// Source-shaped operation and recourse counters, with no asymptotic claim.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ForestMetrics {
    pub edge_deletions: u64,
    pub forest_edge_removals: u64,
    pub vertex_splits: u64,
    pub root_additions: u64,
    pub path_updates: u64,
    pub path_queries: u64,
}

/// Deterministic, exact rooted-forest primitive used as a P8.2 baseline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicRootedForest {
    node_count: usize,
    edges: Vec<ForestEdge>,
    active: Vec<bool>,
    forest_edges: BTreeSet<ForestEdgeId>,
    roots: BTreeSet<usize>,
    values: Vec<i128>,
    metrics: ForestMetrics,
}

impl DynamicRootedForest {
    /// Creates and validates a rooted spanning forest subgraph.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid endpoints, nonpositive lengths, a
    /// non-forest selection, or a component without exactly one root.
    pub fn new(
        node_count: usize,
        edges: Vec<ForestEdge>,
        forest_edges: impl IntoIterator<Item = ForestEdgeId>,
        roots: impl IntoIterator<Item = FlowNodeId>,
    ) -> Result<Self, RootedForestError> {
        if edges.iter().any(|edge| {
            edge.first.0 >= node_count || edge.second.0 >= node_count || edge.length <= 0
        }) {
            return Err(RootedForestError::InvalidEdge);
        }
        let forest_edges = forest_edges.into_iter().collect::<BTreeSet<_>>();
        if forest_edges.iter().any(|edge| edge.0 >= edges.len()) {
            return Err(RootedForestError::EdgeOutOfBounds);
        }
        let roots = roots
            .into_iter()
            .map(|node| node.0)
            .collect::<BTreeSet<_>>();
        if roots.iter().any(|node| *node >= node_count) {
            return Err(RootedForestError::NodeOutOfBounds);
        }
        let result = Self {
            node_count,
            active: vec![true; edges.len()],
            edges,
            forest_edges,
            roots,
            values: vec![0; node_count],
            metrics: ForestMetrics::default(),
        };
        result.validate()?;
        Ok(result)
    }

    /// Deletes an active graph edge and, when necessary, removes it from the
    /// forest before deterministically restoring one root per component.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or already deleted edge.
    pub fn delete_edge(&mut self, edge: ForestEdgeId) -> Result<(), RootedForestError> {
        if edge.0 >= self.edges.len() {
            return Err(RootedForestError::EdgeOutOfBounds);
        }
        if !self.active[edge.0] {
            return Err(RootedForestError::InactiveEdge);
        }
        self.active[edge.0] = false;
        self.metrics.edge_deletions += 1;
        if self.forest_edges.remove(&edge) {
            self.metrics.forest_edge_removals += 1;
        }
        self.restore_roots()?;
        Ok(())
    }

    /// Splits one vertex by moving listed incident active graph edges to a new
    /// singleton forest vertex. Any moved forest edge is removed, preserving
    /// the decremental forest-edge contract.
    ///
    /// # Errors
    ///
    /// Returns an error when the vertex or moved-edge list is invalid.
    pub fn split_vertex(
        &mut self,
        node: FlowNodeId,
        moved: &[ForestEdgeId],
    ) -> Result<FlowNodeId, RootedForestError> {
        if node.0 >= self.node_count {
            return Err(RootedForestError::NodeOutOfBounds);
        }
        let mut seen = BTreeSet::new();
        for edge in moved {
            if edge.0 >= self.edges.len()
                || !self.active[edge.0]
                || !seen.insert(*edge)
                || (self.edges[edge.0].first != node && self.edges[edge.0].second != node)
            {
                return Err(RootedForestError::InvalidSplit);
            }
        }
        let new_node = FlowNodeId(self.node_count);
        self.node_count += 1;
        self.values.push(0);
        self.roots.insert(new_node.0);
        self.metrics.vertex_splits += 1;
        self.metrics.root_additions += 1;
        for edge in moved {
            let entry = &mut self.edges[edge.0];
            if entry.first == node {
                entry.first = new_node;
            } else {
                entry.second = new_node;
            }
            if self.forest_edges.remove(edge) {
                self.metrics.forest_edge_removals += 1;
            }
        }
        self.restore_roots()?;
        Ok(new_node)
    }

    /// Adds `delta` to every vertex on the forest path from `node` to its root.
    ///
    /// # Errors
    ///
    /// Returns an error when the node is invalid or arithmetic overflows.
    pub fn add_to_root_path(
        &mut self,
        node: FlowNodeId,
        delta: i128,
    ) -> Result<(), RootedForestError> {
        let path = self.path_to_root(node.0)?;
        for vertex in path {
            self.values[vertex] = self.values[vertex]
                .checked_add(delta)
                .ok_or(RootedForestError::Overflow)?;
        }
        self.metrics.path_updates += 1;
        Ok(())
    }

    /// Returns the exact sum of values on the forest path to the root.
    ///
    /// # Errors
    ///
    /// Returns an error when the node is invalid or summation overflows.
    pub fn root_path_sum(&mut self, node: FlowNodeId) -> Result<i128, RootedForestError> {
        let value = self
            .path_to_root(node.0)?
            .into_iter()
            .try_fold(0_i128, |sum, vertex| {
                sum.checked_add(self.values[vertex])
                    .ok_or(RootedForestError::Overflow)
            })?;
        self.metrics.path_queries += 1;
        Ok(value)
    }

    /// Computes Definition 5.3's exact stretch for an active graph edge.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid or inactive edge, disconnected root
    /// assignment, or exact arithmetic overflow.
    pub fn stretch(&self, edge: ForestEdgeId) -> Result<ExactRatio, RootedForestError> {
        let graph_edge = self.active_edge(edge)?;
        let first_root = self.root_of(graph_edge.first.0)?;
        let second_root = self.root_of(graph_edge.second.0)?;
        let route_length = if first_root == second_root {
            self.forest_distance(graph_edge.first.0, graph_edge.second.0)?
        } else {
            self.forest_distance(graph_edge.first.0, first_root)?
                .checked_add(self.forest_distance(graph_edge.second.0, second_root)?)
                .ok_or(RootedForestError::Overflow)?
        };
        ExactRatio::new(
            graph_edge
                .length
                .checked_add(route_length)
                .ok_or(RootedForestError::Overflow)?,
            graph_edge.length,
        )
        .map_err(|_| RootedForestError::Overflow)
    }

    /// Independently recomputes every exact stretch before checking certificates.
    ///
    /// # Errors
    ///
    /// Returns an error for incomplete certificates, invalid state, or a bound
    /// lower than the exact Definition 5.3 stretch.
    pub fn verify_stretch_bounds(&self, bounds: &[ExactRatio]) -> Result<(), RootedForestError> {
        if bounds.len() != self.edges.len() {
            return Err(RootedForestError::InvalidCertificate);
        }
        for (index, active) in self.active.iter().enumerate() {
            if *active
                && !bounds[index]
                    .at_least(&self.stretch(ForestEdgeId(index))?)
                    .map_err(|_| RootedForestError::Overflow)?
            {
                return Err(RootedForestError::InvalidCertificate);
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn metrics(&self) -> ForestMetrics {
        self.metrics
    }

    fn active_edge(&self, edge: ForestEdgeId) -> Result<&ForestEdge, RootedForestError> {
        if edge.0 >= self.edges.len() {
            return Err(RootedForestError::EdgeOutOfBounds);
        }
        if !self.active[edge.0] {
            return Err(RootedForestError::InactiveEdge);
        }
        Ok(&self.edges[edge.0])
    }

    fn validate(&self) -> Result<(), RootedForestError> {
        if self.forest_edges.iter().any(|edge| !self.active[edge.0]) || self.has_cycle()? {
            return Err(RootedForestError::InvalidForest);
        }
        for component in self.components()? {
            if component
                .iter()
                .filter(|node| self.roots.contains(node))
                .count()
                != 1
            {
                return Err(RootedForestError::InvalidRoots);
            }
        }
        Ok(())
    }

    fn restore_roots(&mut self) -> Result<(), RootedForestError> {
        let components = self.components()?;
        let mut roots = BTreeSet::new();
        for component in components {
            if let Some(root) = component.iter().find(|node| self.roots.contains(node)) {
                roots.insert(*root);
            } else {
                roots.insert(*component.first().ok_or(RootedForestError::InvalidForest)?);
                self.metrics.root_additions += 1;
            }
        }
        self.roots = roots;
        self.validate()
    }

    fn components(&self) -> Result<Vec<Vec<usize>>, RootedForestError> {
        let adjacency = self.forest_adjacency()?;
        let mut seen = vec![false; self.node_count];
        let mut result = Vec::new();
        for start in 0..self.node_count {
            if seen[start] {
                continue;
            }
            let mut queue = VecDeque::from([start]);
            let mut component = Vec::new();
            seen[start] = true;
            while let Some(node) = queue.pop_front() {
                component.push(node);
                for (next, _) in &adjacency[node] {
                    if !seen[*next] {
                        seen[*next] = true;
                        queue.push_back(*next);
                    }
                }
            }
            component.sort_unstable();
            result.push(component);
        }
        Ok(result)
    }

    fn has_cycle(&self) -> Result<bool, RootedForestError> {
        Ok(self.forest_edges.len() + self.components()?.len() != self.node_count)
    }

    fn forest_adjacency(&self) -> Result<Vec<Vec<(usize, i128)>>, RootedForestError> {
        let mut adjacency = vec![Vec::new(); self.node_count];
        for edge in &self.forest_edges {
            let value = self.active_edge(*edge)?;
            adjacency[value.first.0].push((value.second.0, value.length));
            adjacency[value.second.0].push((value.first.0, value.length));
        }
        Ok(adjacency)
    }

    fn root_of(&self, node: usize) -> Result<usize, RootedForestError> {
        if node >= self.node_count {
            return Err(RootedForestError::NodeOutOfBounds);
        }
        self.components()?
            .into_iter()
            .find(|component| component.contains(&node))
            .and_then(|component| {
                component
                    .into_iter()
                    .find(|value| self.roots.contains(value))
            })
            .ok_or(RootedForestError::InvalidRoots)
    }

    fn path_to_root(&self, node: usize) -> Result<Vec<usize>, RootedForestError> {
        let root = self.root_of(node)?;
        self.forest_path(node, root)
            .map(|path| path.into_iter().map(|(vertex, _)| vertex).collect())
    }

    fn forest_distance(&self, first: usize, second: usize) -> Result<i128, RootedForestError> {
        self.forest_path(first, second)?
            .into_iter()
            .try_fold(0_i128, |sum, (_, length)| {
                sum.checked_add(length).ok_or(RootedForestError::Overflow)
            })
    }

    fn forest_path(
        &self,
        first: usize,
        second: usize,
    ) -> Result<Vec<(usize, i128)>, RootedForestError> {
        let adjacency = self.forest_adjacency()?;
        let mut predecessor = vec![None; self.node_count];
        let mut queue = VecDeque::from([first]);
        predecessor[first] = Some((first, 0));
        while let Some(node) = queue.pop_front() {
            if node == second {
                break;
            }
            for (next, length) in &adjacency[node] {
                if predecessor[*next].is_none() {
                    predecessor[*next] = Some((node, *length));
                    queue.push_back(*next);
                }
            }
        }
        if predecessor[second].is_none() {
            return Err(RootedForestError::InvalidForest);
        }
        let mut path = Vec::new();
        let mut node = second;
        while node != first {
            let (previous, length) = predecessor[node].ok_or(RootedForestError::InvalidForest)?;
            path.push((node, length));
            node = previous;
        }
        path.push((first, 0));
        Ok(path)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RootedForestError {
    #[error("edge endpoints or lengths are invalid")]
    InvalidEdge,
    #[error("edge identifier is outside the graph")]
    EdgeOutOfBounds,
    #[error("node is outside the graph")]
    NodeOutOfBounds,
    #[error("edge is inactive")]
    InactiveEdge,
    #[error("forest selection is not an active acyclic forest")]
    InvalidForest,
    #[error("each forest component must have exactly one root")]
    InvalidRoots,
    #[error("vertex split does not list distinct active incident edges")]
    InvalidSplit,
    #[error("stretch certificate is invalid")]
    InvalidCertificate,
    #[error("exact arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::{DynamicRootedForest, ForestEdge, ForestEdgeId, RootedForestError};
    use crate::{ExactRatio, FlowNodeId};

    fn forest() -> DynamicRootedForest {
        DynamicRootedForest::new(
            3,
            vec![
                ForestEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: 1,
                },
                ForestEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: 2,
                },
                ForestEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                    length: 5,
                },
            ],
            [ForestEdgeId(0), ForestEdgeId(1)],
            [FlowNodeId(0)],
        )
        .unwrap()
    }

    #[test]
    fn computes_definition_5_3_stretch_and_checks_certificates() {
        let forest = forest();
        assert_eq!(
            forest.stretch(ForestEdgeId(2)).unwrap(),
            ExactRatio::new(8, 5).unwrap()
        );
        forest
            .verify_stretch_bounds(&[
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(8, 5).unwrap(),
            ])
            .unwrap();
        assert_eq!(
            forest.verify_stretch_bounds(&[
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(3, 2).unwrap(),
            ]),
            Err(RootedForestError::InvalidCertificate)
        );
    }

    #[test]
    fn supports_exact_root_path_updates_and_queries() {
        let mut forest = forest();
        forest.add_to_root_path(FlowNodeId(2), 3).unwrap();
        assert_eq!(forest.root_path_sum(FlowNodeId(2)).unwrap(), 9);
        assert_eq!(forest.root_path_sum(FlowNodeId(1)).unwrap(), 6);
        assert_eq!(forest.metrics().path_updates, 1);
        assert_eq!(forest.metrics().path_queries, 2);
    }

    #[test]
    fn deletion_removes_forest_edge_and_adds_a_root() {
        let mut forest = forest();
        forest.delete_edge(ForestEdgeId(1)).unwrap();
        assert_eq!(
            forest.stretch(ForestEdgeId(2)).unwrap(),
            ExactRatio::new(1, 1).unwrap()
        );
        assert_eq!(forest.metrics().forest_edge_removals, 1);
        assert_eq!(forest.metrics().root_additions, 1);
        assert_eq!(
            forest.delete_edge(ForestEdgeId(1)),
            Err(RootedForestError::InactiveEdge)
        );
    }

    #[test]
    fn split_detaches_moved_forest_edges_and_records_recourse() {
        let mut forest = forest();
        let split = forest
            .split_vertex(FlowNodeId(1), &[ForestEdgeId(0)])
            .unwrap();
        assert_eq!(split, FlowNodeId(3));
        assert_eq!(forest.metrics().vertex_splits, 1);
        assert_eq!(forest.metrics().forest_edge_removals, 1);
        forest.add_to_root_path(split, 4).unwrap();
        assert_eq!(forest.root_path_sum(split).unwrap(), 4);
    }
}
