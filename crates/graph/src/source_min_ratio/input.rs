//! Exact provenance bridge from IPM coordinates to source structural edges.
//!
//! The source min-ratio construction separates positive structural tree
//! weights from signed current gradients. This module preserves that separation
//! and creates the only source-edge/circulation-arc correspondence used by the
//! finite P9.5 boundary. It deliberately constructs no tree chain and selects
//! no compact cycle.

use thiserror::Error;

use crate::{
    CirculationArcId, CirculationNetwork, ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId,
    SourceLsstError, SourceWeightedEdge,
};

use super::cycle::{ArcBindings, Error as CycleError};

/// One exact IPM coordinate with its stable source and circulation identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Arc {
    /// Stable source-graph edge identity.
    pub source: SourceEdgeId,
    /// Stable circulation-network arc identity.
    pub circulation: CirculationArcId,
    /// Declared circulation/source orientation start.
    pub first: FlowNodeId,
    /// Declared circulation/source orientation end.
    pub second: FlowNodeId,
    /// Signed exact approximate gradient for the current IPM snapshot.
    pub gradient: ExactRatio,
    /// Positive exact approximate length for the current IPM snapshot.
    pub length: ExactRatio,
    /// Positive structural weight supplied to the source tree construction.
    pub tree_weight: ExactRatio,
}

/// Pure, provenance-preserving projection of one live IPM coordinate vector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Input {
    node_count: usize,
    arcs: Vec<Arc>,
}

/// A source graph and the matching circulation bindings built together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Materialization {
    /// Positive-length/positive-weight graph consumed by source tree layers.
    pub graph: SourceDynamicGraph,
    /// Exact one-to-one orientation-preserving circulation bindings.
    pub bindings: ArcBindings,
}

/// A positive common-scale structural graph derived from one exact input.
///
/// The graph is for finite tree/spanner construction only. Candidate scoring
/// and IPM certification continue to use the unscaled [`Input`] coordinates.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuralGraph {
    /// Source graph with every edge length made integral by `length_scale`.
    pub graph: SourceDynamicGraph,
    /// Exact positive factor applied uniformly to lengths and tree weights.
    pub length_scale: ExactRatio,
}

impl Input {
    /// Validates one exact current-coordinate projection.
    ///
    /// `tree_weights` are independent positive structural weights for the
    /// source low-stretch-tree construction. They must not be inferred from
    /// signed gradients.
    ///
    /// # Errors
    ///
    /// Returns an error for a dimension mismatch, a nonpositive length or
    /// structural weight, a loop unsupported by the source graph, or a missing
    /// circulation endpoint.
    pub fn new(
        network: &CirculationNetwork,
        gradients: &[ExactRatio],
        lengths: &[ExactRatio],
        tree_weights: &[ExactRatio],
    ) -> Result<Self, Error> {
        let arc_count = network.arc_count();
        if gradients.len() != arc_count
            || lengths.len() != arc_count
            || tree_weights.len() != arc_count
        {
            return Err(Error::DimensionMismatch);
        }
        let mut arcs = Vec::with_capacity(arc_count);
        for index in 0..arc_count {
            let circulation = CirculationArcId(index);
            let (first, second) = network
                .arc_endpoints(circulation)
                .ok_or(Error::MissingArc { arc: index })?;
            let length = lengths[index];
            let tree_weight = tree_weights[index];
            if !length.is_positive() {
                return Err(Error::NonpositiveLength { arc: index });
            }
            if !tree_weight.is_positive() {
                return Err(Error::NonpositiveTreeWeight { arc: index });
            }
            if first == second {
                return Err(Error::Loop { arc: index });
            }
            arcs.push(Arc {
                source: SourceEdgeId(index),
                circulation,
                first,
                second,
                gradient: gradients[index],
                length,
                tree_weight,
            });
        }
        Ok(Self {
            node_count: network.node_count(),
            arcs,
        })
    }

    /// Returns the exact per-arc provenance records in stable source-ID order.
    #[must_use]
    pub fn arcs(&self) -> &[Arc] {
        &self.arcs
    }

    /// Returns one exact coordinate by its stable circulation-arc identity.
    #[must_use]
    pub fn arc(&self, circulation: CirculationArcId) -> Option<&Arc> {
        self.arcs
            .get(circulation.0)
            .filter(|arc| arc.circulation == circulation)
    }

    /// Returns whether two projections retain the same source/circulation
    /// identities while allowing their exact coordinates to differ.
    ///
    /// This is the transition boundary for immutable source snapshots: it
    /// checks the source IDs, circulation IDs, and directed endpoints, but not
    /// gradients, lengths, or structural tree weights.
    #[must_use]
    pub fn has_same_source_identity(&self, other: &Self) -> bool {
        self.node_count == other.node_count
            && self.arcs.len() == other.arcs.len()
            && self.arcs.iter().zip(&other.arcs).all(|(first, second)| {
                first.source == second.source
                    && first.circulation == second.circulation
                    && first.first == second.first
                    && first.second == second.second
            })
    }

    /// Constructs the source structural graph and its bindings as one checked
    /// operation.
    ///
    /// # Errors
    ///
    /// Returns an error when the supplied circulation network no longer has
    /// this projection's node/arc shape, the source graph cannot represent its
    /// exact positive coordinates, or binding verification fails.
    pub fn materialize(&self, network: &CirculationNetwork) -> Result<Materialization, Error> {
        if network.node_count() != self.node_count || network.arc_count() != self.arcs.len() {
            return Err(Error::NetworkChanged);
        }
        let mut maximum_abs_coordinate = 1_i128;
        let mut edges = Vec::with_capacity(self.arcs.len());
        let mut bindings = Vec::with_capacity(self.arcs.len());
        for arc in &self.arcs {
            if network.arc_endpoints(arc.circulation) != Some((arc.first, arc.second)) {
                return Err(Error::NetworkChanged);
            }
            maximum_abs_coordinate = maximum_abs_coordinate
                .max(coordinate_bound(arc.length)?)
                .max(coordinate_bound(arc.tree_weight)?);
            edges.push(SourceWeightedEdge {
                first: arc.first,
                second: arc.second,
                length: arc.length,
                weight: arc.tree_weight,
            });
            bindings.push((arc.source, arc.circulation));
        }
        let graph = SourceDynamicGraph::new(self.node_count, edges, maximum_abs_coordinate)?;
        let bindings = ArcBindings::new(&graph, network, bindings)?;
        Ok(Materialization { graph, bindings })
    }

    /// Builds an exact globally scaled graph for finite structural algorithms.
    ///
    /// Every scaled length is integral, while a single positive scale preserves
    /// source IDs, endpoints, and all exact length/weight ratios. This does not
    /// replace the unscaled coordinates retained by [`Self::materialize`].
    ///
    /// # Errors
    ///
    /// Returns an error when a common denominator or scaled coordinate cannot
    /// be represented in the exact source domain.
    pub fn normalize_structure(&self) -> Result<StructuralGraph, Error> {
        let denominator = self.arcs.iter().try_fold(1_i128, |scale, arc| {
            checked_lcm(scale, arc.length.denominator())
        })?;
        let length_scale = ExactRatio::new(denominator, 1).map_err(|_| Error::Overflow)?;
        let mut maximum_abs_coordinate = 1_i128;
        let mut edges = Vec::with_capacity(self.arcs.len());
        for arc in &self.arcs {
            let length = arc
                .length
                .checked_mul(length_scale)
                .map_err(|_| Error::Overflow)?;
            if length.denominator() != 1 || !length.is_positive() {
                return Err(Error::InvalidStructuralNormalization {
                    arc: arc.circulation.0,
                });
            }
            let weight = arc
                .tree_weight
                .checked_mul(length_scale)
                .map_err(|_| Error::Overflow)?;
            maximum_abs_coordinate = maximum_abs_coordinate
                .max(coordinate_bound(length)?)
                .max(coordinate_bound(weight)?);
            edges.push(SourceWeightedEdge {
                first: arc.first,
                second: arc.second,
                length,
                weight,
            });
        }
        let graph = SourceDynamicGraph::new(self.node_count, edges, maximum_abs_coordinate)?;
        Ok(StructuralGraph {
            graph,
            length_scale,
        })
    }
}

fn checked_lcm(first: i128, second: i128) -> Result<i128, Error> {
    if first <= 0 || second <= 0 {
        return Err(Error::Overflow);
    }
    let divisor = gcd(first, second);
    first
        .checked_div(divisor)
        .and_then(|reduced| reduced.checked_mul(second))
        .ok_or(Error::Overflow)
}

fn gcd(mut first: i128, mut second: i128) -> i128 {
    while second != 0 {
        let remainder = first % second;
        first = second;
        second = remainder;
    }
    first
}

fn coordinate_bound(value: ExactRatio) -> Result<i128, Error> {
    value
        .numerator()
        .checked_abs()
        .map(|numerator| numerator.max(value.denominator()).max(1))
        .ok_or(Error::Overflow)
}

/// An IPM/source provenance projection could not be constructed.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    #[error("IPM coordinate vectors do not match the circulation arcs")]
    DimensionMismatch,
    #[error("circulation arc {arc} is missing")]
    MissingArc { arc: usize },
    #[error("IPM approximate length for arc {arc} is not positive")]
    NonpositiveLength { arc: usize },
    #[error("source tree weight for arc {arc} is not positive")]
    NonpositiveTreeWeight { arc: usize },
    #[error("circulation arc {arc} is a loop unsupported by the source graph")]
    Loop { arc: usize },
    #[error("the circulation network changed after source input construction")]
    NetworkChanged,
    #[error("source coordinate accounting overflowed")]
    Overflow,
    #[error("scaled structural length for circulation arc {arc} is invalid")]
    InvalidStructuralNormalization { arc: usize },
    #[error("source graph construction failed: {0}")]
    Graph(#[from] SourceLsstError),
    #[error("source/circulation bindings failed: {0}")]
    Binding(#[from] CycleError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Error, Input};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, SourceEdgeId,
        source_min_ratio::{
            chain::Chain,
            cycle::{Cycle, Direction, Segment},
            model::{Branch, BranchId, Level, LevelId, Tree},
        },
    };

    fn ratio(value: i128) -> ExactRatio {
        ExactRatio::new(value, 1).unwrap()
    }

    fn network() -> CirculationNetwork {
        let mut network = CirculationNetwork::new(3);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 3).unwrap();
        network
            .add_arc(FlowNodeId(1), FlowNodeId(2), 2, -2)
            .unwrap();
        network.add_arc(FlowNodeId(0), FlowNodeId(2), 2, 1).unwrap();
        network
    }

    #[test]
    fn preserves_exact_coordinate_and_arc_provenance() {
        let network = network();
        let input = Input::new(
            &network,
            &[ratio(-3), ratio(2), ratio(1)],
            &[ratio(2), ratio(3), ratio(5)],
            &[ratio(7), ratio(11), ratio(13)],
        )
        .unwrap();
        let materialized = input.materialize(&network).unwrap();
        assert_eq!(input.arcs()[1].gradient, ratio(2));
        assert_eq!(materialized.graph.edge_count(), 3);
        assert_eq!(
            materialized
                .graph
                .edge(crate::SourceEdgeId(2))
                .unwrap()
                .weight,
            ratio(13)
        );

        let chain = Chain::new(
            &materialized.graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    Tree::new(
                        FlowNodeId(0),
                        BTreeSet::from([crate::SourceEdgeId(0), crate::SourceEdgeId(1)]),
                    ),
                )],
            )],
        )
        .unwrap();
        let shifts = chain.initial_shifts();
        let selection = chain.select(&shifts).unwrap()[0];
        let cycle = Cycle {
            segments: vec![
                Segment::TreePath {
                    selection,
                    from: FlowNodeId(0),
                    to: FlowNodeId(2),
                },
                Segment::OffTree {
                    source: crate::SourceEdgeId(2),
                    direction: Direction::Reverse,
                },
            ],
        };
        assert_eq!(
            cycle
                .decode(
                    &materialized.graph,
                    &chain,
                    &shifts,
                    &materialized.bindings,
                    &network,
                )
                .unwrap(),
            vec![
                (crate::CirculationArcId(0), 1),
                (crate::CirculationArcId(1), 1),
                (crate::CirculationArcId(2), -1),
            ]
        );
    }

    #[test]
    fn rejects_invalid_structural_coordinates_without_selecting_a_fallback() {
        let network = network();
        assert_eq!(
            Input::new(
                &network,
                &[ratio(0); 3],
                &[ratio(1); 3],
                &[ratio(1), ratio(0), ratio(1)],
            ),
            Err(Error::NonpositiveTreeWeight { arc: 1 })
        );
        assert_eq!(
            Input::new(&network, &[ratio(0); 2], &[ratio(1); 3], &[ratio(1); 3],),
            Err(Error::DimensionMismatch)
        );
    }

    #[test]
    fn separates_stable_source_identity_from_current_coordinates() {
        let network = network();
        let first = Input::new(
            &network,
            &[ratio(-3), ratio(2), ratio(1)],
            &[ratio(2), ratio(3), ratio(5)],
            &[ratio(7), ratio(11), ratio(13)],
        )
        .unwrap();
        let refreshed = Input::new(
            &network,
            &[ratio(9), ratio(-4), ratio(6)],
            &[ratio(19), ratio(23), ratio(29)],
            &[ratio(31), ratio(37), ratio(41)],
        )
        .unwrap();
        assert!(first.has_same_source_identity(&refreshed));

        let mut changed_network = CirculationNetwork::new(3);
        changed_network
            .add_arc(FlowNodeId(0), FlowNodeId(2), 2, 3)
            .unwrap();
        changed_network
            .add_arc(FlowNodeId(1), FlowNodeId(2), 2, -2)
            .unwrap();
        changed_network
            .add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1)
            .unwrap();
        let changed = Input::new(
            &changed_network,
            &[ratio(-3), ratio(2), ratio(1)],
            &[ratio(2), ratio(3), ratio(5)],
            &[ratio(7), ratio(11), ratio(13)],
        )
        .unwrap();
        assert!(!first.has_same_source_identity(&changed));
    }

    #[test]
    fn normalizes_rational_lengths_for_the_structural_graph_only() {
        let network = network();
        let input = Input::new(
            &network,
            &[ratio(-3), ratio(2), ratio(1)],
            &[
                ExactRatio::new(1, 2).unwrap(),
                ExactRatio::new(3, 4).unwrap(),
                ExactRatio::new(5, 6).unwrap(),
            ],
            &[ratio(7), ratio(11), ratio(13)],
        )
        .unwrap();

        let structural = input.normalize_structure().unwrap();

        assert_eq!(structural.length_scale, ratio(12));
        assert_eq!(
            structural.graph.edge(SourceEdgeId(0)).unwrap().length,
            ratio(6)
        );
        assert_eq!(
            structural.graph.edge(SourceEdgeId(1)).unwrap().length,
            ratio(9)
        );
        assert_eq!(
            structural.graph.edge(SourceEdgeId(2)).unwrap().length,
            ratio(10)
        );
        assert_eq!(
            structural.graph.edge(SourceEdgeId(1)).unwrap().weight,
            ratio(132)
        );
        assert_eq!(
            input.arc(crate::CirculationArcId(0)).unwrap().length,
            ExactRatio::new(1, 2).unwrap()
        );
    }
}
