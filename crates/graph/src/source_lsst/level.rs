//! Exact one-level Section 9.1 contraction from a certified partial forest.

use std::collections::{BTreeSet, VecDeque};

use crate::{ExactRatio, FlowNodeId};

use super::{
    LsfContractAudit, LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId, SourceLsstError,
};

/// Stable identifier for one component of a contracted partial forest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentId(pub usize);

/// One forest component and its source vertex/root provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Component {
    pub id: ComponentId,
    pub root: FlowNodeId,
    pub vertices: BTreeSet<FlowNodeId>,
}

/// One cross-component source edge in `H_i = G_i/F_i`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    pub source: SourceEdgeId,
    pub source_first: FlowNodeId,
    pub source_second: FlowNodeId,
    pub first: ComponentId,
    pub second: ComponentId,
    pub original_length: ExactRatio,
    pub stretch_overestimate: ExactRatio,
    pub scaled_length: ExactRatio,
}

/// Exact counters for one immutable contracted level.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Audit {
    pub active_source_edges: u64,
    pub forest_edges: u64,
    pub components: u64,
    pub cross_component_edges: u64,
    pub discarded_loops: u64,
}

/// A source-provenance-preserving `H_i = G_i/F_i` snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level {
    pub components: Vec<Component>,
    pub edges: Vec<Edge>,
    pub discarded_loops: BTreeSet<SourceEdgeId>,
    pub forest_audit: LsfContractAudit,
    pub audit: Audit,
}

impl Level {
    /// Contracts a certified partial forest and exactly rescales every cross edge.
    ///
    /// Source Section 9.1 defines `H_i = G_i/F_i` with length
    /// `stretch_tilde(e) * length_Gi(e)`. Edges internal to one contracted
    /// component are deliberately discarded and retained in `discarded_loops`.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied forest certificate is invalid, a
    /// component does not have a root, or exact length accounting overflows.
    pub fn contract(
        graph: &SourceDynamicGraph,
        forest: &LsfStructuralCertificate,
    ) -> Result<Self, Error> {
        let forest_audit = graph.audit_lsf(forest).map_err(Error::Forest)?;
        let (component_of, components) = components(graph, &forest.forest_edges, &forest.roots)?;
        let mut edges = Vec::new();
        let mut discarded_loops = BTreeSet::new();
        let mut active_source_edges = 0_u64;
        for index in 0..graph.edge_count() {
            let source = SourceEdgeId(index);
            let Some(edge) = graph.edge(source) else {
                continue;
            };
            active_source_edges = active_source_edges.checked_add(1).ok_or(Error::Overflow)?;
            let first = component_of[edge.first.0];
            let second = component_of[edge.second.0];
            if first == second {
                discarded_loops.insert(source);
                continue;
            }
            let stretch_overestimate = *forest
                .stretch_overestimates
                .get(index)
                .ok_or(Error::InvalidCertificate)?;
            let scaled_length = edge
                .length
                .checked_mul(stretch_overestimate)
                .map_err(|_| Error::Overflow)?;
            edges.push(Edge {
                source,
                source_first: edge.first,
                source_second: edge.second,
                first,
                second,
                original_length: edge.length,
                stretch_overestimate,
                scaled_length,
            });
        }
        let audit = Audit {
            active_source_edges,
            forest_edges: u64::try_from(forest.forest_edges.len()).map_err(|_| Error::Overflow)?,
            components: u64::try_from(components.len()).map_err(|_| Error::Overflow)?,
            cross_component_edges: u64::try_from(edges.len()).map_err(|_| Error::Overflow)?,
            discarded_loops: u64::try_from(discarded_loops.len()).map_err(|_| Error::Overflow)?,
        };
        Ok(Self {
            components,
            edges,
            discarded_loops,
            forest_audit,
            audit,
        })
    }

    /// Recomputes the source contraction and every provenance-bearing field.
    ///
    /// # Errors
    ///
    /// Returns an error when the saved level differs from fresh exact evidence.
    pub fn verify(
        &self,
        graph: &SourceDynamicGraph,
        forest: &LsfStructuralCertificate,
    ) -> Result<(), Error> {
        if &Self::contract(graph, forest)? != self {
            return Err(Error::InvalidCertificate);
        }
        Ok(())
    }
}

fn components(
    graph: &SourceDynamicGraph,
    forest_edges: &BTreeSet<SourceEdgeId>,
    roots: &BTreeSet<FlowNodeId>,
) -> Result<(Vec<ComponentId>, Vec<Component>), Error> {
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    for edge_id in forest_edges {
        let edge = graph.edge(*edge_id).ok_or(Error::InvalidCertificate)?;
        adjacency[edge.first.0].push(edge.second);
        adjacency[edge.second.0].push(edge.first);
    }
    let mut component_of = vec![ComponentId(usize::MAX); graph.node_count()];
    let mut result = Vec::new();
    for start in 0..graph.node_count() {
        if component_of[start].0 != usize::MAX {
            continue;
        }
        let id = ComponentId(result.len());
        let mut queue = VecDeque::from([FlowNodeId(start)]);
        let mut vertices = BTreeSet::new();
        component_of[start] = id;
        while let Some(vertex) = queue.pop_front() {
            vertices.insert(vertex);
            for next in &adjacency[vertex.0] {
                if component_of[next.0].0 == usize::MAX {
                    component_of[next.0] = id;
                    queue.push_back(*next);
                }
            }
        }
        let root = vertices
            .iter()
            .find(|vertex| roots.contains(vertex))
            .copied()
            .ok_or(Error::MissingRoot)?;
        if vertices
            .iter()
            .filter(|vertex| roots.contains(vertex))
            .count()
            != 1
        {
            return Err(Error::InvalidCertificate);
        }
        result.push(Component { id, root, vertices });
    }
    Ok((component_of, result))
}

/// A finite Section 9.1 contraction cannot be certified.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("partial forest certificate is invalid: {0}")]
    Forest(#[source] SourceLsstError),
    #[error("contracted forest component is missing a root")]
    MissingRoot,
    #[error("contracted level provenance is invalid")]
    InvalidCertificate,
    #[error("contracted level exact accounting overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{ComponentId, Level};
    use crate::{
        ExactRatio, FlowNodeId,
        source_lsst::oracle,
        source_lsst::{
            LsfPiece, LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId,
            SourceWeightedEdge,
        },
    };

    fn edge(first: usize, second: usize) -> SourceWeightedEdge {
        SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(1, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        }
    }

    fn graph() -> SourceDynamicGraph {
        SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap()
    }

    fn forest() -> LsfStructuralCertificate {
        LsfStructuralCertificate {
            forest_edges: BTreeSet::from([SourceEdgeId(0)]),
            roots: BTreeSet::from([FlowNodeId(0), FlowNodeId(2)]),
            pieces: vec![
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]),
                    forest_edges: BTreeSet::from([SourceEdgeId(0)]),
                },
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(2)]),
                    forest_edges: BTreeSet::new(),
                },
            ],
            stretch_overestimates: vec![
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(1, 1).unwrap(),
            ],
            piece_volume_limit: 2,
        }
    }

    #[test]
    fn contracts_a_certified_partial_forest_with_exact_scaled_lengths() {
        let graph = graph();
        let forest = forest();
        let level = Level::contract(&graph, &forest).unwrap();
        let oracle = oracle::contract(&graph, &forest).unwrap();

        assert_eq!(level.components.len(), 2);
        assert_eq!(level.components[0].root, FlowNodeId(0));
        assert_eq!(level.components[1].root, FlowNodeId(2));
        assert_eq!(level.discarded_loops, BTreeSet::from([SourceEdgeId(0)]));
        assert_eq!(level.edges.len(), 2);
        assert_eq!(level.edges[0].source, SourceEdgeId(1));
        assert_eq!(level.edges[0].first, ComponentId(0));
        assert_eq!(level.edges[0].second, ComponentId(1));
        assert_eq!(level.edges[0].scaled_length, ExactRatio::new(2, 1).unwrap());
        assert_eq!(level.edges[1].source, SourceEdgeId(2));
        assert_eq!(level.edges[1].scaled_length, ExactRatio::new(1, 1).unwrap());
        assert_eq!(level.audit.active_source_edges, 3);
        assert_eq!(level.audit.cross_component_edges, 2);
        assert_eq!(level.audit.discarded_loops, 1);
        assert_eq!(
            level
                .components
                .iter()
                .map(|component| (component.root, component.vertices.clone()))
                .collect::<Vec<_>>(),
            oracle
                .components
                .iter()
                .map(|component| (component.root, component.vertices.clone()))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            level
                .edges
                .iter()
                .map(|edge| {
                    (
                        edge.source,
                        edge.source_first,
                        edge.source_second,
                        edge.first.0,
                        edge.second.0,
                        edge.scaled_length,
                    )
                })
                .collect::<Vec<_>>(),
            oracle
                .edges
                .iter()
                .map(|edge| {
                    (
                        edge.source,
                        edge.source_first,
                        edge.source_second,
                        edge.first_component,
                        edge.second_component,
                        edge.scaled_length,
                    )
                })
                .collect::<Vec<_>>()
        );
        assert_eq!(level.discarded_loops, oracle.discarded_loops);
        level.verify(&graph, &forest).unwrap();
    }
}
