use std::collections::BTreeSet;

use thiserror::Error;

use crate::{
    CertifiedFixedPoint, ExactRatio, FixedPointConfig, FlowNodeId, SourceDynamicGraph, SourceEdgeId,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct An19PetalMetrics {
    pub shortest_path_runs: u64,
    pub edge_relaxations: u64,
    pub radius_events: u64,
    pub certified_comparisons: u64,
}

/// Exact Figure 6 output on AN19's original unit-length vertex domain.
///
/// Interior weighted portal points and the hierarchical constructor are
/// intentionally outside this partial source gate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct An19UnweightedPetal {
    pub vertices: BTreeSet<FlowNodeId>,
    pub path_from_center: Vec<FlowNodeId>,
    pub center_vertex: Option<FlowNodeId>,
    pub radius: ExactRatio,
    pub window_index: usize,
    pub window_start: ExactRatio,
    pub window_end: ExactRatio,
    pub internal_edges: usize,
    pub boundary_edges: usize,
    pub cluster_edges: usize,
    pub metrics: An19PetalMetrics,
}

impl An19UnweightedPetal {
    /// Executes AN19 Figure 6 exactly on an induced unit-length cluster.
    ///
    /// `remaining` is Figure 5's current `Y`, while `cluster` is its fixed
    /// `X`. The fixed shortest path is selected lexicographically by edge ID,
    /// implementing the paper's deterministic unique-path perturbation.
    ///
    /// # Errors
    ///
    /// Returns an error unless all active edges have unit length, the supplied
    /// vertex sets satisfy the Figure 5 containment/connectivity contract, or
    /// the certified logarithmic comparisons can be resolved.
    pub fn construct(
        graph: &SourceDynamicGraph,
        cluster: &BTreeSet<FlowNodeId>,
        remaining: &BTreeSet<FlowNodeId>,
        center: FlowNodeId,
        target: FlowNodeId,
        budget: ExactRatio,
    ) -> Result<Self, An19PetalError> {
        validate_domain(graph, cluster, remaining, center, target, budget)?;
        let mut metrics = An19PetalMetrics::default();
        let cluster_paths = shortest_paths(graph, cluster, center, &mut metrics)?;
        let path = recover_path(center, target, &cluster_paths)?;
        if path.iter().any(|vertex| !remaining.contains(vertex)) {
            return Err(An19PetalError::InvalidDomain);
        }
        let remaining_from_center = shortest_paths(graph, remaining, center, &mut metrics)?;
        for vertex in &path {
            if cluster_paths.distances[vertex.0] != remaining_from_center.distances[vertex.0] {
                return Err(An19PetalError::InvalidDomain);
            }
        }
        let thresholds = membership_thresholds(
            graph,
            remaining,
            &path,
            &remaining_from_center,
            target,
            budget,
            &mut metrics,
        )?;
        let cluster_edges = internal_edge_count(graph, cluster);
        let active_edges = (0..graph.edge_count())
            .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
            .count();
        if cluster_edges == 0 || active_edges < 2 {
            return Err(An19PetalError::InvalidDomain);
        }
        let levels = ceil_log_log(graph.node_count());
        let mut selected = None;
        for index in 1..=levels {
            let window_end = window_radius(budget, index, levels, true)?;
            let vertices = vertices_at_radius(remaining, &thresholds, window_end)?;
            let internal = internal_edge_count(graph, &vertices);
            metrics.certified_comparisons = metrics
                .certified_comparisons
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            if certify_window_condition(active_edges, cluster_edges, internal, index, levels)? {
                selected = Some((index, window_end));
                break;
            }
        }
        let (window_index, window_end) = selected.ok_or(An19PetalError::InvalidRadius)?;
        let window_start = window_radius(budget, window_index, levels, false)?;
        let start_vertices = vertices_at_radius(remaining, &thresholds, window_start)?;
        let start_edges = internal_edge_count(graph, &start_vertices);
        if start_edges == 0 || start_edges >= cluster_edges {
            return Err(An19PetalError::InvalidRadius);
        }
        let mut radius = window_start;
        let (vertices, internal_edges, boundary_edges) = loop {
            let vertices = vertices_at_radius(remaining, &thresholds, radius)?;
            let internal = internal_edge_count(graph, &vertices);
            let boundary = boundary_edge_count(graph, cluster, &vertices);
            metrics.certified_comparisons = metrics
                .certified_comparisons
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            if certify_stopping_condition(
                cluster_edges,
                start_edges,
                internal,
                boundary,
                levels,
                budget,
            )? {
                break (vertices, internal, boundary);
            }
            radius = next_radius_event(remaining, &thresholds, radius, window_end)?
                .ok_or(An19PetalError::InvalidRadius)?;
            metrics.radius_events = metrics
                .radius_events
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
        };
        let center_vertex = path
            .iter()
            .copied()
            .find(|vertex| thresholds.path_distance_from_target[vertex.0] == Some(radius));
        Ok(Self {
            vertices,
            path_from_center: path,
            center_vertex,
            radius,
            window_index,
            window_start,
            window_end,
            internal_edges,
            boundary_edges,
            cluster_edges,
            metrics,
        })
    }
}

#[derive(Clone, Debug)]
struct ShortestPaths {
    distances: Vec<Option<ExactRatio>>,
    predecessors: Vec<Option<(usize, SourceEdgeId)>>,
}

#[derive(Clone, Debug)]
struct MembershipThresholds {
    by_vertex: Vec<Option<ExactRatio>>,
    path_distance_from_target: Vec<Option<ExactRatio>>,
}

fn validate_domain(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    remaining: &BTreeSet<FlowNodeId>,
    center: FlowNodeId,
    target: FlowNodeId,
    budget: ExactRatio,
) -> Result<(), An19PetalError> {
    let one = ratio(1, 1)?;
    if cluster.is_empty()
        || !remaining.is_subset(cluster)
        || !remaining.contains(&center)
        || !remaining.contains(&target)
        || !budget.is_positive()
        || cluster.iter().any(|vertex| vertex.0 >= graph.node_count())
    {
        return Err(An19PetalError::InvalidDomain);
    }
    for index in 0..graph.edge_count() {
        if let Some(edge) = graph.edge(SourceEdgeId(index))
            && edge.length != one
        {
            return Err(An19PetalError::NonunitLength);
        }
    }
    Ok(())
}

fn shortest_paths(
    graph: &SourceDynamicGraph,
    allowed: &BTreeSet<FlowNodeId>,
    source: FlowNodeId,
    metrics: &mut An19PetalMetrics,
) -> Result<ShortestPaths, An19PetalError> {
    let n = graph.node_count();
    let mut distances = vec![None; n];
    let mut predecessors = vec![None; n];
    let mut path_keys = vec![None; n];
    let mut settled = vec![false; n];
    distances[source.0] = Some(ratio(0, 1)?);
    path_keys[source.0] = Some(Vec::new());
    loop {
        let mut next = None;
        for vertex in allowed {
            if settled[vertex.0] || distances[vertex.0].is_none() {
                continue;
            }
            match next {
                None => next = Some(vertex.0),
                Some(old) if path_state_is_better(vertex.0, old, &distances, &path_keys)? => {
                    next = Some(vertex.0);
                }
                Some(_) => {}
            }
        }
        let Some(node) = next else {
            break;
        };
        settled[node] = true;
        for index in 0..graph.edge_count() {
            let Some(edge) = graph.edge(SourceEdgeId(index)) else {
                continue;
            };
            let other = if edge.first.0 == node {
                edge.second.0
            } else if edge.second.0 == node {
                edge.first.0
            } else {
                continue;
            };
            if !allowed.contains(&FlowNodeId(other)) || settled[other] {
                continue;
            }
            metrics.edge_relaxations = metrics
                .edge_relaxations
                .checked_add(1)
                .ok_or(An19PetalError::Overflow)?;
            let candidate = distances[node]
                .ok_or(An19PetalError::Disconnected)?
                .checked_add(edge.length)
                .map_err(|_| An19PetalError::Overflow)?;
            let mut key = path_keys[node]
                .as_ref()
                .ok_or(An19PetalError::Disconnected)?
                .clone();
            key.push(SourceEdgeId(index));
            let improves = match distances[other] {
                None => true,
                Some(old) => {
                    ratio_less(candidate, old)?
                        || (candidate == old
                            && key
                                < *path_keys[other]
                                    .as_ref()
                                    .ok_or(An19PetalError::Disconnected)?)
                }
            };
            if improves {
                distances[other] = Some(candidate);
                predecessors[other] = Some((node, SourceEdgeId(index)));
                path_keys[other] = Some(key);
            }
        }
    }
    metrics.shortest_path_runs = metrics
        .shortest_path_runs
        .checked_add(1)
        .ok_or(An19PetalError::Overflow)?;
    if allowed.iter().any(|vertex| distances[vertex.0].is_none()) {
        return Err(An19PetalError::Disconnected);
    }
    Ok(ShortestPaths {
        distances,
        predecessors,
    })
}

fn path_state_is_better(
    candidate: usize,
    old: usize,
    distances: &[Option<ExactRatio>],
    path_keys: &[Option<Vec<SourceEdgeId>>],
) -> Result<bool, An19PetalError> {
    let candidate_distance = distances[candidate].ok_or(An19PetalError::Disconnected)?;
    let old_distance = distances[old].ok_or(An19PetalError::Disconnected)?;
    if ratio_less(candidate_distance, old_distance)? {
        return Ok(true);
    }
    if candidate_distance != old_distance {
        return Ok(false);
    }
    let candidate_key = path_keys[candidate]
        .as_ref()
        .ok_or(An19PetalError::Disconnected)?;
    let old_key = path_keys[old]
        .as_ref()
        .ok_or(An19PetalError::Disconnected)?;
    Ok((candidate_key, candidate) < (old_key, old))
}

fn recover_path(
    source: FlowNodeId,
    target: FlowNodeId,
    paths: &ShortestPaths,
) -> Result<Vec<FlowNodeId>, An19PetalError> {
    let mut reversed = vec![target];
    let mut current = target.0;
    while current != source.0 {
        let (parent, _) = paths.predecessors[current].ok_or(An19PetalError::Disconnected)?;
        current = parent;
        reversed.push(FlowNodeId(current));
    }
    reversed.reverse();
    Ok(reversed)
}

fn membership_thresholds(
    graph: &SourceDynamicGraph,
    remaining: &BTreeSet<FlowNodeId>,
    path: &[FlowNodeId],
    from_center: &ShortestPaths,
    target: FlowNodeId,
    budget: ExactRatio,
    metrics: &mut An19PetalMetrics,
) -> Result<MembershipThresholds, An19PetalError> {
    let mut by_vertex = vec![None; graph.node_count()];
    let mut path_distance_from_target = vec![None; graph.node_count()];
    let target_position = path
        .iter()
        .position(|vertex| *vertex == target)
        .ok_or(An19PetalError::InvalidDomain)?;
    for (position, point) in path.iter().enumerate().take(target_position + 1) {
        let point_paths = shortest_paths(graph, remaining, *point, metrics)?;
        let distance_from_target = ratio(
            i128::try_from(target_position - position).map_err(|_| An19PetalError::Overflow)?,
            1,
        )?;
        path_distance_from_target[point.0] = Some(distance_from_target);
        if ratio_less(budget, distance_from_target)? {
            continue;
        }
        let center_to_point = from_center.distances[point.0].ok_or(An19PetalError::Disconnected)?;
        for vertex in remaining {
            let point_to_vertex =
                point_paths.distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
            let center_to_vertex =
                from_center.distances[vertex.0].ok_or(An19PetalError::Disconnected)?;
            let excess = center_to_point
                .checked_add(point_to_vertex)
                .and_then(|value| value.checked_sub(center_to_vertex))
                .map_err(|_| An19PetalError::Overflow)?;
            if excess.is_negative() {
                return Err(An19PetalError::InvalidDomain);
            }
            let threshold = distance_from_target
                .checked_add(
                    excess
                        .checked_mul_integer(2)
                        .map_err(|_| An19PetalError::Overflow)?,
                )
                .map_err(|_| An19PetalError::Overflow)?;
            match by_vertex[vertex.0] {
                None => by_vertex[vertex.0] = Some(threshold),
                Some(old) if ratio_less(threshold, old)? => {
                    by_vertex[vertex.0] = Some(threshold);
                }
                Some(_) => {}
            }
        }
    }
    Ok(MembershipThresholds {
        by_vertex,
        path_distance_from_target,
    })
}

fn vertices_at_radius(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    radius: ExactRatio,
) -> Result<BTreeSet<FlowNodeId>, An19PetalError> {
    let mut result = BTreeSet::new();
    for vertex in remaining {
        if let Some(threshold) = thresholds.by_vertex[vertex.0]
            && radius
                .at_least(threshold)
                .map_err(|_| An19PetalError::Overflow)?
        {
            result.insert(*vertex);
        }
    }
    Ok(result)
}

fn window_radius(
    budget: ExactRatio,
    index: usize,
    levels: usize,
    upper: bool,
) -> Result<ExactRatio, An19PetalError> {
    let numerator = if upper {
        levels + index
    } else {
        levels + index - 1
    };
    let denominator = i128::try_from(levels)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .ok_or(An19PetalError::Overflow)?;
    budget
        .checked_mul(ratio(
            i128::try_from(numerator).map_err(|_| An19PetalError::Overflow)?,
            denominator,
        )?)
        .map_err(|_| An19PetalError::Overflow)
}

fn certify_window_condition(
    active_edges: usize,
    cluster_edges: usize,
    petal_edges: usize,
    index: usize,
    levels: usize,
) -> Result<bool, An19PetalError> {
    if petal_edges == 0 || index == levels {
        return Ok(true);
    }
    for precision in [48_u32, 96, 192, 384] {
        match try_window_comparison(
            active_edges,
            cluster_edges,
            petal_edges,
            index,
            levels,
            precision,
        ) {
            Ok(Some(result)) => return Ok(result),
            Ok(None) | Err(An19PetalError::InsufficientPrecision) => {}
            Err(error) => return Err(error),
        }
    }
    Err(An19PetalError::InsufficientPrecision)
}

fn try_window_comparison(
    active_edges: usize,
    cluster_edges: usize,
    petal_edges: usize,
    index: usize,
    levels: usize,
    precision: u32,
) -> Result<Option<bool>, An19PetalError> {
    let mut arithmetic = fixed_arithmetic(active_edges, cluster_edges, precision)?;
    let two = arithmetic
        .enclose_ratio(2, 1)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let log_two = arithmetic
        .logarithm(&two)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let m = arithmetic
        .enclose_ratio(
            i128::try_from(active_edges).map_err(|_| An19PetalError::Overflow)?,
            1,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let ln_edge_count = arithmetic
        .logarithm(&m)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let binary_edge_logarithm = arithmetic
        .divide_intervals(&ln_edge_count, &log_two)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let iterated_logarithm = arithmetic
        .logarithm(&binary_edge_logarithm)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let alpha = arithmetic
        .enclose_ratio(
            i128::try_from(levels - index).map_err(|_| An19PetalError::Overflow)?,
            i128::try_from(levels).map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let exponent_input = arithmetic
        .multiply_intervals(&alpha, &iterated_logarithm)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let exponent = arithmetic
        .exponential(&exponent_input)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let right = arithmetic
        .multiply_intervals(&exponent, &log_two)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let left_ratio_numerator = i128::try_from(cluster_edges)
        .ok()
        .and_then(|value| value.checked_mul(2))
        .ok_or(An19PetalError::Overflow)?;
    let left_input = arithmetic
        .enclose_ratio(
            left_ratio_numerator,
            i128::try_from(petal_edges).map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let left = arithmetic
        .logarithm(&left_input)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    if left.lower_scaled() >= right.upper_scaled() {
        Ok(Some(true))
    } else if left.upper_scaled() < right.lower_scaled() {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

fn certify_stopping_condition(
    cluster_edges: usize,
    start_edges: usize,
    petal_edges: usize,
    boundary_edges: usize,
    levels: usize,
    budget: ExactRatio,
) -> Result<bool, An19PetalError> {
    if boundary_edges == 0 {
        return Ok(true);
    }
    if petal_edges == 0 || start_edges == 0 || start_edges >= cluster_edges {
        return Err(An19PetalError::InvalidRadius);
    }
    let denominator = i128::try_from(petal_edges)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| value.checked_mul(i128::try_from(levels).ok()?))
        .ok_or(An19PetalError::Overflow)?;
    let exact_left = budget
        .checked_mul(ratio(
            i128::try_from(boundary_edges).map_err(|_| An19PetalError::Overflow)?,
            denominator,
        )?)
        .map_err(|_| An19PetalError::Overflow)?;
    for precision in [48_u32, 96, 192, 384] {
        match try_stopping_comparison(
            cluster_edges,
            start_edges,
            petal_edges,
            exact_left,
            precision,
        ) {
            Ok(Some(result)) => return Ok(result),
            Ok(None) | Err(An19PetalError::InsufficientPrecision) => {}
            Err(error) => return Err(error),
        }
    }
    Err(An19PetalError::InsufficientPrecision)
}

fn try_stopping_comparison(
    cluster_edges: usize,
    start_edges: usize,
    petal_edges: usize,
    exact_left: ExactRatio,
    precision: u32,
) -> Result<Option<bool>, An19PetalError> {
    let mut arithmetic = fixed_arithmetic(cluster_edges, petal_edges, precision)?;
    let chi = arithmetic
        .enclose_ratio(
            i128::try_from(cluster_edges).map_err(|_| An19PetalError::Overflow)?,
            i128::try_from(start_edges).map_err(|_| An19PetalError::Overflow)?,
        )
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let log_chi = arithmetic
        .logarithm(&chi)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    let left = arithmetic
        .enclose_ratio(exact_left.numerator(), exact_left.denominator())
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    if left.upper_scaled() < log_chi.lower_scaled() {
        Ok(Some(true))
    } else if left.lower_scaled() >= log_chi.upper_scaled() {
        Ok(Some(false))
    } else {
        Ok(None)
    }
}

fn fixed_arithmetic(
    first_size: usize,
    second_size: usize,
    precision: u32,
) -> Result<CertifiedFixedPoint, An19PetalError> {
    let input_bits = u64::try_from(first_size)
        .ok()
        .and_then(|value| value.checked_add(u64::try_from(second_size).ok()?))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_mul(256))
        .ok_or(An19PetalError::Overflow)?;
    let terms = precision.checked_mul(2).ok_or(An19PetalError::Overflow)?;
    let config = FixedPointConfig::source_bounded(input_bits, precision, terms, 4)
        .map_err(|_| An19PetalError::InsufficientPrecision)?;
    CertifiedFixedPoint::new(config).map_err(|_| An19PetalError::InsufficientPrecision)
}

fn next_radius_event(
    remaining: &BTreeSet<FlowNodeId>,
    thresholds: &MembershipThresholds,
    current: ExactRatio,
    limit: ExactRatio,
) -> Result<Option<ExactRatio>, An19PetalError> {
    let mut next = None;
    for vertex in remaining {
        let Some(candidate) = thresholds.by_vertex[vertex.0] else {
            continue;
        };
        if !ratio_less(current, candidate)? || ratio_less(limit, candidate)? {
            continue;
        }
        match next {
            None => next = Some(candidate),
            Some(old) if ratio_less(candidate, old)? => next = Some(candidate),
            Some(_) => {}
        }
    }
    Ok(next)
}

fn internal_edge_count(graph: &SourceDynamicGraph, vertices: &BTreeSet<FlowNodeId>) -> usize {
    (0..graph.edge_count())
        .filter_map(|index| graph.edge(SourceEdgeId(index)))
        .filter(|edge| vertices.contains(&edge.first) && vertices.contains(&edge.second))
        .count()
}

fn boundary_edge_count(
    graph: &SourceDynamicGraph,
    cluster: &BTreeSet<FlowNodeId>,
    petal: &BTreeSet<FlowNodeId>,
) -> usize {
    (0..graph.edge_count())
        .filter_map(|index| graph.edge(SourceEdgeId(index)))
        .filter(|edge| cluster.contains(&edge.first) && cluster.contains(&edge.second))
        .filter(|edge| petal.contains(&edge.first) != petal.contains(&edge.second))
        .count()
}

fn ceil_log_log(value: usize) -> usize {
    let log = usize::try_from(usize::BITS - value.saturating_sub(1).leading_zeros()).unwrap_or(1);
    usize::try_from(usize::BITS - log.saturating_sub(1).leading_zeros())
        .unwrap_or(1)
        .max(1)
}

fn ratio(numerator: i128, denominator: i128) -> Result<ExactRatio, An19PetalError> {
    ExactRatio::new(numerator, denominator).map_err(|_| An19PetalError::Overflow)
}

fn ratio_less(left: ExactRatio, right: ExactRatio) -> Result<bool, An19PetalError> {
    Ok(!left.at_least(right).map_err(|_| An19PetalError::Overflow)?)
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum An19PetalError {
    #[error("AN19 petal input violates the unit-length source domain")]
    InvalidDomain,
    #[error("AN19 unweighted petal construction received a nonunit edge")]
    NonunitLength,
    #[error("AN19 petal cluster is disconnected")]
    Disconnected,
    #[error("AN19 Figure 6 radius window or stopping event is invalid")]
    InvalidRadius,
    #[error("certified AN19 logarithmic comparison needs more bounded precision")]
    InsufficientPrecision,
    #[error("checked AN19 petal arithmetic overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::An19UnweightedPetal;
    use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceWeightedEdge};
    use std::collections::BTreeSet;

    fn path_graph(nodes: usize) -> SourceDynamicGraph {
        let edges = (0..nodes - 1)
            .map(|node| SourceWeightedEdge {
                first: FlowNodeId(node),
                second: FlowNodeId(node + 1),
                length: ExactRatio::new(1, 1).unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            })
            .collect();
        SourceDynamicGraph::new(nodes, edges, 16).unwrap()
    }

    #[test]
    fn constructs_exact_path_petal_and_radius_window() {
        let graph = path_graph(10);
        let vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(4, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(petal.window_index, 1);
        assert_eq!(petal.window_start, ExactRatio::new(2, 1).unwrap());
        assert_eq!(petal.window_end, ExactRatio::new(3, 1).unwrap());
        assert_eq!(petal.radius, ExactRatio::new(2, 1).unwrap());
        assert_eq!(petal.center_vertex, Some(FlowNodeId(7)));
        assert_eq!(
            petal.vertices,
            BTreeSet::from([FlowNodeId(7), FlowNodeId(8), FlowNodeId(9)])
        );
        assert_eq!(petal.internal_edges, 2);
        assert_eq!(petal.boundary_edges, 1);
        assert_eq!(petal.metrics.radius_events, 0);
        assert!(petal.metrics.certified_comparisons >= 2);
    }

    #[test]
    fn rejects_nonunit_source_edges() {
        let graph = SourceDynamicGraph::new(
            2,
            vec![SourceWeightedEdge {
                first: FlowNodeId(0),
                second: FlowNodeId(1),
                length: ExactRatio::new(2, 1).unwrap(),
                weight: ExactRatio::new(1, 1).unwrap(),
            }],
            16,
        )
        .unwrap();
        let vertices = BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]);
        assert!(
            An19UnweightedPetal::construct(
                &graph,
                &vertices,
                &vertices,
                FlowNodeId(0),
                FlowNodeId(1),
                ExactRatio::new(1, 1).unwrap(),
            )
            .is_err()
        );
    }

    #[test]
    fn uses_lexicographic_edge_ids_for_equal_shortest_paths() {
        let graph = SourceDynamicGraph::new(
            4,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(2),
                    second: FlowNodeId(3),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            16,
        )
        .unwrap();
        let vertices = (0..4).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(3),
            ExactRatio::new(2, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(
            petal.path_from_center,
            vec![FlowNodeId(0), FlowNodeId(1), FlowNodeId(3)]
        );
    }

    #[test]
    fn preserves_fractional_center_as_an_unresolved_portal() {
        let graph = path_graph(10);
        let vertices = (0..10).map(FlowNodeId).collect::<BTreeSet<_>>();
        let petal = An19UnweightedPetal::construct(
            &graph,
            &vertices,
            &vertices,
            FlowNodeId(0),
            FlowNodeId(9),
            ExactRatio::new(3, 1).unwrap(),
        )
        .unwrap();
        assert_eq!(petal.radius, ExactRatio::new(3, 2).unwrap());
        assert_eq!(petal.center_vertex, None);
        assert_eq!(
            petal.vertices,
            BTreeSet::from([FlowNodeId(8), FlowNodeId(9)])
        );
    }
}
