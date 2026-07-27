use rect_graph::{
    BipartiteGraph, DinicBackend, FlowError, FlowNetwork, FlowNodeId, FlowResult, MaxFlowBackend,
    PushRelabelBackend, VertexCover, hopcroft_karp,
};
use thiserror::Error;

use crate::biclique::BicliquePartition;

#[derive(Clone, Debug)]
pub struct CompressedFlowSolution {
    pub flow: FlowResult,
    pub vertex_cover: VertexCover,
    pub network_vertex_count: usize,
    pub network_arc_count: usize,
    pub internal_capacity: u64,
    pub internal_cut_arc_count: usize,
}

/// Exact agreement evidence for the explicit graph and both permanent flow
/// reference backends over a verified biclique partition.
#[derive(Clone, Debug)]
pub struct CompressedFlowParity {
    pub matching_size: usize,
    pub dinic: CompressedFlowSolution,
    pub push_relabel: CompressedFlowSolution,
}

/// Verifies compressed-flow recovery against independent exact references.
///
/// # Errors
///
/// Returns an error when the partition is not exact, either backend fails, or
/// a matching/flow/cover cardinality disagrees.
pub fn audit_biclique_flow_parity(
    graph: &BipartiteGraph,
    partition: &BicliquePartition,
) -> Result<CompressedFlowParity, CompressedFlowError> {
    partition
        .verify_exact_partition(graph)
        .map_err(|_| CompressedFlowError::InvalidPartition)?;
    let matching_size = hopcroft_karp(graph).size;
    let dinic = solve_biclique_flow(
        graph.left_size(),
        graph.right_size(),
        partition,
        &DinicBackend,
    )?;
    let push_relabel = solve_biclique_flow(
        graph.left_size(),
        graph.right_size(),
        partition,
        &PushRelabelBackend,
    )?;
    let dinic_value =
        usize::try_from(dinic.flow.value).map_err(|_| CompressedFlowError::FlowValueConversion)?;
    let push_value = usize::try_from(push_relabel.flow.value)
        .map_err(|_| CompressedFlowError::FlowValueConversion)?;
    if dinic_value != matching_size
        || push_value != matching_size
        || dinic.vertex_cover.size != matching_size
        || push_relabel.vertex_cover.size != matching_size
    {
        return Err(CompressedFlowError::ParityMismatch);
    }
    Ok(CompressedFlowParity {
        matching_size,
        dinic,
        push_relabel,
    })
}

/// Runs an exact flow on the biclique-compressed network and recovers a cover.
///
/// Outer arcs have unit capacity. Internal arcs use
/// `U = min(horizontal_count, vertical_count) + 1`; `U - 1` bounds every
/// possible matching value, so a minimum cut used for certificate recovery
/// cannot prefer an internal arc over an outer unit-arc cover. The large value
/// is a cut-certificate device, not a claim that one matching flow needs to
/// send multiple units through a particular internal arc.
///
/// # Errors
///
/// Returns [`CompressedFlowError`] for invalid dimensions/capacities, backend
/// failure, an internal cut arc, or a cut/cover cardinality mismatch.
pub fn solve_biclique_flow(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &BicliquePartition,
    backend: &impl MaxFlowBackend,
) -> Result<CompressedFlowSolution, CompressedFlowError> {
    let layout = build_network(horizontal_count, vertical_count, partition)?;
    let flow = backend.max_flow_min_cut(&layout.network, layout.source, layout.sink)?;

    let mut internal_cut_arc_count = 0;
    for (biclique_index, biclique) in partition.bicliques.iter().enumerate() {
        let biclique_node = layout.biclique_nodes[biclique_index];
        for &left in &biclique.left {
            if flow.source_side[layout.horizontal_nodes[left].0]
                && !flow.source_side[biclique_node.0]
            {
                internal_cut_arc_count += 1;
            }
        }
        for &right in &biclique.right {
            if flow.source_side[biclique_node.0]
                && !flow.source_side[layout.vertical_nodes[right].0]
            {
                internal_cut_arc_count += 1;
            }
        }
    }
    if internal_cut_arc_count != 0 {
        return Err(CompressedFlowError::InternalArcInMinimumCut {
            count: internal_cut_arc_count,
        });
    }

    let left = layout
        .horizontal_nodes
        .iter()
        .map(|node| !flow.source_side[node.0])
        .collect::<Vec<_>>();
    let right = layout
        .vertical_nodes
        .iter()
        .map(|node| flow.source_side[node.0])
        .collect::<Vec<_>>();
    let size = left.iter().filter(|&&selected| selected).count()
        + right.iter().filter(|&&selected| selected).count();
    let flow_value =
        usize::try_from(flow.value).map_err(|_| CompressedFlowError::FlowValueConversion)?;
    if size != flow_value {
        return Err(CompressedFlowError::CutCoverSizeMismatch {
            cut: flow_value,
            cover: size,
        });
    }
    Ok(CompressedFlowSolution {
        network_vertex_count: layout.network.node_count(),
        network_arc_count: layout.network.arc_count(),
        internal_capacity: layout.internal_capacity,
        internal_cut_arc_count,
        flow,
        vertex_cover: VertexCover { left, right, size },
    })
}

struct NetworkLayout {
    network: FlowNetwork,
    source: FlowNodeId,
    sink: FlowNodeId,
    horizontal_nodes: Vec<FlowNodeId>,
    biclique_nodes: Vec<FlowNodeId>,
    vertical_nodes: Vec<FlowNodeId>,
    internal_capacity: u64,
}

fn build_network(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &BicliquePartition,
) -> Result<NetworkLayout, CompressedFlowError> {
    let node_count = 2_usize
        .checked_add(horizontal_count)
        .and_then(|value| value.checked_add(partition.bicliques.len()))
        .and_then(|value| value.checked_add(vertical_count))
        .ok_or(CompressedFlowError::NetworkSizeOverflow)?;
    let source = FlowNodeId(0);
    let horizontal_start = 1;
    let biclique_start = horizontal_start + horizontal_count;
    let vertical_start = biclique_start + partition.bicliques.len();
    let sink = FlowNodeId(node_count - 1);
    let horizontal_nodes = (0..horizontal_count)
        .map(|index| FlowNodeId(horizontal_start + index))
        .collect::<Vec<_>>();
    let biclique_nodes = (0..partition.bicliques.len())
        .map(|index| FlowNodeId(biclique_start + index))
        .collect::<Vec<_>>();
    let vertical_nodes = (0..vertical_count)
        .map(|index| FlowNodeId(vertical_start + index))
        .collect::<Vec<_>>();
    let internal_capacity = horizontal_count
        .min(vertical_count)
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(CompressedFlowError::CapacityOverflow)?;
    let mut network = FlowNetwork::new(node_count);
    for &node in &horizontal_nodes {
        network.add_arc(source, node, 1)?;
    }
    for (index, biclique) in partition.bicliques.iter().enumerate() {
        let biclique_node = biclique_nodes[index];
        for &left in &biclique.left {
            let node = *horizontal_nodes
                .get(left)
                .ok_or(CompressedFlowError::BicliqueEndpointOutOfBounds)?;
            network.add_arc(node, biclique_node, internal_capacity)?;
        }
        for &right in &biclique.right {
            let node = *vertical_nodes
                .get(right)
                .ok_or(CompressedFlowError::BicliqueEndpointOutOfBounds)?;
            network.add_arc(biclique_node, node, internal_capacity)?;
        }
    }
    for &node in &vertical_nodes {
        network.add_arc(node, sink, 1)?;
    }
    Ok(NetworkLayout {
        network,
        source,
        sink,
        horizontal_nodes,
        biclique_nodes,
        vertical_nodes,
        internal_capacity,
    })
}

#[derive(Debug, Error)]
pub enum CompressedFlowError {
    #[error(transparent)]
    Flow(#[from] FlowError),
    #[error("compressed network node count overflowed usize")]
    NetworkSizeOverflow,
    #[error("internal capacity cannot be represented as u64")]
    CapacityOverflow,
    #[error("biclique contains an endpoint outside its declared side")]
    BicliqueEndpointOutOfBounds,
    #[error("{count} large-capacity internal arcs unexpectedly belong to a minimum cut")]
    InternalArcInMinimumCut { count: usize },
    #[error("flow value cannot be represented as usize")]
    FlowValueConversion,
    #[error("minimum cut value {cut} differs from recovered cover size {cover}")]
    CutCoverSizeMismatch { cut: usize, cover: usize },
    #[error("biclique partition does not exactly represent the explicit graph")]
    InvalidPartition,
    #[error("explicit matching and compressed flow reference backends disagree")]
    ParityMismatch,
}

#[cfg(test)]
mod tests {
    use rect_graph::{BipartiteGraph, DinicBackend, PushRelabelBackend, hopcroft_karp};

    use crate::biclique::BicliquePartition;

    use super::{audit_biclique_flow_parity, solve_biclique_flow};

    #[test]
    fn c0_flow_equals_explicit_matching() {
        let mut graph = BipartiteGraph::new(3, 3);
        for (left, right) in [(0, 0), (0, 1), (1, 1), (2, 1), (2, 2)] {
            graph.add_edge(left, right).unwrap();
        }
        let partition = BicliquePartition::from_explicit_edges(&graph);
        let flow = solve_biclique_flow(3, 3, &partition, &DinicBackend).unwrap();
        let value = usize::try_from(flow.flow.value).unwrap();
        assert_eq!(value, hopcroft_karp(&graph).size);
        assert_eq!(flow.vertex_cover.size, value);
        assert_eq!(flow.internal_cut_arc_count, 0);
        let maximum_matching_bound = flow.internal_capacity.checked_sub(1).unwrap();
        assert!(maximum_matching_bound >= u64::try_from(value).unwrap());
        for (left, right) in graph.edges() {
            assert!(flow.vertex_cover.left[left] || flow.vertex_cover.right[right]);
            let selected_left = !flow.vertex_cover.left[left];
            let selected_right = !flow.vertex_cover.right[right];
            assert!(!(selected_left && selected_right));
        }
    }

    #[test]
    fn push_relabel_matches_dinic_certificate() {
        let mut graph = BipartiteGraph::new(4, 4);
        for (left, right) in [(0, 0), (0, 2), (1, 1), (1, 2), (2, 1), (2, 3), (3, 0)] {
            graph.add_edge(left, right).unwrap();
        }
        let partition = BicliquePartition::from_explicit_edges(&graph);
        let dinic = solve_biclique_flow(4, 4, &partition, &DinicBackend).unwrap();
        let push_relabel = solve_biclique_flow(4, 4, &partition, &PushRelabelBackend).unwrap();
        assert_eq!(push_relabel.flow.value, dinic.flow.value);
        assert_eq!(push_relabel.vertex_cover.size, dinic.vertex_cover.size);
        assert_eq!(push_relabel.internal_cut_arc_count, 0);
    }

    #[test]
    fn parity_audit_agrees_for_every_two_by_two_explicit_graph() {
        for mask in 0_u8..16 {
            let mut graph = BipartiteGraph::new(2, 2);
            for (index, (left, right)) in [(0, 0), (0, 1), (1, 0), (1, 1)].iter().enumerate() {
                if mask & (1 << index) != 0 {
                    graph.add_edge(*left, *right).unwrap();
                }
            }
            let partition = BicliquePartition::from_explicit_edges(&graph);
            let parity = audit_biclique_flow_parity(&graph, &partition).unwrap();
            assert_eq!(parity.matching_size, hopcroft_karp(&graph).size);
        }
    }
}
