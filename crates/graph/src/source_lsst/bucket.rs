//! Exact Section 9.1 stretch/length buckets with finite spanner initialization.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    ExactRatio, FlowNodeId,
    source_spanner::{
        dynamic::{
            batch::{Error as BatchError, State as BatchState},
            rebuild::{
                Error as RebuildError, Parameters as RebuildParameters, State as RebuildState,
            },
        },
        model::{Edge as ModelEdge, EdgeId, Error as ModelError, Graph},
    },
};

use super::{
    SourceEdgeId,
    level::{ComponentId, Edge as LevelEdge, Level},
};

/// Exact dyadic ranges used by a Section 9.1 contracted-edge bucket.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Key {
    pub stretch_exponent: i32,
    pub scaled_length_exponent: i32,
}

/// Finite-domain parameters for Section 9.1 bucket initialization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub maximum_absolute_exponent: u32,
    /// Explicit finite construction policy for cyclic bucket components.
    pub construction: Construction,
}

/// Finite construction policy for one Section 9.1 bucket component.
///
/// `Algorithm4` is the checked source-shaped decomposition path. `CanonicalTree`
/// is a deterministic finite-domain spanner certificate used when the source
/// decomposition witness is outside the currently implemented finite subset.
/// It is a real tree embedding, not an Oracle or a retry after Algorithm 4
/// failure; callers choose the policy before construction starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Construction {
    Algorithm4(RebuildParameters),
    CanonicalTree,
}

/// A stable-source embedding translated from a finite local spanner replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Embedding {
    pub selected: BTreeSet<SourceEdgeId>,
    pub paths: BTreeMap<SourceEdgeId, Vec<SourceEdgeId>>,
}

/// One connected, simple finite component of a stretch/length bucket.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bucket {
    pub key: Key,
    pub vertices: Vec<ComponentId>,
    pub sources: Vec<SourceEdgeId>,
    /// Exact construction used to embed this finite bucket.
    pub replay: Replay,
    pub embedding: Embedding,
}

/// The exact finite replay used to certify one bucket embedding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Replay {
    Algorithm4(RebuildState),
    TreeIdentity,
    CanonicalTree,
}

/// Exact finite measurements for all initialized bucket components.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Audit {
    pub contracted_edges: u64,
    pub bucket_components: u64,
    pub selected_edges: u64,
    pub embedded_edges: u64,
}

/// All finite Section 9.1 buckets for one contracted level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Partition {
    pub buckets: Vec<Bucket>,
    pub audit: Audit,
}

impl Partition {
    /// Splits a contracted level into exact buckets and initializes each spanner.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsupported dyadic range, a parallel bucket
    /// edge, or a component outside the existing finite spanner domain.
    pub fn initialize(level: &Level, parameters: Parameters) -> Result<Self, Error> {
        if parameters.maximum_absolute_exponent == 0 {
            return Err(Error::InvalidParameters);
        }
        let mut grouped = BTreeMap::<Key, Vec<LevelEdge>>::new();
        for edge in &level.edges {
            let key = key(edge, parameters.maximum_absolute_exponent)?;
            grouped.entry(key).or_default().push(edge.clone());
        }
        let mut buckets = Vec::new();
        for (key, edges) in grouped {
            for group in connected_components(&edges) {
                buckets.push(initialize_bucket(key, group, parameters.clone())?);
            }
        }
        let audit = Audit {
            contracted_edges: u64::try_from(level.edges.len()).map_err(|_| Error::Overflow)?,
            bucket_components: u64::try_from(buckets.len()).map_err(|_| Error::Overflow)?,
            selected_edges: buckets.iter().try_fold(0_u64, |total, bucket| {
                total
                    .checked_add(
                        u64::try_from(bucket.embedding.selected.len())
                            .map_err(|_| Error::Overflow)?,
                    )
                    .ok_or(Error::Overflow)
            })?,
            embedded_edges: buckets.iter().try_fold(0_u64, |total, bucket| {
                total
                    .checked_add(
                        u64::try_from(bucket.embedding.paths.len()).map_err(|_| Error::Overflow)?,
                    )
                    .ok_or(Error::Overflow)
            })?,
        };
        Ok(Self { buckets, audit })
    }

    /// Recomputes the bucket partition and every translated finite embedding.
    ///
    /// # Errors
    ///
    /// Returns an error when fresh exact initialization differs from this value.
    pub fn verify(&self, level: &Level, parameters: Parameters) -> Result<(), Error> {
        if &Self::initialize(level, parameters)? != self {
            return Err(Error::InvalidPartition);
        }
        Ok(())
    }
}

struct Group {
    vertices: BTreeSet<ComponentId>,
    edges: Vec<LevelEdge>,
}

fn key(edge: &LevelEdge, maximum: u32) -> Result<Key, Error> {
    let stretch_exponent = exponent(edge.stretch_overestimate.clone())?;
    let scaled_length_exponent = exponent(edge.scaled_length.clone())?;
    if stretch_exponent.unsigned_abs() > maximum || scaled_length_exponent.unsigned_abs() > maximum
    {
        return Err(Error::OutsideFiniteDomain);
    }
    Ok(Key {
        stretch_exponent,
        scaled_length_exponent,
    })
}

fn exponent(value: ExactRatio) -> Result<i32, Error> {
    if !value.is_positive() {
        return Err(Error::InvalidRatio);
    }
    let numerator = value.numerator_i128().map_err(|_| Error::Overflow)?;
    let denominator = value.denominator_i128().map_err(|_| Error::Overflow)?;
    let numerator_bits =
        i32::try_from(i128::BITS - numerator.leading_zeros()).map_err(|_| Error::Overflow)?;
    let denominator_bits =
        i32::try_from(i128::BITS - denominator.leading_zeros()).map_err(|_| Error::Overflow)?;
    let mut result = numerator_bits
        .checked_sub(denominator_bits)
        .ok_or(Error::Overflow)?;
    if result >= 0 {
        let shift = u32::try_from(result).map_err(|_| Error::Overflow)?;
        if (numerator >> shift) < denominator {
            result = result.checked_sub(1).ok_or(Error::Overflow)?;
        }
    } else {
        let shift = result.unsigned_abs();
        let quotient = denominator >> shift;
        let mask = (1_i128 << shift).checked_sub(1).ok_or(Error::Overflow)?;
        let ceiling = quotient
            .checked_add(i128::from((denominator & mask) != 0))
            .ok_or(Error::Overflow)?;
        if numerator < ceiling {
            result = result.checked_sub(1).ok_or(Error::Overflow)?;
        }
    }
    Ok(result)
}

fn connected_components(edges: &[LevelEdge]) -> Vec<Group> {
    let mut adjacency = BTreeMap::<ComponentId, Vec<usize>>::new();
    for (index, edge) in edges.iter().enumerate() {
        adjacency.entry(edge.first).or_default().push(index);
        adjacency.entry(edge.second).or_default().push(index);
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for start in adjacency.keys().copied() {
        if !seen.insert(start) {
            continue;
        }
        let mut vertices = BTreeSet::from([start]);
        let mut edge_indices = BTreeSet::new();
        let mut queue = VecDeque::from([start]);
        while let Some(vertex) = queue.pop_front() {
            for index in adjacency.get(&vertex).into_iter().flatten() {
                edge_indices.insert(*index);
                let edge = &edges[*index];
                for next in [edge.first, edge.second] {
                    if seen.insert(next) {
                        vertices.insert(next);
                        queue.push_back(next);
                    }
                }
            }
        }
        result.push(Group {
            vertices,
            edges: edge_indices
                .into_iter()
                .map(|index| edges[index].clone())
                .collect(),
        });
    }
    result
}

fn initialize_bucket(key: Key, group: Group, parameters: Parameters) -> Result<Bucket, Error> {
    let vertices = group.vertices.iter().copied().collect::<Vec<_>>();
    let sources = group
        .edges
        .iter()
        .map(|edge| edge.source)
        .collect::<Vec<_>>();
    let (replay, embedding) = if group.edges.len().checked_add(1) == Some(vertices.len()) {
        let embedding = Embedding {
            selected: sources.iter().copied().collect(),
            paths: sources
                .iter()
                .copied()
                .map(|source| (source, vec![source]))
                .collect(),
        };
        verify_embedding(&group, &embedding)?;
        (Replay::TreeIdentity, embedding)
    } else {
        match parameters.construction {
            Construction::Algorithm4(parameters) => {
                let local = vertices
                    .iter()
                    .enumerate()
                    .map(|(index, vertex)| (*vertex, FlowNodeId(index)))
                    .collect::<BTreeMap<_, _>>();
                let graph = Graph::new(
                    vertices.len(),
                    group
                        .edges
                        .iter()
                        .map(|edge| {
                            Ok(ModelEdge {
                                first: *local.get(&edge.first).ok_or(Error::InvalidPartition)?,
                                second: *local.get(&edge.second).ok_or(Error::InvalidPartition)?,
                            })
                        })
                        .collect::<Result<Vec<_>, Error>>()?,
                )
                .map_err(Error::Graph)?;
                let replay =
                    RebuildState::new(BatchState::new(&graph).map_err(Error::Batch)?, parameters)
                        .map_err(Error::Spanner)?;
                let embedding = translate(&replay, &sources)?;
                (Replay::Algorithm4(replay), embedding)
            }
            Construction::CanonicalTree => {
                let embedding = canonical_tree_embedding(&group, &vertices)?;
                (Replay::CanonicalTree, embedding)
            }
        }
    };
    Ok(Bucket {
        key,
        vertices,
        sources,
        replay,
        embedding,
    })
}

fn canonical_tree_embedding(group: &Group, vertices: &[ComponentId]) -> Result<Embedding, Error> {
    let selected = canonical_tree_edges(vertices, &group.edges)?;
    let expected = vertices
        .len()
        .checked_sub(1)
        .ok_or(Error::InvalidPartition)?;
    if selected.len() != expected {
        return Err(Error::InvalidPartition);
    }

    let mut adjacency = BTreeMap::<ComponentId, Vec<(ComponentId, SourceEdgeId)>>::new();
    for edge in &group.edges {
        if !selected.contains(&edge.source) {
            continue;
        }
        adjacency
            .entry(edge.first)
            .or_default()
            .push((edge.second, edge.source));
        adjacency
            .entry(edge.second)
            .or_default()
            .push((edge.first, edge.source));
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_by_key(|(_, source)| *source);
    }

    let paths = group
        .edges
        .iter()
        .map(|edge| {
            let path = if selected.contains(&edge.source) {
                vec![edge.source]
            } else {
                tree_path(&adjacency, edge.first, edge.second)?
            };
            Ok((edge.source, path))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    let embedding = Embedding { selected, paths };
    verify_embedding(group, &embedding)?;
    Ok(embedding)
}

fn canonical_tree_edges(
    vertices: &[ComponentId],
    edges: &[LevelEdge],
) -> Result<BTreeSet<SourceEdgeId>, Error> {
    let index = vertices
        .iter()
        .enumerate()
        .map(|(index, vertex)| (*vertex, index))
        .collect::<BTreeMap<_, _>>();
    let mut union_find = UnionFind::new(vertices.len());
    let mut selected = BTreeSet::new();
    let mut ordered = edges.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|edge| edge.source);
    for edge in ordered {
        let first = *index.get(&edge.first).ok_or(Error::InvalidPartition)?;
        let second = *index.get(&edge.second).ok_or(Error::InvalidPartition)?;
        if union_find.union(first, second) {
            selected.insert(edge.source);
        }
    }
    Ok(selected)
}

fn tree_path(
    adjacency: &BTreeMap<ComponentId, Vec<(ComponentId, SourceEdgeId)>>,
    start: ComponentId,
    target: ComponentId,
) -> Result<Vec<SourceEdgeId>, Error> {
    let mut predecessor = BTreeMap::<ComponentId, Option<(ComponentId, SourceEdgeId)>>::new();
    let mut queue = VecDeque::from([start]);
    predecessor.insert(start, None);
    while let Some(vertex) = queue.pop_front() {
        if vertex == target {
            break;
        }
        for (next, source) in adjacency.get(&vertex).into_iter().flatten() {
            if predecessor.contains_key(next) {
                continue;
            }
            predecessor.insert(*next, Some((vertex, *source)));
            queue.push_back(*next);
        }
    }
    if !predecessor.contains_key(&target) {
        return Err(Error::InvalidPartition);
    }
    let mut path = Vec::new();
    let mut current = target;
    while current != start {
        let (previous, source) = predecessor
            .get(&current)
            .and_then(|entry| *entry)
            .ok_or(Error::InvalidPartition)?;
        path.push(source);
        current = previous;
    }
    path.reverse();
    Ok(path)
}

fn verify_embedding(group: &Group, embedding: &Embedding) -> Result<(), Error> {
    let sources = group
        .edges
        .iter()
        .map(|edge| edge.source)
        .collect::<BTreeSet<_>>();
    if sources.len() != group.edges.len()
        || (embedding.selected.is_empty() && group.vertices.len() > 1)
    {
        return Err(Error::InvalidPartition);
    }
    if !embedding.selected.is_subset(&sources)
        || embedding.paths.keys().copied().collect::<BTreeSet<_>>() != sources
    {
        return Err(Error::InvalidPartition);
    }
    let edges = group
        .edges
        .iter()
        .map(|edge| (edge.source, edge))
        .collect::<BTreeMap<_, _>>();
    for edge in &group.edges {
        let path = embedding
            .paths
            .get(&edge.source)
            .ok_or(Error::InvalidPartition)?;
        if path.is_empty()
            || (!embedding.selected.contains(&edge.source) && path.contains(&edge.source))
        {
            return Err(Error::InvalidPartition);
        }
        let mut current = edge.first;
        let mut visited = BTreeSet::from([current]);
        for source in path {
            if !embedding.selected.contains(source) {
                return Err(Error::InvalidPartition);
            }
            let mapped = edges.get(source).ok_or(Error::InvalidPartition)?;
            current = if mapped.first == current {
                mapped.second
            } else if mapped.second == current {
                mapped.first
            } else {
                return Err(Error::InvalidPartition);
            };
            if !visited.insert(current) {
                return Err(Error::InvalidPartition);
            }
        }
        if current != edge.second {
            return Err(Error::InvalidPartition);
        }
    }
    Ok(())
}

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, value: usize) -> usize {
        if self.parent[value] != value {
            let root = self.find(self.parent[value]);
            self.parent[value] = root;
        }
        self.parent[value]
    }

    fn union(&mut self, left: usize, right: usize) -> bool {
        let mut left_root = self.find(left);
        let mut right_root = self.find(right);
        if left_root == right_root {
            return false;
        }
        if self.rank[left_root] < self.rank[right_root] {
            std::mem::swap(&mut left_root, &mut right_root);
        }
        self.parent[right_root] = left_root;
        if self.rank[left_root] == self.rank[right_root] {
            self.rank[left_root] += 1;
        }
        true
    }
}

fn translate(replay: &RebuildState, sources: &[SourceEdgeId]) -> Result<Embedding, Error> {
    let source = |edge: EdgeId| sources.get(edge.0).copied().ok_or(Error::InvalidPartition);
    let selected = replay
        .snapshot
        .selected
        .iter()
        .map(|edge| source(*edge))
        .collect::<Result<BTreeSet<_>, _>>()?;
    let paths = replay
        .snapshot
        .embeddings
        .iter()
        .map(|(edge, path)| {
            Ok((
                source(*edge)?,
                path.iter()
                    .map(|item| source(*item))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    if paths.len() != sources.len()
        || paths.keys().copied().collect::<BTreeSet<_>>() != sources.iter().copied().collect()
    {
        return Err(Error::InvalidPartition);
    }
    Ok(Embedding { selected, paths })
}

/// Exact finite bucket initialization cannot proceed.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("bucket parameters are invalid")]
    InvalidParameters,
    #[error("bucket ratio is not positive")]
    InvalidRatio,
    #[error("bucket lies outside the explicit finite dyadic domain")]
    OutsideFiniteDomain,
    #[error("contracted bucket provenance is invalid")]
    InvalidPartition,
    #[error("contracted bucket is not a simple graph: {0}")]
    Graph(#[source] ModelError),
    #[error("finite spanner input is invalid: {0}")]
    Batch(#[source] BatchError),
    #[error("finite spanner replay failed: {0}")]
    Spanner(#[source] RebuildError),
    #[error("finite bucket accounting overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Construction, Error, Key, Parameters, Partition, Replay, exponent};
    use crate::{
        ExactRatio, FlowNodeId,
        source_lsst::{
            LsfPiece, LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId,
            SourceWeightedEdge, level::Level,
        },
        source_spanner::{
            dynamic::rebuild::Parameters as RebuildParameters,
            experiment::domain::ExhaustiveDomain, model::Error as ModelError,
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

    fn level() -> Level {
        let graph =
            SourceDynamicGraph::new(4, vec![edge(0, 1), edge(1, 2), edge(2, 3)], 8).unwrap();
        let forest = LsfStructuralCertificate {
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
        };
        Level::contract(&graph, &forest).unwrap()
    }

    fn parallel_level() -> Level {
        let graph =
            SourceDynamicGraph::new(3, vec![edge(0, 1), edge(0, 1), edge(1, 2)], 8).unwrap();
        let forest = LsfStructuralCertificate {
            forest_edges: BTreeSet::new(),
            roots: BTreeSet::from([FlowNodeId(0), FlowNodeId(1), FlowNodeId(2)]),
            pieces: vec![
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(0)]),
                    forest_edges: BTreeSet::new(),
                },
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(1)]),
                    forest_edges: BTreeSet::new(),
                },
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(2)]),
                    forest_edges: BTreeSet::new(),
                },
            ],
            stretch_overestimates: vec![
                ExactRatio::new(1, 1).unwrap(),
                ExactRatio::new(1, 1).unwrap(),
                ExactRatio::new(1, 1).unwrap(),
            ],
            piece_volume_limit: 1,
        };
        Level::contract(&graph, &forest).unwrap()
    }

    fn parameters() -> Parameters {
        Parameters {
            maximum_absolute_exponent: 4,
            construction: Construction::Algorithm4(RebuildParameters {
                phi: ExactRatio::new(1, 2).unwrap(),
                domain: ExhaustiveDomain { maximum_nodes: 8 },
                maximum_hops: 2,
                maximum_vertex_congestion: 100,
                maximum_rounds: 1,
            }),
        }
    }

    #[test]
    fn partitions_exact_dyadic_ranges_and_translates_spanner_provenance() {
        let level = level();
        let partition = Partition::initialize(&level, parameters()).unwrap();

        assert_eq!(partition.buckets.len(), 2);
        assert_eq!(partition.audit.contracted_edges, 2);
        assert_eq!(partition.audit.bucket_components, 2);
        assert_eq!(partition.audit.selected_edges, 2);
        assert_eq!(partition.audit.embedded_edges, 2);
        let first = partition
            .buckets
            .iter()
            .find(|bucket| {
                bucket.key
                    == Key {
                        stretch_exponent: 1,
                        scaled_length_exponent: 1,
                    }
            })
            .unwrap();
        assert_eq!(first.sources, vec![SourceEdgeId(1)]);
        assert_eq!(first.embedding.selected, BTreeSet::from([SourceEdgeId(1)]));
        assert_eq!(
            first.embedding.paths[&SourceEdgeId(1)],
            vec![SourceEdgeId(1)]
        );
        partition.verify(&level, parameters()).unwrap();
    }

    #[test]
    fn computes_floor_log_two_without_floating_point() {
        assert_eq!(exponent(ExactRatio::new(1, 3).unwrap()).unwrap(), -2);
        assert_eq!(exponent(ExactRatio::new(1, 2).unwrap()).unwrap(), -1);
        assert_eq!(exponent(ExactRatio::new(3, 2).unwrap()).unwrap(), 0);
        assert_eq!(exponent(ExactRatio::new(2, 1).unwrap()).unwrap(), 1);
    }

    #[test]
    fn canonical_tree_certifies_a_cyclic_parallel_bucket_without_algorithm_four() {
        let mut parameters = parameters();
        parameters.construction = Construction::CanonicalTree;
        let level = parallel_level();

        let partition = Partition::initialize(&level, parameters.clone()).unwrap();

        assert_eq!(partition.buckets.len(), 1);
        let bucket = &partition.buckets[0];
        assert_eq!(bucket.replay, Replay::CanonicalTree);
        assert_eq!(
            bucket.embedding.selected,
            BTreeSet::from([SourceEdgeId(0), SourceEdgeId(2)])
        );
        assert_eq!(
            bucket.embedding.paths[&SourceEdgeId(0)],
            vec![SourceEdgeId(0)]
        );
        assert_eq!(
            bucket.embedding.paths[&SourceEdgeId(1)],
            vec![SourceEdgeId(0)]
        );
        assert_eq!(
            bucket.embedding.paths[&SourceEdgeId(2)],
            vec![SourceEdgeId(2)]
        );
        partition.verify(&level, parameters).unwrap();
    }

    #[test]
    fn algorithm_four_policy_rejects_a_parallel_bucket_without_retrying_canonical_tree() {
        assert_eq!(
            Partition::initialize(&parallel_level(), parameters()),
            Err(Error::Graph(ModelError::InvalidGraph))
        );
    }
}
