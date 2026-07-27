use std::collections::{BTreeSet, HashMap};

use rect_core::{BicliqueId, HorizontalChordId, VerticalChordId};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BicliqueConstructionBackend {
    RecursiveSortReference,
    Presorted,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliqueConstructionMetrics {
    pub initial_sort_count: usize,
    pub recursive_sort_count: usize,
    pub stable_partition_visits: usize,
    pub scratch_buffer_acquisitions: usize,
    pub scratch_growth_count: usize,
    pub scratch_point_capacity: usize,
    pub recursive_node_count: usize,
    pub emitted_vertex_occurrences: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliqueConstruction {
    pub backend: BicliqueConstructionBackend,
    pub partition: BicliquePartition,
    pub metrics: BicliqueConstructionMetrics,
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
    pub block_count: usize,
    pub total_block_size: usize,
    pub explicit_edge_count: usize,
    pub represented_edge_count: usize,
    pub duplicate_edge_count: usize,
    pub missing_edge_count: usize,
    pub fabricated_edge_count: usize,
    pub missing_edges: Vec<(HorizontalChordId, VerticalChordId)>,
    pub fabricated_edges: Vec<(HorizontalChordId, VerticalChordId)>,
    pub duplicate_edges: Vec<(HorizontalChordId, VerticalChordId)>,
    pub offending_edge_limit: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BicliquePartitionAudit {
    pub block_count: usize,
    pub total_block_size: usize,
    pub explicit_edge_count: usize,
    pub represented_edge_count: usize,
    pub duplicate_edge_count: usize,
    pub missing_edge_count: usize,
    pub fabricated_edge_count: usize,
    pub missing_edges: Vec<(HorizontalChordId, VerticalChordId)>,
    pub fabricated_edges: Vec<(HorizontalChordId, VerticalChordId)>,
    pub duplicate_edges: Vec<(HorizontalChordId, VerticalChordId)>,
    pub offending_edge_limit: usize,
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
        Ok(Self::comparability_theorem_8_with_backend(
            embedding,
            BicliqueConstructionBackend::RecursiveSortReference,
        )?
        .partition)
    }

    /// Constructs the Theorem 8 partition with an explicitly selected backend.
    ///
    /// The reference backend preserves the historical recursive re-sorting
    /// implementation. The presorted backend sorts each coordinate once and
    /// derives every recursive coordinate order by stable filtering.
    ///
    /// # Errors
    ///
    /// Returns [`BicliqueError`] when a cross-side coordinate equality or a
    /// non-decreasing recursive subproblem violates the source construction.
    pub fn comparability_theorem_8_with_backend(
        embedding: &DominanceEmbedding,
        backend: BicliqueConstructionBackend,
    ) -> Result<BicliqueConstruction, BicliqueError> {
        verify_coordinate_order_assumptions(embedding)?;
        let mut partition = Self::default();
        let mut metrics = BicliqueConstructionMetrics::default();
        match backend {
            BicliqueConstructionBackend::RecursiveSortReference => {
                let left = (0..embedding.horizontal.len()).collect::<Vec<_>>();
                let right = (0..embedding.vertical.len()).collect::<Vec<_>>();
                partition_recursive_reference(
                    embedding,
                    &left,
                    &right,
                    4,
                    &mut partition.bicliques,
                    &mut metrics,
                )?;
            }
            BicliqueConstructionBackend::Presorted => {
                let orders = initial_coordinate_orders(embedding, &mut metrics);
                let mut scratch = PresortedScratch::default();
                partition_recursive_presorted(
                    embedding,
                    orders,
                    4,
                    &mut partition.bicliques,
                    &mut scratch,
                    &mut metrics,
                )?;
                metrics.scratch_point_capacity = scratch.point_capacity();
            }
        }
        for (index, biclique) in partition.bicliques.iter_mut().enumerate() {
            biclique.id = BicliqueId(index);
        }
        Ok(BicliqueConstruction {
            backend,
            partition,
            metrics,
        })
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
        self.verify_structure(graph.left_size(), graph.right_size())?;
        for biclique in &self.bicliques {
            for &left in &biclique.left {
                for &right in &biclique.right {
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
        let audit = self.audit(graph, 64);
        if audit.fabricated_edge_count != 0 {
            return Err(BicliqueError::FabricatedEdges {
                count: audit.fabricated_edge_count,
            });
        }
        if audit.missing_edge_count != 0 {
            return Err(BicliqueError::MissingEdges {
                count: audit.missing_edge_count,
            });
        }
        if audit.duplicate_edge_count != 0 {
            return Err(BicliqueError::DuplicateEdges {
                count: audit.duplicate_edge_count,
            });
        }
        Ok(())
    }

    /// Verifies the compact representation without expanding block products.
    ///
    /// # Errors
    ///
    /// Returns [`BicliqueError`] for empty sides, duplicate vertex IDs, or
    /// endpoints outside the declared chord families.
    pub fn verify_structure(
        &self,
        horizontal_count: usize,
        vertical_count: usize,
    ) -> Result<(), BicliqueError> {
        let mut ids = BTreeSet::new();
        for biclique in &self.bicliques {
            if !ids.insert(biclique.id) {
                return Err(BicliqueError::DuplicateBicliqueId { id: biclique.id });
            }
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
                if left >= horizontal_count {
                    return Err(BicliqueError::LeftOutOfBounds {
                        id: biclique.id,
                        left,
                    });
                }
            }
            for &right in &biclique.right {
                if right >= vertical_count {
                    return Err(BicliqueError::RightOutOfBounds {
                        id: biclique.id,
                        right,
                    });
                }
            }
        }
        Ok(())
    }

    /// Verifies every compact block using coordinate extrema only.
    ///
    /// For each block and coordinate this computes the maximum left value and
    /// minimum right value. Strict separation proves every Cartesian-product
    /// pair is a valid dominance edge without enumerating that product.
    ///
    /// # Errors
    ///
    /// Returns [`BicliqueError`] when a block is structurally invalid or fails
    /// strict coordinate separation.
    pub fn verify_dominance_blocks(
        &self,
        embedding: &DominanceEmbedding,
    ) -> Result<(), BicliqueError> {
        self.verify_structure(embedding.horizontal.len(), embedding.vertical.len())?;
        for biclique in &self.bicliques {
            let mut max_left = [i128::MIN; 4];
            let mut min_right = [i128::MAX; 4];
            for &left in &biclique.left {
                for (coordinate, value) in embedding.horizontal[left].coordinates.iter().enumerate()
                {
                    max_left[coordinate] = max_left[coordinate].max(*value);
                }
            }
            for &right in &biclique.right {
                for (coordinate, value) in embedding.vertical[right].coordinates.iter().enumerate()
                {
                    min_right[coordinate] = min_right[coordinate].min(*value);
                }
            }
            for coordinate in 0..4 {
                if max_left[coordinate] >= min_right[coordinate] {
                    return Err(BicliqueError::CoordinateSeparationViolation {
                        id: biclique.id,
                        coordinate,
                        max_left: max_left[coordinate],
                        min_right: min_right[coordinate],
                    });
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn certificate(&self, graph: &BipartiteGraph) -> BicliquePartitionCertificate {
        let audit = self.audit(graph, 64);
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
            block_count: audit.block_count,
            total_block_size: audit.total_block_size,
            explicit_edge_count: audit.explicit_edge_count,
            represented_edge_count: audit.represented_edge_count,
            duplicate_edge_count: audit.duplicate_edge_count,
            missing_edge_count: audit.missing_edge_count,
            fabricated_edge_count: audit.fabricated_edge_count,
            missing_edges: audit.missing_edges,
            fabricated_edges: audit.fabricated_edges,
            duplicate_edges: audit.duplicate_edges,
            offending_edge_limit: audit.offending_edge_limit,
        }
    }

    /// Audits the represented edge multiset against an explicit graph.
    ///
    /// Counts remain exact even when the diagnostic edge vectors reach
    /// `offending_edge_limit`.
    #[must_use]
    pub fn audit(
        &self,
        graph: &BipartiteGraph,
        offending_edge_limit: usize,
    ) -> BicliquePartitionAudit {
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
        let duplicate_edge_count = multiplicities
            .values()
            .map(|&count| count.saturating_sub(1))
            .sum();
        let represented = multiplicities.keys().copied().collect::<BTreeSet<_>>();
        let missing_edge_count = explicit.difference(&represented).count();
        let fabricated_edge_count = multiplicities
            .iter()
            .filter(|(edge, _)| !explicit.contains(edge))
            .map(|(_, &count)| count)
            .sum();
        let missing_edges = explicit
            .difference(&represented)
            .take(offending_edge_limit)
            .map(|&(left, right)| (HorizontalChordId(left), VerticalChordId(right)))
            .collect();
        let fabricated_edges = multiplicities
            .keys()
            .filter(|edge| !explicit.contains(edge))
            .take(offending_edge_limit)
            .map(|&(left, right)| (HorizontalChordId(left), VerticalChordId(right)))
            .collect();
        let duplicate_edges = multiplicities
            .iter()
            .filter(|(_, count)| **count > 1)
            .take(offending_edge_limit)
            .map(|(&(left, right), _)| (HorizontalChordId(left), VerticalChordId(right)))
            .collect();
        BicliquePartitionAudit {
            block_count: self.bicliques.len(),
            total_block_size: self.total_vertex_occurrences(),
            explicit_edge_count: explicit.len(),
            represented_edge_count,
            duplicate_edge_count,
            missing_edge_count,
            fabricated_edge_count,
            missing_edges,
            fabricated_edges,
            duplicate_edges,
            offending_edge_limit,
        }
    }
}

#[derive(Clone, Copy)]
enum SidePoint {
    Left(usize),
    Right(usize),
}

fn partition_recursive_reference(
    embedding: &DominanceEmbedding,
    left: &[usize],
    right: &[usize],
    dimensions: usize,
    output: &mut Vec<Biclique>,
    metrics: &mut BicliqueConstructionMetrics,
) -> Result<(), BicliqueError> {
    metrics.recursive_node_count += 1;
    if left.is_empty() || right.is_empty() {
        return Ok(());
    }
    if dimensions == 0 {
        metrics.emitted_vertex_occurrences += left.len() + right.len();
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
    metrics.recursive_sort_count += 1;
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
    partition_recursive_reference(
        embedding,
        &low_left,
        &high_right,
        dimensions - 1,
        output,
        metrics,
    )?;
    verify_recursive_reduction(
        dimensions,
        points.len(),
        dimensions,
        low_left.len() + low_right.len(),
    )?;
    partition_recursive_reference(
        embedding, &low_left, &low_right, dimensions, output, metrics,
    )?;
    verify_recursive_reduction(
        dimensions,
        points.len(),
        dimensions,
        high_left.len() + high_right.len(),
    )?;
    partition_recursive_reference(
        embedding,
        &high_left,
        &high_right,
        dimensions,
        output,
        metrics,
    )?;
    Ok(())
}

type CoordinateOrders = [Vec<SidePoint>; 4];

#[derive(Default)]
struct PresortedScratch {
    available: Vec<CoordinateOrders>,
}

impl PresortedScratch {
    fn acquire(
        &mut self,
        capacity: usize,
        metrics: &mut BicliqueConstructionMetrics,
    ) -> CoordinateOrders {
        metrics.scratch_buffer_acquisitions += 1;
        let mut orders = self.available.pop().unwrap_or_default();
        for order in &mut orders {
            order.clear();
            if order.capacity() < capacity {
                order.reserve(capacity);
                metrics.scratch_growth_count += 1;
            }
        }
        orders
    }

    fn release(&mut self, mut orders: CoordinateOrders) {
        for order in &mut orders {
            order.clear();
        }
        self.available.push(orders);
    }

    fn point_capacity(&self) -> usize {
        self.available
            .iter()
            .flat_map(|orders| orders.iter())
            .map(Vec::capacity)
            .sum()
    }
}

#[derive(Clone, Copy)]
enum PresortedChild {
    Cross,
    Low,
    High,
}

fn initial_coordinate_orders(
    embedding: &DominanceEmbedding,
    metrics: &mut BicliqueConstructionMetrics,
) -> CoordinateOrders {
    std::array::from_fn(|coordinate| {
        let mut order = (0..embedding.horizontal.len())
            .map(SidePoint::Left)
            .chain((0..embedding.vertical.len()).map(SidePoint::Right))
            .collect::<Vec<_>>();
        order.sort_by_key(|&point| point_key(embedding, point, coordinate));
        metrics.initial_sort_count += 1;
        order
    })
}

fn partition_recursive_presorted(
    embedding: &DominanceEmbedding,
    orders: CoordinateOrders,
    dimensions: usize,
    output: &mut Vec<Biclique>,
    scratch: &mut PresortedScratch,
    metrics: &mut BicliqueConstructionMetrics,
) -> Result<(), BicliqueError> {
    metrics.recursive_node_count += 1;
    let vertex_count = orders[0].len();
    let left_count = orders[0]
        .iter()
        .filter(|point| matches!(point, SidePoint::Left(_)))
        .count();
    let right_count = vertex_count - left_count;
    if left_count == 0 || right_count == 0 {
        scratch.release(orders);
        return Ok(());
    }
    if dimensions == 0 {
        let (left, right) = split_sides(&orders[0]);
        metrics.emitted_vertex_occurrences += vertex_count;
        output.push(Biclique {
            id: BicliqueId(output.len()),
            left,
            right,
        });
        scratch.release(orders);
        return Ok(());
    }

    let coordinate = dimensions - 1;
    let split = vertex_count / 2;
    if split == 0 || split == vertex_count {
        scratch.release(orders);
        return Err(BicliqueError::NonDecreasingRecursion {
            dimensions,
            vertex_count,
        });
    }
    let pivot_key = point_key(embedding, orders[coordinate][split - 1], coordinate);

    for (child, child_dimensions) in [
        (PresortedChild::Cross, dimensions - 1),
        (PresortedChild::Low, dimensions),
        (PresortedChild::High, dimensions),
    ] {
        let mut child_orders = scratch.acquire(vertex_count, metrics);
        for order_index in 0..4 {
            for &point in &orders[order_index] {
                metrics.stable_partition_visits += 1;
                let is_low = point_key(embedding, point, coordinate) <= pivot_key;
                let include = match (child, point) {
                    (PresortedChild::Cross, SidePoint::Left(_)) | (PresortedChild::Low, _) => {
                        is_low
                    }
                    (PresortedChild::Cross, SidePoint::Right(_)) | (PresortedChild::High, _) => {
                        !is_low
                    }
                };
                if include {
                    child_orders[order_index].push(point);
                }
            }
        }
        verify_recursive_reduction(
            dimensions,
            vertex_count,
            child_dimensions,
            child_orders[0].len(),
        )?;
        partition_recursive_presorted(
            embedding,
            child_orders,
            child_dimensions,
            output,
            scratch,
            metrics,
        )?;
    }
    scratch.release(orders);
    Ok(())
}

fn point_key(
    embedding: &DominanceEmbedding,
    point: SidePoint,
    coordinate: usize,
) -> (i128, u8, usize) {
    match point {
        SidePoint::Left(index) => (
            embedding.horizontal[index].coordinates[coordinate],
            0,
            index,
        ),
        SidePoint::Right(index) => (embedding.vertical[index].coordinates[coordinate], 1, index),
    }
}

fn verify_coordinate_order_assumptions(
    embedding: &DominanceEmbedding,
) -> Result<(), BicliqueError> {
    for coordinate in 0..4 {
        let horizontal_by_value = embedding
            .horizontal
            .iter()
            .enumerate()
            .map(|(left, point)| (point.coordinates[coordinate], left))
            .collect::<HashMap<_, _>>();
        for (right, vertical) in embedding.vertical.iter().enumerate() {
            if let Some(&left) = horizontal_by_value.get(&vertical.coordinates[coordinate]) {
                return Err(BicliqueError::CrossSideCoordinateEquality {
                    left,
                    right,
                    coordinate,
                });
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
    #[error("biclique ID {id:?} is repeated")]
    DuplicateBicliqueId { id: BicliqueId },
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
    #[error(
        "biclique {id:?} violates strict dominance at coordinate {coordinate}: max left {max_left} >= min right {min_right}"
    )]
    CoordinateSeparationViolation {
        id: BicliqueId,
        coordinate: usize,
        max_left: i128,
        min_right: i128,
    },
}

#[cfg(test)]
mod tests {
    use rect_core::{
        BicliqueId, HorizontalChord, HorizontalChordId, VerticalChord, VerticalChordId,
    };

    use crate::embedding::{DominanceEmbedding, DominancePoint};

    use super::{Biclique, BicliqueConstructionBackend, BicliqueError, BicliquePartition};

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
    fn presorted_backend_matches_reference_exactly() {
        for seed in 0_u64..256 {
            let seed_index = usize::try_from(seed).unwrap();
            let horizontal_count = seed_index % 9;
            let vertical_count = (seed_index / 3) % 9;
            let embedding = synthetic_embedding(seed, horizontal_count, vertical_count);
            let reference = BicliquePartition::comparability_theorem_8_with_backend(
                &embedding,
                BicliqueConstructionBackend::RecursiveSortReference,
            )
            .unwrap();
            let presorted = BicliquePartition::comparability_theorem_8_with_backend(
                &embedding,
                BicliqueConstructionBackend::Presorted,
            )
            .unwrap();

            assert_eq!(
                presorted.partition, reference.partition,
                "partition mismatch for seed {seed}"
            );
            assert_eq!(presorted.metrics.initial_sort_count, 4);
            assert_eq!(presorted.metrics.recursive_sort_count, 0);
            assert_eq!(
                presorted.metrics.emitted_vertex_occurrences,
                presorted.partition.total_vertex_occurrences()
            );
            assert_eq!(reference.metrics.initial_sort_count, 0);
            assert_eq!(
                reference.metrics.emitted_vertex_occurrences,
                reference.partition.total_vertex_occurrences()
            );
            if horizontal_count != 0 && vertical_count != 0 {
                assert!(reference.metrics.recursive_sort_count > 0);
                assert!(presorted.metrics.stable_partition_visits > 0);
                assert!(presorted.metrics.scratch_buffer_acquisitions > 0);
                assert!(presorted.metrics.scratch_growth_count > 0);
                assert!(presorted.metrics.scratch_point_capacity > 0);
            }
        }
    }

    fn synthetic_embedding(
        seed: u64,
        horizontal_count: usize,
        vertical_count: usize,
    ) -> DominanceEmbedding {
        let horizontal = (0..horizontal_count)
            .map(|index| DominancePoint {
                coordinates: std::array::from_fn(|coordinate| {
                    synthetic_coordinate(seed, coordinate, index, false)
                }),
            })
            .collect();
        let vertical = (0..vertical_count)
            .map(|index| DominancePoint {
                coordinates: std::array::from_fn(|coordinate| {
                    synthetic_coordinate(seed, coordinate, index, true)
                }),
            })
            .collect();
        DominanceEmbedding {
            horizontal,
            vertical,
        }
    }

    fn synthetic_coordinate(seed: u64, coordinate: usize, index: usize, right: bool) -> i128 {
        let mut value = seed
            ^ ((coordinate as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15))
            ^ ((index as u64 + 1).wrapping_mul(0xbf58_476d_1ce4_e5b9));
        value ^= value >> 30;
        value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value ^= value >> 27;
        value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^= value >> 31;
        let bucket = i128::from(u8::try_from(value % 13).unwrap()) - 6;
        bucket * 2 + i128::from(right)
    }

    #[test]
    fn dominance_block_validation_uses_coordinate_extrema() {
        let horizontal = [HorizontalChord::new(HorizontalChordId(0), 0, 2, 0).unwrap()];
        let vertical = [VerticalChord::new(VerticalChordId(0), 1, -1, 1).unwrap()];
        let embedding = DominanceEmbedding::new(&horizontal, &vertical).unwrap();
        let partition = BicliquePartition {
            bicliques: vec![Biclique {
                id: BicliqueId(0),
                left: vec![0],
                right: vec![0],
            }],
        };
        partition.verify_dominance_blocks(&embedding).unwrap();
    }

    #[test]
    fn dominance_block_validation_rejects_bad_coordinate() {
        let horizontal = [HorizontalChord::new(HorizontalChordId(0), 0, 2, 0).unwrap()];
        let vertical = [VerticalChord::new(VerticalChordId(0), 1, -1, 0).unwrap()];
        let mut embedding = DominanceEmbedding::new(&horizontal, &vertical).unwrap();
        embedding.vertical[0].coordinates[0] = embedding.horizontal[0].coordinates[0];
        let partition = BicliquePartition {
            bicliques: vec![Biclique {
                id: BicliqueId(0),
                left: vec![0],
                right: vec![0],
            }],
        };
        assert!(matches!(
            partition.verify_dominance_blocks(&embedding),
            Err(BicliqueError::CoordinateSeparationViolation { .. })
        ));
    }

    #[test]
    fn compact_structure_rejects_duplicate_ids_and_bounds() {
        let partition = BicliquePartition {
            bicliques: vec![
                Biclique {
                    id: BicliqueId(0),
                    left: vec![0],
                    right: vec![0],
                },
                Biclique {
                    id: BicliqueId(0),
                    left: vec![0],
                    right: vec![0],
                },
            ],
        };
        assert!(matches!(
            partition.verify_structure(1, 1),
            Err(BicliqueError::DuplicateBicliqueId { .. })
        ));
        let out_of_bounds = BicliquePartition {
            bicliques: vec![Biclique {
                id: BicliqueId(0),
                left: vec![1],
                right: vec![0],
            }],
        };
        assert!(matches!(
            out_of_bounds.verify_structure(1, 1),
            Err(BicliqueError::LeftOutOfBounds { .. })
        ));
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

    #[test]
    fn audit_counts_all_discrepancies_while_bounding_examples() {
        let mut graph = rect_graph::BipartiteGraph::new(2, 2);
        graph.add_edge(0, 0).unwrap();
        graph.add_edge(1, 1).unwrap();
        let partition = BicliquePartition {
            bicliques: vec![
                Biclique {
                    id: BicliqueId(0),
                    left: vec![0],
                    right: vec![0],
                },
                Biclique {
                    id: BicliqueId(1),
                    left: vec![0],
                    right: vec![0, 1],
                },
            ],
        };
        let audit = partition.audit(&graph, 1);
        assert_eq!(audit.block_count, 2);
        assert_eq!(audit.total_block_size, 5);
        assert_eq!(audit.explicit_edge_count, 2);
        assert_eq!(audit.represented_edge_count, 3);
        assert_eq!(audit.missing_edge_count, 1);
        assert_eq!(audit.fabricated_edge_count, 1);
        assert_eq!(audit.duplicate_edge_count, 1);
        assert_eq!(audit.missing_edges.len(), 1);
        assert_eq!(audit.fabricated_edges.len(), 1);
        assert_eq!(audit.duplicate_edges.len(), 1);
    }
}
