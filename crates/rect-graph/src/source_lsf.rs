use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId};

type RootedOrder = (Vec<Option<usize>>, Vec<usize>, Vec<usize>);

/// Appendix B.3 tree machinery used by the dynamic low-stretch forest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchFreeTree {
    root: usize,
    tree_edges: BTreeSet<SourceEdgeId>,
    adjacency: Vec<Vec<(usize, SourceEdgeId)>>,
    parent: Vec<Option<usize>>,
    depth: Vec<usize>,
    auxiliary_parent: Vec<Option<usize>>,
    maximum_auxiliary_depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CongestionOrder {
    pub ordered_tree_edges: Vec<SourceEdgeId>,
    pub exact_congestion: Vec<ExactRatio>,
}

impl BranchFreeTree {
    /// Builds the heavy-light/balanced-BST auxiliary tree from Lemma B.9.
    ///
    /// # Errors
    ///
    /// Returns an error unless `tree_edges` form a spanning tree of `graph`
    /// rooted at `root` and all exact arithmetic remains defined.
    pub fn new(
        graph: &SourceDynamicGraph,
        tree_edges: impl IntoIterator<Item = SourceEdgeId>,
        root: FlowNodeId,
    ) -> Result<Self, SourceLsfConstructionError> {
        if root.0 >= graph.node_count() {
            return Err(SourceLsfConstructionError::InvalidTree);
        }
        let tree_edges = tree_edges.into_iter().collect::<BTreeSet<_>>();
        if tree_edges.len().checked_add(1) != Some(graph.node_count()) {
            return Err(SourceLsfConstructionError::InvalidTree);
        }
        let mut adjacency = vec![Vec::new(); graph.node_count()];
        for id in &tree_edges {
            let edge = graph
                .edge(*id)
                .ok_or(SourceLsfConstructionError::InvalidTree)?;
            adjacency[edge.first.0].push((edge.second.0, *id));
            adjacency[edge.second.0].push((edge.first.0, *id));
        }
        let (parent, depth, order) = rooted_order(&adjacency, root.0)?;
        let heavy_child = heavy_children(&adjacency, &parent, &order)?;
        let auxiliary_parent = build_auxiliary_parents(&parent, &heavy_child)?;
        let maximum_auxiliary_depth = auxiliary_height(&auxiliary_parent, root.0)?;
        Ok(Self {
            root: root.0,
            tree_edges,
            adjacency,
            parent,
            depth,
            auxiliary_parent,
            maximum_auxiliary_depth,
        })
    }

    #[must_use]
    pub const fn maximum_auxiliary_depth(&self) -> usize {
        self.maximum_auxiliary_depth
    }

    /// Returns `R upward T_H`, including every supplied terminal.
    ///
    /// # Errors
    ///
    /// Returns an error for a terminal outside the tree.
    pub fn ancestor_closure(
        &self,
        terminals: impl IntoIterator<Item = FlowNodeId>,
    ) -> Result<BTreeSet<FlowNodeId>, SourceLsfConstructionError> {
        let mut result = BTreeSet::new();
        for terminal in terminals {
            if terminal.0 >= self.parent.len() {
                return Err(SourceLsfConstructionError::NodeOutOfBounds);
            }
            let mut current = Some(terminal.0);
            while let Some(node) = current {
                result.insert(FlowNodeId(node));
                current = self.auxiliary_parent[node];
            }
        }
        Ok(result)
    }

    /// Checks branch freedom directly in the original rooted tree.
    #[must_use]
    pub fn is_branch_free(&self, roots: &BTreeSet<FlowNodeId>) -> bool {
        roots.iter().all(|left| {
            roots.iter().all(|right| {
                self.lowest_common_ancestor(left.0, right.0)
                    .is_some_and(|lca| roots.contains(&FlowNodeId(lca)))
            })
        })
    }

    /// Computes Definition B.5 congestion and the deterministic increasing
    /// permutation `pi`, breaking exact ties by stable edge identifier.
    ///
    /// # Errors
    ///
    /// Returns an error when an active graph edge has no tree path or exact
    /// rational accumulation overflows.
    pub fn congestion_order(
        &self,
        graph: &SourceDynamicGraph,
    ) -> Result<CongestionOrder, SourceLsfConstructionError> {
        let edge_to_slot = self
            .tree_edges
            .iter()
            .copied()
            .enumerate()
            .map(|(slot, id)| (id, slot))
            .collect::<BTreeMap<_, _>>();
        let zero = ratio(0)?;
        let mut exact_congestion = vec![zero; self.tree_edges.len()];
        for index in 0..graph.edge_count() {
            let Some(edge) = graph.edge(SourceEdgeId(index)) else {
                continue;
            };
            let reciprocal = edge.length.reciprocal().map_err(map_ratio)?;
            for id in self.path(edge.first.0, edge.second.0)?.1 {
                let slot = *edge_to_slot
                    .get(&id)
                    .ok_or(SourceLsfConstructionError::InvalidTree)?;
                exact_congestion[slot] = exact_congestion[slot]
                    .checked_add(reciprocal)
                    .map_err(map_ratio)?;
            }
        }
        let mut ordered_tree_edges = self.tree_edges.iter().copied().collect::<Vec<_>>();
        for index in 1..ordered_tree_edges.len() {
            let mut cursor = index;
            while cursor > 0
                && congestion_precedes(
                    ordered_tree_edges[cursor],
                    ordered_tree_edges[cursor - 1],
                    &edge_to_slot,
                    &exact_congestion,
                )?
            {
                ordered_tree_edges.swap(cursor, cursor - 1);
                cursor -= 1;
            }
        }
        Ok(CongestionOrder {
            ordered_tree_edges,
            exact_congestion,
        })
    }

    /// Constructs `F_T(R,pi)` by deleting the minimum-permutation edge from
    /// every path between adjacent branch-free roots.
    ///
    /// # Errors
    ///
    /// Returns an error unless roots are nonempty and branch-free and
    /// `ordered_tree_edges` is a permutation of the spanning tree.
    pub fn forest_for_roots(
        &self,
        roots: &BTreeSet<FlowNodeId>,
        ordered_tree_edges: &[SourceEdgeId],
    ) -> Result<BTreeSet<SourceEdgeId>, SourceLsfConstructionError> {
        if roots.is_empty() || !self.is_branch_free(roots) {
            return Err(SourceLsfConstructionError::InvalidRoots);
        }
        let rank = permutation_ranks(&self.tree_edges, ordered_tree_edges)?;
        let roots_vec = roots.iter().copied().collect::<Vec<_>>();
        let mut removed = BTreeSet::new();
        for first in 0..roots_vec.len() {
            for second in first + 1..roots_vec.len() {
                let (vertices, edges) = self.path(roots_vec[first].0, roots_vec[second].0)?;
                if vertices
                    .iter()
                    .filter(|vertex| roots.contains(&FlowNodeId(**vertex)))
                    .count()
                    != 2
                {
                    continue;
                }
                let removed_edge = edges
                    .into_iter()
                    .min_by_key(|id| rank[id])
                    .ok_or(SourceLsfConstructionError::InvalidRoots)?;
                removed.insert(removed_edge);
            }
        }
        if removed.len().checked_add(1) != Some(roots.len()) {
            return Err(SourceLsfConstructionError::InvalidRoots);
        }
        Ok(self.tree_edges.difference(&removed).copied().collect())
    }

    fn path(
        &self,
        first: usize,
        second: usize,
    ) -> Result<(Vec<usize>, Vec<SourceEdgeId>), SourceLsfConstructionError> {
        if first >= self.parent.len() || second >= self.parent.len() {
            return Err(SourceLsfConstructionError::NodeOutOfBounds);
        }
        let mut left = first;
        let mut right = second;
        let mut left_vertices = vec![left];
        let mut right_vertices = vec![right];
        let mut left_edges = Vec::new();
        let mut right_edges = Vec::new();
        while self.depth[left] > self.depth[right] {
            let (parent, edge) = self.parent_step(left)?;
            left_edges.push(edge);
            left = parent;
            left_vertices.push(left);
        }
        while self.depth[right] > self.depth[left] {
            let (parent, edge) = self.parent_step(right)?;
            right_edges.push(edge);
            right = parent;
            right_vertices.push(right);
        }
        while left != right {
            let (left_parent, left_edge) = self.parent_step(left)?;
            let (right_parent, right_edge) = self.parent_step(right)?;
            left_edges.push(left_edge);
            right_edges.push(right_edge);
            left = left_parent;
            right = right_parent;
            left_vertices.push(left);
            right_vertices.push(right);
        }
        right_vertices.pop();
        right_vertices.reverse();
        left_vertices.extend(right_vertices);
        right_edges.reverse();
        left_edges.extend(right_edges);
        Ok((left_vertices, left_edges))
    }

    fn parent_step(
        &self,
        node: usize,
    ) -> Result<(usize, SourceEdgeId), SourceLsfConstructionError> {
        let parent = self.parent[node].ok_or(SourceLsfConstructionError::InvalidTree)?;
        let edge = self.adjacency[node]
            .iter()
            .find(|(next, _)| *next == parent)
            .map(|(_, id)| *id)
            .ok_or(SourceLsfConstructionError::InvalidTree)?;
        Ok((parent, edge))
    }

    fn lowest_common_ancestor(&self, mut left: usize, mut right: usize) -> Option<usize> {
        while self.depth[left] > self.depth[right] {
            left = self.parent[left]?;
        }
        while self.depth[right] > self.depth[left] {
            right = self.parent[right]?;
        }
        while left != right {
            left = self.parent[left]?;
            right = self.parent[right]?;
        }
        Some(left)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceLsfConstructionError {
    #[error("tree edges do not form a rooted spanning tree")]
    InvalidTree,
    #[error("root set is empty, non-branch-free, or inconsistent")]
    InvalidRoots,
    #[error("node is outside the source tree")]
    NodeOutOfBounds,
    #[error("tree-edge order is not a valid permutation")]
    InvalidPermutation,
    #[error("checked source construction arithmetic overflowed")]
    Overflow,
}

fn rooted_order(
    adjacency: &[Vec<(usize, SourceEdgeId)>],
    root: usize,
) -> Result<RootedOrder, SourceLsfConstructionError> {
    let mut parent = vec![None; adjacency.len()];
    let mut depth = vec![0_usize; adjacency.len()];
    let mut order = Vec::with_capacity(adjacency.len());
    let mut queue = VecDeque::from([root]);
    let mut seen = vec![false; adjacency.len()];
    seen[root] = true;
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for (next, _) in &adjacency[node] {
            if !seen[*next] {
                seen[*next] = true;
                parent[*next] = Some(node);
                depth[*next] = depth[node]
                    .checked_add(1)
                    .ok_or(SourceLsfConstructionError::Overflow)?;
                queue.push_back(*next);
            }
        }
    }
    if seen.contains(&false) {
        return Err(SourceLsfConstructionError::InvalidTree);
    }
    Ok((parent, depth, order))
}

fn heavy_children(
    adjacency: &[Vec<(usize, SourceEdgeId)>],
    parent: &[Option<usize>],
    order: &[usize],
) -> Result<Vec<Option<usize>>, SourceLsfConstructionError> {
    let mut subtree_size = vec![1_usize; adjacency.len()];
    let mut heavy_child = vec![None; adjacency.len()];
    for node in order.iter().rev().copied() {
        let mut best = None;
        for (child, _) in &adjacency[node] {
            if parent[*child] != Some(node) {
                continue;
            }
            subtree_size[node] = subtree_size[node]
                .checked_add(subtree_size[*child])
                .ok_or(SourceLsfConstructionError::Overflow)?;
            if best.is_none_or(|old| subtree_size[*child] > subtree_size[old]) {
                best = Some(*child);
            }
        }
        heavy_child[node] = best;
    }
    Ok(heavy_child)
}

fn build_auxiliary_parents(
    parent: &[Option<usize>],
    heavy_child: &[Option<usize>],
) -> Result<Vec<Option<usize>>, SourceLsfConstructionError> {
    let mut auxiliary_parent = vec![None; parent.len()];
    for head in 0..parent.len() {
        if parent[head].is_some_and(|value| heavy_child[value] == Some(head)) {
            continue;
        }
        let mut chain = vec![head];
        let mut current = head;
        while let Some(next) = heavy_child[current] {
            chain.push(next);
            current = next;
        }
        auxiliary_parent[head] = parent[head];
        if chain.len() > 1 {
            assign_balanced_parents(&chain[1..], head, &mut auxiliary_parent)?;
        }
    }
    Ok(auxiliary_parent)
}

fn assign_balanced_parents(
    vertices: &[usize],
    parent: usize,
    auxiliary_parent: &mut [Option<usize>],
) -> Result<(), SourceLsfConstructionError> {
    if vertices.is_empty() {
        return Ok(());
    }
    let middle = vertices.len() / 2;
    let root = vertices[middle];
    if auxiliary_parent[root].replace(parent).is_some() {
        return Err(SourceLsfConstructionError::InvalidTree);
    }
    assign_balanced_parents(&vertices[..middle], root, auxiliary_parent)?;
    assign_balanced_parents(&vertices[middle + 1..], root, auxiliary_parent)
}

fn auxiliary_height(
    parent: &[Option<usize>],
    root: usize,
) -> Result<usize, SourceLsfConstructionError> {
    let mut maximum = 0;
    for start in 0..parent.len() {
        let mut depth = 0_usize;
        let mut current = start;
        let mut seen = BTreeSet::new();
        while current != root {
            if !seen.insert(current) {
                return Err(SourceLsfConstructionError::InvalidTree);
            }
            current = parent[current].ok_or(SourceLsfConstructionError::InvalidTree)?;
            depth = depth
                .checked_add(1)
                .ok_or(SourceLsfConstructionError::Overflow)?;
        }
        maximum = maximum.max(depth);
    }
    Ok(maximum)
}

fn congestion_precedes(
    left: SourceEdgeId,
    right: SourceEdgeId,
    edge_to_slot: &BTreeMap<SourceEdgeId, usize>,
    congestion: &[ExactRatio],
) -> Result<bool, SourceLsfConstructionError> {
    let left_value = congestion[edge_to_slot[&left]];
    let right_value = congestion[edge_to_slot[&right]];
    if left_value == right_value {
        return Ok(left < right);
    }
    right_value.at_least(left_value).map_err(map_ratio)
}

fn permutation_ranks(
    tree_edges: &BTreeSet<SourceEdgeId>,
    order: &[SourceEdgeId],
) -> Result<BTreeMap<SourceEdgeId, usize>, SourceLsfConstructionError> {
    if order.len() != tree_edges.len()
        || order.iter().copied().collect::<BTreeSet<_>>() != *tree_edges
    {
        return Err(SourceLsfConstructionError::InvalidPermutation);
    }
    Ok(order
        .iter()
        .copied()
        .enumerate()
        .map(|(rank, id)| (id, rank))
        .collect())
}

fn ratio(value: i128) -> Result<ExactRatio, SourceLsfConstructionError> {
    ExactRatio::new(value, 1).map_err(map_ratio)
}

fn map_ratio(_: crate::StableMinRatioError) -> SourceLsfConstructionError {
    SourceLsfConstructionError::Overflow
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::BranchFreeTree;
    use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge};

    fn edge(first: usize, second: usize, length: i128) -> SourceWeightedEdge {
        SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(length, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        }
    }

    #[test]
    fn constructs_branch_free_closure_and_decremental_forest() {
        let graph = SourceDynamicGraph::new(
            5,
            vec![
                edge(0, 1, 1),
                edge(1, 2, 1),
                edge(2, 3, 1),
                edge(1, 4, 1),
                edge(3, 4, 2),
            ],
            8,
        )
        .unwrap();
        let tree = BranchFreeTree::new(
            &graph,
            [
                SourceEdgeId(0),
                SourceEdgeId(1),
                SourceEdgeId(2),
                SourceEdgeId(3),
            ],
            FlowNodeId(0),
        )
        .unwrap();
        assert!(tree.maximum_auxiliary_depth() <= 4);
        let roots = tree
            .ancestor_closure([FlowNodeId(3), FlowNodeId(4)])
            .unwrap();
        assert!(tree.is_branch_free(&roots));
        let order = tree.congestion_order(&graph).unwrap();
        let forest = tree
            .forest_for_roots(&roots, &order.ordered_tree_edges)
            .unwrap();
        assert_eq!(forest.len() + roots.len(), graph.node_count());

        let expanded = tree
            .ancestor_closure(roots.iter().copied().chain([FlowNodeId(2)]))
            .unwrap();
        let smaller_forest = tree
            .forest_for_roots(&expanded, &order.ordered_tree_edges)
            .unwrap();
        assert!(smaller_forest.is_subset(&forest));
    }

    #[test]
    fn rejects_non_branch_free_root_set() {
        let graph = SourceDynamicGraph::new(3, vec![edge(0, 1, 1), edge(0, 2, 1)], 4).unwrap();
        let tree =
            BranchFreeTree::new(&graph, [SourceEdgeId(0), SourceEdgeId(1)], FlowNodeId(0)).unwrap();
        let roots = BTreeSet::from([FlowNodeId(1), FlowNodeId(2)]);
        assert!(!tree.is_branch_free(&roots));
    }
}
