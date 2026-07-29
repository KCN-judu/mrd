use graph::{FlowNetwork, FlowNodeId, MaxFlowBackend, VertexCover};

use super::{Error, Solution};
use crate::biclique::Partition;

pub mod source;

/// Runs an exact flow on the biclique-compressed network and recovers a cover.
///
/// Outer arcs have unit capacity. Internal arcs use
/// `U = min(horizontal_count, vertical_count) + 1`; `U - 1` bounds every
/// possible matching value, so a minimum cut used for certificate recovery
/// cannot prefer an internal arc over an outer unit-arc cover.
///
/// # Errors
///
/// Returns an error for invalid dimensions/capacities, backend failure, an
/// internal cut arc, or a cut/cover cardinality mismatch.
pub fn solve(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &Partition,
    backend: &impl MaxFlowBackend,
) -> Result<Solution, Error> {
    let layout = build_network(horizontal_count, vertical_count, partition)?;
    let flow = backend.max_flow_min_cut(&layout.network, layout.source, layout.sink)?;

    let mut internal_cut_arc_count = 0;
    for (block_index, block) in partition.blocks.iter().enumerate() {
        let block_node = layout.block_nodes[block_index];
        for &left in &block.left {
            if flow.source_side[layout.horizontal_nodes[left].0] && !flow.source_side[block_node.0]
            {
                internal_cut_arc_count += 1;
            }
        }
        for &right in &block.right {
            if flow.source_side[block_node.0] && !flow.source_side[layout.vertical_nodes[right].0] {
                internal_cut_arc_count += 1;
            }
        }
    }
    if internal_cut_arc_count != 0 {
        return Err(Error::InternalArcInMinimumCut {
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
    let flow_value = usize::try_from(flow.value).map_err(|_| Error::FlowValueConversion)?;
    if size != flow_value {
        return Err(Error::CutCoverSizeMismatch {
            cut: flow_value,
            cover: size,
        });
    }
    Ok(Solution {
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
    block_nodes: Vec<FlowNodeId>,
    vertical_nodes: Vec<FlowNodeId>,
    internal_capacity: u64,
}

fn build_network(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &Partition,
) -> Result<NetworkLayout, Error> {
    let node_count = 2_usize
        .checked_add(horizontal_count)
        .and_then(|value| value.checked_add(partition.blocks.len()))
        .and_then(|value| value.checked_add(vertical_count))
        .ok_or(Error::NetworkSizeOverflow)?;
    let source = FlowNodeId(0);
    let horizontal_start = 1;
    let block_start = horizontal_start + horizontal_count;
    let vertical_start = block_start + partition.blocks.len();
    let sink = FlowNodeId(node_count - 1);
    let horizontal_nodes = (0..horizontal_count)
        .map(|index| FlowNodeId(horizontal_start + index))
        .collect::<Vec<_>>();
    let block_nodes = (0..partition.blocks.len())
        .map(|index| FlowNodeId(block_start + index))
        .collect::<Vec<_>>();
    let vertical_nodes = (0..vertical_count)
        .map(|index| FlowNodeId(vertical_start + index))
        .collect::<Vec<_>>();
    let internal_capacity = horizontal_count
        .min(vertical_count)
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(Error::CapacityOverflow)?;
    let mut network = FlowNetwork::new(node_count);
    for &node in &horizontal_nodes {
        network.add_arc(source, node, 1)?;
    }
    for (index, block) in partition.blocks.iter().enumerate() {
        let block_node = block_nodes[index];
        for &left in &block.left {
            let node = *horizontal_nodes
                .get(left)
                .ok_or(Error::BicliqueEndpointOutOfBounds)?;
            network.add_arc(node, block_node, internal_capacity)?;
        }
        for &right in &block.right {
            let node = *vertical_nodes
                .get(right)
                .ok_or(Error::BicliqueEndpointOutOfBounds)?;
            network.add_arc(block_node, node, internal_capacity)?;
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
        block_nodes,
        vertical_nodes,
        internal_capacity,
    })
}
