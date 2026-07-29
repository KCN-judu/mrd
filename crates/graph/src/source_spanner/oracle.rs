//! Bounded enumerating simple-path Oracle for static spanner differentials.

use std::collections::BTreeSet;

use crate::FlowNodeId;

use super::model::{EdgeId, Error, Graph};

/// Enumerates all simple paths up to `maximum_hops` in lexicographic edge-id order.
///
/// # Errors
///
/// Returns an error for an out-of-range endpoint or a zero hop bound.
pub fn simple_paths(
    graph: &Graph,
    start: FlowNodeId,
    target: FlowNodeId,
    allowed: Option<&BTreeSet<EdgeId>>,
    maximum_hops: usize,
) -> Result<Vec<Vec<EdgeId>>, Error> {
    if start.0 >= graph.node_count() || target.0 >= graph.node_count() || maximum_hops == 0 {
        return Err(Error::InvalidGraph);
    }
    let mut paths = Vec::new();
    let mut used = BTreeSet::from([start]);
    let mut current = Vec::new();
    visit(
        graph,
        start,
        target,
        allowed,
        maximum_hops,
        &mut used,
        &mut current,
        &mut paths,
    )?;
    Ok(paths)
}

#[allow(clippy::too_many_arguments)]
fn visit(
    graph: &Graph,
    current_vertex: FlowNodeId,
    target: FlowNodeId,
    allowed: Option<&BTreeSet<EdgeId>>,
    maximum_hops: usize,
    used: &mut BTreeSet<FlowNodeId>,
    current: &mut Vec<EdgeId>,
    paths: &mut Vec<Vec<EdgeId>>,
) -> Result<(), Error> {
    if current_vertex == target && !current.is_empty() {
        paths.push(current.clone());
        return Ok(());
    }
    if current.len() == maximum_hops {
        return Ok(());
    }
    for index in 0..graph.edge_count() {
        let edge_id = EdgeId(index);
        if allowed.is_some_and(|set| !set.contains(&edge_id)) {
            continue;
        }
        let edge = graph.edge(edge_id).ok_or(Error::InvalidGraph)?;
        let next = if edge.first == current_vertex {
            edge.second
        } else if edge.second == current_vertex {
            edge.first
        } else {
            continue;
        };
        if !used.insert(next) {
            continue;
        }
        current.push(edge_id);
        visit(
            graph,
            next,
            target,
            allowed,
            maximum_hops,
            used,
            current,
            paths,
        )?;
        current.pop();
        used.remove(&next);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::simple_paths;
    use crate::{
        FlowNodeId,
        source_spanner::model::{Edge, EdgeId, Graph},
    };

    #[test]
    fn enumerates_bounded_simple_paths_in_edge_order() {
        let graph = Graph::new(
            3,
            vec![
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                },
                Edge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                },
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                },
            ],
        )
        .unwrap();
        assert_eq!(
            simple_paths(&graph, FlowNodeId(0), FlowNodeId(2), None, 2).unwrap(),
            vec![vec![EdgeId(0), EdgeId(1)], vec![EdgeId(2)]]
        );
    }
}
