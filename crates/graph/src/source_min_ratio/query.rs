//! Checked public query boundary over a hidden-stability ledger.

use crate::{CirculationArcId, CirculationNetwork, SourceDynamicGraph, StableMinRatioLedger};

use super::{
    chain::{Chain, Shifts},
    cycle::{ArcBindings, Cycle, Error},
};

/// Public, exact result of decoding one source-shaped compact-cycle candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Result {
    /// Signed circulation arcs obtained without an enumerating cycle query.
    pub arcs: Vec<(CirculationArcId, i8)>,
    /// Number of checked hidden-stability coordinates retained by the ledger.
    pub stable_edge_count: usize,
}

/// Decodes a compact candidate without a hidden-stability state.
///
/// This is the pure compact-cycle boundary: it validates source graph, chain,
/// shift, binding, and circulation semantics only. It does not search for a
/// candidate, authorize an approximation guarantee, or inspect a stability
/// witness.
///
/// # Errors
///
/// Returns an error when the compact candidate cannot decode to a circulation.
pub fn decode(
    candidate: &Cycle,
    graph: &SourceDynamicGraph,
    chain: &Chain,
    shifts: &Shifts,
    bindings: &ArcBindings,
    network: &CirculationNetwork,
) -> std::result::Result<Vec<(CirculationArcId, i8)>, Error> {
    candidate.decode(graph, chain, shifts, bindings, network)
}

/// Decodes a candidate using an already validated hidden-stability state.
///
/// The ledger's witness is neither accepted nor returned here. This operation
/// validates only exact compact-cycle semantics; it makes no approximation or
/// dynamic-query claim.
///
/// # Errors
///
/// Returns an error when the compact candidate cannot decode to a circulation.
pub fn decode_candidate(
    ledger: &StableMinRatioLedger,
    candidate: &Cycle,
    graph: &SourceDynamicGraph,
    chain: &Chain,
    shifts: &Shifts,
    bindings: &ArcBindings,
    network: &CirculationNetwork,
) -> std::result::Result<Result, Error> {
    Ok(Result {
        arcs: decode(candidate, graph, chain, shifts, bindings, network)?,
        stable_edge_count: ledger.edges().len(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{decode, decode_candidate};
    use crate::{
        CirculationNetwork, ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId,
        SourceWeightedEdge, StableEdge, StableMinRatioLedger, StableWitness,
        source_min_ratio::{
            chain::Chain,
            cycle::{ArcBindings, Cycle, Direction, Segment},
            model::{Branch, BranchId, Level, LevelId, Tree},
        },
    };

    #[test]
    fn exposes_decoded_arcs_without_exposing_the_hidden_witness() {
        let edge = |first, second| SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(1, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        };
        let graph =
            SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap();
        let chain = Chain::new(
            &graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    Tree::new(
                        FlowNodeId(0),
                        BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)]),
                    ),
                )],
            )],
        )
        .unwrap();
        let mut network = CirculationNetwork::new(3);
        let a = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let b = network.add_arc(FlowNodeId(1), FlowNodeId(2), 1, 0).unwrap();
        let c = network.add_arc(FlowNodeId(0), FlowNodeId(2), 1, 0).unwrap();
        let bindings = ArcBindings::new(
            &graph,
            &network,
            vec![
                (SourceEdgeId(0), a),
                (SourceEdgeId(1), b),
                (SourceEdgeId(2), c),
            ],
        )
        .unwrap();
        let ledger = StableMinRatioLedger::new(
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
            ExactRatio::new(1, 4).unwrap(),
            ExactRatio::new(1, 2).unwrap(),
            StableWitness {
                circulation: vec![1, 1],
                upper_bounds: vec![1, 1],
            },
        )
        .unwrap();
        let selection = chain.select(&chain.initial_shifts()).unwrap()[0];
        let result = decode_candidate(
            &ledger,
            &Cycle {
                segments: vec![
                    Segment::TreePath {
                        selection,
                        from: FlowNodeId(0),
                        to: FlowNodeId(2),
                    },
                    Segment::OffTree {
                        source: SourceEdgeId(2),
                        direction: Direction::Reverse,
                    },
                ],
            },
            &graph,
            &chain,
            &chain.initial_shifts(),
            &bindings,
            &network,
        )
        .unwrap();
        assert_eq!(result.arcs, vec![(a, 1), (b, 1), (c, -1)]);
        assert_eq!(result.stable_edge_count, 2);
    }

    #[test]
    fn pure_decode_has_no_hidden_stability_dependency() {
        let edge = |first, second| SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(1, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        };
        let graph =
            SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap();
        let chain = Chain::new(
            &graph,
            vec![Level::new(
                LevelId(0),
                vec![Branch::new(
                    BranchId(0),
                    0,
                    Tree::new(
                        FlowNodeId(0),
                        BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)]),
                    ),
                )],
            )],
        )
        .unwrap();
        let mut network = CirculationNetwork::new(3);
        let first = network.add_arc(FlowNodeId(0), FlowNodeId(1), 1, 0).unwrap();
        let second = network.add_arc(FlowNodeId(1), FlowNodeId(2), 1, 0).unwrap();
        let third = network.add_arc(FlowNodeId(0), FlowNodeId(2), 1, 0).unwrap();
        let bindings = ArcBindings::new(
            &graph,
            &network,
            vec![
                (SourceEdgeId(0), first),
                (SourceEdgeId(1), second),
                (SourceEdgeId(2), third),
            ],
        )
        .unwrap();
        let selection = chain.select(&chain.initial_shifts()).unwrap()[0];

        let arcs = decode(
            &Cycle {
                segments: vec![
                    Segment::TreePath {
                        selection,
                        from: FlowNodeId(0),
                        to: FlowNodeId(2),
                    },
                    Segment::OffTree {
                        source: SourceEdgeId(2),
                        direction: Direction::Reverse,
                    },
                ],
            },
            &graph,
            &chain,
            &chain.initial_shifts(),
            &bindings,
            &network,
        )
        .unwrap();

        assert_eq!(arcs, vec![(first, 1), (second, 1), (third, -1)]);
    }
}
