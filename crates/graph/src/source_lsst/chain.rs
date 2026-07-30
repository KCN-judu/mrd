//! Finite one-level Section 9.1 tree chain and exact semantic certificates.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    ExactRatio, FlowNodeId,
    source_an19::{experiment::hierarchy::Lsst as An19Lsst, petal::Error as An19Error},
};

use super::{
    LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge,
    bucket::{
        Audit as BucketAudit, Error as BucketError, Parameters as BucketParameters, Partition,
    },
    level::{Error as LevelError, Level},
};

/// Explicit finite-domain controls for one Section 9.1 tree chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub root: FlowNodeId,
    /// Maximum numerator or denominator accepted in structural lengths.
    pub maximum_coordinate: i128,
    pub buckets: BucketParameters,
}

/// Exact measurements of the partial-forest and low-congestion spanner level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LevelAudit {
    pub weighted_forest_stretch: ExactRatio,
    pub maximum_forest_stretch: ExactRatio,
    pub maximum_embedding_hops: u64,
    pub maximum_embedding_vertex_congestion: u64,
    pub encoded_embedding_length: u64,
    pub bucket_audit: BucketAudit,
}

/// Exact Definition 2.2-style measurements of the recovered source tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TreeAudit {
    pub weighted_stretch: ExactRatio,
    pub total_weight: ExactRatio,
    pub maximum_stretch: ExactRatio,
}

/// A finite depth-one `T_0 = F_0 union F_1` construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chain {
    pub level: Level,
    pub partition: Partition,
    pub terminal_edges: BTreeSet<SourceEdgeId>,
    pub tree_edges: BTreeSet<SourceEdgeId>,
    pub level_audit: LevelAudit,
    pub tree_audit: TreeAudit,
}

impl Chain {
    /// Builds and verifies a finite depth-one Section 9.1 tree chain.
    ///
    /// The partial forest is `F_0`; the terminal tree on the selected
    /// contracted spanner is `F_1`. The returned source tree is `F_0 union F_1`.
    ///
    /// # Errors
    ///
    /// Returns an error for an out-of-range source length, an
    /// unsupported bucket, a failed AN19-shaped terminal-tree construction, or
    /// an invalid recovered source tree.
    pub fn build(
        graph: &SourceDynamicGraph,
        forest: &LsfStructuralCertificate,
        parameters: Parameters,
    ) -> Result<Self, Error> {
        validate_input(graph, parameters)?;
        let level = Level::contract(graph, forest).map_err(Error::Level)?;
        let partition = Partition::initialize(&level, parameters.buckets).map_err(Error::Bucket)?;
        let terminal_edges = terminal_edges(&level, &partition, parameters.root)?;
        let tree_edges = forest
            .forest_edges
            .union(&terminal_edges)
            .copied()
            .collect::<BTreeSet<_>>();
        let level_audit = level_audit(&level, &partition)?;
        let tree_audit = tree_audit(graph, &tree_edges)?;
        Ok(Self {
            level,
            partition,
            terminal_edges,
            tree_edges,
            level_audit,
            tree_audit,
        })
    }

    /// Recomputes every finite level, path, and source-tree certificate.
    ///
    /// # Errors
    ///
    /// Returns an error when fresh construction differs from this chain.
    pub fn verify(
        &self,
        graph: &SourceDynamicGraph,
        forest: &LsfStructuralCertificate,
        parameters: Parameters,
    ) -> Result<(), Error> {
        if &Self::build(graph, forest, parameters)? != self {
            return Err(Error::InvalidChain);
        }
        Ok(())
    }
}

fn validate_input(graph: &SourceDynamicGraph, parameters: Parameters) -> Result<(), Error> {
    if parameters.root.0 >= graph.node_count() || parameters.maximum_coordinate <= 0 {
        return Err(Error::InvalidParameters);
    }
    for index in 0..graph.edge_count() {
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        if coordinate_bound(edge.length)? > parameters.maximum_coordinate {
            return Err(Error::LengthOutsideFiniteDomain);
        }
    }
    Ok(())
}

fn terminal_edges(
    level: &Level,
    partition: &Partition,
    root: FlowNodeId,
) -> Result<BTreeSet<SourceEdgeId>, Error> {
    if level.components.len() == 1 {
        return Ok(BTreeSet::new());
    }
    let selected = partition
        .buckets
        .iter()
        .flat_map(|bucket| bucket.embedding.selected.iter().copied())
        .collect::<BTreeSet<_>>();
    let by_source = level
        .edges
        .iter()
        .map(|edge| (edge.source, edge))
        .collect::<BTreeMap<_, _>>();
    let mut edges = Vec::with_capacity(selected.len());
    let mut sources = Vec::with_capacity(selected.len());
    let mut maximum_coordinate = 1_i128;
    for source in selected {
        let edge = by_source.get(&source).ok_or(Error::InvalidChain)?;
        maximum_coordinate = maximum_coordinate.max(coordinate_bound(edge.scaled_length)?);
        edges.push(SourceWeightedEdge {
            first: FlowNodeId(edge.first.0),
            second: FlowNodeId(edge.second.0),
            length: edge.scaled_length,
            weight: ratio(1)?,
        });
        sources.push(source);
    }
    let root_component = level
        .components
        .iter()
        .find(|component| component.vertices.contains(&root))
        .map(|component| component.id)
        .ok_or(Error::InvalidParameters)?;
    let contracted = SourceDynamicGraph::new(level.components.len(), edges, maximum_coordinate)
        .map_err(|_| Error::InvalidChain)?;
    let terminal =
        An19Lsst::construct(&contracted, FlowNodeId(root_component.0)).map_err(Error::An19)?;
    terminal
        .tree_edges
        .iter()
        .map(|edge| sources.get(edge.0).copied().ok_or(Error::InvalidChain))
        .collect()
}

fn level_audit(level: &Level, partition: &Partition) -> Result<LevelAudit, Error> {
    let by_source = level
        .edges
        .iter()
        .map(|edge| (edge.source, edge))
        .collect::<BTreeMap<_, _>>();
    let mut congestion = vec![0_u64; level.components.len()];
    let mut maximum_embedding_hops = 0_u64;
    let mut encoded_embedding_length = 0_u64;
    for bucket in &partition.buckets {
        for (source, path) in &bucket.embedding.paths {
            if path
                .iter()
                .any(|edge| !bucket.embedding.selected.contains(edge))
            {
                return Err(Error::InvalidEmbedding);
            }
            let input = by_source.get(source).ok_or(Error::InvalidEmbedding)?;
            let vertices = path_vertices(input.first, input.second, path, &by_source)?;
            maximum_embedding_hops =
                maximum_embedding_hops.max(u64::try_from(path.len()).map_err(|_| Error::Overflow)?);
            encoded_embedding_length = encoded_embedding_length
                .checked_add(u64::try_from(path.len()).map_err(|_| Error::Overflow)?)
                .ok_or(Error::Overflow)?;
            for vertex in vertices {
                let entry = congestion
                    .get_mut(vertex.0)
                    .ok_or(Error::InvalidEmbedding)?;
                *entry = entry.checked_add(1).ok_or(Error::Overflow)?;
            }
        }
    }
    Ok(LevelAudit {
        weighted_forest_stretch: level.forest_audit.weighted_initial_stretch,
        maximum_forest_stretch: level.forest_audit.maximum_stretch,
        maximum_embedding_hops,
        maximum_embedding_vertex_congestion: congestion.into_iter().max().unwrap_or(0),
        encoded_embedding_length,
        bucket_audit: partition.audit,
    })
}

fn path_vertices(
    start: super::level::ComponentId,
    target: super::level::ComponentId,
    path: &[SourceEdgeId],
    edges: &BTreeMap<SourceEdgeId, &super::level::Edge>,
) -> Result<Vec<super::level::ComponentId>, Error> {
    if path.is_empty() {
        return Err(Error::InvalidEmbedding);
    }
    let mut current = start;
    let mut vertices = vec![current];
    let mut seen = BTreeSet::from([current]);
    for source in path {
        let edge = edges.get(source).ok_or(Error::InvalidEmbedding)?;
        current = if edge.first == current {
            edge.second
        } else if edge.second == current {
            edge.first
        } else {
            return Err(Error::InvalidEmbedding);
        };
        if !seen.insert(current) {
            return Err(Error::InvalidEmbedding);
        }
        vertices.push(current);
    }
    if current != target {
        return Err(Error::InvalidEmbedding);
    }
    Ok(vertices)
}

fn tree_audit(
    graph: &SourceDynamicGraph,
    tree: &BTreeSet<SourceEdgeId>,
) -> Result<TreeAudit, Error> {
    let adjacency = tree_adjacency(graph, tree)?;
    let mut weighted_stretch = ratio(0)?;
    let mut total_weight = ratio(0)?;
    let mut maximum_stretch = ratio(0)?;
    for index in 0..graph.edge_count() {
        let Some(edge) = graph.edge(SourceEdgeId(index)) else {
            continue;
        };
        let distance = tree_distance(&adjacency, edge.first, edge.second)?;
        let stretch = edge
            .length
            .checked_add(distance)
            .and_then(|value| value.checked_mul(edge.length.reciprocal()?))
            .map_err(|_| Error::Overflow)?;
        weighted_stretch = weighted_stretch
            .checked_add(
                edge.weight
                    .checked_mul(stretch)
                    .map_err(|_| Error::Overflow)?,
            )
            .map_err(|_| Error::Overflow)?;
        total_weight = total_weight
            .checked_add(edge.weight)
            .map_err(|_| Error::Overflow)?;
        if stretch
            .at_least(maximum_stretch)
            .map_err(|_| Error::Overflow)?
        {
            maximum_stretch = stretch;
        }
    }
    Ok(TreeAudit {
        weighted_stretch,
        total_weight,
        maximum_stretch,
    })
}

fn tree_adjacency(
    graph: &SourceDynamicGraph,
    tree: &BTreeSet<SourceEdgeId>,
) -> Result<Vec<Vec<(FlowNodeId, ExactRatio)>>, Error> {
    if tree.len().checked_add(1) != Some(graph.node_count()) {
        return Err(Error::InvalidTree);
    }
    let mut adjacency = vec![Vec::new(); graph.node_count()];
    for source in tree {
        let edge = graph.edge(*source).ok_or(Error::InvalidTree)?;
        adjacency[edge.first.0].push((edge.second, edge.length));
        adjacency[edge.second.0].push((edge.first, edge.length));
    }
    let mut seen = BTreeSet::from([FlowNodeId(0)]);
    let mut queue = VecDeque::from([FlowNodeId(0)]);
    while let Some(vertex) = queue.pop_front() {
        for (next, _) in &adjacency[vertex.0] {
            if seen.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    if seen.len() != graph.node_count() {
        return Err(Error::InvalidTree);
    }
    Ok(adjacency)
}

fn tree_distance(
    adjacency: &[Vec<(FlowNodeId, ExactRatio)>],
    start: FlowNodeId,
    target: FlowNodeId,
) -> Result<ExactRatio, Error> {
    let zero = ratio(0)?;
    let mut queue = VecDeque::from([(start, zero)]);
    let mut seen = BTreeSet::from([start]);
    while let Some((vertex, distance)) = queue.pop_front() {
        if vertex == target {
            return Ok(distance);
        }
        for (next, length) in &adjacency[vertex.0] {
            if seen.insert(*next) {
                queue.push_back((
                    *next,
                    distance.checked_add(*length).map_err(|_| Error::Overflow)?,
                ));
            }
        }
    }
    Err(Error::InvalidTree)
}

fn coordinate_bound(value: ExactRatio) -> Result<i128, Error> {
    Ok(value
        .numerator()
        .checked_abs()
        .ok_or(Error::Overflow)?
        .max(value.denominator()))
}

fn ratio(value: i128) -> Result<ExactRatio, Error> {
    ExactRatio::new(value, 1).map_err(|_| Error::Overflow)
}

/// A finite Section 9.1 tree chain cannot be certified.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("finite tree-chain parameters are invalid")]
    InvalidParameters,
    #[error("source edge length is outside the finite rational domain")]
    LengthOutsideFiniteDomain,
    #[error("contracted level construction failed: {0}")]
    Level(#[source] LevelError),
    #[error("bucket initialization failed: {0}")]
    Bucket(#[source] BucketError),
    #[error("AN19-shaped terminal tree failed: {0}")]
    An19(#[source] An19Error),
    #[error("finite spanner embedding certificate is invalid")]
    InvalidEmbedding,
    #[error("recovered source edges are not a spanning tree")]
    InvalidTree,
    #[error("finite tree-chain provenance is invalid")]
    InvalidChain,
    #[error("finite tree-chain exact arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Chain, Error, Parameters};
    use crate::{
        ExactRatio, FlowNodeId,
        source_lsf::oracle::Lsst as Oracle,
        source_lsst::{
            LsfPiece, LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId,
            SourceWeightedEdge,
            bucket::{Error as BucketError, Parameters as BucketParameters},
        },
        source_spanner::{
            dynamic::rebuild::Parameters as RebuildParameters, experiment::domain::ExhaustiveDomain,
        },
    };

    fn edge(first: usize, second: usize) -> SourceWeightedEdge {
        edge_with_length(first, second, ExactRatio::new(1, 1).unwrap())
    }

    fn edge_with_length(first: usize, second: usize, length: ExactRatio) -> SourceWeightedEdge {
        SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length,
            weight: ExactRatio::new(1, 1).unwrap(),
        }
    }

    fn graph() -> SourceDynamicGraph {
        SourceDynamicGraph::new(4, vec![edge(0, 1), edge(1, 2), edge(2, 3)], 8).unwrap()
    }

    fn forest() -> LsfStructuralCertificate {
        LsfStructuralCertificate {
            forest_edges: BTreeSet::from([SourceEdgeId(0)]),
            roots: BTreeSet::from([FlowNodeId(0), FlowNodeId(2), FlowNodeId(3)]),
            pieces: vec![
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]),
                    forest_edges: BTreeSet::from([SourceEdgeId(0)]),
                },
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(2)]),
                    forest_edges: BTreeSet::new(),
                },
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(3)]),
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

    fn parameters() -> Parameters {
        Parameters {
            root: FlowNodeId(0),
            maximum_coordinate: 8,
            buckets: BucketParameters {
                maximum_absolute_exponent: 4,
                spanner: RebuildParameters {
                    phi: ExactRatio::new(1, 2).unwrap(),
                    domain: ExhaustiveDomain { maximum_nodes: 8 },
                    maximum_hops: 2,
                    maximum_vertex_congestion: 100,
                    maximum_rounds: 1,
                },
            },
        }
    }

    #[test]
    fn combines_a_partial_forest_and_terminal_tree_into_a_source_tree() {
        let graph = graph();
        let forest = forest();
        let chain = Chain::build(&graph, &forest, parameters()).unwrap();
        let oracle = Oracle::solve(&graph).unwrap();

        assert_eq!(
            chain.terminal_edges,
            BTreeSet::from([SourceEdgeId(1), SourceEdgeId(2)])
        );
        assert_eq!(
            chain.tree_edges,
            BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1), SourceEdgeId(2)])
        );
        assert_eq!(chain.level_audit.maximum_embedding_hops, 1);
        assert_eq!(chain.level_audit.maximum_embedding_vertex_congestion, 2);
        assert_eq!(chain.level_audit.encoded_embedding_length, 2);
        assert_eq!(
            chain.tree_audit.weighted_stretch,
            ExactRatio::new(6, 1).unwrap()
        );
        assert_eq!(
            chain.tree_audit.maximum_stretch,
            ExactRatio::new(2, 1).unwrap()
        );
        assert_eq!(chain.tree_edges, oracle.edges);
        assert_eq!(chain.tree_audit.weighted_stretch, oracle.weighted_stretch);
        chain.verify(&graph, &forest, parameters()).unwrap();
    }

    #[test]
    fn accepts_bounded_rational_lengths_and_rejects_domain_violations() {
        let rational = SourceDynamicGraph::new(
            4,
            vec![
                edge_with_length(0, 1, ExactRatio::new(1, 2).unwrap()),
                edge_with_length(1, 2, ExactRatio::new(3, 4).unwrap()),
                edge_with_length(2, 3, ExactRatio::new(5, 8).unwrap()),
            ],
            8,
        )
        .unwrap();
        assert!(Chain::build(&rational, &forest(), parameters()).is_ok());

        let out_of_range = SourceDynamicGraph::new(
            2,
            vec![edge_with_length(0, 1, ExactRatio::new(9, 1).unwrap())],
            16,
        )
        .unwrap();
        assert!(matches!(
            Chain::build(&out_of_range, &forest(), parameters()),
            Err(Error::LengthOutsideFiniteDomain)
        ));

        let bounded = SourceDynamicGraph::new(
            4,
            vec![
                edge_with_length(0, 1, ExactRatio::new(4, 1).unwrap()),
                edge_with_length(1, 2, ExactRatio::new(4, 1).unwrap()),
                edge_with_length(2, 3, ExactRatio::new(4, 1).unwrap()),
            ],
            8,
        )
        .unwrap();
        let mut narrow_buckets = parameters();
        narrow_buckets.buckets.maximum_absolute_exponent = 2;
        assert!(matches!(
            Chain::build(&bounded, &forest(), narrow_buckets),
            Err(Error::Bucket(BucketError::OutsideFiniteDomain))
        ));

        let parallel =
            SourceDynamicGraph::new(4, vec![edge(0, 1), edge(1, 2), edge(1, 2), edge(2, 3)], 8)
                .unwrap();
        let mut parallel_forest = forest();
        parallel_forest.stretch_overestimates = vec![
            ExactRatio::new(2, 1).unwrap(),
            ExactRatio::new(2, 1).unwrap(),
            ExactRatio::new(2, 1).unwrap(),
            ExactRatio::new(1, 1).unwrap(),
        ];
        parallel_forest.piece_volume_limit = 4;
        assert!(matches!(
            Chain::build(&parallel, &parallel_forest, parameters()),
            Err(Error::Bucket(BucketError::Graph(_)))
        ));

        let graph = graph();
        let forest = forest();
        let mut corrupted = Chain::build(&graph, &forest, parameters()).unwrap();
        corrupted.tree_audit.total_weight = ExactRatio::new(0, 1).unwrap();
        assert!(matches!(
            corrupted.verify(&graph, &forest, parameters()),
            Err(Error::InvalidChain)
        ));
    }
}
