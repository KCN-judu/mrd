//! Direct compact-cycle decoding through selected source-tree branches.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use thiserror::Error;

use crate::{
    CirculationArcId, CirculationNetwork, FlowNodeId, MinCostCirculationError, SourceDynamicGraph,
    SourceEdgeId,
};

use super::{
    chain::{Chain, Error as ChainError, Selection, Shifts},
    model::Branch,
};

/// Orientation relative to a source edge's declared first-to-second endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// Traverse from the source edge's first endpoint to its second endpoint.
    Forward,
    /// Traverse from the source edge's second endpoint to its first endpoint.
    Reverse,
}

impl Direction {
    const fn signed(self) -> i8 {
        match self {
            Self::Forward => 1,
            Self::Reverse => -1,
        }
    }
}

/// One compact segment referring either to an off-tree source edge or a path in
/// a selected source-tree branch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Segment {
    /// A single source edge outside the selected tree path.
    OffTree {
        /// Stable source edge ID.
        source: SourceEdgeId,
        /// Traversal orientation.
        direction: Direction,
    },
    /// The unique tree path between two source vertices in a selected branch.
    TreePath {
        /// Stable selected branch identity.
        selection: Selection,
        /// Path start.
        from: FlowNodeId,
        /// Path end.
        to: FlowNodeId,
    },
}

/// A compact signed circulation represented by source-edge references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Cycle {
    /// Ordered compact source segments.
    pub segments: Vec<Segment>,
}

/// Immutable source-edge to circulation-arc mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcBindings {
    arcs: BTreeMap<SourceEdgeId, CirculationArcId>,
}

impl ArcBindings {
    /// Checks that every binding is unique and has exactly the source edge's
    /// declared orientation in both domains.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate, inactive, unknown, or endpoint-mismatched
    /// bindings.
    pub fn new(
        graph: &SourceDynamicGraph,
        network: &CirculationNetwork,
        bindings: Vec<(SourceEdgeId, CirculationArcId)>,
    ) -> Result<Self, Error> {
        let mut arcs = BTreeMap::new();
        for (source, arc) in bindings {
            let edge = graph.edge(source).ok_or(Error::InvalidBinding)?;
            if network.arc_endpoints(arc) != Some((edge.first, edge.second))
                || arcs.insert(source, arc).is_some()
            {
                return Err(Error::InvalidBinding);
            }
        }
        Ok(Self { arcs })
    }

    fn arc(&self, source: SourceEdgeId) -> Result<CirculationArcId, Error> {
        self.arcs
            .get(&source)
            .copied()
            .ok_or(Error::MissingBinding(source))
    }
}

impl Cycle {
    /// Decodes source references directly through the selected tree branches
    /// and validates the result as a nonempty signed circulation.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid tree selection or path, a missing source
    /// binding, or a nonconserving decoded circulation.
    pub fn decode(
        &self,
        graph: &SourceDynamicGraph,
        chain: &Chain,
        shifts: &Shifts,
        bindings: &ArcBindings,
        network: &CirculationNetwork,
    ) -> Result<Vec<(CirculationArcId, i8)>, Error> {
        let active = chain.select(shifts).map_err(Error::Chain)?;
        let active = active.into_iter().collect::<BTreeSet<_>>();
        let mut decoded = Vec::new();
        for segment in &self.segments {
            match segment {
                Segment::OffTree { source, direction } => {
                    decoded.push((bindings.arc(*source)?, direction.signed()));
                }
                Segment::TreePath {
                    selection,
                    from,
                    to,
                } => {
                    if !active.contains(selection) {
                        return Err(Error::InactiveBranch);
                    }
                    let branch = chain.branch(*selection).map_err(Error::Chain)?;
                    decoded.extend(path(graph, branch, *from, *to, bindings)?);
                }
            }
        }
        network
            .validate_signed_circulation(&decoded)
            .map_err(Error::Circulation)?;
        Ok(decoded)
    }
}

fn path(
    graph: &SourceDynamicGraph,
    branch: &Branch,
    from: FlowNodeId,
    to: FlowNodeId,
    bindings: &ArcBindings,
) -> Result<Vec<(CirculationArcId, i8)>, Error> {
    if from == to || from.0 >= graph.node_count() || to.0 >= graph.node_count() {
        return Err(Error::InvalidPath);
    }
    let mut adjacency = vec![Vec::<(FlowNodeId, SourceEdgeId)>::new(); graph.node_count()];
    for source in branch.tree().source_edges() {
        let edge = graph.edge(*source).ok_or(Error::InvalidPath)?;
        adjacency[edge.first.0].push((edge.second, *source));
        adjacency[edge.second.0].push((edge.first, *source));
    }
    let mut predecessor = vec![None; graph.node_count()];
    let mut queue = VecDeque::from([from]);
    predecessor[from.0] = Some((from, SourceEdgeId(usize::MAX)));
    while let Some(vertex) = queue.pop_front() {
        for (next, source) in &adjacency[vertex.0] {
            if predecessor[next.0].is_none() {
                predecessor[next.0] = Some((vertex, *source));
                queue.push_back(*next);
            }
        }
    }
    if predecessor[to.0].is_none() {
        return Err(Error::InvalidPath);
    }
    let mut reversed = Vec::new();
    let mut current = to;
    while current != from {
        let (previous, source) = predecessor[current.0].ok_or(Error::InvalidPath)?;
        let edge = graph.edge(source).ok_or(Error::InvalidPath)?;
        let direction = if edge.first == previous && edge.second == current {
            1
        } else {
            -1
        };
        reversed.push((bindings.arc(source)?, direction));
        current = previous;
    }
    reversed.reverse();
    Ok(reversed)
}

/// Compact-cycle decoding could not establish a checked source circulation.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// The selected source-tree chain is invalid.
    #[error("tree-chain selection failed: {0}")]
    Chain(#[source] ChainError),
    /// A source edge has no matching circulation arc.
    #[error("source edge {0:?} has no circulation-arc binding")]
    MissingBinding(SourceEdgeId),
    /// Source and circulation edge endpoints do not match exactly.
    #[error("source-to-circulation binding is invalid")]
    InvalidBinding,
    /// A compact tree path does not belong to the current selected branch.
    #[error("compact cycle references an inactive tree branch")]
    InactiveBranch,
    /// A requested source-tree path is degenerate or absent.
    #[error("compact tree path is invalid")]
    InvalidPath,
    /// Decoded signed arc occurrences are not a circulation.
    #[error("decoded circulation is invalid: {0}")]
    Circulation(#[source] MinCostCirculationError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{ArcBindings, Cycle, Direction, Error, Segment};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId,
        SourceWeightedEdge,
        source_min_ratio::{
            chain::Chain,
            model::{Branch, BranchId, Level, LevelId, Tree},
        },
    };

    fn graph() -> SourceDynamicGraph {
        SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap()
    }

    fn edge(first: usize, second: usize) -> SourceWeightedEdge {
        SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(1, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        }
    }

    #[test]
    fn decodes_selected_tree_path_without_an_enumerating_oracle() {
        let graph = graph();
        let chain = Chain::new(
            &graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    Tree::new(
                        FlowNodeId(0),
                        BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)]),
                    ),
                )],
            )],
        )
        .unwrap();
        let mut network = CirculationNetwork::new(3);
        let first = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let second = network.add_arc(FlowNodeId(1), FlowNodeId(2), 1, 0).unwrap();
        let off_tree = network.add_arc(FlowNodeId(0), FlowNodeId(2), 1, 0).unwrap();
        let bindings = ArcBindings::new(
            &graph,
            &network,
            vec![
                (SourceEdgeId(0), first),
                (SourceEdgeId(1), second),
                (SourceEdgeId(2), off_tree),
            ],
        )
        .unwrap();
        let selection = chain.select(&chain.initial_shifts()).unwrap()[0];
        let cycle = Cycle {
            segments: vec![
                Segment::TreePath {
                    selection,
                    from: FlowNodeId(0),
                    to: FlowNodeId(2),
                },
                Segment::OffTree {
                    source: SourceEdgeId(2),
                    direction: Direction::Reverse,
                },
            ],
        };
        assert_eq!(
            cycle
                .decode(&graph, &chain, &chain.initial_shifts(), &bindings, &network)
                .unwrap(),
            vec![(first, 1), (second, 1), (off_tree, -1)]
        );
    }

    #[test]
    fn rejects_endpoint_mismatched_bindings() {
        let graph = graph();
        let mut network = CirculationNetwork::new(3);
        let reverse = network.add_arc(FlowNodeId(1), FlowNodeId(0), 1, 0).unwrap();
        assert_eq!(
            ArcBindings::new(&graph, &network, vec![(SourceEdgeId(0), reverse)]),
            Err(Error::InvalidBinding)
        );
    }

    #[test]
    fn rejects_missing_bindings_degenerate_paths_and_nonconserving_cycles() {
        let graph = graph();
        let chain = Chain::new(
            &graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    Tree::new(
                        FlowNodeId(0),
                        BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)]),
                    ),
                )],
            )],
        )
        .unwrap();
        let mut network = CirculationNetwork::new(3);
        let first = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let bindings = ArcBindings::new(&graph, &network, vec![(SourceEdgeId(0), first)]).unwrap();
        let selection = chain.select(&chain.initial_shifts()).unwrap()[0];
        assert_eq!(
            Cycle {
                segments: vec![Segment::OffTree {
                    source: SourceEdgeId(2),
                    direction: Direction::Forward
                }]
            }
            .decode(&graph, &chain, &chain.initial_shifts(), &bindings, &network),
            Err(Error::MissingBinding(SourceEdgeId(2)))
        );
        assert_eq!(
            Cycle {
                segments: vec![Segment::TreePath {
                    selection,
                    from: FlowNodeId(0),
                    to: FlowNodeId(0)
                }]
            }
            .decode(&graph, &chain, &chain.initial_shifts(), &bindings, &network),
            Err(Error::InvalidPath)
        );
        assert!(matches!(
            Cycle {
                segments: vec![Segment::OffTree {
                    source: SourceEdgeId(0),
                    direction: Direction::Forward
                }]
            }
            .decode(&graph, &chain, &chain.initial_shifts(), &bindings, &network),
            Err(Error::Circulation(_))
        ));
    }
}
