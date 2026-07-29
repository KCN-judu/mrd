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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parameters {
    pub maximum_absolute_exponent: u32,
    pub spanner: RebuildParameters,
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
    pub replay: RebuildState,
    pub embedding: Embedding,
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
                buckets.push(initialize_bucket(key, group, parameters.spanner)?);
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
    let stretch_exponent = exponent(edge.stretch_overestimate)?;
    let scaled_length_exponent = exponent(edge.scaled_length)?;
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
    let numerator = value.numerator();
    let denominator = value.denominator();
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

fn initialize_bucket(
    key: Key,
    group: Group,
    parameters: RebuildParameters,
) -> Result<Bucket, Error> {
    let vertices = group.vertices.into_iter().collect::<Vec<_>>();
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
    let sources = group
        .edges
        .iter()
        .map(|edge| edge.source)
        .collect::<Vec<_>>();
    let replay = RebuildState::new(BatchState::new(&graph).map_err(Error::Batch)?, parameters)
        .map_err(Error::Spanner)?;
    let embedding = translate(&replay, &sources)?;
    Ok(Bucket {
        key,
        vertices,
        sources,
        replay,
        embedding,
    })
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

    use super::{Key, Parameters, Partition, exponent};
    use crate::{
        ExactRatio, FlowNodeId,
        source_lsst::{
            LsfPiece, LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId,
            SourceWeightedEdge, level::Level,
        },
        source_spanner::{
            dynamic::rebuild::Parameters as RebuildParameters, experiment::domain::ExhaustiveDomain,
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

    fn parameters() -> Parameters {
        Parameters {
            maximum_absolute_exponent: 4,
            spanner: RebuildParameters {
                phi: ExactRatio::new(1, 2).unwrap(),
                domain: ExhaustiveDomain { maximum_nodes: 8 },
                maximum_hops: 2,
                maximum_vertex_congestion: 100,
                maximum_rounds: 1,
            },
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
}
