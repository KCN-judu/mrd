use std::collections::VecDeque;
use std::mem::size_of;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BipartiteGraph {
    left_size: usize,
    right_size: usize,
    adjacency: Vec<Vec<usize>>,
}

impl BipartiteGraph {
    #[must_use]
    pub fn new(left_size: usize, right_size: usize) -> Self {
        Self {
            left_size,
            right_size,
            adjacency: vec![Vec::new(); left_size],
        }
    }

    /// # Errors
    ///
    /// Returns [`GraphError::EndpointOutOfBounds`] for an invalid endpoint.
    pub fn add_edge(&mut self, left: usize, right: usize) -> Result<(), GraphError> {
        if left >= self.left_size || right >= self.right_size {
            return Err(GraphError::EndpointOutOfBounds {
                left,
                right,
                left_size: self.left_size,
                right_size: self.right_size,
            });
        }
        if !self.adjacency[left].contains(&right) {
            self.adjacency[left].push(right);
            self.adjacency[left].sort_unstable();
        }
        Ok(())
    }

    #[must_use]
    pub const fn left_size(&self) -> usize {
        self.left_size
    }

    #[must_use]
    pub const fn right_size(&self) -> usize {
        self.right_size
    }

    #[must_use]
    pub fn neighbors(&self, left: usize) -> &[usize] {
        &self.adjacency[left]
    }

    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.adjacency.iter().map(Vec::len).sum()
    }

    /// Estimates heap payload bytes retained by the adjacency representation.
    ///
    /// The estimate uses current Vec capacities and excludes allocator metadata
    /// and the inline graph fields.
    #[must_use]
    pub fn owned_bytes_estimate(&self) -> usize {
        self.adjacency
            .capacity()
            .saturating_mul(size_of::<Vec<usize>>())
            .saturating_add(
                self.adjacency
                    .iter()
                    .map(Vec::capacity)
                    .sum::<usize>()
                    .saturating_mul(size_of::<usize>()),
            )
    }

    pub fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.adjacency
            .iter()
            .enumerate()
            .flat_map(|(left, rights)| rights.iter().map(move |&right| (left, right)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Matching {
    pub left_to_right: Vec<Option<usize>>,
    pub right_to_left: Vec<Option<usize>>,
    pub size: usize,
}

#[must_use]
pub fn hopcroft_karp(graph: &BipartiteGraph) -> Matching {
    let mut matching = Matching {
        left_to_right: vec![None; graph.left_size],
        right_to_left: vec![None; graph.right_size],
        size: 0,
    };
    let mut distance = vec![usize::MAX; graph.left_size];

    while breadth_first_layers(graph, &matching, &mut distance) {
        for left in 0..graph.left_size {
            if matching.left_to_right[left].is_none()
                && augment(graph, left, &mut matching, &mut distance)
            {
                matching.size += 1;
            }
        }
    }
    matching
}

fn breadth_first_layers(
    graph: &BipartiteGraph,
    matching: &Matching,
    distance: &mut [usize],
) -> bool {
    let mut queue = VecDeque::new();
    for (left, matched) in matching.left_to_right.iter().enumerate() {
        if matched.is_none() {
            distance[left] = 0;
            queue.push_back(left);
        } else {
            distance[left] = usize::MAX;
        }
    }

    let mut found_augmenting = false;
    while let Some(left) = queue.pop_front() {
        for &right in graph.neighbors(left) {
            if let Some(next_left) = matching.right_to_left[right] {
                if distance[next_left] == usize::MAX {
                    distance[next_left] = distance[left] + 1;
                    queue.push_back(next_left);
                }
            } else {
                found_augmenting = true;
            }
        }
    }
    found_augmenting
}

fn augment(
    graph: &BipartiteGraph,
    left: usize,
    matching: &mut Matching,
    distance: &mut [usize],
) -> bool {
    for &right in graph.neighbors(left) {
        let can_use = match matching.right_to_left[right] {
            None => true,
            Some(next_left) => {
                distance[next_left] == distance[left] + 1
                    && augment(graph, next_left, matching, distance)
            }
        };
        if can_use {
            matching.left_to_right[left] = Some(right);
            matching.right_to_left[right] = Some(left);
            return true;
        }
    }
    distance[left] = usize::MAX;
    false
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VertexCover {
    pub left: Vec<bool>,
    pub right: Vec<bool>,
    pub size: usize,
}

#[must_use]
/// Recovers a Konig minimum vertex cover from a maximum matching.
///
/// # Panics
///
/// Panics when `matching` dimensions differ from `graph` dimensions.
pub fn minimum_vertex_cover(graph: &BipartiteGraph, matching: &Matching) -> VertexCover {
    assert_eq!(graph.left_size, matching.left_to_right.len());
    assert_eq!(graph.right_size, matching.right_to_left.len());
    let mut reachable_left = vec![false; graph.left_size];
    let mut reachable_right = vec![false; graph.right_size];
    let mut queue = VecDeque::new();
    for (left, matched) in matching.left_to_right.iter().enumerate() {
        if matched.is_none() {
            reachable_left[left] = true;
            queue.push_back(left);
        }
    }
    while let Some(left) = queue.pop_front() {
        for &right in graph.neighbors(left) {
            if matching.left_to_right[left] == Some(right) || reachable_right[right] {
                continue;
            }
            reachable_right[right] = true;
            if let Some(next_left) = matching.right_to_left[right]
                && !reachable_left[next_left]
            {
                reachable_left[next_left] = true;
                queue.push_back(next_left);
            }
        }
    }

    let left = reachable_left
        .iter()
        .map(|reachable| !reachable)
        .collect::<Vec<_>>();
    let right = reachable_right;
    let size = left.iter().filter(|&&selected| selected).count()
        + right.iter().filter(|&&selected| selected).count();
    VertexCover { left, right, size }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GraphError {
    #[error(
        "edge ({left}, {right}) is outside bipartite graph dimensions ({left_size}, {right_size})"
    )]
    EndpointOutOfBounds {
        left: usize,
        right: usize,
        left_size: usize,
        right_size: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::{BipartiteGraph, hopcroft_karp, minimum_vertex_cover};

    #[test]
    fn matching_and_cover_agree() {
        let mut graph = BipartiteGraph::new(3, 3);
        for edge in [(0, 0), (0, 1), (1, 1), (2, 1), (2, 2)] {
            graph.add_edge(edge.0, edge.1).unwrap();
        }
        let matching = hopcroft_karp(&graph);
        let cover = minimum_vertex_cover(&graph, &matching);
        assert_eq!(matching.size, 3);
        assert_eq!(cover.size, matching.size);
        assert!(
            graph
                .edges()
                .all(|(left, right)| cover.left[left] || cover.right[right])
        );
    }
}
