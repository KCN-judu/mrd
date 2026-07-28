use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::{
    ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceGraphUpdate, SourceUpdateBatch,
    SourceWeightedEdge,
};

type RootedOrder = (Vec<Option<usize>>, Vec<usize>, Vec<usize>);

/// Appendix B.3 tree machinery used by the dynamic low-stretch forest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BranchFreeTree {
    root: usize,
    tree_edges: BTreeSet<SourceEdgeId>,
    tree_edge_data: BTreeMap<SourceEdgeId, (usize, usize, ExactRatio)>,
    adjacency: Vec<Vec<(usize, SourceEdgeId)>>,
    parent: Vec<Option<usize>>,
    depth: Vec<usize>,
    auxiliary_parent: Vec<Option<usize>>,
    auxiliary_depth: Vec<usize>,
    maximum_auxiliary_depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CongestionOrder {
    pub ordered_tree_edges: Vec<SourceEdgeId>,
    pub exact_congestion: Vec<ExactRatio>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GlobalStretchCertificate {
    pub stretch_overestimates: Vec<ExactRatio>,
    pub auxiliary_levels: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DynamicLsfCoreMetrics {
    pub batches: u64,
    pub root_additions: u64,
    pub forest_edge_removals: u64,
    pub stretch_checks: u64,
    pub vertex_splits: u64,
}

/// Source update mechanics around a fixed Appendix B.3 tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicLsfCore {
    graph: SourceDynamicGraph,
    tree: BranchFreeTree,
    ordered_tree_edges: Vec<SourceEdgeId>,
    roots: BTreeSet<FlowNodeId>,
    forest_edges: BTreeSet<SourceEdgeId>,
    global_stretch: GlobalStretchCertificate,
    metrics: DynamicLsfCoreMetrics,
}

struct BatchRootInputs {
    terminals: Vec<FlowNodeId>,
    isolated_roots: Vec<FlowNodeId>,
    split_count: u64,
}

impl DynamicLsfCore {
    /// Initializes the fixed-tree dynamic LSF core from seed terminals.
    ///
    /// # Errors
    ///
    /// Returns an error when the tree, congestion order, root closure, forest,
    /// or global stretch vector cannot be certified.
    pub fn new(
        graph: SourceDynamicGraph,
        tree_edges: impl IntoIterator<Item = SourceEdgeId>,
        tree_root: FlowNodeId,
        seed_terminals: impl IntoIterator<Item = FlowNodeId>,
    ) -> Result<Self, SourceLsfConstructionError> {
        let tree = BranchFreeTree::new(&graph, tree_edges, tree_root)?;
        let order = tree.congestion_order(&graph)?;
        let roots = tree.ancestor_closure(seed_terminals)?;
        if roots.is_empty() {
            return Err(SourceLsfConstructionError::InvalidRoots);
        }
        let forest_edges = tree.forest_for_roots(&roots, &order.ordered_tree_edges)?;
        let global_stretch =
            tree.global_stretch_overestimates(&graph, &order.ordered_tree_edges)?;
        let stretch_checks = tree.certify_global_stretch_for_roots(
            &graph,
            &roots,
            &order.ordered_tree_edges,
            &global_stretch,
        )?;
        Ok(Self {
            graph,
            tree,
            ordered_tree_edges: order.ordered_tree_edges,
            roots,
            forest_edges,
            global_stretch,
            metrics: DynamicLsfCoreMetrics {
                stretch_checks,
                ..DynamicLsfCoreMetrics::default()
            },
        })
    }

    #[must_use]
    pub const fn graph(&self) -> &SourceDynamicGraph {
        &self.graph
    }

    #[must_use]
    pub const fn roots(&self) -> &BTreeSet<FlowNodeId> {
        &self.roots
    }

    #[must_use]
    pub const fn forest_edges(&self) -> &BTreeSet<SourceEdgeId> {
        &self.forest_edges
    }

    #[must_use]
    pub const fn global_stretch(&self) -> &GlobalStretchCertificate {
        &self.global_stretch
    }

    #[must_use]
    pub const fn metrics(&self) -> DynamicLsfCoreMetrics {
        self.metrics
    }

    /// Applies a source update batch atomically. Edge updates add both endpoint
    /// closures; a split adds the old endpoint closure and one isolated root.
    ///
    /// # Errors
    ///
    /// Returns an error for a graph update failure, nondecremental forest, or
    /// stretch certification failure.
    pub fn apply_edge_batch(
        &mut self,
        batch: &SourceUpdateBatch,
    ) -> Result<(), SourceLsfConstructionError> {
        let mut candidate = self.clone();
        let inputs = batch_root_inputs(&candidate.graph, batch)?;
        candidate
            .graph
            .apply_batch(batch)
            .map_err(|_| SourceLsfConstructionError::InvalidUpdate)?;
        while candidate.global_stretch.stretch_overestimates.len() < candidate.graph.edge_count() {
            candidate
                .global_stretch
                .stretch_overestimates
                .push(ratio(1)?);
        }
        let mut added = BTreeSet::new();
        for terminal in inputs.terminals {
            if terminal.0 < candidate.tree.parent.len() {
                added.extend(candidate.tree.ancestor_closure([terminal])?);
            } else {
                added.insert(terminal);
            }
        }
        added.extend(inputs.isolated_roots);
        let old_root_count = candidate.roots.len();
        candidate.roots.extend(added);
        let next_forest = candidate
            .tree
            .forest_for_roots(&candidate.roots, &candidate.ordered_tree_edges)?;
        if !next_forest.is_subset(&candidate.forest_edges) {
            return Err(SourceLsfConstructionError::NondecrementalForest);
        }
        let removed = candidate
            .forest_edges
            .len()
            .checked_sub(next_forest.len())
            .ok_or(SourceLsfConstructionError::Overflow)?;
        candidate.forest_edges = next_forest;
        let checks = candidate.tree.certify_global_stretch_for_roots(
            &candidate.graph,
            &candidate.roots,
            &candidate.ordered_tree_edges,
            &candidate.global_stretch,
        )?;
        candidate.metrics.batches = candidate
            .metrics
            .batches
            .checked_add(1)
            .ok_or(SourceLsfConstructionError::Overflow)?;
        candidate.metrics.root_additions = candidate
            .metrics
            .root_additions
            .checked_add(
                u64::try_from(candidate.roots.len() - old_root_count)
                    .map_err(|_| SourceLsfConstructionError::Overflow)?,
            )
            .ok_or(SourceLsfConstructionError::Overflow)?;
        candidate.metrics.forest_edge_removals = candidate
            .metrics
            .forest_edge_removals
            .checked_add(u64::try_from(removed).map_err(|_| SourceLsfConstructionError::Overflow)?)
            .ok_or(SourceLsfConstructionError::Overflow)?;
        candidate.metrics.stretch_checks = candidate
            .metrics
            .stretch_checks
            .checked_add(checks)
            .ok_or(SourceLsfConstructionError::Overflow)?;
        candidate.metrics.vertex_splits = candidate
            .metrics
            .vertex_splits
            .checked_add(inputs.split_count)
            .ok_or(SourceLsfConstructionError::Overflow)?;
        *self = candidate;
        Ok(())
    }
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
        let mut tree_edge_data = BTreeMap::new();
        for id in &tree_edges {
            let edge = graph
                .edge(*id)
                .ok_or(SourceLsfConstructionError::InvalidTree)?;
            adjacency[edge.first.0].push((edge.second.0, *id));
            adjacency[edge.second.0].push((edge.first.0, *id));
            tree_edge_data.insert(*id, (edge.first.0, edge.second.0, edge.length));
        }
        let (parent, depth, order) = rooted_order(&adjacency, root.0)?;
        let heavy_child = heavy_children(&adjacency, &parent, &order)?;
        let auxiliary_parent = build_auxiliary_parents(&parent, &heavy_child)?;
        let (auxiliary_depth, maximum_auxiliary_depth) =
            auxiliary_depths(&auxiliary_parent, root.0)?;
        Ok(Self {
            root: root.0,
            tree_edges,
            tree_edge_data,
            adjacency,
            parent,
            depth,
            auxiliary_parent,
            auxiliary_depth,
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
                if left.0 >= self.parent.len() || right.0 >= self.parent.len() {
                    left == right || left.0 >= self.parent.len() || right.0 >= self.parent.len()
                } else {
                    self.lowest_common_ancestor(left.0, right.0)
                        .is_some_and(|lca| roots.contains(&FlowNodeId(lca)))
                }
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
        let roots_vec = roots
            .iter()
            .copied()
            .filter(|root| root.0 < self.parent.len())
            .collect::<Vec<_>>();
        if roots_vec.is_empty() {
            return Err(SourceLsfConstructionError::InvalidRoots);
        }
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
        if removed.len().checked_add(1) != Some(roots_vec.len()) {
            return Err(SourceLsfConstructionError::InvalidRoots);
        }
        Ok(self.tree_edges.difference(&removed).copied().collect())
    }

    /// Computes Equation (56)'s fixed global stretch overestimates by summing
    /// twice the exact stretch in every auxiliary-depth prefix forest.
    ///
    /// # Errors
    ///
    /// Returns an error when an exact prefix forest or rational stretch cannot
    /// be certified.
    pub fn global_stretch_overestimates(
        &self,
        graph: &SourceDynamicGraph,
        ordered_tree_edges: &[SourceEdgeId],
    ) -> Result<GlobalStretchCertificate, SourceLsfConstructionError> {
        permutation_ranks(&self.tree_edges, ordered_tree_edges)?;
        let zero = ratio(0)?;
        let mut sums = vec![zero; graph.edge_count()];
        for level in 0..=self.maximum_auxiliary_depth {
            let roots = self
                .auxiliary_depth
                .iter()
                .enumerate()
                .filter(|(_, depth)| **depth <= level)
                .map(|(node, _)| FlowNodeId(node))
                .collect::<BTreeSet<_>>();
            let forest = self.forest_for_roots(&roots, ordered_tree_edges)?;
            let stretches = exact_forest_stretches(self, graph, &forest, &roots)?;
            for (sum, stretch) in sums.iter_mut().zip(stretches) {
                *sum = sum.checked_add(stretch).map_err(map_ratio)?;
            }
        }
        let stretch_overestimates = sums
            .into_iter()
            .map(|sum| sum.checked_mul_integer(2).map_err(map_ratio))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(GlobalStretchCertificate {
            stretch_overestimates,
            auxiliary_levels: self
                .maximum_auxiliary_depth
                .checked_add(1)
                .ok_or(SourceLsfConstructionError::Overflow)?,
        })
    }

    /// Recomputes a current ancestor-closed forest and proves every active
    /// exact stretch is bounded by the fixed Equation (56) vector.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid roots, dimensions, or a violated bound.
    pub fn certify_global_stretch_for_roots(
        &self,
        graph: &SourceDynamicGraph,
        roots: &BTreeSet<FlowNodeId>,
        ordered_tree_edges: &[SourceEdgeId],
        certificate: &GlobalStretchCertificate,
    ) -> Result<u64, SourceLsfConstructionError> {
        if certificate.stretch_overestimates.len() != graph.edge_count() {
            return Err(SourceLsfConstructionError::InvalidStretch);
        }
        let forest = self.forest_for_roots(roots, ordered_tree_edges)?;
        let exact = exact_forest_stretches(self, graph, &forest, roots)?;
        let mut checks = 0_u64;
        for (index, stretch) in exact.into_iter().enumerate() {
            if graph.edge(SourceEdgeId(index)).is_none() {
                continue;
            }
            if !certificate.stretch_overestimates[index]
                .at_least(stretch)
                .map_err(map_ratio)?
            {
                return Err(SourceLsfConstructionError::InvalidStretch);
            }
            checks = checks
                .checked_add(1)
                .ok_or(SourceLsfConstructionError::Overflow)?;
        }
        Ok(checks)
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

fn batch_root_inputs(
    graph: &SourceDynamicGraph,
    batch: &SourceUpdateBatch,
) -> Result<BatchRootInputs, SourceLsfConstructionError> {
    let mut terminals = Vec::new();
    let mut isolated_roots = Vec::new();
    let mut pending = BTreeMap::<SourceEdgeId, SourceWeightedEdge>::new();
    let mut next_edge = graph.edge_count();
    let mut next_node = graph.node_count();
    let mut split_count = 0_u64;
    for update in &batch.updates {
        match update {
            SourceGraphUpdate::Insert(edge) => {
                terminals.extend([edge.first, edge.second]);
                pending.insert(SourceEdgeId(next_edge), edge.clone());
                next_edge = next_edge
                    .checked_add(1)
                    .ok_or(SourceLsfConstructionError::Overflow)?;
            }
            SourceGraphUpdate::Delete(id) => {
                let edge = graph
                    .edge(*id)
                    .or_else(|| pending.get(id))
                    .ok_or(SourceLsfConstructionError::InvalidUpdate)?;
                terminals.extend([edge.first, edge.second]);
            }
            SourceGraphUpdate::SplitVertex { vertex, .. } => {
                terminals.push(*vertex);
                isolated_roots.push(FlowNodeId(next_node));
                next_node = next_node
                    .checked_add(1)
                    .ok_or(SourceLsfConstructionError::Overflow)?;
                split_count = split_count
                    .checked_add(1)
                    .ok_or(SourceLsfConstructionError::Overflow)?;
            }
        }
    }
    Ok(BatchRootInputs {
        terminals,
        isolated_roots,
        split_count,
    })
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
    #[error("global stretch certificate is invalid")]
    InvalidStretch,
    #[error("dynamic LSF update is invalid")]
    InvalidUpdate,
    #[error("updated rooted forest is not a subset of the previous forest")]
    NondecrementalForest,
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

fn auxiliary_depths(
    parent: &[Option<usize>],
    root: usize,
) -> Result<(Vec<usize>, usize), SourceLsfConstructionError> {
    let mut maximum = 0;
    let mut depths = vec![0_usize; parent.len()];
    for (start, slot) in depths.iter_mut().enumerate() {
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
        *slot = depth;
        maximum = maximum.max(depth);
    }
    Ok((depths, maximum))
}

fn exact_forest_stretches(
    tree: &BranchFreeTree,
    graph: &SourceDynamicGraph,
    forest: &BTreeSet<SourceEdgeId>,
    roots: &BTreeSet<FlowNodeId>,
) -> Result<Vec<ExactRatio>, SourceLsfConstructionError> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    for id in forest {
        let (first, second, _) = tree
            .tree_edge_data
            .get(id)
            .ok_or(SourceLsfConstructionError::InvalidTree)?;
        adjacency[*first].push((*second, *id));
        adjacency[*second].push((*first, *id));
    }
    let (component_of, component_roots) = forest_components(&adjacency, roots)?;
    let zero = ratio(0)?;
    let mut result = vec![zero; graph.edge_count()];
    for (index, slot) in result.iter_mut().enumerate() {
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        let route = if component_of[edge.first.0] == component_of[edge.second.0] {
            forest_distance(tree, &adjacency, edge.first.0, edge.second.0)?
        } else {
            forest_distance(
                tree,
                &adjacency,
                edge.first.0,
                component_roots[component_of[edge.first.0]],
            )?
            .checked_add(forest_distance(
                tree,
                &adjacency,
                edge.second.0,
                component_roots[component_of[edge.second.0]],
            )?)
            .map_err(map_ratio)?
        };
        *slot = edge
            .length
            .checked_add(route)
            .and_then(|value| value.checked_mul(edge.length.reciprocal()?))
            .map_err(map_ratio)?;
    }
    Ok(result)
}

fn forest_components(
    adjacency: &[Vec<(usize, SourceEdgeId)>],
    roots: &BTreeSet<FlowNodeId>,
) -> Result<(Vec<usize>, Vec<usize>), SourceLsfConstructionError> {
    let mut component_of = vec![usize::MAX; adjacency.len()];
    let mut component_roots = Vec::new();
    for start in 0..adjacency.len() {
        if component_of[start] != usize::MAX {
            continue;
        }
        let component = component_roots.len();
        let mut queue = VecDeque::from([start]);
        let mut members = Vec::new();
        component_of[start] = component;
        while let Some(node) = queue.pop_front() {
            members.push(node);
            for (next, _) in &adjacency[node] {
                if component_of[*next] == usize::MAX {
                    component_of[*next] = component;
                    queue.push_back(*next);
                }
            }
        }
        let component_root = members
            .into_iter()
            .filter(|node| roots.contains(&FlowNodeId(*node)))
            .collect::<Vec<_>>();
        if component_root.len() != 1 {
            return Err(SourceLsfConstructionError::InvalidRoots);
        }
        component_roots.push(component_root[0]);
    }
    Ok((component_of, component_roots))
}

fn forest_distance(
    tree: &BranchFreeTree,
    adjacency: &[Vec<(usize, SourceEdgeId)>],
    start: usize,
    target: usize,
) -> Result<ExactRatio, SourceLsfConstructionError> {
    let zero = ratio(0)?;
    let mut queue = VecDeque::from([(start, zero)]);
    let mut seen = vec![false; adjacency.len()];
    seen[start] = true;
    while let Some((node, distance)) = queue.pop_front() {
        if node == target {
            return Ok(distance);
        }
        for (next, id) in &adjacency[node] {
            if !seen[*next] {
                seen[*next] = true;
                let length = tree
                    .tree_edge_data
                    .get(id)
                    .ok_or(SourceLsfConstructionError::InvalidTree)?
                    .2;
                queue.push_back((*next, distance.checked_add(length).map_err(map_ratio)?));
            }
        }
    }
    Err(SourceLsfConstructionError::InvalidTree)
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

    use super::{BranchFreeTree, DynamicLsfCore};
    use crate::{
        ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceGraphUpdate,
        SourceUpdateBatch, SourceWeightedEdge,
    };

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
        let global = tree
            .global_stretch_overestimates(&graph, &order.ordered_tree_edges)
            .unwrap();
        assert_eq!(global.auxiliary_levels, tree.maximum_auxiliary_depth() + 1);
        assert_eq!(
            tree.certify_global_stretch_for_roots(
                &graph,
                &roots,
                &order.ordered_tree_edges,
                &global,
            )
            .unwrap(),
            5
        );
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

    #[test]
    fn edge_updates_add_roots_and_only_remove_forest_edges() {
        let graph = SourceDynamicGraph::new(
            4,
            vec![edge(0, 1, 1), edge(1, 2, 1), edge(2, 3, 1), edge(0, 3, 2)],
            8,
        )
        .unwrap();
        let mut core = DynamicLsfCore::new(
            graph,
            [SourceEdgeId(0), SourceEdgeId(1), SourceEdgeId(2)],
            FlowNodeId(0),
            [FlowNodeId(0)],
        )
        .unwrap();
        let initial_forest = core.forest_edges().clone();
        core.apply_edge_batch(&SourceUpdateBatch {
            updates: vec![SourceGraphUpdate::Insert(edge(1, 3, 1))],
        })
        .unwrap();
        assert!(core.forest_edges().is_subset(&initial_forest));
        assert_eq!(
            core.global_stretch().stretch_overestimates[SourceEdgeId(4).0],
            ExactRatio::new(1, 1).unwrap()
        );
        core.apply_edge_batch(&SourceUpdateBatch {
            updates: vec![SourceGraphUpdate::SplitVertex {
                vertex: FlowNodeId(1),
                moved_edges: vec![SourceEdgeId(4)],
            }],
        })
        .unwrap();
        assert_eq!(core.graph().node_count(), 5);
        assert!(core.roots().contains(&FlowNodeId(4)));
        assert_eq!(core.metrics().vertex_splits, 1);
        let after_insert = core.forest_edges().clone();
        core.apply_edge_batch(&SourceUpdateBatch {
            updates: vec![SourceGraphUpdate::Delete(SourceEdgeId(0))],
        })
        .unwrap();
        assert!(core.forest_edges().is_subset(&after_insert));
        assert!(!core.forest_edges().contains(&SourceEdgeId(0)));
        assert_eq!(core.metrics().batches, 3);
        assert!(core.metrics().root_additions > 0);
        assert!(core.metrics().stretch_checks > 0);
    }
}
