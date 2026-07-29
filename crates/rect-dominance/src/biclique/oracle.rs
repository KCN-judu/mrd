use rect_core::BicliqueId;

use super::{
    Backend, Block, Construction, Error, Metrics, Partition, verify_coordinate_order_assumptions,
    verify_recursive_reduction,
};
use crate::embedding::DominanceEmbedding;

/// Constructs the definition-level recursive-sort Theorem 8 partition.
///
/// # Errors
///
/// Returns an error when the source construction assumptions fail.
pub fn construct(embedding: &DominanceEmbedding) -> Result<Construction, Error> {
    verify_coordinate_order_assumptions(embedding)?;
    let mut partition = Partition::default();
    let mut metrics = Metrics::default();
    let left = (0..embedding.horizontal.len()).collect::<Vec<_>>();
    let right = (0..embedding.vertical.len()).collect::<Vec<_>>();
    partition_recursive(
        embedding,
        &left,
        &right,
        4,
        &mut partition.blocks,
        &mut metrics,
    )?;
    Ok(Construction {
        backend: Backend::Oracle,
        partition,
        metrics,
    })
}

fn partition_recursive(
    embedding: &DominanceEmbedding,
    left: &[usize],
    right: &[usize],
    dimensions: usize,
    output: &mut Vec<Block>,
    metrics: &mut Metrics,
) -> Result<(), Error> {
    metrics.recursive_node_count += 1;
    if left.is_empty() || right.is_empty() {
        return Ok(());
    }
    if dimensions == 0 {
        metrics.emitted_vertex_occurrences += left.len() + right.len();
        output.push(Block {
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
        .map(Side::Left)
        .chain(right.iter().copied().map(Side::Right))
        .collect::<Vec<_>>();
    metrics.recursive_sort_count += 1;
    points.sort_by_key(|point| match *point {
        Side::Left(index) => (
            embedding.horizontal[index].coordinates[coordinate],
            0_u8,
            index,
        ),
        Side::Right(index) => (
            embedding.vertical[index].coordinates[coordinate],
            1_u8,
            index,
        ),
    });
    let split = points.len() / 2;
    if split == 0 || split == points.len() {
        return Err(Error::NonDecreasingRecursion {
            dimensions,
            vertex_count: points.len(),
        });
    }
    let (low_points, high_points) = points.split_at(split);
    let (low_left, low_right) = split_sides(low_points);
    let (high_left, high_right) = split_sides(high_points);

    for (child_left, child_right, child_dimensions) in [
        (&low_left, &high_right, dimensions - 1),
        (&low_left, &low_right, dimensions),
        (&high_left, &high_right, dimensions),
    ] {
        verify_recursive_reduction(
            dimensions,
            points.len(),
            child_dimensions,
            child_left.len() + child_right.len(),
        )?;
        partition_recursive(
            embedding,
            child_left,
            child_right,
            child_dimensions,
            output,
            metrics,
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Side {
    Left(usize),
    Right(usize),
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
