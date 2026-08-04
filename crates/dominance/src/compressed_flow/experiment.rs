use graph::{FlowNetwork, FlowNodeId, FlowResult, MaxFlowBackend, VertexCover};

use super::{Error, Solution};
use crate::biclique::Partition;

pub mod source;

/// Exact immutable topology of one materialized compressed flow network.
///
/// This crate-internal value supports differential evidence without exposing
/// the mutable network builder used by the solver.
#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct NetworkSnapshot {
    pub node_count: usize,
    pub source: FlowNodeId,
    pub sink: FlowNodeId,
    pub arcs: Vec<(FlowNodeId, FlowNodeId, u64)>,
}

/// Materializes an exact structural snapshot of the compressed flow network.
///
/// # Errors
///
/// Returns the same construction errors as [`solve`].
#[cfg(test)]
pub(crate) fn network_snapshot(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &Partition,
) -> Result<NetworkSnapshot, Error> {
    let layout = build_network(horizontal_count, vertical_count, partition)?;
    Ok(NetworkSnapshot {
        node_count: layout.graph.node_count(),
        source: layout.source,
        sink: layout.sink,
        arcs: layout.graph.arcs().collect(),
    })
}

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
    let network = construct_network(horizontal_count, vertical_count, partition)?;
    solve_network(&network, backend)
}

/// Materializes the exact compressed flow network without running a flow
/// backend. This is a measurement boundary: callers can account for network
/// construction separately from max-flow execution without changing the
/// mathematical reduction.
///
/// # Errors
///
/// Returns construction errors for invalid dimensions, capacities, or
/// biclique endpoints.
pub fn construct_network(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &Partition,
) -> Result<Network, Error> {
    build_network(horizontal_count, vertical_count, partition)
}

/// Runs an exact flow and minimum-cover recovery on a previously materialized
/// compressed network.
///
/// # Errors
///
/// Returns backend, cut, or cover-certificate errors under the same contract
/// as [`solve`].
pub fn solve_network(network: &Network, backend: &impl MaxFlowBackend) -> Result<Solution, Error> {
    let flow = execute_flow(network, backend)?;
    recover_vertex_cover(network, flow)
}

/// Executes only the max-flow/min-cut backend on a materialized network.
///
/// # Errors
///
/// Returns a backend error without recovering a vertex cover.
pub fn execute_flow(network: &Network, backend: &impl MaxFlowBackend) -> Result<FlowResult, Error> {
    Ok(backend.max_flow_min_cut(&network.graph, network.source, network.sink)?)
}

/// Recovers and validates the minimum vertex cover from an exact flow result.
///
/// # Errors
///
/// Returns an error if the cut uses an internal arc or its recovered cover is
/// inconsistent with the minimum-cut value.
pub fn recover_vertex_cover(network: &Network, flow: FlowResult) -> Result<Solution, Error> {
    let mut internal_cut_arc_count = 0;
    for (block_index, block) in network.partition.blocks.iter().enumerate() {
        let block_node = network.block_node(block_index);
        for &left in &block.left {
            if flow.source_side[network.horizontal_node(left).0] && !flow.source_side[block_node.0]
            {
                internal_cut_arc_count += 1;
            }
        }
        for &right in &block.right {
            if flow.source_side[block_node.0] && !flow.source_side[network.vertical_node(right).0] {
                internal_cut_arc_count += 1;
            }
        }
    }
    if internal_cut_arc_count != 0 {
        return Err(Error::InternalArcInMinimumCut {
            count: internal_cut_arc_count,
        });
    }

    let left = (0..network.horizontal_count)
        .map(|index| !flow.source_side[network.horizontal_node(index).0])
        .collect::<Vec<_>>();
    let right = (0..network.vertical_count)
        .map(|index| flow.source_side[network.vertical_node(index).0])
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
        network_vertex_count: network.graph.node_count(),
        network_arc_count: network.graph.arc_count(),
        internal_capacity: network.internal_capacity,
        internal_cut_arc_count,
        flow,
        vertex_cover: VertexCover { left, right, size },
    })
}

/// Immutable topology of one compressed bipartite-flow reduction.
///
/// The underlying flow graph stays private so callers cannot mutate a network
/// between construction timing and exact solution.
pub struct Network {
    graph: FlowNetwork,
    source: FlowNodeId,
    sink: FlowNodeId,
    horizontal_start: usize,
    block_start: usize,
    vertical_start: usize,
    horizontal_count: usize,
    vertical_count: usize,
    internal_capacity: u64,
    partition: Partition,
}

impl Network {
    /// Returns the number of materialized flow-network nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the number of materialized flow-network arcs.
    #[must_use]
    pub fn arc_count(&self) -> usize {
        self.graph.arc_count()
    }

    const fn horizontal_node(&self, index: usize) -> FlowNodeId {
        FlowNodeId(self.horizontal_start + index)
    }

    const fn block_node(&self, index: usize) -> FlowNodeId {
        FlowNodeId(self.block_start + index)
    }

    const fn vertical_node(&self, index: usize) -> FlowNodeId {
        FlowNodeId(self.vertical_start + index)
    }
}

fn build_network(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &Partition,
) -> Result<Network, Error> {
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
    let internal_capacity = horizontal_count
        .min(vertical_count)
        .checked_add(1)
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(Error::CapacityOverflow)?;
    let mut network = FlowNetwork::new(node_count);
    for index in 0..horizontal_count {
        network.add_arc(source, FlowNodeId(horizontal_start + index), 1)?;
    }
    for (index, block) in partition.blocks.iter().enumerate() {
        let block_node = FlowNodeId(block_start + index);
        for &left in &block.left {
            if left >= horizontal_count {
                return Err(Error::BicliqueEndpointOutOfBounds);
            }
            let node = FlowNodeId(horizontal_start + left);
            network.add_arc(node, block_node, internal_capacity)?;
        }
        for &right in &block.right {
            if right >= vertical_count {
                return Err(Error::BicliqueEndpointOutOfBounds);
            }
            let node = FlowNodeId(vertical_start + right);
            network.add_arc(block_node, node, internal_capacity)?;
        }
    }
    for index in 0..vertical_count {
        network.add_arc(FlowNodeId(vertical_start + index), sink, 1)?;
    }
    Ok(Network {
        graph: network,
        source,
        sink,
        horizontal_start,
        block_start,
        vertical_start,
        horizontal_count,
        vertical_count,
        internal_capacity,
        partition: partition.clone(),
    })
}
