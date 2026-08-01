//! Source-flow-compatible compressed-network model and recovery.
//!
//! This module only transforms a verified compressed biclique network and
//! recovers certificates from a caller-supplied, certified terminal flow. It
//! deliberately does not choose a flow or call a permanent flow backend.

use std::collections::VecDeque;

use graph::{
    CirculationArcId, CirculationNetwork, ExactRatio, FixedPointConfig, FlowNodeId,
    MinCostCirculationError, MinCostSolution, VertexCover,
    source_flow::{
        Backend, Error as SourceFlowError,
        iteration::{
            Completion, Driver, Error as IterationError, Factory, PotentialBudget, Session,
        },
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
    horizontal_arcs: Vec<Option<CirculationArcId>>,
    blocks: Vec<Block>,
    vertical_arcs: Vec<Option<CirculationArcId>>,
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

/// One source run started from a caller-supplied inclusive cost target.
///
/// The target remains observable at this boundary because it is a checked
/// precondition of the Appendix B.1 initial point. A successful run recovers a
/// cost at most this target; it is not a target-search policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetRun {
    /// The caller-supplied inclusive integral target used by the source driver.
    pub target: i128,
    /// Exact additive-half completion and every accepted source transition.
    pub completion: Completion,
    /// Recovered matching and Konig cover for this compressed circulation.
    pub solution: Solution,
}

/// Exact evidence that a caller-supplied cover certifies `F_opt > target`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverBelowProof {
    /// The target that the optimal cost strictly exceeds.
    pub target: i128,
    /// The verified vertex-cover size bounding the maximum matching.
    pub cover_size: usize,
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
        let (active_horizontal_indices, active_vertical_indices) =
            active_endpoint_indices(horizontal_count, vertical_count, partition)?;
        let node_count = 2_usize
            .checked_add(active_horizontal_indices.len())
            .and_then(|value| value.checked_add(partition.blocks.len()))
            .and_then(|value| value.checked_add(active_vertical_indices.len()))
            .ok_or(Error::NetworkSizeOverflow)?;
        let source = FlowNodeId(0);
        let horizontal_start = 1;
        let block_start = horizontal_start + active_horizontal_indices.len();
        let vertical_start = block_start + partition.blocks.len();
        let sink = FlowNodeId(node_count - 1);
        let mut horizontal_nodes = vec![None; horizontal_count];
        for (local, original) in active_horizontal_indices.iter().copied().enumerate() {
            horizontal_nodes[original] = Some(FlowNodeId(horizontal_start + local));
        }
        let block_nodes = (0..partition.blocks.len())
            .map(|index| FlowNodeId(block_start + index))
            .collect::<Vec<_>>();
        let mut vertical_nodes = vec![None; vertical_count];
        for (local, original) in active_vertical_indices.iter().copied().enumerate() {
            vertical_nodes[original] = Some(FlowNodeId(vertical_start + local));
        }
        let maximum_matching = i128::try_from(
            active_horizontal_indices
                .len()
                .min(active_vertical_indices.len()),
        )
        .map_err(|_| Error::CapacityOverflow)?;
        let internal_capacity = maximum_matching
            .checked_add(1)
            .ok_or(Error::CapacityOverflow)?;

        let mut network = CirculationNetwork::new(node_count);
        let mut horizontal_arcs = vec![None; horizontal_count];
        for (index, node) in horizontal_nodes.iter().enumerate() {
            if let Some(node) = node {
                horizontal_arcs[index] = Some(network.add_arc(source, *node, 1, 0)?);
            }
        }
        let mut blocks = Vec::with_capacity(partition.blocks.len());
        for (index, block) in partition.blocks.iter().enumerate() {
            let block_node = block_nodes[index];
            let left_arcs = block
                .left
                .iter()
                .copied()
                .map(|left| {
                    let node = horizontal_nodes
                        .get(left)
                        .and_then(|node| *node)
                        .ok_or(Error::BicliqueEndpointOutOfBounds)?;
                    Ok(network.add_arc(node, block_node, internal_capacity, 0)?)
                })
                .collect::<Result<Vec<_>, Error>>()?;
            let right_arcs = block
                .right
                .iter()
                .copied()
                .map(|right| {
                    let node = vertical_nodes
                        .get(right)
                        .and_then(|node| *node)
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
        let mut vertical_arcs = vec![None; vertical_count];
        for (index, node) in vertical_nodes.iter().enumerate() {
            if let Some(node) = node {
                vertical_arcs[index] = Some(network.add_arc(*node, sink, 1, 0)?);
            }
        }
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

    /// Runs a source driver under its checked potential-reduction budget.
    ///
    /// The budget proves a finite additive-half horizon only conditional on
    /// successfully preparing every fresh source projection with one fixed
    /// `kappa`. It is not a general coordinate-maintenance policy and does not
    /// enable the experimental backend's complete-solver gate.
    ///
    /// # Errors
    ///
    /// Returns an error when source projection preparation, the potential
    /// budget, additive-half termination, or compressed recovery rejects.
    pub fn run_source_with_potential_budget<F: Factory>(
        &self,
        driver: &mut Driver<F>,
        budget: &PotentialBudget,
    ) -> Result<Run, Error> {
        let completion = driver
            .run_with_potential_budget(&self.network, budget)
            .map_err(Error::SourceIteration)?;
        let solution = self.recover_source_session(driver.session())?;
        Ok(Run {
            completion,
            solution,
        })
    }

    /// Builds and runs the Appendix B.1 source path for one caller-supplied
    /// inclusive integral target.
    ///
    /// This entry never queries an Oracle, derives a target from a lower bound,
    /// or interprets a source failure as evidence about a different target. It
    /// only recovers the original circulation after source-flow has checked
    /// that terminal recovery returns a cost at most this target.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied target cannot certify a strict
    /// augmented initial point, source iteration cannot terminate, terminal
    /// recovery exceeds the target, or the recovered circulation cannot decode to a
    /// matching and Konig cover.
    pub fn run_with_target<F: Factory>(
        &self,
        target: i128,
        maximum_abs_input: i128,
        fixed_point_config: FixedPointConfig,
        kappa: ExactRatio,
        factory: F,
    ) -> Result<TargetRun, Error> {
        let mut driver = Backend
            .begin_with_target(
                &self.network,
                target,
                maximum_abs_input,
                fixed_point_config,
                kappa,
                factory,
            )
            .map_err(Error::SourceFlow)?;
        let completed = driver.run().map_err(Error::SourceFlow)?;
        let solution = self.recover_certified(&completed.recovered.original)?;
        Ok(TargetRun {
            target: completed.target,
            completion: completed.completion,
            solution,
        })
    }

    /// Certifies `F_opt > target` from a caller-supplied vertex cover.
    ///
    /// The compressed circulation encodes `F_opt = -max_matching` through its
    /// negative return arc. By Konig's theorem a vertex cover of size `c`
    /// bounds `max_matching <= c`, so a cover with `c < -target` certifies
    /// `max_matching < -target`, hence `F_opt > target`. The cover is supplied
    /// and verified exactly against the immutable biclique partition; no
    /// reference solver constructs it. A certificate that does not strictly
    /// exceed the target, or that omits a compressed conflict edge, is an
    /// explicit rejection rather than an infeasibility decision.
    ///
    /// # Errors
    ///
    /// Returns an error when the cover dimensions do not match, the declared
    /// size does not match the recomputed size, a compressed biclique edge is
    /// uncovered, or the verified cover size does not prove `F_opt > target`.
    pub fn certify_cover_below(
        &self,
        cover: &VertexCover,
        target: i128,
    ) -> Result<CoverBelowProof, Error> {
        if cover.left.len() != self.horizontal_arcs.len()
            || cover.right.len() != self.vertical_arcs.len()
        {
            return Err(Error::CoverCertificateDimensionMismatch);
        }
        let recomputed = cover.left.iter().filter(|selected| **selected).count()
            + cover.right.iter().filter(|selected| **selected).count();
        if recomputed != cover.size {
            return Err(Error::CoverCertificateSizeMismatch {
                declared: cover.size,
                recomputed,
            });
        }
        for block in &self.blocks {
            for &left_endpoint in &block.left {
                for &right_endpoint in &block.right {
                    if !cover.left[left_endpoint] && !cover.right[right_endpoint] {
                        return Err(Error::CoverCertificateUncoveredEdge {
                            left: left_endpoint,
                            right: right_endpoint,
                        });
                    }
                }
            }
        }
        let threshold = target.checked_neg().ok_or(Error::TargetOverflow)?;
        let cover_i128 =
            i128::try_from(cover.size).map_err(|_| Error::CoverCertificateSizeOverflow)?;
        if cover_i128 >= threshold {
            return Err(Error::CoverCertificateInsufficient {
                target,
                cover_size: cover.size,
            });
        }
        Ok(CoverBelowProof {
            target,
            cover_size: cover.size,
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
        for (left, arc) in self.horizontal_arcs.iter().enumerate() {
            let Some(arc) = arc else {
                continue;
            };
            let expected = i128::from(left_match[left].is_some());
            if Self::flow(solution, *arc)? != expected {
                return Err(Error::OuterFlowMismatch);
            }
        }
        for (right, arc) in self.vertical_arcs.iter().enumerate() {
            let Some(arc) = arc else {
                continue;
            };
            let expected = i128::from(right_match[right].is_some());
            if Self::flow(solution, *arc)? != expected {
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

fn active_endpoint_indices(
    horizontal_count: usize,
    vertical_count: usize,
    partition: &Partition,
) -> Result<(Vec<usize>, Vec<usize>), Error> {
    let mut active_horizontal = vec![false; horizontal_count];
    let mut active_vertical = vec![false; vertical_count];
    for block in &partition.blocks {
        for &left in &block.left {
            *active_horizontal
                .get_mut(left)
                .ok_or(Error::BicliqueEndpointOutOfBounds)? = true;
        }
        for &right in &block.right {
            *active_vertical
                .get_mut(right)
                .ok_or(Error::BicliqueEndpointOutOfBounds)? = true;
        }
    }
    let horizontal = active_horizontal
        .iter()
        .enumerate()
        .filter_map(|(index, active)| (*active).then_some(index))
        .collect();
    let vertical = active_vertical
        .iter()
        .enumerate()
        .filter_map(|(index, active)| (*active).then_some(index))
        .collect();
    Ok((horizontal, vertical))
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
    #[error("cover-certificate dimensions do not match the compressed network")]
    CoverCertificateDimensionMismatch,
    #[error("cover-certificate declared size {declared} differs from recomputed size {recomputed}")]
    CoverCertificateSizeMismatch { declared: usize, recomputed: usize },
    #[error("cover certificate omits compressed conflict edge ({left}, {right})")]
    CoverCertificateUncoveredEdge { left: usize, right: usize },
    #[error("target negation overflowed the supported exact domain")]
    TargetOverflow,
    #[error("verified cover size exceeds the supported exact domain")]
    CoverCertificateSizeOverflow,
    #[error("cover size {cover_size} does not prove F_opt > target {target}")]
    CoverCertificateInsufficient { target: i128, cover_size: usize },
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use graph::{
        BipartiteGraph, CertifiedIpmError, CertifiedIpmSnapshot, CirculationArcId,
        CirculationNetwork, ExactRatio, FixedPointConfig, FlowNodeId, FractionalCirculation,
        source_flow::{
            Backend, Error as SourceFlowError,
            iteration::{
                self, DefinitionProjectionFactory, FixedProjectionFactory, PotentialBudget,
                ReciprocalSlackProjectionFactory, ScheduledProjectionFactory,
            },
        },
        source_min_ratio::input::Input,
    };

    use super::{Circulation, Error, Solution};
    use crate::{
        biclique::{Block, Partition},
        compressed_flow::oracle,
        embedding::DominanceEmbedding,
        formal::{FormalAdmissibleAnalysis, analyze_formal_admissible_family},
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

    fn formal_isolated_lattice(mask: u16) -> FormalRectilinearPolygon {
        let points = [
            Point::new(3, 3),
            Point::new(6, 3),
            Point::new(9, 3),
            Point::new(3, 6),
            Point::new(6, 6),
            Point::new(9, 6),
            Point::new(3, 9),
            Point::new(6, 9),
            Point::new(9, 9),
        ];
        assert!(mask > 0);
        FormalRectilinearPolygon::new(
            RectilinearPolygon::new(rectangle(0, 0, 12, 12), vec![]).unwrap(),
            Ornament {
                isolated_points: points
                    .iter()
                    .enumerate()
                    .filter_map(|(index, point)| ((mask & (1 << index)) != 0).then_some(*point))
                    .collect(),
                segments: vec![],
            },
        )
        .unwrap()
    }

    fn ratio(numerator: i128, denominator: i128) -> ExactRatio {
        ExactRatio::new(numerator, denominator).unwrap()
    }

    fn strict_interior_fixture(
        circulation: &Circulation,
    ) -> (graph::MinCostSolution, FractionalCirculation) {
        let reference = graph::min_cost::experiment::solve(circulation.network()).unwrap();
        let block_count = i128::try_from(circulation.blocks.len()).unwrap();
        let horizontal_count = i128::try_from(
            circulation
                .horizontal_arcs
                .iter()
                .filter(|arc| arc.is_some())
                .count(),
        )
        .unwrap();
        let vertical_count = i128::try_from(
            circulation
                .vertical_arcs
                .iter()
                .filter(|arc| arc.is_some())
                .count(),
        )
        .unwrap();
        let scale = block_count.max(horizontal_count).max(vertical_count).max(1);
        let alpha = ratio(1, 16 * block_count * scale);
        let zero = ratio(0, 1);
        let mut interior = vec![zero.clone(); circulation.network().arc_count()];

        for block in &circulation.blocks {
            assert!(!block.left.is_empty());
            assert!(!block.right.is_empty());
            let left_count = i128::try_from(block.left.len()).unwrap();
            let right_count = i128::try_from(block.right.len()).unwrap();
            let per_right = alpha.checked_mul(&ratio(left_count, right_count)).unwrap();
            for (&left, &arc) in block.left.iter().zip(&block.left_arcs) {
                interior[arc.0] = interior[arc.0].checked_add(&alpha).unwrap();
                let outer = circulation.horizontal_arcs[left].unwrap();
                interior[outer.0] = interior[outer.0].checked_add(&alpha).unwrap();
            }
            for (&right, &arc) in block.right.iter().zip(&block.right_arcs) {
                interior[arc.0] = interior[arc.0].checked_add(&per_right).unwrap();
                let outer = circulation.vertical_arcs[right].unwrap();
                interior[outer.0] = interior[outer.0].checked_add(&per_right).unwrap();
            }
        }
        let return_flow = circulation
            .horizontal_arcs
            .iter()
            .flatten()
            .copied()
            .try_fold(zero, |sum, arc| sum.checked_add(&interior[arc.0]))
            .unwrap();
        interior[circulation.return_arc.0] = return_flow;
        for (arc, flow) in interior.iter().enumerate() {
            assert!(flow.is_positive(), "arc {arc} has no strict interior flow");
        }
        let interior = FractionalCirculation {
            cost: circulation.network().fractional_cost(&interior).unwrap(),
            arc_flows: interior,
        };
        circulation
            .network()
            .verify_fractional_solution(&interior)
            .unwrap();

        (reference, interior)
    }

    fn interpolation_snapshot(
        circulation: &Circulation,
        reference: &graph::MinCostSolution,
        interior: &FractionalCirculation,
        epsilon: &ExactRatio,
    ) -> CertifiedIpmSnapshot {
        interpolation_snapshot_with_config(
            circulation,
            reference,
            interior,
            epsilon,
            FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
        )
    }

    fn interpolation_snapshot_with_config(
        circulation: &Circulation,
        reference: &graph::MinCostSolution,
        interior: &FractionalCirculation,
        epsilon: &ExactRatio,
        fixed_point_config: FixedPointConfig,
    ) -> CertifiedIpmSnapshot {
        let retained = ratio(1, 1).checked_sub(epsilon).unwrap();
        let arc_flows = reference
            .arc_flows
            .iter()
            .copied()
            .zip(&interior.arc_flows)
            .map(|(optimal, strictly_interior)| {
                ratio(optimal, 1)
                    .checked_mul(&retained)
                    .unwrap()
                    .checked_add(&strictly_interior.checked_mul(epsilon).unwrap())
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let cost = circulation.network().fractional_cost(&arc_flows).unwrap();
        CertifiedIpmSnapshot::evaluate(
            circulation.network(),
            &FractionalCirculation { arc_flows, cost },
            ratio(reference.cost, 1),
            1_024,
            fixed_point_config,
        )
        .unwrap()
    }

    fn boundary_source_fixture(circulation: &Circulation) -> CertifiedIpmSnapshot {
        let (reference, interior) = strict_interior_fixture(circulation);
        let denominator = 1_000_000;
        let mut terminal = 1;
        let mut nonterminal = denominator - 1;
        assert!(
            interpolation_snapshot(
                circulation,
                &reference,
                &interior,
                &ratio(terminal, denominator),
            )
            .certify_additive_half_termination(circulation.network())
            .is_ok()
        );
        assert_eq!(
            interpolation_snapshot(
                circulation,
                &reference,
                &interior,
                &ratio(nonterminal, denominator),
            )
            .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
        while nonterminal - terminal > 1 {
            let middle = terminal + (nonterminal - terminal) / 2;
            let snapshot = interpolation_snapshot(
                circulation,
                &reference,
                &interior,
                &ratio(middle, denominator),
            );
            if snapshot
                .certify_additive_half_termination(circulation.network())
                .is_ok()
            {
                terminal = middle;
            } else {
                nonterminal = middle;
            }
        }
        interpolation_snapshot(
            circulation,
            &reference,
            &interior,
            &ratio(nonterminal, denominator),
        )
    }

    /// Builds a near-boundary strictly nonterminal source starting point for a
    /// population differential. A lower-precision rational interval search
    /// only locates a candidate. The returned snapshot always uses the normal
    /// source configuration and independently certifies nontermination.
    fn population_source_fixture(circulation: &Circulation) -> CertifiedIpmSnapshot {
        let (reference, interior) = strict_interior_fixture(circulation);
        let denominator = 1_i128 << 20;
        let coarse_config = FixedPointConfig::source_bounded(1 << 20, 48, 24, 3).unwrap();
        let mut terminal = 1;
        let mut nonterminal = denominator / 2;

        while nonterminal - terminal > 1 {
            let middle = terminal + (nonterminal - terminal) / 2;
            let snapshot = interpolation_snapshot_with_config(
                circulation,
                &reference,
                &interior,
                &ratio(middle, denominator),
                coarse_config,
            );
            if snapshot
                .certify_additive_half_termination(circulation.network())
                .is_ok()
            {
                terminal = middle;
            } else {
                nonterminal = middle;
            }
        }

        let snapshot = interpolation_snapshot(
            circulation,
            &reference,
            &interior,
            &ratio(nonterminal, denominator),
        );
        if matches!(
            snapshot.certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        ) {
            return snapshot;
        }

        // A coarse interval may be too wide near the threshold. In that case,
        // reconstruct the same bracket at the production configuration rather
        // than accepting an uncertified starting point.
        let mut terminal = 1;
        let mut nonterminal = denominator / 2;
        while nonterminal - terminal > 1 {
            let middle = terminal + (nonterminal - terminal) / 2;
            let snapshot = interpolation_snapshot(
                circulation,
                &reference,
                &interior,
                &ratio(middle, denominator),
            );
            if snapshot
                .certify_additive_half_termination(circulation.network())
                .is_ok()
            {
                terminal = middle;
            } else {
                nonterminal = middle;
            }
        }
        let snapshot = interpolation_snapshot(
            circulation,
            &reference,
            &interior,
            &ratio(nonterminal, denominator),
        );
        assert_eq!(
            snapshot.certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
        snapshot
    }

    fn terminal_driver() -> impl iteration::Factory {
        |_: &CertifiedIpmSnapshot, _: &graph::CirculationNetwork| {
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        }
    }

    fn boundary_source_driver(
        snapshot: CertifiedIpmSnapshot,
        maximum_iterations: u64,
    ) -> iteration::Driver<DefinitionProjectionFactory> {
        Backend
            .begin_source_iterations(
                snapshot,
                DefinitionProjectionFactory::new(ratio(1, 2)),
                maximum_iterations,
            )
            .unwrap()
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

    fn nonterminal_successor_projection_input(circulation: &Circulation) -> Input {
        let mut gradients = vec![ratio(0, 1); circulation.network().arc_count()];
        gradients[circulation.return_arc.0] = ratio(-399_999_997, 3_000_000);
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

    fn near_terminal_source_fixture(circulation: &Circulation) -> CertifiedIpmSnapshot {
        let flow = ratio(547_590, 1_000_000);
        let snapshot = CertifiedIpmSnapshot::evaluate(
            circulation.network(),
            &FractionalCirculation {
                cost: circulation
                    .network()
                    .fractional_cost(&vec![flow.clone(); circulation.network().arc_count()])
                    .unwrap(),
                arc_flows: vec![flow; circulation.network().arc_count()],
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
        snapshot
    }

    #[test]
    fn recovers_reference_flow_value_matching_and_cover() {
        let (graph, partition) = two_by_two_partition();
        let circulation = Circulation::from_partition(2, 2, &partition).unwrap();
        let snapshot = boundary_source_fixture(&circulation);
        let mut driver = boundary_source_driver(snapshot, 1);
        let run = circulation.run_source(&mut driver).unwrap();
        assert_eq!(run.completion.records.len(), 1);
        assert_eq!(driver.factory().preparation_count(), 1);
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
        let snapshot = boundary_source_fixture(&circulation);
        let mut driver = boundary_source_driver(snapshot, 1);
        let run = circulation.run_source(&mut driver).unwrap();
        assert_eq!(run.completion.records.len(), 1);
        assert_eq!(driver.factory().preparation_count(), 1);
        let recovered = run.solution;
        let reference = oracle::audit(&graph, &partition).unwrap();

        assert_eq!(recovered.flow_value, reference.matching_size);
        assert_eq!(recovered.vertex_cover.size, reference.matching_size);
        for (left, right) in graph.edges() {
            assert!(recovered.vertex_cover.left[left] || recovered.vertex_cover.right[right]);
        }
    }

    #[derive(Clone, Copy)]
    enum FormalOutputComparison {
        Exact,
        Equivalent,
    }

    /// Complete source-relevant network identity for population fixtures.
    ///
    /// The original chord indices intentionally do not participate: after
    /// `Circulation::from_partition` prunes isolated endpoints, only this
    /// network is consumed by source-flow preparation. Every original mask
    /// still runs a separate source session and certificate recovery.
    #[derive(Debug, Eq, PartialEq)]
    struct PopulationFixtureKey {
        node_count: usize,
        demands: Vec<i128>,
        arcs: Vec<(usize, usize, i128, i128)>,
    }

    fn population_fixture_key(circulation: &Circulation) -> PopulationFixtureKey {
        let network = circulation.network();
        PopulationFixtureKey {
            node_count: network.node_count(),
            demands: network.demands().to_vec(),
            arcs: (0..network.arc_count())
                .map(|index| {
                    let arc = CirculationArcId(index);
                    let (from, to) = network.arc_endpoints(arc).unwrap();
                    let (capacity, cost) = network.arc_capacity_cost(arc).unwrap();
                    (from.0, to.0, capacity, cost)
                })
                .collect(),
        }
    }

    fn cached_population_source_fixture(
        circulation: &Circulation,
        cache: &mut Vec<(PopulationFixtureKey, CertifiedIpmSnapshot)>,
    ) -> CertifiedIpmSnapshot {
        let key = population_fixture_key(circulation);
        if let Some((_, snapshot)) = cache.iter().find(|(candidate, _)| *candidate == key) {
            return snapshot.clone();
        }
        let snapshot = population_source_fixture(circulation);
        cache.push((key, snapshot.clone()));
        snapshot
    }

    fn assert_recovered_formal_certificate(
        analysis: &FormalAdmissibleAnalysis,
        recovered: &Solution,
        label: &str,
    ) -> (Vec<bool>, Vec<bool>) {
        assert_eq!(
            recovered.flow_value, analysis.explicit_matching.size,
            "{label}"
        );
        assert_eq!(recovered.matching.len(), recovered.flow_value, "{label}");
        assert_eq!(recovered.vertex_cover.size, recovered.flow_value, "{label}");
        assert_eq!(
            recovered.vertex_cover.left.len(),
            analysis.families.horizontal.len(),
            "{label}"
        );
        assert_eq!(
            recovered.vertex_cover.right.len(),
            analysis.families.vertical.len(),
            "{label}"
        );
        let mut matched_horizontal = vec![false; analysis.families.horizontal.len()];
        let mut matched_vertical = vec![false; analysis.families.vertical.len()];
        for (horizontal, vertical) in &recovered.matching {
            assert!(
                analysis
                    .explicit_conflict_graph
                    .edges()
                    .any(|edge| edge == (*horizontal, *vertical)),
                "{label}"
            );
            assert!(!matched_horizontal[*horizontal], "{label}");
            assert!(!matched_vertical[*vertical], "{label}");
            matched_horizontal[*horizontal] = true;
            matched_vertical[*vertical] = true;
        }
        for (horizontal, vertical) in analysis.explicit_conflict_graph.edges() {
            assert!(
                recovered.vertex_cover.left[horizontal] || recovered.vertex_cover.right[vertical],
                "{label}"
            );
        }

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
        assert_eq!(
            selected_horizontal
                .iter()
                .filter(|selected| **selected)
                .count()
                + selected_vertical
                    .iter()
                    .filter(|selected| **selected)
                    .count(),
            analysis.effective_number,
            "{label}"
        );
        for (horizontal, vertical) in analysis.explicit_conflict_graph.edges() {
            assert!(
                !selected_horizontal[horizontal] || !selected_vertical[vertical],
                "{label}"
            );
        }
        (selected_horizontal, selected_vertical)
    }

    fn assert_source_formal_differential(
        polygon: &FormalRectilinearPolygon,
        label: &str,
        comparison: FormalOutputComparison,
        maximum_iterations: u64,
        population_fixture_cache: &mut Vec<(PopulationFixtureKey, CertifiedIpmSnapshot)>,
    ) -> u64 {
        let analysis = analyze_formal_admissible_family(polygon).unwrap();
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
        let snapshot = match comparison {
            FormalOutputComparison::Exact => boundary_source_fixture(&circulation),
            FormalOutputComparison::Equivalent => {
                cached_population_source_fixture(&circulation, population_fixture_cache)
            }
        };
        let mut driver = boundary_source_driver(snapshot, maximum_iterations);
        let run = circulation
            .run_source(&mut driver)
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        assert!(!run.completion.records.is_empty(), "{label}");
        let update_count = u64::try_from(run.completion.records.len()).unwrap();
        assert!(update_count <= maximum_iterations, "{label}");
        assert_eq!(
            driver.factory().preparation_count(),
            update_count,
            "{label}"
        );
        let recovered = run.solution;
        let (selected_horizontal, selected_vertical) =
            assert_recovered_formal_certificate(&analysis, &recovered, label);
        let completion = CoordinateCompressedCompletion
            .complete_formal(
                polygon,
                &analysis.families.horizontal,
                &analysis.families.vertical,
                &selected_horizontal,
                &selected_vertical,
            )
            .unwrap();
        let reference_completion = CoordinateCompressedCompletion
            .complete_formal(
                polygon,
                &analysis.families.horizontal,
                &analysis.families.vertical,
                &analysis.selected_horizontal,
                &analysis.selected_vertical,
            )
            .unwrap();
        assert_eq!(
            completion.rectangles.len(),
            analysis.optimum_rectangle_count,
            "{label}"
        );
        if matches!(comparison, FormalOutputComparison::Exact) {
            let expected_matching = analysis
                .explicit_matching
                .left_to_right
                .iter()
                .enumerate()
                .filter_map(|(left, right)| right.map(|right| (left, right)))
                .collect::<Vec<_>>();
            assert_eq!(recovered.matching, expected_matching, "{label}");
            assert_eq!(
                recovered.vertex_cover, analysis.explicit_vertex_cover,
                "{label}"
            );
            assert_eq!(selected_horizontal, analysis.selected_horizontal, "{label}");
            assert_eq!(selected_vertical, analysis.selected_vertical, "{label}");
            assert_eq!(
                completion.rectangles, reference_completion.rectangles,
                "{label}"
            );
        }
        update_count
    }

    #[test]
    fn source_cover_completes_a_formal_polygon_to_its_optimum() {
        let mut population_fixture_cache = Vec::new();
        assert_eq!(
            assert_source_formal_differential(
                &formal_source_figure_three(),
                "figure-three",
                FormalOutputComparison::Exact,
                1,
                &mut population_fixture_cache,
            ),
            1
        );
    }

    #[test]
    fn source_cover_matches_a_formal_isolated_lattice_population() {
        let mut exercised = 0;
        let mut maximum_updates = 0;
        let mut population_fixture_cache = Vec::new();
        for mask in 1_u16..1 << 9 {
            let polygon = formal_isolated_lattice(mask);
            let analysis = analyze_formal_admissible_family(&polygon).unwrap();
            if analysis.families.horizontal.is_empty()
                || analysis.families.vertical.is_empty()
                || analysis.explicit_conflict_graph.edges().next().is_none()
            {
                continue;
            }
            let updates = assert_source_formal_differential(
                &polygon,
                &format!("mask-{mask}"),
                FormalOutputComparison::Equivalent,
                8,
                &mut population_fixture_cache,
            );
            maximum_updates = maximum_updates.max(updates);
            exercised += 1;
        }
        assert!(!population_fixture_cache.is_empty());
        assert_eq!(exercised, 410);
        assert_eq!(maximum_updates, 2);
    }

    #[test]
    fn prunes_isolated_outer_endpoints_but_recovers_original_cover_dimensions() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(2, 3, &partition).unwrap();
        assert_eq!(
            circulation.horizontal_arcs.iter().flatten().count(),
            1,
            "only the participating horizontal endpoint belongs to the circulation"
        );
        assert_eq!(
            circulation.vertical_arcs.iter().flatten().count(),
            1,
            "only the participating vertical endpoint belongs to the circulation"
        );
        assert_eq!(circulation.network().arc_count(), 5);

        let (reference, _) = strict_interior_fixture(&circulation);
        let recovered = circulation.recover_certified(&reference).unwrap();

        assert_eq!(recovered.matching, vec![(0, 0)]);
        assert_eq!(recovered.vertex_cover.left, vec![true, false]);
        assert_eq!(recovered.vertex_cover.right, vec![false, false, false]);
        assert_eq!(recovered.vertex_cover.size, 1);
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
                arc_flows: vec![quarter.clone(); 2],
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
        let factory = FixedProjectionFactory::new(input, ratio(1, 2));
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
        let factory = FixedProjectionFactory::new(input, ratio(1, 2));
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

    #[test]
    fn scheduled_coordinates_recertify_distinct_compressed_successor_snapshots() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(1, 1, &partition).unwrap();
        let (snapshot, initial) = nonterminal_source_fixture(&circulation);
        let successor = nonterminal_successor_projection_input(&circulation);
        assert_ne!(initial, successor);
        let factory =
            ScheduledProjectionFactory::new(vec![initial.clone(), successor.clone()], ratio(1, 2))
                .unwrap();
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
        assert_eq!(driver.factory().remaining_count(), 0);
        assert_eq!(driver.records()[0].input, initial);
        assert_eq!(driver.records()[1].input, successor);
        assert_ne!(driver.records()[0].snapshot, driver.records()[1].snapshot);
        assert_eq!(driver.session().snapshot().update_metrics().iterations, 2);
        assert_eq!(
            driver
                .session()
                .snapshot()
                .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
    }

    #[test]
    fn reciprocal_slack_coordinates_rebuild_each_compressed_successor_snapshot() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(1, 1, &partition).unwrap();
        let (snapshot, _) = nonterminal_source_fixture(&circulation);
        let factory = ReciprocalSlackProjectionFactory::new(ratio(1, 2));
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
        assert_ne!(driver.records()[0].input, driver.records()[1].input);
        assert_ne!(driver.records()[0].snapshot, driver.records()[1].snapshot);
        assert_eq!(
            driver
                .session()
                .snapshot()
                .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
    }

    #[test]
    fn reciprocal_slack_coordinates_survive_multiple_structural_successors() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(1, 1, &partition).unwrap();
        let (snapshot, _) = nonterminal_source_fixture(&circulation);
        let factory = ReciprocalSlackProjectionFactory::new(ratio(1, 2));
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 3)
            .unwrap();

        assert_eq!(
            circulation.run_source(&mut driver),
            Err(Error::SourceIteration(iteration::Error::IterationLimit {
                maximum_iterations: 3,
            }))
        );
        assert_eq!(driver.factory().preparation_count(), 3);
        assert_eq!(driver.records().len(), 3);
        assert!(
            driver
                .records()
                .windows(2)
                .all(|pair| pair[0].snapshot != pair[1].snapshot && pair[0].input != pair[1].input)
        );
        assert_eq!(
            driver
                .session()
                .snapshot()
                .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
    }

    #[test]
    fn reciprocal_slack_coordinates_continue_past_the_former_exact_scoring_overflow() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(1, 1, &partition).unwrap();
        let (snapshot, _) = nonterminal_source_fixture(&circulation);
        let factory = ReciprocalSlackProjectionFactory::new(ratio(1, 2));
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 64)
            .unwrap();

        assert_eq!(
            circulation.run_source(&mut driver),
            Err(Error::SourceIteration(iteration::Error::IterationLimit {
                maximum_iterations: 64,
            }))
        );
        assert_eq!(driver.factory().preparation_count(), 64);
        assert_eq!(driver.records().len(), 64);
        assert!(driver.records().windows(2).all(|pair| {
            pair[0].snapshot != pair[1].snapshot && pair[0].input != pair[1].input
        }));
        assert_eq!(
            driver
                .session()
                .snapshot()
                .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
    }

    #[test]
    fn definition_coordinates_rebuild_across_multiple_nonterminal_successors() {
        let partition = single_edge_partition();
        let circulation = Circulation::from_partition(1, 1, &partition).unwrap();
        let (snapshot, _) = nonterminal_source_fixture(&circulation);
        let factory = DefinitionProjectionFactory::new(ratio(1, 2));
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 64)
            .unwrap();

        assert_eq!(
            circulation.run_source(&mut driver),
            Err(Error::SourceIteration(iteration::Error::IterationLimit {
                maximum_iterations: 64,
            }))
        );
        assert_eq!(driver.factory().preparation_count(), 64);
        assert_eq!(driver.records().len(), 64);
        assert!(driver.records().windows(2).all(|pair| {
            pair[0].snapshot != pair[1].snapshot && pair[0].input != pair[1].input
        }));
        assert_eq!(
            driver
                .session()
                .snapshot()
                .certify_additive_half_termination(circulation.network()),
            Err(CertifiedIpmError::NotAtAdditiveHalfBoundary)
        );
    }

    #[test]
    fn recovers_a_single_edge_compressed_solution_after_one_nonterminal_source_update() {
        let circulation = Circulation::from_partition(1, 1, &single_edge_partition()).unwrap();
        let snapshot = near_terminal_source_fixture(&circulation);
        let factory = ReciprocalSlackProjectionFactory::new(ratio(1, 2));
        let mut driver = Backend
            .begin_source_iterations(snapshot, factory, 1)
            .unwrap();

        let run = circulation.run_source(&mut driver).unwrap();

        assert_eq!(run.completion.records.len(), 1);
        assert_eq!(driver.factory().preparation_count(), 1);
        assert_eq!(driver.records().len(), 1);
        assert!(
            driver.records()[0]
                .selected
                .step
                .direction
                .iter()
                .any(|coordinate| !coordinate.is_zero())
        );
        driver
            .session()
            .snapshot()
            .certify_additive_half_termination(circulation.network())
            .unwrap();
        assert_eq!(run.solution.flow_value, 1);
        assert_eq!(run.solution.matching, vec![(0, 0)]);
        assert_eq!(run.solution.vertex_cover.size, 1);
    }

    #[test]
    fn potential_budget_recovers_a_nonterminal_compressed_solution_without_a_manual_limit() {
        let circulation = Circulation::from_partition(1, 1, &single_edge_partition()).unwrap();
        let snapshot = near_terminal_source_fixture(&circulation);
        let budget = PotentialBudget::new(&snapshot, circulation.network(), ratio(1, 2)).unwrap();
        assert!(budget.maximum_updates() > 0);

        let mut driver = boundary_source_driver(snapshot, 0);
        let run = circulation
            .run_source_with_potential_budget(&mut driver, &budget)
            .unwrap();

        assert_eq!(run.completion.records.len(), 1);
        assert!(u64::try_from(run.completion.records.len()).unwrap() <= budget.maximum_updates());
        assert_eq!(run.solution.matching, vec![(0, 0)]);
        assert_eq!(run.solution.vertex_cover.size, 1);
    }

    #[test]
    fn target_entry_starts_the_augmented_source_path_for_a_supplied_optimum() {
        let circulation = Circulation::from_partition(1, 1, &single_edge_partition()).unwrap();
        let expected_augmented = circulation
            .network()
            .initial_point_augmentation(2)
            .unwrap()
            .network;
        let calls = Rc::new(Cell::new(0));
        let observed_calls = Rc::clone(&calls);
        let factory = move |snapshot: &CertifiedIpmSnapshot, active: &CirculationNetwork| {
            observed_calls.set(observed_calls.get() + 1);
            assert_eq!(active, &expected_augmented);
            assert_eq!(snapshot.optimal_cost(), ExactRatio::new(-1, 1).unwrap());
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        };

        assert_eq!(
            circulation.run_with_target(
                -1,
                2,
                FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
                ratio(1, 2),
                factory,
            ),
            Err(Error::SourceFlow(SourceFlowError::Iteration(
                iteration::Error::NoSourceCandidate
            )))
        );
        assert_eq!(calls.get(), 1);
        assert_eq!(Backend.require_complete(), Err(SourceFlowError::Incomplete));
    }

    #[test]
    fn target_entry_rejects_a_non_strict_initial_point_before_factory_execution() {
        let circulation =
            Circulation::from_partition(2, 2, &complete_two_by_two_partition()).unwrap();
        let augmentation = circulation.network().initial_point_augmentation(3).unwrap();
        assert!(augmentation.initial_flow.cost.is_integral());
        let invalid_target = augmentation.initial_flow.cost.numerator_i128().unwrap();
        let calls = Rc::new(Cell::new(0));
        let observed_calls = Rc::clone(&calls);
        let factory = move |_: &CertifiedIpmSnapshot, _: &CirculationNetwork| {
            observed_calls.set(observed_calls.get() + 1);
            Err::<iteration::Projection, iteration::Error>(iteration::Error::NoSourceCandidate)
        };

        assert_eq!(
            circulation.run_with_target(
                invalid_target,
                3,
                FixedPointConfig::source_bounded(1 << 20, 96, 48, 3).unwrap(),
                ratio(1, 2),
                factory,
            ),
            Err(Error::SourceFlow(SourceFlowError::Ipm(
                CertifiedIpmError::InvalidSourceDomain
            )))
        );
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn cover_certificate_proves_optimum_above_a_supplied_target() {
        let circulation = Circulation::from_partition(1, 1, &single_edge_partition()).unwrap();
        let cover = graph::VertexCover {
            left: vec![true],
            right: vec![false],
            size: 1,
        };
        let proof = circulation.certify_cover_below(&cover, -2).unwrap();
        assert_eq!(proof.target, -2);
        assert_eq!(proof.cover_size, 1);
        assert_eq!(Backend.require_complete(), Err(SourceFlowError::Incomplete));
    }

    #[test]
    fn cover_certificate_rejects_a_target_that_is_not_exceeded() {
        let circulation = Circulation::from_partition(1, 1, &single_edge_partition()).unwrap();
        let cover = graph::VertexCover {
            left: vec![true],
            right: vec![false],
            size: 1,
        };
        assert_eq!(
            circulation.certify_cover_below(&cover, -1),
            Err(Error::CoverCertificateInsufficient {
                target: -1,
                cover_size: 1
            })
        );
    }

    #[test]
    fn cover_certificate_rejects_a_cover_that_omits_a_conflict_edge() {
        let circulation =
            Circulation::from_partition(2, 2, &complete_two_by_two_partition()).unwrap();
        let cover = graph::VertexCover {
            left: vec![true, false],
            right: vec![false, false],
            size: 1,
        };
        assert_eq!(
            circulation.certify_cover_below(&cover, -3),
            Err(Error::CoverCertificateUncoveredEdge { left: 1, right: 0 })
        );
    }

    #[test]
    fn cover_certificate_rejects_a_wrong_declared_size() {
        let circulation = Circulation::from_partition(1, 1, &single_edge_partition()).unwrap();
        let cover = graph::VertexCover {
            left: vec![true],
            right: vec![false],
            size: 0,
        };
        assert_eq!(
            circulation.certify_cover_below(&cover, -2),
            Err(Error::CoverCertificateSizeMismatch {
                declared: 0,
                recomputed: 1
            })
        );
    }

    #[test]
    fn cover_certificate_agrees_with_the_recovered_minimum_cover() {
        let circulation =
            Circulation::from_partition(2, 2, &complete_two_by_two_partition()).unwrap();
        let cover = graph::VertexCover {
            left: vec![true, true],
            right: vec![false, false],
            size: 2,
        };
        let proof = circulation.certify_cover_below(&cover, -3).unwrap();
        assert_eq!(proof.cover_size, 2);
        assert!(circulation.certify_cover_below(&cover, -2).is_err());
    }
}
