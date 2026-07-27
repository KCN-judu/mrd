use std::collections::{BTreeSet, VecDeque};

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

    #[must_use]
    pub const fn arc_count(&self) -> usize {
        self.arcs.len()
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

/// Counters for the deterministic highest-label push-relabel backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PushRelabelMetrics {
    pub push_count: u64,
    pub relabel_count: u64,
    pub global_relabel_count: u64,
    pub gap_count: u64,
}

/// Exact integral highest-label push-relabel with global relabel and gap
/// heuristics. It is a practical backend, not the later almost-linear backend.
#[derive(Clone, Copy, Debug, Default)]
pub struct PushRelabelBackend;

/// Selects a permanently available exact integral max-flow backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlowBackendKind {
    #[default]
    Dinic,
    PushRelabel,
}

impl FlowBackendKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dinic => "dinic",
            Self::PushRelabel => "push-relabel",
        }
    }
}

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

impl PushRelabelBackend {
    /// Computes a maximum flow and returns structural operation counters.
    ///
    /// # Errors
    ///
    /// Returns the same terminal and overflow errors as [`MaxFlowBackend`].
    pub fn max_flow_min_cut_with_metrics(
        &self,
        network: &FlowNetwork,
        source: FlowNodeId,
        sink: FlowNodeId,
    ) -> Result<(FlowResult, PushRelabelMetrics), FlowError> {
        validate_terminals(network, source, sink)?;
        let node_count = network.node_count;
        let mut residual = vec![Vec::new(); node_count];
        for arc in &network.arcs {
            add_residual_arc(&mut residual, arc.from.0, arc.to.0, arc.capacity);
        }

        let mut height = global_relabel(&residual, source.0, sink.0);
        let mut excess = vec![0_u128; node_count];
        let mut current = vec![0_usize; node_count];
        let mut metrics = PushRelabelMetrics {
            global_relabel_count: 1,
            ..PushRelabelMetrics::default()
        };
        let mut height_count = count_heights(&height);
        let mut active = BTreeSet::new();

        // The source is deliberately set to n after the first global relabel.
        height_count[height[source.0]] -= 1;
        height[source.0] = node_count;
        height_count[node_count] += 1;
        for edge_index in 0..residual[source.0].len() {
            let amount = residual[source.0][edge_index].capacity;
            if amount == 0 {
                continue;
            }
            push(&mut residual, &mut excess, source.0, edge_index, amount);
            metrics.push_count += 1;
        }
        enqueue_active(&mut active, &excess, &height, source.0, sink.0);

        let mut work_since_global = 0_usize;
        let global_interval = node_count.saturating_mul(2).max(1);
        while let Some(&(key_height, node)) = active.iter().next_back() {
            active.remove(&(key_height, node));
            if node == source.0 || node == sink.0 || excess[node] == 0 || height[node] != key_height
            {
                continue;
            }
            while excess[node] != 0 {
                if current[node] == residual[node].len() {
                    let old_height = height[node];
                    let unreachable = node_count.saturating_add(1);
                    let next_height = residual[node]
                        .iter()
                        .filter(|edge| edge.capacity > 0)
                        .map(|edge| height[edge.to].saturating_add(1).min(unreachable))
                        .min()
                        .unwrap_or(unreachable);
                    height_count[old_height] -= 1;
                    height[node] = next_height;
                    height_count[next_height] += 1;
                    current[node] = 0;
                    metrics.relabel_count += 1;
                    work_since_global += 1;
                    if old_height < node_count && height_count[old_height] == 0 {
                        apply_gap(
                            old_height,
                            source.0,
                            sink.0,
                            &mut height,
                            &mut height_count,
                            &excess,
                            &mut active,
                        );
                        metrics.gap_count += 1;
                    }
                    continue;
                }
                let edge_index = current[node];
                let edge = residual[node][edge_index];
                if edge.capacity > 0 && height[node] == height[edge.to].saturating_add(1) {
                    let amount = edge
                        .capacity
                        .min(u64::try_from(excess[node]).unwrap_or(u64::MAX));
                    push(&mut residual, &mut excess, node, edge_index, amount);
                    metrics.push_count += 1;
                    enqueue_active(&mut active, &excess, &height, source.0, sink.0);
                } else {
                    current[node] += 1;
                }
            }
            enqueue_active(&mut active, &excess, &height, source.0, sink.0);
            if work_since_global >= global_interval {
                height = global_relabel(&residual, source.0, sink.0);
                height[source.0] = node_count;
                height_count = count_heights(&height);
                current.fill(0);
                active.clear();
                enqueue_active(&mut active, &excess, &height, source.0, sink.0);
                metrics.global_relabel_count += 1;
                work_since_global = 0;
            }
        }
        let value = u64::try_from(excess[sink.0]).map_err(|_| FlowError::ValueOverflow)?;
        Ok((
            FlowResult {
                value,
                source_side: residual_reachable(&residual, source.0),
            },
            metrics,
        ))
    }
}

impl MaxFlowBackend for PushRelabelBackend {
    fn max_flow_min_cut(
        &self,
        network: &FlowNetwork,
        source: FlowNodeId,
        sink: FlowNodeId,
    ) -> Result<FlowResult, FlowError> {
        self.max_flow_min_cut_with_metrics(network, source, sink)
            .map(|(result, _)| result)
    }
}

impl MaxFlowBackend for FlowBackendKind {
    fn max_flow_min_cut(
        &self,
        network: &FlowNetwork,
        source: FlowNodeId,
        sink: FlowNodeId,
    ) -> Result<FlowResult, FlowError> {
        match self {
            Self::Dinic => DinicBackend.max_flow_min_cut(network, source, sink),
            Self::PushRelabel => PushRelabelBackend.max_flow_min_cut(network, source, sink),
        }
    }
}

fn validate_terminals(
    network: &FlowNetwork,
    source: FlowNodeId,
    sink: FlowNodeId,
) -> Result<(), FlowError> {
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
    Ok(())
}

fn push(
    graph: &mut [Vec<ResidualEdge>],
    excess: &mut [u128],
    from: usize,
    edge_index: usize,
    amount: u64,
) {
    let edge = graph[from][edge_index];
    graph[from][edge_index].capacity -= amount;
    graph[edge.to][edge.reverse].capacity += amount;
    excess[from] = excess[from].saturating_sub(u128::from(amount));
    excess[edge.to] += u128::from(amount);
}

fn global_relabel(graph: &[Vec<ResidualEdge>], source: usize, sink: usize) -> Vec<usize> {
    let unreachable = graph.len().saturating_add(1);
    let mut height = vec![unreachable; graph.len()];
    height[sink] = 0;
    let mut queue = VecDeque::from([sink]);
    while let Some(node) = queue.pop_front() {
        for (predecessor, edges) in graph.iter().enumerate() {
            if height[predecessor] == unreachable
                && edges
                    .iter()
                    .any(|edge| edge.to == node && edge.capacity > 0)
            {
                height[predecessor] = height[node] + 1;
                queue.push_back(predecessor);
            }
        }
    }
    height[source] = graph.len();
    height
}

fn count_heights(height: &[usize]) -> Vec<usize> {
    let mut counts = vec![0; height.len() + 2];
    for &value in height {
        counts[value] += 1;
    }
    counts
}

fn enqueue_active(
    active: &mut BTreeSet<(usize, usize)>,
    excess: &[u128],
    height: &[usize],
    source: usize,
    sink: usize,
) {
    for node in 0..height.len() {
        if node != source && node != sink && excess[node] > 0 {
            active.insert((height[node], node));
        }
    }
}

fn apply_gap(
    gap: usize,
    source: usize,
    sink: usize,
    height: &mut [usize],
    height_count: &mut [usize],
    excess: &[u128],
    active: &mut BTreeSet<(usize, usize)>,
) {
    let unreachable = height.len() + 1;
    for node in 0..height.len() {
        if node != source && node != sink && height[node] > gap && height[node] < height.len() {
            height_count[height[node]] -= 1;
            height[node] = unreachable;
            height_count[unreachable] += 1;
        }
    }
    active.clear();
    enqueue_active(active, excess, height, source, sink);
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
    use super::{DinicBackend, FlowNetwork, FlowNodeId, MaxFlowBackend, PushRelabelBackend};

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

    #[test]
    fn push_relabel_matches_dinic_on_deterministic_networks() {
        let arcs = [
            (0, 1),
            (0, 2),
            (0, 3),
            (1, 0),
            (1, 2),
            (1, 3),
            (2, 0),
            (2, 1),
            (2, 3),
            (3, 0),
            (3, 1),
            (3, 2),
        ];
        for seed in 0_u64..1_024 {
            let mut network = FlowNetwork::new(4);
            for (index, (from, to)) in arcs.iter().enumerate() {
                let mixed = seed
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(index as u64 * 1_442_695_040_888_963_407);
                let shift = u32::try_from(index % 31).expect("modulo 31 fits in u32");
                let capacity = mixed.rotate_left(shift) % 8;
                network
                    .add_arc(FlowNodeId(*from), FlowNodeId(*to), capacity)
                    .unwrap();
            }
            let dinic = DinicBackend
                .max_flow_min_cut(&network, FlowNodeId(0), FlowNodeId(3))
                .unwrap();
            let (push_relabel, metrics) = PushRelabelBackend
                .max_flow_min_cut_with_metrics(&network, FlowNodeId(0), FlowNodeId(3))
                .unwrap();
            assert_eq!(push_relabel.value, dinic.value, "seed {seed}");
            assert_eq!(
                cut_capacity(&network, &push_relabel.source_side),
                push_relabel.value
            );
            assert!(push_relabel.source_side[0]);
            assert!(!push_relabel.source_side[3]);
            assert!(metrics.global_relabel_count >= 1);
        }
    }

    fn cut_capacity(network: &FlowNetwork, source_side: &[bool]) -> u64 {
        network
            .arcs
            .iter()
            .filter(|arc| source_side[arc.from.0] && !source_side[arc.to.0])
            .map(|arc| arc.capacity)
            .sum()
    }
}
