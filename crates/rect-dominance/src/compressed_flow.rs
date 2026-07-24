use rect_graph::{FlowError, FlowNetwork, FlowNodeId, FlowResult, MaxFlowBackend, VertexCover};
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

/// Runs an exact flow on the biclique-compressed network and recovers a cover.
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
}

#[cfg(test)]
mod tests {
    use rect_graph::{BipartiteGraph, DinicBackend, hopcroft_karp};

    use crate::biclique::BicliquePartition;

    use super::solve_biclique_flow;

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
    }
}
