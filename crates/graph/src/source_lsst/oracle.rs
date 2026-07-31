//! Small exact contraction Oracle for Section 9.1 differentials.

use std::collections::{BTreeSet, VecDeque};

use crate::{ExactRatio, FlowNodeId};

use super::{LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId, SourceLsstError};

/// Oracle component provenance without a production component identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Component {
    pub root: FlowNodeId,
    pub vertices: BTreeSet<FlowNodeId>,
}

/// Oracle cross-component edge and its independently materialized length.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    pub source: SourceEdgeId,
    pub source_first: FlowNodeId,
    pub source_second: FlowNodeId,
    pub first_component: usize,
    pub second_component: usize,
    pub scaled_length: ExactRatio,
}

/// Definition-level contraction result for a bounded source graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub components: Vec<Component>,
    pub edges: Vec<Edge>,
    pub discarded_loops: BTreeSet<SourceEdgeId>,
}

/// Independently contracts a certified forest by direct component enumeration.
///
/// # Errors
///
/// Returns an error when the forest certificate is invalid, a component lacks
/// its sole root, or exact scaled-length arithmetic overflows.
pub fn contract(
    graph: &SourceDynamicGraph,
    forest: &LsfStructuralCertificate,
) -> Result<Snapshot, Error> {
    graph.audit_lsf(forest).map_err(Error::Forest)?;
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    for source in &forest.forest_edges {
        let edge = graph.edge(*source).ok_or(Error::InvalidCertificate)?;
        adjacency[edge.first.0].push(edge.second);
        adjacency[edge.second.0].push(edge.first);
    }
    let mut component_of = vec![usize::MAX; graph.node_count()];
    let mut components = Vec::new();
    for start in 0..graph.node_count() {
        if component_of[start] != usize::MAX {
            continue;
        }
        let component = components.len();
        let mut vertices = BTreeSet::new();
        let mut queue = VecDeque::from([FlowNodeId(start)]);
        component_of[start] = component;
        while let Some(vertex) = queue.pop_front() {
            vertices.insert(vertex);
            for next in &adjacency[vertex.0] {
                if component_of[next.0] == usize::MAX {
                    component_of[next.0] = component;
                    queue.push_back(*next);
                }
            }
        }
        let roots = vertices
            .iter()
            .filter(|vertex| forest.roots.contains(vertex))
            .copied()
            .collect::<Vec<_>>();
        if roots.len() != 1 {
            return Err(Error::InvalidCertificate);
        }
        components.push(Component {
            root: roots[0],
            vertices,
        });
    }
    let mut edges = Vec::new();
    let mut discarded_loops = BTreeSet::new();
    for index in 0..graph.edge_count() {
        let source = SourceEdgeId(index);
        let Some(edge) = graph.edge(source) else {
            continue;
        };
        let first_component = component_of[edge.first.0];
        let second_component = component_of[edge.second.0];
        if first_component == second_component {
            discarded_loops.insert(source);
            continue;
        }
        let stretch = forest
            .stretch_overestimates
            .get(index)
            .ok_or(Error::InvalidCertificate)?
            .clone();
        edges.push(Edge {
            source,
            source_first: edge.first,
            source_second: edge.second,
            first_component,
            second_component,
            scaled_length: edge
                .length
                .checked_mul(&stretch)
                .map_err(|_| Error::Overflow)?,
        });
    }
    Ok(Snapshot {
        components,
        edges,
        discarded_loops,
    })
}

/// The exact contraction Oracle cannot construct a snapshot.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("partial forest certificate is invalid: {0}")]
    Forest(#[source] SourceLsstError),
    #[error("Oracle contraction provenance is invalid")]
    InvalidCertificate,
    #[error("Oracle contraction exact arithmetic overflowed")]
    Overflow,
}
