use std::collections::HashMap;

use rect_core::BicliqueId;
use rect_graph::BipartiteGraph;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::embedding::{DominanceEmbedding, DominancePoint};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Biclique {
    pub id: BicliqueId,
    pub left: Vec<usize>,
    pub right: Vec<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliquePartition {
    pub bicliques: Vec<Biclique>,
}

impl BicliquePartition {
    #[must_use]
    pub fn from_explicit_edges(graph: &BipartiteGraph) -> Self {
        let bicliques = graph
            .edges()
            .enumerate()
            .map(|(index, (left, right))| Biclique {
                id: BicliqueId(index),
                left: vec![left],
                right: vec![right],
            })
            .collect();
        Self { bicliques }
    }

    #[must_use]
    pub fn comparability_theorem_8(embedding: &DominanceEmbedding) -> Self {
        let mut partition = Self::default();
        let left = (0..embedding.horizontal.len()).collect::<Vec<_>>();
        let right = (0..embedding.vertical.len()).collect::<Vec<_>>();
        partition_recursive(embedding, &left, &right, 4, &mut partition.bicliques);
        for (index, biclique) in partition.bicliques.iter_mut().enumerate() {
            biclique.id = BicliqueId(index);
        }
        partition
    }

    #[must_use]
    pub fn total_vertex_occurrences(&self) -> usize {
        self.bicliques
            .iter()
            .map(|biclique| biclique.left.len() + biclique.right.len())
            .sum()
    }

    /// Verifies edge-set equality and multiplicity one against an explicit graph.
    ///
    /// # Errors
    ///
    /// Returns [`BicliqueError`] for an invalid endpoint, omitted edge,
    /// spurious edge, or duplicate representation.
    pub fn verify_exact_partition(&self, graph: &BipartiteGraph) -> Result<(), BicliqueError> {
        let mut represented = HashMap::<(usize, usize), usize>::new();
        for biclique in &self.bicliques {
            if biclique.left.is_empty() || biclique.right.is_empty() {
                return Err(BicliqueError::EmptySide { id: biclique.id });
            }
            for &left in &biclique.left {
                if left >= graph.left_size() {
                    return Err(BicliqueError::LeftOutOfBounds {
                        id: biclique.id,
                        left,
                    });
                }
                for &right in &biclique.right {
                    if right >= graph.right_size() {
                        return Err(BicliqueError::RightOutOfBounds {
                            id: biclique.id,
                            right,
                        });
                    }
                    if !graph.neighbors(left).contains(&right) {
                        return Err(BicliqueError::SpuriousEdge {
                            id: biclique.id,
                            left,
                            right,
                        });
                    }
                    *represented.entry((left, right)).or_default() += 1;
                }
            }
        }
        for edge in graph.edges() {
            match represented.remove(&edge) {
                Some(1) => {}
                Some(multiplicity) => {
                    return Err(BicliqueError::DuplicateEdge {
                        left: edge.0,
                        right: edge.1,
                        multiplicity,
                    });
                }
                None => {
                    return Err(BicliqueError::MissingEdge {
                        left: edge.0,
                        right: edge.1,
                    });
                }
            }
        }
        if let Some((&(left, right), _)) = represented.iter().next() {
            return Err(BicliqueError::SpuriousEdge {
                id: BicliqueId(usize::MAX),
                left,
                right,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum SidePoint {
    Left(usize),
    Right(usize),
}

fn partition_recursive(
    embedding: &DominanceEmbedding,
    left: &[usize],
    right: &[usize],
    dimensions: usize,
    output: &mut Vec<Biclique>,
) {
    if left.is_empty() || right.is_empty() {
        return;
    }
    if dimensions == 0 {
        output.push(Biclique {
            id: BicliqueId(output.len()),
            left: left.to_vec(),
            right: right.to_vec(),
        });
        return;
    }

    let coordinate = dimensions - 1;
    let mut points = left
        .iter()
        .copied()
        .map(SidePoint::Left)
        .chain(right.iter().copied().map(SidePoint::Right))
        .collect::<Vec<_>>();
    points.sort_by_key(|point| match *point {
        SidePoint::Left(index) => (
            embedding.horizontal[index].coordinates[coordinate],
            0_u8,
            index,
        ),
        SidePoint::Right(index) => (
            embedding.vertical[index].coordinates[coordinate],
            1_u8,
            index,
        ),
    });
    let split = points.len() / 2;
    let (low_points, high_points) = points.split_at(split);
    let (low_left, low_right) = split_sides(low_points);
    let (high_left, high_right) = split_sides(high_points);

    partition_recursive(embedding, &low_left, &high_right, dimensions - 1, output);
    partition_recursive(embedding, &low_left, &low_right, dimensions, output);
    partition_recursive(embedding, &high_left, &high_right, dimensions, output);
}

fn split_sides(points: &[SidePoint]) -> (Vec<usize>, Vec<usize>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    for &point in points {
        match point {
            SidePoint::Left(index) => left.push(index),
            SidePoint::Right(index) => right.push(index),
        }
    }
    (left, right)
}

#[allow(dead_code)]
fn _point_coordinate(point: DominancePoint, coordinate: usize) -> i128 {
    point.coordinates[coordinate]
}

#[derive(Debug, Error)]
pub enum BicliqueError {
    #[error("biclique {id:?} has an empty side")]
    EmptySide { id: BicliqueId },
    #[error("biclique {id:?} contains out-of-bounds left vertex {left}")]
    LeftOutOfBounds { id: BicliqueId, left: usize },
    #[error("biclique {id:?} contains out-of-bounds right vertex {right}")]
    RightOutOfBounds { id: BicliqueId, right: usize },
    #[error("biclique {id:?} represents non-edge ({left}, {right})")]
    SpuriousEdge {
        id: BicliqueId,
        left: usize,
        right: usize,
    },
    #[error("partition omits graph edge ({left}, {right})")]
    MissingEdge { left: usize, right: usize },
    #[error("partition represents edge ({left}, {right}) {multiplicity} times")]
    DuplicateEdge {
        left: usize,
        right: usize,
        multiplicity: usize,
    },
}

#[cfg(test)]
mod tests {
    use rect_core::{HorizontalChord, HorizontalChordId, VerticalChord, VerticalChordId};

    use crate::embedding::DominanceEmbedding;

    use super::BicliquePartition;

    #[test]
    fn theorem_8_recursion_is_an_edge_partition() {
        let horizontal = [
            HorizontalChord::new(HorizontalChordId(0), 0, 4, 0).unwrap(),
            HorizontalChord::new(HorizontalChordId(1), 1, 2, 3).unwrap(),
            HorizontalChord::new(HorizontalChordId(2), -2, 1, 1).unwrap(),
        ];
        let vertical = [
            VerticalChord::new(VerticalChordId(0), 0, -1, 2).unwrap(),
            VerticalChord::new(VerticalChordId(1), 2, 0, 4).unwrap(),
            VerticalChord::new(VerticalChordId(2), 4, 0, 1).unwrap(),
        ];
        let embedding = DominanceEmbedding::new(&horizontal, &vertical).unwrap();
        let graph = embedding.explicit_graph().unwrap();
        let partition = BicliquePartition::comparability_theorem_8(&embedding);
        partition.verify_exact_partition(&graph).unwrap();
    }
}
