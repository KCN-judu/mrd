use std::collections::{BTreeSet, HashMap};

use rect_core::BicliqueId;
use rect_graph::BipartiteGraph;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::embedding::DominanceEmbedding;

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliqueBlock {
    pub id: BicliqueId,
    pub left: Vec<usize>,
    pub right: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliquePartitionCertificate {
    pub blocks: Vec<BicliqueBlock>,
    pub explicit_edge_count: usize,
    pub represented_edge_count: usize,
    pub duplicate_edge_count: usize,
    pub missing_edge_count: usize,
    pub fabricated_edge_count: usize,
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

    /// Constructs the Cardinal--Yuditsky Theorem 8 edge partition.
    ///
    /// # Errors
    ///
    /// Returns [`BicliqueError`] when a cross-side coordinate equality or a
    /// non-decreasing recursive subproblem violates the source construction.
    pub fn comparability_theorem_8(embedding: &DominanceEmbedding) -> Result<Self, BicliqueError> {
        verify_coordinate_order_assumptions(embedding)?;
        let mut partition = Self::default();
        let left = (0..embedding.horizontal.len()).collect::<Vec<_>>();
        let right = (0..embedding.vertical.len()).collect::<Vec<_>>();
        partition_recursive(embedding, &left, &right, 4, &mut partition.bicliques)?;
        for (index, biclique) in partition.bicliques.iter_mut().enumerate() {
            biclique.id = BicliqueId(index);
        }
        Ok(partition)
    }

    #[must_use]
    pub fn total_vertex_occurrences(&self) -> usize {
        self.bicliques
            .iter()
            .map(|biclique| biclique.left.len() + biclique.right.len())
            .sum()
    }

    /// Verifies block structure, edge-set equality, and multiplicity one.
    ///
    /// # Errors
    ///
    /// Returns [`BicliqueError`] for duplicate IDs, invalid endpoints, a
    /// non-biclique block, omitted edges, fabricated edges, or duplicates.
    pub fn verify_exact_partition(&self, graph: &BipartiteGraph) -> Result<(), BicliqueError> {
        for biclique in &self.bicliques {
            if biclique.left.is_empty() || biclique.right.is_empty() {
                return Err(BicliqueError::EmptySide { id: biclique.id });
            }
            if biclique.left.iter().copied().collect::<BTreeSet<_>>().len() != biclique.left.len() {
                return Err(BicliqueError::DuplicateLeftVertex { id: biclique.id });
            }
            if biclique
                .right
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .len()
                != biclique.right.len()
            {
                return Err(BicliqueError::DuplicateRightVertex { id: biclique.id });
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
                }
            }
        }
        let certificate = self.certificate(graph);
        if certificate.fabricated_edge_count != 0 {
            return Err(BicliqueError::FabricatedEdges {
                count: certificate.fabricated_edge_count,
            });
        }
        if certificate.missing_edge_count != 0 {
            return Err(BicliqueError::MissingEdges {
                count: certificate.missing_edge_count,
            });
        }
        if certificate.duplicate_edge_count != 0 {
            return Err(BicliqueError::DuplicateEdges {
                count: certificate.duplicate_edge_count,
            });
        }
        Ok(())
    }

    #[must_use]
    pub fn certificate(&self, graph: &BipartiteGraph) -> BicliquePartitionCertificate {
        let explicit = graph.edges().collect::<BTreeSet<_>>();
        let mut multiplicities = HashMap::<(usize, usize), usize>::new();
        let mut represented_edge_count = 0;
        for biclique in &self.bicliques {
            for &left in &biclique.left {
                for &right in &biclique.right {
                    represented_edge_count += 1;
                    *multiplicities.entry((left, right)).or_default() += 1;
                }
            }
        }
        let duplicate_edge_count = multiplicities.values().map(|&count| count - 1).sum();
        let represented = multiplicities.keys().copied().collect::<BTreeSet<_>>();
        let missing_edge_count = explicit.difference(&represented).count();
        let fabricated_edge_count = multiplicities
            .iter()
            .filter(|(edge, _)| !explicit.contains(edge))
            .map(|(_, &count)| count)
            .sum();
        BicliquePartitionCertificate {
            blocks: self
                .bicliques
                .iter()
                .map(|biclique| BicliqueBlock {
                    id: biclique.id,
                    left: biclique.left.clone(),
                    right: biclique.right.clone(),
                })
                .collect(),
            explicit_edge_count: explicit.len(),
            represented_edge_count,
            duplicate_edge_count,
            missing_edge_count,
            fabricated_edge_count,
        }
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
) -> Result<(), BicliqueError> {
    if left.is_empty() || right.is_empty() {
        return Ok(());
    }
    if dimensions == 0 {
        output.push(Biclique {
            id: BicliqueId(output.len()),
            left: left.to_vec(),
            right: right.to_vec(),
        });
        return Ok(());
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
    if split == 0 || split == points.len() {
        return Err(BicliqueError::NonDecreasingRecursion {
            dimensions,
            vertex_count: points.len(),
        });
    }
    let (low_points, high_points) = points.split_at(split);
    let (low_left, low_right) = split_sides(low_points);
    let (high_left, high_right) = split_sides(high_points);

    verify_recursive_reduction(
        dimensions,
        points.len(),
        dimensions - 1,
        low_left.len() + high_right.len(),
    )?;
    partition_recursive(embedding, &low_left, &high_right, dimensions - 1, output)?;
    verify_recursive_reduction(
        dimensions,
        points.len(),
        dimensions,
        low_left.len() + low_right.len(),
    )?;
    partition_recursive(embedding, &low_left, &low_right, dimensions, output)?;
    verify_recursive_reduction(
        dimensions,
        points.len(),
        dimensions,
        high_left.len() + high_right.len(),
    )?;
    partition_recursive(embedding, &high_left, &high_right, dimensions, output)?;
    Ok(())
}

fn verify_coordinate_order_assumptions(
    embedding: &DominanceEmbedding,
) -> Result<(), BicliqueError> {
    for (left, horizontal) in embedding.horizontal.iter().enumerate() {
        for (right, vertical) in embedding.vertical.iter().enumerate() {
            for coordinate in 0..4 {
                if horizontal.coordinates[coordinate] == vertical.coordinates[coordinate] {
                    return Err(BicliqueError::CrossSideCoordinateEquality {
                        left,
                        right,
                        coordinate,
                    });
                }
            }
        }
    }
    Ok(())
}

fn verify_recursive_reduction(
    parent_dimensions: usize,
    parent_vertices: usize,
    child_dimensions: usize,
    child_vertices: usize,
) -> Result<(), BicliqueError> {
    if child_vertices == 0
        || child_dimensions < parent_dimensions
        || (child_dimensions == parent_dimensions && child_vertices < parent_vertices)
    {
        return Ok(());
    }
    Err(BicliqueError::NonDecreasingRecursion {
        dimensions: child_dimensions,
        vertex_count: child_vertices,
    })
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

#[derive(Debug, Error)]
pub enum BicliqueError {
    #[error("biclique {id:?} has an empty side")]
    EmptySide { id: BicliqueId },
    #[error("biclique {id:?} repeats a left vertex ID")]
    DuplicateLeftVertex { id: BicliqueId },
    #[error("biclique {id:?} repeats a right vertex ID")]
    DuplicateRightVertex { id: BicliqueId },
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
    #[error("partition contains {count} fabricated edge occurrences")]
    FabricatedEdges { count: usize },
    #[error("partition omits {count} explicit edges")]
    MissingEdges { count: usize },
    #[error("partition contains {count} duplicate edge occurrences")]
    DuplicateEdges { count: usize },
    #[error(
        "Theorem 8 recursive subproblem did not decrease: dimensions={dimensions}, vertices={vertex_count}"
    )]
    NonDecreasingRecursion {
        dimensions: usize,
        vertex_count: usize,
    },
    #[error(
        "cross-side vertices ({left}, {right}) share coordinate {coordinate}, violating strict-order assumptions"
    )]
    CrossSideCoordinateEquality {
        left: usize,
        right: usize,
        coordinate: usize,
    },
}

#[cfg(test)]
mod tests {
    use rect_core::{
        BicliqueId, HorizontalChord, HorizontalChordId, VerticalChord, VerticalChordId,
    };

    use crate::embedding::DominanceEmbedding;

    use super::{Biclique, BicliqueError, BicliquePartition};

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
        let partition = BicliquePartition::comparability_theorem_8(&embedding).unwrap();
        partition.verify_exact_partition(&graph).unwrap();
        let certificate = partition.certificate(&graph);
        assert_eq!(
            certificate.explicit_edge_count,
            certificate.represented_edge_count
        );
        assert_eq!(certificate.duplicate_edge_count, 0);
        assert_eq!(certificate.missing_edge_count, 0);
        assert_eq!(certificate.fabricated_edge_count, 0);
    }

    #[test]
    fn audit_rejects_duplicate_vertex_ids_inside_a_block() {
        let mut graph = rect_graph::BipartiteGraph::new(1, 1);
        graph.add_edge(0, 0).unwrap();
        let partition = BicliquePartition {
            bicliques: vec![Biclique {
                id: BicliqueId(0),
                left: vec![0, 0],
                right: vec![0],
            }],
        };
        assert!(matches!(
            partition.verify_exact_partition(&graph),
            Err(BicliqueError::DuplicateLeftVertex { .. })
        ));
    }
}
