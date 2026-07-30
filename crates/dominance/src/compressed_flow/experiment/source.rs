//! Source-flow-compatible compressed-network model and recovery.
//!
//! This module only transforms a verified compressed biclique network and
//! recovers certificates from a caller-supplied, certified terminal flow. It
//! deliberately does not choose a flow or call a permanent flow backend.

use std::collections::VecDeque;

use graph::{
    CirculationArcId, CirculationNetwork, FlowNodeId, MinCostCirculationError, MinCostSolution,
    VertexCover,
    source_flow::{
        Backend, Error as SourceFlowError,
        iteration::{Completion, Driver, Error as IterationError, Factory, Session},
    },
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

/// One completed source-driver run and its recovered compressed certificate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Run {
    /// Exact additive-half completion and every accepted source transition.
    pub completion: Completion,
    /// Recovered matching and Konig cover for this compressed circulation.
    pub solution: Solution,
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

    /// Recovers a matching and Konig cover from one terminated source-flow
    /// session without selecting a reference flow backend.
    ///
    /// `Backend::recover_terminated` first verifies that the session snapshot
    /// certifies this exact circulation network, then checks additive-half
    /// termination and applies the local exact recovery boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when the session snapshot is not certified for this
    /// circulation, has not reached additive-half termination, recovery fails,
    /// or the resulting integral flow cannot recover a compressed certificate.
    pub fn recover_source_session(&self, session: &Session) -> Result<Solution, Error> {
        let terminal = Backend
            .recover_terminated(session.snapshot(), &self.network)
            .map_err(Error::SourceFlow)?;
        self.recover_certified(&terminal.rounding.solution)
    }

    /// Runs a certified source driver and recovers its compressed certificate.
    ///
    /// The driver owns exact projection preparation and cannot reach this
    /// handoff until it certifies additive-half termination. This method only
    /// composes that terminal session with the existing local recovery path;
    /// it does not select a flow or use a reference backend.
    ///
    /// # Errors
    ///
    /// Returns an error when the driver cannot reach additive-half termination
    /// on this exact circulation or terminal recovery cannot reconstruct a
    /// valid compressed matching and cover.
    pub fn run_source<F: Factory>(&self, driver: &mut Driver<F>) -> Result<Run, Error> {
        let completion = driver.run(&self.network).map_err(Error::SourceIteration)?;
        let solution = self.recover_source_session(driver.session())?;
        Ok(Run {
            completion,
            solution,
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
    #[error("source-flow session recovery failed: {0}")]
    SourceFlow(#[source] SourceFlowError),
    #[error("source-flow iteration failed: {0}")]
    SourceIteration(#[source] IterationError),
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
        BipartiteGraph, CertifiedIpmError, CertifiedIpmSnapshot, CirculationNetwork, ExactRatio,
        FixedPointConfig, FlowNodeId, FractionalCirculation, StableEdge, StableMinRatioLedger,
        StableWitness,
        source_flow::{
            Backend,
            iteration::{self, FixedProjectionFactory},
        },
        source_min_ratio::{
            input::Input,
            spanner::{Parameters as SpannerParameters, Snapshot as SpannerSnapshot},
            terminal::Tree as TerminalTree,
        },
        source_spanner::experiment::domain::ExhaustiveDomain,
    };

    use super::{Circulation, Error};
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

    fn single_edge_partition() -> Partition {
        Partition {
            blocks: vec![Block {
                id: BicliqueId(0),
                left: vec![0],
                right: vec![0],
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

    fn ratio(numerator: i128, denominator: i128) -> ExactRatio {
        ExactRatio::new(numerator, denominator).unwrap()
    }

    fn terminal_source_fixture(
        circulation: &Circulation,
    ) -> (CertifiedIpmSnapshot, graph::MinCostSolution) {
        let reference = graph::min_cost::experiment::solve(circulation.network()).unwrap();
        let block_count = i128::try_from(circulation.blocks.len()).unwrap();
        let horizontal_count = i128::try_from(circulation.horizontal_arcs.len()).unwrap();
        let vertical_count = i128::try_from(circulation.vertical_arcs.len()).unwrap();
        let scale = block_count.max(horizontal_count).max(vertical_count).max(1);
        let alpha = ratio(1, 16 * block_count * scale);
        let zero = ratio(0, 1);
        let mut interior = vec![zero; circulation.network().arc_count()];

        for block in &circulation.blocks {
            assert!(!block.left.is_empty());
            assert!(!block.right.is_empty());
            let left_count = i128::try_from(block.left.len()).unwrap();
            let right_count = i128::try_from(block.right.len()).unwrap();
            let per_right = alpha.checked_mul(ratio(left_count, right_count)).unwrap();
            for (&left, &arc) in block.left.iter().zip(&block.left_arcs) {
                interior[arc.0] = interior[arc.0].checked_add(alpha).unwrap();
                let outer = circulation.horizontal_arcs[left];
                interior[outer.0] = interior[outer.0].checked_add(alpha).unwrap();
            }
            for (&right, &arc) in block.right.iter().zip(&block.right_arcs) {
                interior[arc.0] = interior[arc.0].checked_add(per_right).unwrap();
                let outer = circulation.vertical_arcs[right];
                interior[outer.0] = interior[outer.0].checked_add(per_right).unwrap();
            }
        }
        let return_flow = circulation
            .horizontal_arcs
            .iter()
            .copied()
            .try_fold(zero, |sum, arc| sum.checked_add(interior[arc.0]))
            .unwrap();
        interior[circulation.return_arc.0] = return_flow;
        for (arc, flow) in interior.iter().enumerate() {
            assert!(flow.is_positive(), "arc {arc} has no strict interior flow");
        }
        let interior_cost = circulation.network().fractional_cost(&interior).unwrap();
        circulation
            .network()
            .verify_fractional_solution(&FractionalCirculation {
                arc_flows: interior.clone(),
                cost: interior_cost,
            })
            .unwrap();

        let optimum = ratio(reference.cost, 1);
        let difference = interior_cost.checked_sub(optimum).unwrap();
        assert!(difference.is_positive());
        let candidate_epsilon = ratio(
            difference.denominator(),
            difference.numerator().checked_mul(128).unwrap(),
        );
        let epsilon = if ratio(1, 1).at_least(candidate_epsilon).unwrap() {
            candidate_epsilon
        } else {
            ratio(1, 2)
        };
        let retained = ratio(1, 1).checked_sub(epsilon).unwrap();
        let arc_flows = reference
            .arc_flows
            .iter()
            .copied()
            .zip(interior)
            .map(|(optimal, strictly_interior)| {
                ratio(optimal, 1)
                    .checked_mul(retained)
                    .unwrap()
                    .checked_add(strictly_interior.checked_mul(epsilon).unwrap())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let cost = circulation.network().fractional_cost(&arc_flows).unwrap();
        let snapshot = CertifiedIpmSnapshot::evaluate(
            circulation.network(),
            &FractionalCirculation { arc_flows, cost },
            optimum,
            1_024,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();
        snapshot
            .certify_additive_half_termination(circulation.network())
            .unwrap();
        (snapshot, reference)
    }

    fn terminal_driver() -> impl iteration::Factory {
        |_: &CertifiedIpmSnapshot, _: &graph::CirculationNetwork| {
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        }
    }

    fn source_ledger() -> StableMinRatioLedger {
        StableMinRatioLedger::new(
            2,
            vec![
                StableEdge {
                    from: FlowNodeId(0),
                    to: FlowNodeId(1),
                    gradient: -1,
                    length: 1,
                },
                StableEdge {
                    from: FlowNodeId(1),
                    to: FlowNodeId(0),
                    gradient: 0,
                    length: 1,
                },
            ],
            ratio(1, 4),
            ratio(1, 2),
            StableWitness {
                circulation: vec![1, 1],
                upper_bounds: vec![1, 1],
            },
        )
        .unwrap()
    }

    fn source_spanner_parameters() -> SpannerParameters {
        SpannerParameters {
            root: FlowNodeId(0),
            maximum_absolute_exponent: 7,
            phi: ratio(1, 2),
            domain: ExhaustiveDomain { maximum_nodes: 8 },
            maximum_hops: 4,
            maximum_vertex_congestion: 100,
            maximum_rounds: 1,
        }
    }

    fn nonterminal_projection_input(circulation: &Circulation) -> Input {
        let mut gradients = vec![ratio(0, 1); circulation.network().arc_count()];
        gradients[circulation.return_arc.0] = ratio(-400, 3);
        let lengths = vec![
            ratio(11, 4),
            ratio(4, 1),
            ratio(8, 1),
            ratio(5, 1),
            ratio(8, 1),
        ];
        assert_eq!(lengths.len(), circulation.network().arc_count());
        Input::new(circulation.network(), &gradients, &lengths, &lengths).unwrap()
    }

    fn nonterminal_source_fixture(circulation: &Circulation) -> (CertifiedIpmSnapshot, Input) {
        let quarter = ratio(1, 4);
        let arc_flows = vec![quarter; circulation.network().arc_count()];
        let snapshot = CertifiedIpmSnapshot::evaluate(
            circulation.network(),
            &FractionalCirculation {
                cost: circulation.network().fractional_cost(&arc_flows).unwrap(),
                arc_flows,
            },
            ratio(-1, 1),
            2,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();
        assert_eq!(
            snapshot.certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
        (snapshot, nonterminal_projection_input(circulation))
    }

    #[test]
    fn recovers_reference_flow_value_matching_and_cover() {
        let (graph, partition) = two_by_two_partition();
        let circulation = Circulation::from_partition(2, 2, &partition).unwrap();
        let (snapshot, _) = terminal_source_fixture(&circulation);
        let mut driver = Backend
            .begin_source_iterations(snapshot, terminal_driver(), 0)
            .unwrap();
        let run = circulation.run_source(&mut driver).unwrap();
        assert!(run.completion.records.is_empty());
        let recovered = run.solution;
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
        let (snapshot, _) = terminal_source_fixture(&circulation);
        let mut driver = Backend
            .begin_source_iterations(snapshot, terminal_driver(), 0)
            .unwrap();
        let run = circulation.run_source(&mut driver).unwrap();
        assert!(run.completion.records.is_empty());
        let recovered = run.solution;
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
        let (snapshot, _) = terminal_source_fixture(&circulation);
        let mut driver = Backend
            .begin_source_iterations(snapshot, terminal_driver(), 0)
            .unwrap();
        let run = circulation.run_source(&mut driver).unwrap();
        assert!(run.completion.records.is_empty());
        let recovered = run.solution;
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

        let factory = |_: &CertifiedIpmSnapshot, _: &graph::CirculationNetwork| {
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        };
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 0)
            .unwrap();
        let run = circulation.run_source(&mut driver).unwrap();
        assert!(run.completion.records.is_empty());
        let recovered = run.solution;
        assert_eq!(recovered.flow_value, 2);
        assert_eq!(recovered.matching.len(), 2);
        assert_eq!(recovered.vertex_cover.size, 2);
    }

    #[test]
    fn source_driver_rejects_a_terminal_snapshot_for_another_circulation() {
        let partition = complete_two_by_two_partition();
        let circulation = Circulation::from_partition(2, 2, &partition).unwrap();
        let mut other = CirculationNetwork::new(2);
        other.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        other.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let quarter = ratio(1, 4);
        let snapshot = CertifiedIpmSnapshot::evaluate(
            &other,
            &FractionalCirculation {
                arc_flows: vec![quarter; 2],
                cost: quarter,
            },
            ratio(0, 1),
            4,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
        .unwrap();
        let mut driver = Backend
            .begin_source_iterations(snapshot, terminal_driver(), 0)
            .unwrap();

        assert_eq!(
            circulation.run_source(&mut driver),
            Err(Error::SourceIteration(iteration::Error::Ipm(
                CertifiedIpmError::NetworkMismatch
            )))
        );
    }

    #[test]
    fn records_a_nonterminal_compressed_source_update_with_an_explicit_limit_witness() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(1, 1, &partition).unwrap();
        let (snapshot, input) = nonterminal_source_fixture(&circulation);
        let expected_input = input.clone();
        let factory = FixedProjectionFactory::new(
            input,
            source_ledger(),
            source_spanner_parameters(),
            ratio(1, 2),
        );
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 1)
            .unwrap();

        assert_eq!(
            circulation.run_source(&mut driver),
            Err(Error::SourceIteration(iteration::Error::IterationLimit {
                maximum_iterations: 1,
            }))
        );
        assert_eq!(driver.factory().preparation_count(), 1);
        assert_eq!(driver.records().len(), 1);
        let record = &driver.records()[0];
        assert_eq!(record.sequence, 0);
        assert_eq!(record.input, expected_input);
        assert_eq!(
            record.approximation.edge_count,
            circulation.network().arc_count()
        );
        assert_eq!(
            record.approximation.factor_two_length_checks,
            u64::try_from(circulation.network().arc_count()).unwrap()
        );
        assert_eq!(
            record.approximation.scaled_gradient_checks,
            u64::try_from(circulation.network().arc_count()).unwrap()
        );
        assert!(
            record
                .selected
                .step
                .direction
                .iter()
                .any(|coordinate| !coordinate.is_zero())
        );
        assert_eq!(driver.session().snapshot().update_metrics().iterations, 1);
        assert_eq!(
            driver
                .session()
                .snapshot()
                .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
    }

    #[test]
    fn rebuilds_and_recertifies_the_compressed_projection_for_each_nonterminal_snapshot() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(1, 1, &partition).unwrap();
        let (snapshot, input) = nonterminal_source_fixture(&circulation);
        let factory = FixedProjectionFactory::new(
            input,
            source_ledger(),
            source_spanner_parameters(),
            ratio(1, 2),
        );
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 2)
            .unwrap();

        assert_eq!(
            circulation.run_source(&mut driver),
            Err(Error::SourceIteration(iteration::Error::IterationLimit {
                maximum_iterations: 2,
            }))
        );
        assert_eq!(driver.factory().preparation_count(), 2);
        assert_eq!(driver.records().len(), 2);
        assert_eq!(driver.records()[0].sequence, 0);
        assert_eq!(driver.records()[1].sequence, 1);
        assert_ne!(driver.records()[0].snapshot, driver.records()[1].snapshot);
        assert_eq!(
            driver.records()[0].approximation.edge_count,
            circulation.network().arc_count()
        );
        assert_eq!(
            driver.records()[1].approximation.edge_count,
            circulation.network().arc_count()
        );
        assert_eq!(driver.session().snapshot().update_metrics().iterations, 2);
        assert_eq!(
            driver
                .session()
                .snapshot()
                .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
    }
}
