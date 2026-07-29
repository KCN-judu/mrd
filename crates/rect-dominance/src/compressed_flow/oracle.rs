use rect_graph::{BipartiteGraph, DinicBackend, PushRelabelBackend, hopcroft_karp};

use super::{Error, Parity};
use crate::biclique::Partition;

/// Verifies compressed-flow recovery against independent exact references.
///
/// # Errors
///
/// Returns an error when the partition is not exact, either backend fails, or
/// a matching/flow/cover cardinality disagrees.
pub fn audit(graph: &BipartiteGraph, partition: &Partition) -> Result<Parity, Error> {
    partition
        .verify_exact_partition(graph)
        .map_err(|_| Error::InvalidPartition)?;
    let matching_size = hopcroft_karp(graph).size;
    let dinic = super::experiment::solve(
        graph.left_size(),
        graph.right_size(),
        partition,
        &DinicBackend,
    )?;
    let push_relabel = super::experiment::solve(
        graph.left_size(),
        graph.right_size(),
        partition,
        &PushRelabelBackend,
    )?;
    let dinic_value = usize::try_from(dinic.flow.value).map_err(|_| Error::FlowValueConversion)?;
    let push_value =
        usize::try_from(push_relabel.flow.value).map_err(|_| Error::FlowValueConversion)?;
    if dinic_value != matching_size
        || push_value != matching_size
        || dinic.vertex_cover.size != matching_size
        || push_relabel.vertex_cover.size != matching_size
    {
        return Err(Error::ParityMismatch);
    }
    Ok(Parity {
        matching_size,
        dinic,
        push_relabel,
    })
}
