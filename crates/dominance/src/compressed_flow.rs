use graph::{FlowError, FlowResult, VertexCover};
use thiserror::Error;

pub mod experiment;
pub mod oracle;

#[derive(Clone, Debug)]
pub struct Solution {
    pub flow: FlowResult,
    pub vertex_cover: VertexCover,
    pub network_vertex_count: usize,
    pub network_arc_count: usize,
    pub internal_capacity: u64,
    pub internal_cut_arc_count: usize,
}

/// Exact agreement evidence for the explicit graph and both permanent flow
/// reference backends over a verified biclique partition.
#[derive(Clone, Debug)]
pub struct Parity {
    pub matching_size: usize,
    pub dinic: Solution,
    pub push_relabel: Solution,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Flow(#[from] FlowError),
    #[error("compressed network node count overflowed usize")]
    NetworkSizeOverflow,
    #[error("internal capacity cannot be represented as u64")]
    CapacityOverflow,
    #[error("biclique contains an endpoint outside its declared side")]
    BicliqueEndpointOutOfBounds,
    #[error("{count} large-capacity internal arcs unexpectedly belong to a minimum cut")]
    InternalArcInMinimumCut { count: usize },
    #[error("flow value cannot be represented as usize")]
    FlowValueConversion,
    #[error("minimum cut value {cut} differs from recovered cover size {cover}")]
    CutCoverSizeMismatch { cut: usize, cover: usize },
    #[error("biclique partition does not exactly represent the explicit graph")]
    InvalidPartition,
    #[error("explicit matching and compressed flow reference backends disagree")]
    ParityMismatch,
}

#[cfg(test)]
mod tests {
    use graph::{BipartiteGraph, DinicBackend, PushRelabelBackend, hopcroft_karp};

    use mrd_domain::BicliqueId;

    use crate::biclique::{Block, Partition};

    use super::{Error, experiment, oracle};

    #[test]
    fn c0_flow_equals_explicit_matching() {
        let mut graph = BipartiteGraph::new(3, 3);
        for (left, right) in [(0, 0), (0, 1), (1, 1), (2, 1), (2, 2)] {
            graph.add_edge(left, right).unwrap();
        }
        let partition = Partition::from_explicit_edges(&graph);
        let flow = experiment::solve(3, 3, &partition, &DinicBackend).unwrap();
        let value = usize::try_from(flow.flow.value).unwrap();
        assert_eq!(value, hopcroft_karp(&graph).size);
        assert_eq!(flow.vertex_cover.size, value);
        assert_eq!(flow.internal_cut_arc_count, 0);
        let maximum_matching_bound = flow.internal_capacity.checked_sub(1).unwrap();
        assert!(maximum_matching_bound >= u64::try_from(value).unwrap());
        for (left, right) in graph.edges() {
            assert!(flow.vertex_cover.left[left] || flow.vertex_cover.right[right]);
            let selected_left = !flow.vertex_cover.left[left];
            let selected_right = !flow.vertex_cover.right[right];
            assert!(!(selected_left && selected_right));
        }
    }

    #[test]
    fn push_relabel_matches_dinic_certificate() {
        let mut graph = BipartiteGraph::new(4, 4);
        for (left, right) in [(0, 0), (0, 2), (1, 1), (1, 2), (2, 1), (2, 3), (3, 0)] {
            graph.add_edge(left, right).unwrap();
        }
        let partition = Partition::from_explicit_edges(&graph);
        let dinic = experiment::solve(4, 4, &partition, &DinicBackend).unwrap();
        let push_relabel = experiment::solve(4, 4, &partition, &PushRelabelBackend).unwrap();
        assert_eq!(push_relabel.flow.value, dinic.flow.value);
        assert_eq!(push_relabel.vertex_cover.size, dinic.vertex_cover.size);
        assert_eq!(push_relabel.internal_cut_arc_count, 0);
    }

    #[test]
    fn parity_audit_agrees_for_every_two_by_two_explicit_graph() {
        for mask in 0_u8..16 {
            let mut graph = BipartiteGraph::new(2, 2);
            for (index, (left, right)) in [(0, 0), (0, 1), (1, 0), (1, 1)].iter().enumerate() {
                if mask & (1 << index) != 0 {
                    graph.add_edge(*left, *right).unwrap();
                }
            }
            let partition = Partition::from_explicit_edges(&graph);
            let parity = oracle::audit(&graph, &partition).unwrap();
            assert_eq!(parity.matching_size, hopcroft_karp(&graph).size);
        }
    }

    #[test]
    fn rejects_out_of_bounds_biclique_endpoint_before_flow() {
        let partition = Partition {
            blocks: vec![Block {
                id: BicliqueId(0),
                left: vec![1],
                right: vec![0],
            }],
        };
        let error = experiment::solve(1, 1, &partition, &DinicBackend).unwrap_err();
        assert!(matches!(error, Error::BicliqueEndpointOutOfBounds));
    }
}
