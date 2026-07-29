use std::collections::BTreeSet;

use thiserror::Error;

use crate::FlowNodeId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EdgeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    pub first: FlowNodeId,
    pub second: FlowNodeId,
}

/// An unweighted simple undirected graph with deterministic edge identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Graph {
    node_count: usize,
    edges: Vec<Edge>,
}

impl Graph {
    /// Creates a simple graph.
    ///
    /// # Errors
    ///
    /// Returns an error for a loop, parallel edge, or invalid endpoint.
    pub fn new(node_count: usize, edges: Vec<Edge>) -> Result<Self, Error> {
        let result = Self { node_count, edges };
        result.verify_simple()?;
        Ok(result)
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub fn edge(&self, edge: EdgeId) -> Option<Edge> {
        self.edges.get(edge.0).copied()
    }

    /// Returns the maximum degree in the selected subgraph.
    ///
    /// # Errors
    ///
    /// Returns an error when degree accounting overflows.
    pub fn maximum_degree(&self, allowed: Option<&BTreeSet<EdgeId>>) -> Result<u64, Error> {
        let mut degree = vec![0_u64; self.node_count];
        for (index, edge) in self.edges.iter().enumerate() {
            let id = EdgeId(index);
            if allowed.is_some_and(|set| !set.contains(&id)) {
                continue;
            }
            degree[edge.first.0] = degree[edge.first.0].checked_add(1).ok_or(Error::Overflow)?;
            degree[edge.second.0] = degree[edge.second.0]
                .checked_add(1)
                .ok_or(Error::Overflow)?;
        }
        Ok(*degree.iter().max().unwrap_or(&0))
    }

    fn verify_simple(&self) -> Result<(), Error> {
        let mut pairs = BTreeSet::new();
        for edge in &self.edges {
            if edge.first.0 >= self.node_count
                || edge.second.0 >= self.node_count
                || edge.first == edge.second
            {
                return Err(Error::InvalidGraph);
            }
            if !pairs.insert((edge.first.min(edge.second), edge.first.max(edge.second))) {
                return Err(Error::InvalidGraph);
            }
        }
        Ok(())
    }
}

/// An explicit simple path in the target graph for every input edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Embedding {
    paths: Vec<Vec<EdgeId>>,
}

impl Embedding {
    /// Creates and verifies an embedding from `source` to `target`.
    ///
    /// `allowed` restricts target edges to a selected subgraph.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing, noncontiguous, nonsimple, or forbidden
    /// target path.
    pub fn new(
        source: &Graph,
        target: &Graph,
        allowed: Option<&BTreeSet<EdgeId>>,
        paths: Vec<Vec<EdgeId>>,
    ) -> Result<Self, Error> {
        let result = Self { paths };
        result.verify(source, target, allowed)?;
        Ok(result)
    }

    #[must_use]
    pub fn path(&self, source_edge: EdgeId) -> Option<&[EdgeId]> {
        self.paths.get(source_edge.0).map(Vec::as_slice)
    }

    /// Verifies the explicit paths and recomputes their exact measurements.
    ///
    /// # Errors
    ///
    /// Returns an error when a path is missing, noncontiguous, nonsimple, or
    /// contains an edge outside the selected target subgraph.
    pub fn verify(
        &self,
        source: &Graph,
        target: &Graph,
        allowed: Option<&BTreeSet<EdgeId>>,
    ) -> Result<Metrics, Error> {
        if self.paths.len() != source.edge_count() {
            return Err(Error::InvalidEmbedding);
        }
        let mut vertex_congestion = vec![0_u64; target.node_count()];
        let mut edge_congestion = vec![0_u64; target.edge_count()];
        let mut maximum_path_length = 0_u64;
        let mut total_path_length = 0_u64;
        for (index, path) in self.paths.iter().enumerate() {
            let source_edge = source.edge(EdgeId(index)).ok_or(Error::InvalidEmbedding)?;
            let vertices = path_vertices(source_edge.first, path, target, allowed)?;
            if vertices.last() != Some(&source_edge.second)
                || vertices.iter().collect::<BTreeSet<_>>().len() != vertices.len()
            {
                return Err(Error::InvalidEmbedding);
            }
            let length = u64::try_from(path.len()).map_err(|_| Error::Overflow)?;
            maximum_path_length = maximum_path_length.max(length);
            total_path_length = total_path_length
                .checked_add(length)
                .ok_or(Error::Overflow)?;
            for vertex in vertices {
                vertex_congestion[vertex.0] = vertex_congestion[vertex.0]
                    .checked_add(1)
                    .ok_or(Error::Overflow)?;
            }
            for edge in path {
                edge_congestion[edge.0] = edge_congestion[edge.0]
                    .checked_add(1)
                    .ok_or(Error::Overflow)?;
            }
        }
        Ok(Metrics {
            maximum_path_length,
            maximum_vertex_congestion: *vertex_congestion.iter().max().unwrap_or(&0),
            maximum_edge_congestion: *edge_congestion.iter().max().unwrap_or(&0),
            total_path_length,
        })
    }
}

/// A selected subgraph of `J` and an embedding from `J` into that subgraph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub edges: BTreeSet<EdgeId>,
    pub embedding: Embedding,
}

/// Exact measurements for one embedding.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Metrics {
    pub maximum_path_length: u64,
    pub maximum_vertex_congestion: u64,
    pub maximum_edge_congestion: u64,
    pub total_path_length: u64,
}

/// Theorem 8.1's static input, candidate subgraph, and composed embedding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Audit {
    pub direct: Metrics,
    pub into_subgraph: Metrics,
    pub composed: Metrics,
    pub subgraph_edge_count: u64,
    pub subgraph_maximum_degree: u64,
    pub composed_embedding: Embedding,
}

impl Audit {
    /// Verifies `J~ subset J`, both explicit embeddings, and their composition.
    ///
    /// # Errors
    ///
    /// Returns an error when `J` is not contained in the host vertex domain,
    /// a candidate uses a non-`J` edge, or any direct/composed path is invalid.
    pub fn verify(
        host: &Graph,
        input: &Graph,
        input_to_host: &Embedding,
        candidate: &Candidate,
    ) -> Result<Self, Error> {
        if input.node_count() > host.node_count()
            || candidate
                .edges
                .iter()
                .any(|edge| input.edge(*edge).is_none())
        {
            return Err(Error::InvalidCandidate);
        }
        let direct = input_to_host.verify(input, host, None)?;
        let into_subgraph = candidate
            .embedding
            .verify(input, input, Some(&candidate.edges))?;
        let composed_paths = candidate
            .embedding
            .paths
            .iter()
            .map(|path| {
                path.iter().try_fold(Vec::new(), |mut output, edge| {
                    output.extend_from_slice(
                        input_to_host.path(*edge).ok_or(Error::InvalidEmbedding)?,
                    );
                    Ok::<_, Error>(output)
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let composed_embedding = Embedding::new(input, host, None, composed_paths)?;
        let composed = composed_embedding.verify(input, host, None)?;
        Ok(Self {
            direct,
            into_subgraph,
            composed,
            subgraph_edge_count: u64::try_from(candidate.edges.len())
                .map_err(|_| Error::Overflow)?,
            subgraph_maximum_degree: input.maximum_degree(Some(&candidate.edges))?,
            composed_embedding,
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("static graph must be simple, undirected, and use in-range vertices")]
    InvalidGraph,
    #[error("static embedding path is invalid")]
    InvalidEmbedding,
    #[error("static spanner candidate is invalid")]
    InvalidCandidate,
    #[error("static spanner accounting overflowed")]
    Overflow,
}

fn path_vertices(
    start: FlowNodeId,
    path: &[EdgeId],
    target: &Graph,
    allowed: Option<&BTreeSet<EdgeId>>,
) -> Result<Vec<FlowNodeId>, Error> {
    if path.is_empty() {
        return Err(Error::InvalidEmbedding);
    }
    let mut current = start;
    let mut vertices = vec![current];
    for edge_id in path {
        if allowed.is_some_and(|set| !set.contains(edge_id)) {
            return Err(Error::InvalidEmbedding);
        }
        let edge = target.edge(*edge_id).ok_or(Error::InvalidEmbedding)?;
        current = if edge.first == current {
            edge.second
        } else if edge.second == current {
            edge.first
        } else {
            return Err(Error::InvalidEmbedding);
        };
        vertices.push(current);
    }
    Ok(vertices)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Audit, Candidate, Edge, EdgeId, Embedding, Error, Graph};
    use crate::FlowNodeId;

    fn edge(first: usize, second: usize) -> Edge {
        Edge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
        }
    }

    #[test]
    fn audits_composed_static_embedding() {
        let host = Graph::new(4, vec![edge(0, 1), edge(1, 2), edge(2, 3)]).unwrap();
        let input = Graph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)]).unwrap();
        let direct = Embedding::new(
            &input,
            &host,
            None,
            vec![vec![EdgeId(0)], vec![EdgeId(1)], vec![EdgeId(0), EdgeId(1)]],
        )
        .unwrap();
        let candidate = Candidate {
            edges: BTreeSet::from([EdgeId(0), EdgeId(1)]),
            embedding: Embedding::new(
                &input,
                &input,
                Some(&BTreeSet::from([EdgeId(0), EdgeId(1)])),
                vec![vec![EdgeId(0)], vec![EdgeId(1)], vec![EdgeId(0), EdgeId(1)]],
            )
            .unwrap(),
        };
        let audit = Audit::verify(&host, &input, &direct, &candidate).unwrap();
        assert_eq!(audit.subgraph_edge_count, 2);
        assert_eq!(audit.composed.maximum_path_length, 2);
        assert_eq!(
            audit.composed_embedding.path(EdgeId(2)).unwrap(),
            [EdgeId(0), EdgeId(1)]
        );
    }

    #[test]
    fn rejects_forbidden_and_non_simple_paths() {
        let graph = Graph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)]).unwrap();
        assert_eq!(
            Embedding::new(
                &graph,
                &graph,
                Some(&BTreeSet::from([EdgeId(0), EdgeId(1)])),
                vec![vec![EdgeId(0)], vec![EdgeId(1)], vec![EdgeId(2)]]
            ),
            Err(Error::InvalidEmbedding)
        );
        assert_eq!(
            Embedding::new(
                &graph,
                &graph,
                None,
                vec![
                    vec![EdgeId(0)],
                    vec![EdgeId(1)],
                    vec![EdgeId(0), EdgeId(0), EdgeId(2)]
                ]
            ),
            Err(Error::InvalidEmbedding)
        );
    }
}
