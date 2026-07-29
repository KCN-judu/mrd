//! Algorithm 4 Task 3 and final finite image/composition audit.

use std::collections::BTreeSet;

use super::{
    super::model::{Audit, Candidate, EdgeId, Embedding, Error as ModelError, Graph},
    first_embedding::Trace,
    witness::Union,
};

/// Task 3's direct finite `J -> W` embedding and Algorithm 4 output evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Output {
    pub image: BTreeSet<EdgeId>,
    pub input_to_witness: Embedding,
    pub input_to_image: Embedding,
    pub audit: Audit,
}

/// Finishes Algorithm 4 when the finite Task 2 witness paths are complete.
///
/// # Errors
///
/// Returns an error for an unembedded witness edge, an input edge absent from
/// the finite witness, or a failed image/composition certificate.
pub fn finish(
    host: &Graph,
    input: &Graph,
    input_to_host: &Embedding,
    witness: &Union,
    first: &Trace,
) -> Result<Output, Error> {
    if !first.unembedded.is_empty() || first.paths.len() != witness.graph.edge_count() {
        return Err(Error::UnembeddedWitness);
    }
    let mut input_to_witness_paths = Vec::with_capacity(input.edge_count());
    let mut input_to_image_paths = Vec::with_capacity(input.edge_count());
    let mut image = BTreeSet::new();
    for index in 0..input.edge_count() {
        let input_edge = input.edge(EdgeId(index)).ok_or(Error::InvalidWitness)?;
        let witness_edge = matching_edge(&witness.graph, input_edge.first, input_edge.second)
            .ok_or(Error::MissingWitnessPath)?;
        let witness_path = first
            .paths
            .get(witness_edge.0)
            .and_then(Option::as_ref)
            .ok_or(Error::UnembeddedWitness)?;
        image.extend(witness_path.iter().copied());
        input_to_witness_paths.push(vec![witness_edge]);
        input_to_image_paths.push(witness_path.clone());
    }
    let input_to_witness = Embedding::new(input, &witness.graph, None, input_to_witness_paths)
        .map_err(Error::Model)?;
    let input_to_image =
        Embedding::new(input, input, Some(&image), input_to_image_paths).map_err(Error::Model)?;
    let candidate = Candidate {
        edges: image.clone(),
        embedding: input_to_image.clone(),
    };
    let audit = Audit::verify(host, input, input_to_host, &candidate).map_err(Error::Model)?;
    Ok(Output {
        image,
        input_to_witness,
        input_to_image,
        audit,
    })
}

fn matching_edge(
    graph: &Graph,
    first: crate::FlowNodeId,
    second: crate::FlowNodeId,
) -> Option<EdgeId> {
    (0..graph.edge_count()).map(EdgeId).find(|edge_id| {
        graph.edge(*edge_id).is_some_and(|edge| {
            (edge.first == first && edge.second == second)
                || (edge.first == second && edge.second == first)
        })
    })
}

/// Task 3 cannot produce a valid finite Algorithm 4 output.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("Algorithm 4 Task 3 has an unembedded witness edge")]
    UnembeddedWitness,
    #[error("Algorithm 4 Task 3 witness omits an input edge")]
    MissingWitnessPath,
    #[error("Algorithm 4 Task 3 witness is invalid")]
    InvalidWitness,
    #[error("Algorithm 4 Task 3 embedding audit failed: {0}")]
    Model(#[source] ModelError),
}

#[cfg(test)]
mod tests {
    use super::finish;
    use crate::{
        ExactRatio, FlowNodeId,
        source_spanner::{
            algorithm4::{
                first_embedding::{Parameters, embed},
                witness,
            },
            experiment::{decomposition::single_level, domain::ExhaustiveDomain},
            model::{Edge, EdgeId, Embedding, Graph},
        },
    };

    fn cycle() -> Graph {
        Graph::new(
            4,
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
                    first: FlowNodeId(2),
                    second: FlowNodeId(3),
                },
                Edge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(3),
                },
            ],
        )
        .unwrap()
    }

    fn identity(graph: &Graph) -> Embedding {
        Embedding::new(
            graph,
            graph,
            None,
            (0..graph.edge_count())
                .map(|edge| vec![EdgeId(edge)])
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn builds_the_image_and_composed_embedding() {
        let domain = ExhaustiveDomain { maximum_nodes: 8 };
        let input = cycle();
        let direct = identity(&input);
        let decomposition = single_level(&input, ExactRatio::new(1, 2).unwrap(), domain).unwrap();
        let witness = witness::build(&input, &decomposition, domain).unwrap();
        let first = embed(
            &input,
            &input,
            &direct,
            &witness,
            Parameters {
                maximum_hops: 2,
                maximum_vertex_congestion: 100,
                maximum_rounds: 1,
            },
        )
        .unwrap();
        let output = finish(&input, &input, &direct, &witness, &first).unwrap();
        assert_eq!(output.image.len(), input.edge_count());
        assert_eq!(output.audit.composed.maximum_path_length, 1);
    }
}
