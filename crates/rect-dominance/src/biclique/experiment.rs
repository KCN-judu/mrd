use rect_core::BicliqueId;

use super::{
    Backend, Block, Construction, Error, Metrics, Partition, verify_coordinate_order_assumptions,
    verify_recursive_reduction,
};
use crate::embedding::DominanceEmbedding;

/// Constructs the presorted Theorem 8 partition used by experiments.
///
/// # Errors
///
/// Returns an error when the source construction assumptions fail.
pub fn construct(embedding: &DominanceEmbedding) -> Result<Construction, Error> {
    verify_coordinate_order_assumptions(embedding)?;
    let mut partition = Partition::default();
    let mut metrics = Metrics::default();
    let orders = initial_coordinate_orders(embedding, &mut metrics);
    let mut scratch = Scratch::default();
    partition_recursive(
        embedding,
        orders,
        4,
        &mut partition.blocks,
        &mut scratch,
        &mut metrics,
    )?;
    metrics.scratch_point_capacity = scratch.point_capacity();
    Ok(Construction {
        backend: Backend::Experiment,
        partition,
        metrics,
    })
}

#[derive(Clone, Copy)]
enum Side {
    Left(usize),
    Right(usize),
}

type CoordinateOrders = [Vec<Side>; 4];

#[derive(Default)]
struct Scratch {
    available: Vec<CoordinateOrders>,
}

impl Scratch {
    fn acquire(&mut self, capacity: usize, metrics: &mut Metrics) -> CoordinateOrders {
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
enum Child {
    Cross,
    Low,
    High,
}

fn initial_coordinate_orders(
    embedding: &DominanceEmbedding,
    metrics: &mut Metrics,
) -> CoordinateOrders {
    std::array::from_fn(|coordinate| {
        let mut order = (0..embedding.horizontal.len())
            .map(Side::Left)
            .chain((0..embedding.vertical.len()).map(Side::Right))
            .collect::<Vec<_>>();
        order.sort_by_key(|&point| point_key(embedding, point, coordinate));
        metrics.initial_sort_count += 1;
        order
    })
}

fn partition_recursive(
    embedding: &DominanceEmbedding,
    orders: CoordinateOrders,
    dimensions: usize,
    output: &mut Vec<Block>,
    scratch: &mut Scratch,
    metrics: &mut Metrics,
) -> Result<(), Error> {
    metrics.recursive_node_count += 1;
    let vertex_count = orders[0].len();
    let left_count = orders[0]
        .iter()
        .filter(|point| matches!(point, Side::Left(_)))
        .count();
    if left_count == 0 || left_count == vertex_count {
        scratch.release(orders);
        return Ok(());
    }
    if dimensions == 0 {
        let (left, right) = split_sides(&orders[0]);
        metrics.emitted_vertex_occurrences += vertex_count;
        output.push(Block {
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
        return Err(Error::NonDecreasingRecursion {
            dimensions,
            vertex_count,
        });
    }
    let pivot_key = point_key(embedding, orders[coordinate][split - 1], coordinate);
    for (child, child_dimensions) in [
        (Child::Cross, dimensions - 1),
        (Child::Low, dimensions),
        (Child::High, dimensions),
    ] {
        let mut child_orders = scratch.acquire(vertex_count, metrics);
        for order_index in 0..4 {
            for &point in &orders[order_index] {
                metrics.stable_partition_visits += 1;
                let is_low = point_key(embedding, point, coordinate) <= pivot_key;
                let include = match (child, point) {
                    (Child::Cross, Side::Left(_)) | (Child::Low, _) => is_low,
                    (Child::Cross, Side::Right(_)) | (Child::High, _) => !is_low,
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
        partition_recursive(
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

fn point_key(embedding: &DominanceEmbedding, point: Side, coordinate: usize) -> (i128, u8, usize) {
    match point {
        Side::Left(index) => (
            embedding.horizontal[index].coordinates[coordinate],
            0,
            index,
        ),
        Side::Right(index) => (embedding.vertical[index].coordinates[coordinate], 1, index),
    }
}

fn split_sides(points: &[Side]) -> (Vec<usize>, Vec<usize>) {
    points
        .iter()
        .fold((Vec::new(), Vec::new()), |mut sides, point| {
            match *point {
                Side::Left(index) => sides.0.push(index),
                Side::Right(index) => sides.1.push(index),
            }
            sides
        })
}
