use std::collections::{BTreeSet, VecDeque};

use thiserror::Error;

use crate::FlowNodeId;

/// Identifier for an edge of a simple undirected decremental graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpannerEdgeId(pub usize);

/// One unweighted undirected input edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpannerEdge {
    pub first: FlowNodeId,
    pub second: FlowNodeId,
}

/// Explicit subgraph and path embedding certificate for the current graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpannerCertificate {
    pub spanner_edges: BTreeSet<SpannerEdgeId>,
    pub embedding_paths: Vec<Vec<SpannerEdgeId>>,
    pub reembedded_edges: BTreeSet<SpannerEdgeId>,
}

/// Exact certificate measurements, not a theorem-level complexity bound.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SpannerMetrics {
    pub deletion_count: u64,
    pub vertex_split_count: u64,
    pub certificate_count: u64,
    pub reembedded_edge_count: u64,
    pub maximum_path_length: u64,
    pub maximum_vertex_congestion: u64,
}

/// Checked, scope-limited decremental simple-undirected spanner certificates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecrementalSpanner {
    node_count: usize,
    edges: Vec<SpannerEdge>,
    active: Vec<bool>,
    certificate: SpannerCertificate,
    metrics: SpannerMetrics,
}

impl DecrementalSpanner {
    /// Constructs a deterministic greedy unweighted spanner certificate.
    ///
    /// Edges are considered in identifier order. An edge is retained exactly
    /// when the already retained subgraph has no path with at most
    /// `maximum_hops` edges between its endpoints. This is a static/rebuild
    /// Oracle, not the dynamic expander-based construction of Theorem 8.2.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid graph, a zero hop bound, or an invalid
    /// internally produced certificate.
    pub fn build_greedy(
        node_count: usize,
        edges: Vec<SpannerEdge>,
        maximum_hops: usize,
    ) -> Result<Self, SpannerError> {
        if maximum_hops == 0 {
            return Err(SpannerError::InvalidHopBound);
        }
        validate_simple(node_count, &edges)?;
        let active = vec![true; edges.len()];
        let certificate = greedy_certificate(node_count, &edges, &active, maximum_hops)?;
        Self::new(node_count, edges, certificate)
    }

    /// Initializes a simple undirected graph and validates its certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for a loop, parallel edge, invalid endpoint, or invalid
    /// spanner embedding certificate.
    pub fn new(
        node_count: usize,
        edges: Vec<SpannerEdge>,
        certificate: SpannerCertificate,
    ) -> Result<Self, SpannerError> {
        validate_simple(node_count, &edges)?;
        let mut result = Self {
            node_count,
            active: vec![true; edges.len()],
            edges,
            certificate,
            metrics: SpannerMetrics::default(),
        };
        result.install_certificate()?;
        Ok(result)
    }

    /// Applies a deletion-only batch and validates the replacement certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicates, inactive edges, or an invalid certificate.
    pub fn delete_edges(
        &mut self,
        deleted: &[SpannerEdgeId],
        certificate: SpannerCertificate,
    ) -> Result<(), SpannerError> {
        let mut seen = BTreeSet::new();
        for edge in deleted {
            if edge.0 >= self.edges.len() || !self.active[edge.0] || !seen.insert(*edge) {
                return Err(SpannerError::InvalidDeletion);
            }
        }
        for edge in deleted {
            self.active[edge.0] = false;
        }
        self.metrics.deletion_count +=
            u64::try_from(deleted.len()).map_err(|_| SpannerError::Overflow)?;
        self.certificate = certificate;
        self.install_certificate()
    }

    /// Rebuilds a deterministic greedy certificate for the current active
    /// graph. This has no theorem-level recourse or runtime guarantee.
    ///
    /// # Errors
    ///
    /// Returns an error for a zero hop bound, an invalid graph, or exact
    /// counter overflow.
    pub fn rebuild_greedy(&mut self, maximum_hops: usize) -> Result<(), SpannerError> {
        if maximum_hops == 0 {
            return Err(SpannerError::InvalidHopBound);
        }
        self.certificate =
            greedy_certificate(self.node_count, &self.edges, &self.active, maximum_hops)?;
        self.install_certificate()
    }

    /// Applies one permitted vertex split by moving listed active incident edges
    /// to a new vertex, then validates a replacement certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid split or certificate.
    pub fn split_vertex(
        &mut self,
        node: FlowNodeId,
        moved: &[SpannerEdgeId],
        certificate: SpannerCertificate,
    ) -> Result<FlowNodeId, SpannerError> {
        if node.0 >= self.node_count {
            return Err(SpannerError::NodeOutOfBounds);
        }
        let mut seen = BTreeSet::new();
        for edge in moved {
            if edge.0 >= self.edges.len()
                || !self.active[edge.0]
                || !seen.insert(*edge)
                || (self.edges[edge.0].first != node && self.edges[edge.0].second != node)
            {
                return Err(SpannerError::InvalidSplit);
            }
        }
        let split = FlowNodeId(self.node_count);
        self.node_count += 1;
        for edge in moved {
            let entry = &mut self.edges[edge.0];
            if entry.first == node {
                entry.first = split;
            } else {
                entry.second = split;
            }
        }
        validate_simple(self.node_count, &self.edges)?;
        self.metrics.vertex_split_count += 1;
        self.certificate = certificate;
        self.install_certificate()?;
        Ok(split)
    }

    /// Recomputes and validates every active embedding path and connectivity.
    ///
    /// # Errors
    ///
    /// Returns an error when a path does not use active spanner edges, fails to
    /// connect its input endpoints, repeats a vertex, or omits reachability.
    pub fn verify_embedding(&self) -> Result<(), SpannerError> {
        if self.certificate.embedding_paths.len() != self.edges.len()
            || self
                .certificate
                .spanner_edges
                .iter()
                .any(|edge| edge.0 >= self.edges.len() || !self.active[edge.0])
        {
            return Err(SpannerError::InvalidCertificate);
        }
        let adjacency = self.spanner_adjacency();
        for (index, edge) in self.edges.iter().enumerate() {
            if !self.active[index] {
                if !self.certificate.embedding_paths[index].is_empty() {
                    return Err(SpannerError::InvalidCertificate);
                }
                continue;
            }
            validate_path(
                edge,
                &self.certificate.embedding_paths[index],
                &self.edges,
                &self.certificate.spanner_edges,
            )?;
            if !reachable(&adjacency, edge.first.0, edge.second.0) {
                return Err(SpannerError::InvalidCertificate);
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn metrics(&self) -> SpannerMetrics {
        self.metrics
    }

    fn install_certificate(&mut self) -> Result<(), SpannerError> {
        self.verify_embedding()?;
        let mut congestion = vec![0_u64; self.node_count];
        let mut maximum_path_length = 0_u64;
        for (index, path) in self.certificate.embedding_paths.iter().enumerate() {
            if !self.active[index] {
                continue;
            }
            maximum_path_length = maximum_path_length
                .max(u64::try_from(path.len()).map_err(|_| SpannerError::Overflow)?);
            let vertices = path_vertices(self.edges[index].first.0, path, &self.edges)?;
            for vertex in vertices {
                congestion[vertex] = congestion[vertex]
                    .checked_add(1)
                    .ok_or(SpannerError::Overflow)?;
            }
        }
        self.metrics.certificate_count += 1;
        self.metrics.reembedded_edge_count +=
            u64::try_from(self.certificate.reembedded_edges.len())
                .map_err(|_| SpannerError::Overflow)?;
        self.metrics.maximum_path_length =
            self.metrics.maximum_path_length.max(maximum_path_length);
        self.metrics.maximum_vertex_congestion = self
            .metrics
            .maximum_vertex_congestion
            .max(*congestion.iter().max().unwrap_or(&0));
        Ok(())
    }

    fn spanner_adjacency(&self) -> Vec<Vec<usize>> {
        let mut adjacency = vec![Vec::new(); self.node_count];
        for edge in &self.certificate.spanner_edges {
            let value = &self.edges[edge.0];
            adjacency[value.first.0].push(value.second.0);
            adjacency[value.second.0].push(value.first.0);
        }
        adjacency
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SpannerError {
    #[error("graph must be simple, undirected, and have valid endpoints")]
    InvalidGraph,
    #[error("node is outside the graph")]
    NodeOutOfBounds,
    #[error("deletion batch is invalid")]
    InvalidDeletion,
    #[error("vertex split is invalid")]
    InvalidSplit,
    #[error("spanner or embedding certificate is invalid")]
    InvalidCertificate,
    #[error("greedy spanner hop bound must be positive")]
    InvalidHopBound,
    #[error("exact counter arithmetic overflowed")]
    Overflow,
}

fn greedy_certificate(
    node_count: usize,
    edges: &[SpannerEdge],
    active: &[bool],
    maximum_hops: usize,
) -> Result<SpannerCertificate, SpannerError> {
    if active.len() != edges.len() {
        return Err(SpannerError::InvalidGraph);
    }
    validate_simple(node_count, edges)?;
    let mut spanner_edges = BTreeSet::new();
    let mut paths = vec![Vec::new(); edges.len()];
    for (index, edge) in edges.iter().enumerate() {
        if !active[index] {
            continue;
        }
        let id = SpannerEdgeId(index);
        let path = bounded_spanner_path(
            node_count,
            edge.first.0,
            edge.second.0,
            edges,
            &spanner_edges,
            maximum_hops,
        )?;
        if let Some(path) = path {
            paths[index] = path;
        } else {
            spanner_edges.insert(id);
            paths[index] = vec![id];
        }
    }
    let reembedded_edges = (0..edges.len())
        .filter(|index| active[*index] && !spanner_edges.contains(&SpannerEdgeId(*index)))
        .map(SpannerEdgeId)
        .collect();
    Ok(SpannerCertificate {
        spanner_edges,
        embedding_paths: paths,
        reembedded_edges,
    })
}

fn bounded_spanner_path(
    node_count: usize,
    start: usize,
    target: usize,
    edges: &[SpannerEdge],
    spanner: &BTreeSet<SpannerEdgeId>,
    maximum_hops: usize,
) -> Result<Option<Vec<SpannerEdgeId>>, SpannerError> {
    let mut adjacency = vec![Vec::<(usize, SpannerEdgeId)>::new(); node_count];
    for id in spanner {
        let edge = edges.get(id.0).ok_or(SpannerError::InvalidCertificate)?;
        adjacency[edge.first.0].push((edge.second.0, *id));
        adjacency[edge.second.0].push((edge.first.0, *id));
    }
    let mut predecessor = vec![None; node_count];
    let mut depth = vec![0_usize; node_count];
    let mut queue = VecDeque::from([start]);
    predecessor[start] = Some((start, SpannerEdgeId(usize::MAX)));
    while let Some(node) = queue.pop_front() {
        if node == target {
            let mut path = Vec::new();
            let mut current = target;
            while current != start {
                let (previous, edge) =
                    predecessor[current].ok_or(SpannerError::InvalidCertificate)?;
                path.push(edge);
                current = previous;
            }
            path.reverse();
            return Ok(Some(path));
        }
        if depth[node] == maximum_hops {
            continue;
        }
        for (next, edge) in &adjacency[node] {
            if predecessor[*next].is_none() {
                predecessor[*next] = Some((node, *edge));
                depth[*next] = depth[node].checked_add(1).ok_or(SpannerError::Overflow)?;
                queue.push_back(*next);
            }
        }
    }
    Ok(None)
}

fn validate_simple(node_count: usize, edges: &[SpannerEdge]) -> Result<(), SpannerError> {
    let mut pairs = BTreeSet::new();
    for edge in edges {
        if edge.first.0 >= node_count || edge.second.0 >= node_count || edge.first == edge.second {
            return Err(SpannerError::InvalidGraph);
        }
        let pair = (edge.first.min(edge.second), edge.first.max(edge.second));
        if !pairs.insert(pair) {
            return Err(SpannerError::InvalidGraph);
        }
    }
    Ok(())
}

fn validate_path(
    edge: &SpannerEdge,
    path: &[SpannerEdgeId],
    edges: &[SpannerEdge],
    spanner: &BTreeSet<SpannerEdgeId>,
) -> Result<(), SpannerError> {
    if path.is_empty() {
        return Err(SpannerError::InvalidCertificate);
    }
    let vertices = path_vertices(edge.first.0, path, edges)?;
    if vertices.last() != Some(&edge.second.0)
        || vertices.iter().collect::<BTreeSet<_>>().len() != vertices.len()
        || path.iter().any(|id| !spanner.contains(id))
    {
        return Err(SpannerError::InvalidCertificate);
    }
    Ok(())
}

fn path_vertices(
    start: usize,
    path: &[SpannerEdgeId],
    edges: &[SpannerEdge],
) -> Result<Vec<usize>, SpannerError> {
    let mut vertices = vec![start];
    let mut current = start;
    for id in path {
        let edge = edges.get(id.0).ok_or(SpannerError::InvalidCertificate)?;
        current = if edge.first.0 == current {
            edge.second.0
        } else if edge.second.0 == current {
            edge.first.0
        } else {
            return Err(SpannerError::InvalidCertificate);
        };
        vertices.push(current);
    }
    Ok(vertices)
}

fn reachable(adjacency: &[Vec<usize>], start: usize, target: usize) -> bool {
    let mut seen = vec![false; adjacency.len()];
    let mut queue = VecDeque::from([start]);
    seen[start] = true;
    while let Some(node) = queue.pop_front() {
        if node == target {
            return true;
        }
        for next in &adjacency[node] {
            if !seen[*next] {
                seen[*next] = true;
                queue.push_back(*next);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{DecrementalSpanner, SpannerCertificate, SpannerEdge, SpannerEdgeId, SpannerError};
    use crate::FlowNodeId;

    fn certificate(paths: Vec<Vec<SpannerEdgeId>>, edges: &[usize]) -> SpannerCertificate {
        SpannerCertificate {
            spanner_edges: edges
                .iter()
                .copied()
                .map(SpannerEdgeId)
                .collect::<BTreeSet<_>>(),
            embedding_paths: paths,
            reembedded_edges: BTreeSet::new(),
        }
    }

    fn spanner() -> DecrementalSpanner {
        DecrementalSpanner::new(
            3,
            vec![
                SpannerEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                },
                SpannerEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                },
                SpannerEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                },
            ],
            certificate(
                vec![
                    vec![SpannerEdgeId(0)],
                    vec![SpannerEdgeId(1)],
                    vec![SpannerEdgeId(0), SpannerEdgeId(1)],
                ],
                &[0, 1],
            ),
        )
        .unwrap()
    }

    #[test]
    fn validates_simple_undirected_embedding_and_measures_congestion() {
        let state = spanner();
        state.verify_embedding().unwrap();
        assert_eq!(state.metrics().maximum_path_length, 2);
        assert!(state.metrics().maximum_vertex_congestion >= 2);
    }

    #[test]
    fn greedily_constructs_and_rebuilds_a_checked_certificate() {
        let edges = vec![
            SpannerEdge {
                first: FlowNodeId(0),
                second: FlowNodeId(1),
            },
            SpannerEdge {
                first: FlowNodeId(1),
                second: FlowNodeId(2),
            },
            SpannerEdge {
                first: FlowNodeId(0),
                second: FlowNodeId(2),
            },
        ];
        let mut state = DecrementalSpanner::build_greedy(3, edges, 2).unwrap();
        state.verify_embedding().unwrap();
        assert_eq!(state.metrics().reembedded_edge_count, 1);
        state
            .delete_edges(
                &[SpannerEdgeId(2)],
                certificate(
                    vec![vec![SpannerEdgeId(0)], vec![SpannerEdgeId(1)], vec![]],
                    &[0, 1],
                ),
            )
            .unwrap();
        state.rebuild_greedy(2).unwrap();
        state.verify_embedding().unwrap();
    }

    #[test]
    fn rejects_zero_greedy_hop_bound() {
        assert_eq!(
            DecrementalSpanner::build_greedy(0, Vec::new(), 0),
            Err(SpannerError::InvalidHopBound)
        );
    }

    #[test]
    fn deletion_requires_a_replacement_certificate_for_current_graph() {
        let mut state = spanner();
        state
            .delete_edges(
                &[SpannerEdgeId(2)],
                certificate(
                    vec![vec![SpannerEdgeId(0)], vec![SpannerEdgeId(1)], Vec::new()],
                    &[0, 1],
                ),
            )
            .unwrap();
        assert_eq!(state.metrics().deletion_count, 1);
        assert_eq!(
            state.delete_edges(
                &[SpannerEdgeId(2)],
                certificate(
                    vec![vec![SpannerEdgeId(0)], vec![SpannerEdgeId(1)], Vec::new()],
                    &[0, 1]
                )
            ),
            Err(SpannerError::InvalidDeletion)
        );
    }

    #[test]
    fn split_requires_explicit_incident_edges_and_revalidates_paths() {
        let mut state = spanner();
        let split = state
            .split_vertex(
                FlowNodeId(1),
                &[SpannerEdgeId(1)],
                certificate(
                    vec![
                        vec![SpannerEdgeId(0)],
                        vec![SpannerEdgeId(1)],
                        vec![SpannerEdgeId(2)],
                    ],
                    &[0, 1, 2],
                ),
            )
            .unwrap();
        assert_eq!(split, FlowNodeId(3));
        assert_eq!(state.metrics().vertex_split_count, 1);
    }

    #[test]
    fn rejects_parallel_edges_and_non_path_certificates() {
        assert_eq!(
            DecrementalSpanner::new(
                2,
                vec![
                    SpannerEdge {
                        first: FlowNodeId(0),
                        second: FlowNodeId(1)
                    },
                    SpannerEdge {
                        first: FlowNodeId(1),
                        second: FlowNodeId(0)
                    },
                ],
                certificate(vec![vec![SpannerEdgeId(0)], vec![SpannerEdgeId(0)]], &[0]),
            ),
            Err(SpannerError::InvalidGraph)
        );
    }
}
