use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct FlowNodeId(pub usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ArcSpec {
    from: FlowNodeId,
    to: FlowNodeId,
    capacity: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlowNetwork {
    node_count: usize,
    arcs: Vec<ArcSpec>,
}

impl FlowNetwork {
    #[must_use]
    pub const fn new(node_count: usize) -> Self {
        Self {
            node_count,
            arcs: Vec::new(),
        }
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    /// # Errors
    ///
    /// Returns [`FlowError::NodeOutOfBounds`] when either endpoint is invalid.
    pub fn add_arc(
        &mut self,
        from: FlowNodeId,
        to: FlowNodeId,
        capacity: u64,
    ) -> Result<(), FlowError> {
        if from.0 >= self.node_count || to.0 >= self.node_count {
            return Err(FlowError::NodeOutOfBounds {
                node: from.0.max(to.0),
                node_count: self.node_count,
            });
        }
        self.arcs.push(ArcSpec { from, to, capacity });
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FlowResult {
    pub value: u64,
    pub source_side: Vec<bool>,
}

pub trait MaxFlowBackend {
    /// Computes an integral maximum flow and the source-reachable side of its residual cut.
    ///
    /// # Errors
    ///
    /// Returns a [`FlowError`] for invalid terminals or an overflowing flow value.
    fn max_flow_min_cut(
        &self,
        network: &FlowNetwork,
        source: FlowNodeId,
        sink: FlowNodeId,
    ) -> Result<FlowResult, FlowError>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DinicBackend;

#[derive(Clone, Copy, Debug)]
struct ResidualEdge {
    to: usize,
    reverse: usize,
    capacity: u64,
}

impl MaxFlowBackend for DinicBackend {
    fn max_flow_min_cut(
        &self,
        network: &FlowNetwork,
        source: FlowNodeId,
        sink: FlowNodeId,
    ) -> Result<FlowResult, FlowError> {
        if source.0 >= network.node_count {
            return Err(FlowError::NodeOutOfBounds {
                node: source.0,
                node_count: network.node_count,
            });
        }
        if sink.0 >= network.node_count {
            return Err(FlowError::NodeOutOfBounds {
                node: sink.0,
                node_count: network.node_count,
            });
        }
        if source == sink {
            return Err(FlowError::IdenticalTerminals);
        }

        let mut residual = vec![Vec::new(); network.node_count];
        for arc in &network.arcs {
            add_residual_arc(&mut residual, arc.from.0, arc.to.0, arc.capacity);
        }

        let mut value = 0_u64;
        loop {
            let level = build_levels(&residual, source.0);
            if level[sink.0] < 0 {
                break;
            }
            let mut next_edge = vec![0; network.node_count];
            loop {
                let pushed = send_flow(
                    &mut residual,
                    &level,
                    &mut next_edge,
                    source.0,
                    sink.0,
                    u64::MAX,
                );
                if pushed == 0 {
                    break;
                }
                value = value.checked_add(pushed).ok_or(FlowError::ValueOverflow)?;
            }
        }

        let source_side = residual_reachable(&residual, source.0);
        Ok(FlowResult { value, source_side })
    }
}

fn add_residual_arc(graph: &mut [Vec<ResidualEdge>], from: usize, to: usize, capacity: u64) {
    let forward_reverse = graph[to].len();
    let backward_reverse = graph[from].len();
    graph[from].push(ResidualEdge {
        to,
        reverse: forward_reverse,
        capacity,
    });
    graph[to].push(ResidualEdge {
        to: from,
        reverse: backward_reverse,
        capacity: 0,
    });
}

fn build_levels(graph: &[Vec<ResidualEdge>], source: usize) -> Vec<i64> {
    let mut level = vec![-1; graph.len()];
    level[source] = 0;
    let mut queue = VecDeque::from([source]);
    while let Some(node) = queue.pop_front() {
        for edge in &graph[node] {
            if edge.capacity > 0 && level[edge.to] < 0 {
                level[edge.to] = level[node] + 1;
                queue.push_back(edge.to);
            }
        }
    }
    level
}

fn send_flow(
    graph: &mut [Vec<ResidualEdge>],
    level: &[i64],
    next_edge: &mut [usize],
    node: usize,
    sink: usize,
    available: u64,
) -> u64 {
    if node == sink {
        return available;
    }
    while next_edge[node] < graph[node].len() {
        let edge_index = next_edge[node];
        let edge = graph[node][edge_index];
        if edge.capacity > 0 && level[edge.to] == level[node] + 1 {
            let pushed = send_flow(
                graph,
                level,
                next_edge,
                edge.to,
                sink,
                available.min(edge.capacity),
            );
            if pushed > 0 {
                graph[node][edge_index].capacity -= pushed;
                graph[edge.to][edge.reverse].capacity += pushed;
                return pushed;
            }
        }
        next_edge[node] += 1;
    }
    0
}

fn residual_reachable(graph: &[Vec<ResidualEdge>], source: usize) -> Vec<bool> {
    let mut reachable = vec![false; graph.len()];
    reachable[source] = true;
    let mut queue = VecDeque::from([source]);
    while let Some(node) = queue.pop_front() {
        for edge in &graph[node] {
            if edge.capacity > 0 && !reachable[edge.to] {
                reachable[edge.to] = true;
                queue.push_back(edge.to);
            }
        }
    }
    reachable
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FlowError {
    #[error("flow node {node} is outside network with {node_count} nodes")]
    NodeOutOfBounds { node: usize, node_count: usize },
    #[error("flow source and sink must be distinct")]
    IdenticalTerminals,
    #[error("maximum-flow value overflowed u64")]
    ValueOverflow,
}

#[cfg(test)]
mod tests {
    use super::{DinicBackend, FlowNetwork, FlowNodeId, MaxFlowBackend};

    #[test]
    fn computes_value_and_cut() {
        let mut network = FlowNetwork::new(4);
        for (from, to, capacity) in [(0, 1, 2), (0, 2, 1), (1, 3, 1), (2, 3, 2)] {
            network
                .add_arc(FlowNodeId(from), FlowNodeId(to), capacity)
                .unwrap();
        }
        let result = DinicBackend
            .max_flow_min_cut(&network, FlowNodeId(0), FlowNodeId(3))
            .unwrap();
        assert_eq!(result.value, 2);
        assert!(result.source_side[0]);
        assert!(!result.source_side[3]);
    }
}
