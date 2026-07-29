//! Source-flow-compatible compressed-network model and recovery.
//!
//! This module only transforms a verified compressed biclique network and
//! recovers certificates from a caller-supplied, certified terminal flow. It
//! deliberately does not choose a flow or call a permanent flow backend.

use std::collections::VecDeque;

use graph::{
    CirculationArcId, CirculationNetwork, FlowNodeId, MinCostCirculationError, MinCostSolution,
    VertexCover,
};
use thiserror::Error;

use crate::biclique::Partition;

/// A compressed bipartite flow network represented as a min-cost circulation.
///
/// The sole negative-cost arc returns flow from the sink to the source, so an
/// optimal circulation minimizes exactly the negated maximum matching value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Circulation {
    network: CirculationNetwork,
    horizontal_arcs: Vec<CirculationArcId>,
    blocks: Vec<Block>,
    vertical_arcs: Vec<CirculationArcId>,
    return_arc: CirculationArcId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Block {
    left: Vec<usize>,
    left_arcs: Vec<CirculationArcId>,
    right: Vec<usize>,
    right_arcs: Vec<CirculationArcId>,
}

/// A recovered matching and Konig cover from a certified terminal circulation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Solution {
    pub flow_value: usize,
    pub matching: Vec<(usize, usize)>,
    pub vertex_cover: VertexCover,
}

impl Circulation {
    /// Builds the exact min-cost circulation for one biclique partition.
    ///
    /// # Errors
    ///
    /// Returns an error when node counts, capacities, or block endpoints are
    /// not representable in the exact circulation domain.
    pub fn from_partition(
        horizontal_count: usize,
        vertical_count: usize,
        partition: &Partition,
    ) -> Result<Self, Error> {
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
        let maximum_matching = i128::try_from(horizontal_count.min(vertical_count))
            .map_err(|_| Error::CapacityOverflow)?;
        let internal_capacity = maximum_matching
            .checked_add(1)
            .ok_or(Error::CapacityOverflow)?;

        let mut network = CirculationNetwork::new(node_count);
        let horizontal_arcs = horizontal_nodes
            .iter()
            .copied()
            .map(|node| network.add_arc(source, node, 1, 0))
            .collect::<Result<Vec<_>, _>>()?;
        let mut blocks = Vec::with_capacity(partition.blocks.len());
        for (index, block) in partition.blocks.iter().enumerate() {
            let block_node = block_nodes[index];
            let left_arcs = block
                .left
                .iter()
                .copied()
                .map(|left| {
                    let node = *horizontal_nodes
                        .get(left)
                        .ok_or(Error::BicliqueEndpointOutOfBounds)?;
                    Ok(network.add_arc(node, block_node, internal_capacity, 0)?)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let right_arcs = block
                .right
                .iter()
                .copied()
                .map(|right| {
                    let node = *vertical_nodes
                        .get(right)
                        .ok_or(Error::BicliqueEndpointOutOfBounds)?;
                    Ok(network.add_arc(block_node, node, internal_capacity, 0)?)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            blocks.push(Block {
                left: block.left.clone(),
                left_arcs,
                right: block.right.clone(),
                right_arcs,
            });
        }
        let vertical_arcs = vertical_nodes
            .iter()
            .copied()
            .map(|node| network.add_arc(node, sink, 1, 0))
            .collect::<Result<Vec<_>, _>>()?;
        let return_arc = network.add_arc(sink, source, maximum_matching, -1)?;

        Ok(Self {
            network,
            horizontal_arcs,
            blocks,
            vertical_arcs,
            return_arc,
        })
    }

    /// Returns the immutable min-cost circulation consumed by source flow.
    #[must_use]
    pub const fn network(&self) -> &CirculationNetwork {
        &self.network
    }

    /// Recovers a matching and minimum vertex cover from a certified optimum.
    ///
    /// The caller must establish optimality separately, for example with the
    /// source-flow additive-half termination and exact recovery boundary. This
    /// method validates only exact feasibility and the objective encoding; it
    /// never performs an optimality search.
    ///
    /// # Errors
    ///
    /// Returns an error for an infeasible flow, malformed objective encoding,
    /// invalid block decomposition, or an invalid recovered matching or cover.
    pub fn recover_certified(&self, solution: &MinCostSolution) -> Result<Solution, Error> {
        self.network.verify_feasible_solution(solution)?;
        let return_value = Self::flow(solution, self.return_arc)?;
        let expected_cost = return_value.checked_neg().ok_or(Error::ObjectiveOverflow)?;
        if solution.cost != expected_cost {
            return Err(Error::ObjectiveMismatch {
                expected: expected_cost,
                actual: solution.cost,
            });
        }
        let flow_value = usize::try_from(return_value).map_err(|_| Error::FlowValueConversion)?;

        let mut left_match = vec![None; self.horizontal_arcs.len()];
        let mut right_match = vec![None; self.vertical_arcs.len()];
        for block in &self.blocks {
            let left = active_endpoints(&block.left, &block.left_arcs, solution)?;
            let right = active_endpoints(&block.right, &block.right_arcs, solution)?;
            if left.len() != right.len() {
                return Err(Error::BlockFlowMismatch);
            }
            for (left, right) in left.into_iter().zip(right) {
                if left_match[left].is_some() || right_match[right].is_some() {
                    return Err(Error::RepeatedMatchingEndpoint);
                }
                left_match[left] = Some(right);
                right_match[right] = Some(left);
            }
        }
        self.verify_outer_arcs(solution, &left_match, &right_match)?;

        let matching = left_match
            .iter()
            .enumerate()
            .filter_map(|(left, right)| right.map(|right| (left, right)))
            .collect::<Vec<_>>();
        if matching.len() != flow_value {
            return Err(Error::MatchingValueMismatch {
                matching: matching.len(),
                flow: flow_value,
            });
        }
        let vertex_cover = self.minimum_vertex_cover(&left_match, &right_match)?;
        if vertex_cover.size != flow_value {
            return Err(Error::CoverValueMismatch {
                cover: vertex_cover.size,
                flow: flow_value,
            });
        }
        Ok(Solution {
            flow_value,
            matching,
            vertex_cover,
        })
    }

    fn flow(solution: &MinCostSolution, arc: CirculationArcId) -> Result<i128, Error> {
        solution
            .arc_flows
            .get(arc.0)
            .copied()
            .ok_or(Error::MalformedSolution)
    }

    fn verify_outer_arcs(
        &self,
        solution: &MinCostSolution,
        left_match: &[Option<usize>],
        right_match: &[Option<usize>],
    ) -> Result<(), Error> {
        for (left, arc) in self.horizontal_arcs.iter().copied().enumerate() {
            let expected = i128::from(left_match[left].is_some());
            if Self::flow(solution, arc)? != expected {
                return Err(Error::OuterFlowMismatch);
            }
        }
        for (right, arc) in self.vertical_arcs.iter().copied().enumerate() {
            let expected = i128::from(right_match[right].is_some());
            if Self::flow(solution, arc)? != expected {
                return Err(Error::OuterFlowMismatch);
            }
        }
        Ok(())
    }

    fn minimum_vertex_cover(
        &self,
        left_match: &[Option<usize>],
        right_match: &[Option<usize>],
    ) -> Result<VertexCover, Error> {
        let mut reachable_left = vec![false; left_match.len()];
        let mut reachable_right = vec![false; right_match.len()];
        let mut queue = VecDeque::new();
        for (left, matched) in left_match.iter().enumerate() {
            if matched.is_none() {
                reachable_left[left] = true;
                queue.push_back(left);
            }
        }
        while let Some(left) = queue.pop_front() {
            for block in &self.blocks {
                if block.left.contains(&left) {
                    let matched_right = left_match[left];
                    for &right in &block.right {
                        if matched_right == Some(right) || reachable_right[right] {
                            continue;
                        }
                        reachable_right[right] = true;
                        if let Some(next_left) = right_match[right]
                            && !reachable_left[next_left]
                        {
                            reachable_left[next_left] = true;
                            queue.push_back(next_left);
                        }
                    }
                }
            }
        }
        let left: Vec<bool> = reachable_left.iter().map(|reachable| !*reachable).collect();
        let right = reachable_right;
        for block in &self.blocks {
            for &left_endpoint in &block.left {
                for &right_endpoint in &block.right {
                    if !left[left_endpoint] && !right[right_endpoint] {
                        return Err(Error::UncoveredBicliqueEdge);
                    }
                }
            }
        }
        let size = left.iter().filter(|selected| **selected).count()
            + right.iter().filter(|selected| **selected).count();
        Ok(VertexCover { left, right, size })
    }
}

fn active_endpoints(
    endpoints: &[usize],
    arcs: &[CirculationArcId],
    solution: &MinCostSolution,
) -> Result<Vec<usize>, Error> {
    if endpoints.len() != arcs.len() {
        return Err(Error::MalformedModel);
    }
    endpoints
        .iter()
        .copied()
        .zip(arcs.iter().copied())
        .filter_map(
            |(endpoint, arc)| match solution.arc_flows.get(arc.0).copied() {
                Some(0) => None,
                Some(1) => Some(Ok(endpoint)),
                Some(_) => Some(Err(Error::NonUnitBlockFlow)),
                None => Some(Err(Error::MalformedSolution)),
            },
        )
        .collect()
}

/// A source-flow-compatible compressed-network transformation failed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] MinCostCirculationError),
    #[error("compressed network node count overflowed usize")]
    NetworkSizeOverflow,
    #[error("compressed capacity cannot be represented in the exact domain")]
    CapacityOverflow,
    #[error("biclique contains an endpoint outside its declared side")]
    BicliqueEndpointOutOfBounds,
    #[error("certified circulation has an invalid arc-flow dimension")]
    MalformedSolution,
    #[error("source-return objective overflowed")]
    ObjectiveOverflow,
    #[error("circulation objective {actual} differs from encoded value {expected}")]
    ObjectiveMismatch { expected: i128, actual: i128 },
    #[error("encoded flow value cannot be represented as usize")]
    FlowValueConversion,
    #[error("a biclique block has unequal active left and right incidences")]
    BlockFlowMismatch,
    #[error("a biclique block carries a nonunit outer incidence")]
    NonUnitBlockFlow,
    #[error("recovered matching repeats a horizontal or vertical endpoint")]
    RepeatedMatchingEndpoint,
    #[error("compressed outer arcs disagree with the recovered matching")]
    OuterFlowMismatch,
    #[error("recovered matching size {matching} differs from flow value {flow}")]
    MatchingValueMismatch { matching: usize, flow: usize },
    #[error("recovered cover size {cover} differs from flow value {flow}")]
    CoverValueMismatch { cover: usize, flow: usize },
    #[error("recovered cover omits a compressed biclique edge")]
    UncoveredBicliqueEdge,
    #[error("compressed circulation model is malformed")]
    MalformedModel,
}

#[cfg(test)]
mod tests {
    use graph::{
        BipartiteGraph, CertifiedIpmSnapshot, ExactRatio, FixedPointConfig, FractionalCirculation,
        source_flow::Backend,
    };

    use super::Circulation;
    use crate::{
        biclique::{Block, Partition},
        compressed_flow::oracle,
        embedding::DominanceEmbedding,
        formal::analyze_formal_admissible_family,
    };
    use mrd_domain::{
        BicliqueId, FormalRectilinearPolygon, HorizontalChord, HorizontalChordId, Ornament,
        OrnamentSegment, OrthogonalLoop, Point, RectilinearPolygon, VerticalChord, VerticalChordId,
    };
    use sg_oracle::polygon::CoordinateCompressedCompletion;

    fn two_by_two_partition() -> (BipartiteGraph, Partition) {
        let mut graph = BipartiteGraph::new(2, 2);
        for left in 0..2 {
            for right in 0..2 {
                graph.add_edge(left, right).unwrap();
            }
        }
        (graph.clone(), Partition::from_explicit_edges(&graph))
    }

    fn complete_two_by_two_partition() -> Partition {
        Partition {
            blocks: vec![Block {
                id: BicliqueId(0),
                left: vec![0, 1],
                right: vec![0, 1],
            }],
        }
    }

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    fn formal_source_figure_three() -> FormalRectilinearPolygon {
        FormalRectilinearPolygon::new(
            RectilinearPolygon::new(rectangle(0, 0, 12, 12), vec![rectangle(2, 6, 5, 9)]).unwrap(),
            Ornament {
                isolated_points: vec![Point::new(6, 3), Point::new(6, 9), Point::new(8, 9)],
                segments: vec![
                    OrnamentSegment::new(Point::new(10, 0), Point::new(10, 3)).unwrap(),
                    OrnamentSegment::new(Point::new(2, 3), Point::new(5, 3)).unwrap(),
                    OrnamentSegment::new(Point::new(10, 6), Point::new(12, 6)).unwrap(),
                    OrnamentSegment::new(Point::new(10, 9), Point::new(10, 12)).unwrap(),
                ],
            },
        )
        .unwrap()
    }

    #[test]
    fn recovers_reference_flow_value_matching_and_cover() {
        let (graph, partition) = two_by_two_partition();
        let circulation = Circulation::from_partition(2, 2, &partition).unwrap();
        let terminal = graph::min_cost::experiment::solve(circulation.network()).unwrap();
        let recovered = circulation.recover_certified(&terminal).unwrap();
        let reference = oracle::audit(&graph, &partition).unwrap();

        assert_eq!(recovered.flow_value, reference.matching_size);
        assert_eq!(recovered.matching.len(), reference.matching_size);
        assert_eq!(recovered.vertex_cover.size, reference.matching_size);
        for (left, right) in &recovered.matching {
            assert!(graph.edges().any(|edge| edge == (*left, *right)));
        }
        for (left, right) in graph.edges() {
            assert!(recovered.vertex_cover.left[left] || recovered.vertex_cover.right[right]);
        }
    }

    #[test]
    fn differentially_recovers_an_mrd_chord_graph() {
        let horizontal = [
            HorizontalChord::new(HorizontalChordId(0), 0, 4, 0).unwrap(),
            HorizontalChord::new(HorizontalChordId(1), 1, 2, 3).unwrap(),
            HorizontalChord::new(HorizontalChordId(2), -2, 1, 1).unwrap(),
        ];
        let vertical = [
            VerticalChord::new(VerticalChordId(0), 0, -1, 2).unwrap(),
            VerticalChord::new(VerticalChordId(1), 2, 0, 4).unwrap(),
            VerticalChord::new(VerticalChordId(2), 4, 0, 1).unwrap(),
        ];
        let embedding = DominanceEmbedding::new(&horizontal, &vertical).unwrap();
        let graph = embedding.explicit_graph().unwrap();
        let partition = Partition::comparability_theorem_8(&embedding).unwrap();
        partition.verify_exact_partition(&graph).unwrap();
        partition.verify_dominance_blocks(&embedding).unwrap();

        let circulation = Circulation::from_partition(3, 3, &partition).unwrap();
        let terminal = graph::min_cost::experiment::solve(circulation.network()).unwrap();
        let recovered = circulation.recover_certified(&terminal).unwrap();
        let reference = oracle::audit(&graph, &partition).unwrap();

        assert_eq!(recovered.flow_value, reference.matching_size);
        assert_eq!(recovered.vertex_cover.size, reference.matching_size);
        for (left, right) in graph.edges() {
            assert!(recovered.vertex_cover.left[left] || recovered.vertex_cover.right[right]);
        }
    }

    #[test]
    fn source_cover_completes_a_formal_polygon_to_its_optimum() {
        let polygon = formal_source_figure_three();
        let analysis = analyze_formal_admissible_family(&polygon).unwrap();
        let embedding =
            DominanceEmbedding::new(&analysis.families.horizontal, &analysis.families.vertical)
                .unwrap();
        let partition = Partition::comparability_theorem_8(&embedding).unwrap();
        partition
            .verify_exact_partition(&analysis.explicit_conflict_graph)
            .unwrap();
        let circulation = Circulation::from_partition(
            analysis.families.horizontal.len(),
            analysis.families.vertical.len(),
            &partition,
        )
        .unwrap();
        let terminal = graph::min_cost::experiment::solve(circulation.network()).unwrap();
        let recovered = circulation.recover_certified(&terminal).unwrap();
        assert_eq!(recovered.flow_value, analysis.explicit_matching.size);

        let selected_horizontal = recovered
            .vertex_cover
            .left
            .iter()
            .map(|covered| !covered)
            .collect::<Vec<_>>();
        let selected_vertical = recovered
            .vertex_cover
            .right
            .iter()
            .map(|covered| !covered)
            .collect::<Vec<_>>();
        let completion = CoordinateCompressedCompletion
            .complete_formal(
                &polygon,
                &analysis.families.horizontal,
                &analysis.families.vertical,
                &selected_horizontal,
                &selected_vertical,
            )
            .unwrap();
        assert_eq!(
            completion.rectangles.len(),
            analysis.optimum_rectangle_count
        );
    }

    #[test]
    fn source_terminal_recovery_maps_to_a_compressed_cover() {
        let partition = complete_two_by_two_partition();
        let circulation = Circulation::from_partition(2, 2, &partition).unwrap();
        let seven_eighths = ExactRatio::new(7, 8).unwrap();
        let mut arc_flows = vec![seven_eighths; circulation.network().arc_count()];
        arc_flows[circulation.return_arc.0] = ExactRatio::new(7, 4).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            circulation.network(),
            &FractionalCirculation {
                cost: circulation.network().fractional_cost(&arc_flows).unwrap(),
                arc_flows,
            },
            ExactRatio::new(-2, 1).unwrap(),
            3,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();

        let terminal = Backend
            .recover_terminated(&snapshot, circulation.network())
            .unwrap();
        let recovered = circulation
            .recover_certified(&terminal.rounding.solution)
            .unwrap();
        assert_eq!(recovered.flow_value, 2);
        assert_eq!(recovered.matching.len(), 2);
        assert_eq!(recovered.vertex_cover.size, 2);
    }
}
