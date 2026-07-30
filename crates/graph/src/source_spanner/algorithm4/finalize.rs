//! Algorithm 4 Task 3 and final finite image/composition audit.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    super::model::{Audit, Candidate, EdgeId, Embedding, Error as ModelError, Graph},
    first_embedding::Trace,
    second_embedding,
    witness::Union,
};

/// Task 3's finite `J -> W` embedding and Algorithm 4 output evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Output {
    pub image: BTreeSet<EdgeId>,
    pub input_to_witness: Embedding,
    pub input_to_image: Embedding,
    pub audit: Audit,
}

/// Finishes Algorithm 4 when the finite Task 2 and Task 3 paths are complete.
///
/// # Errors
///
/// Returns an error for an unembedded witness or input edge, or a failed
/// image/composition certificate.
pub fn finish(
    host: &Graph,
    input: &Graph,
    input_to_host: &Embedding,
    witness: &Union,
    first: &Trace,
    second: &second_embedding::Trace,
) -> Result<Output, Error> {
    if !first.unembedded.is_empty()
        || !second.unembedded.is_empty()
        || first.paths.len() != witness.graph.edge_count()
        || second.paths.len() != input.edge_count()
    {
        return Err(Error::UnembeddedWitness);
    }
    let mut input_to_image_paths = Vec::with_capacity(input.edge_count());
    let mut image = BTreeSet::new();
    for index in 0..input.edge_count() {
        let input_edge = input_edge(input, index)?;
        let witness_path = second
            .paths
            .get(index)
            .and_then(Option::as_ref)
            .ok_or(Error::UnembeddedWitness)?;
        let mut image_path = Vec::new();
        let mut witness_current = input_edge.first;
        for witness_edge in witness_path {
            let edge = witness
                .graph
                .edge(*witness_edge)
                .ok_or(Error::InvalidWitness)?;
            let path = first
                .paths
                .get(witness_edge.0)
                .and_then(Option::as_ref)
                .ok_or(Error::UnembeddedWitness)?;
            if edge.first == witness_current {
                image_path.extend(path.iter().copied());
                witness_current = edge.second;
            } else if edge.second == witness_current {
                image_path.extend(path.iter().rev().copied());
                witness_current = edge.first;
            } else {
                return Err(Error::InvalidWitness);
            }
        }
        let image_path = loop_erase(input, input_edge.first, image_path)?;
        image.extend(image_path.iter().copied());
        input_to_image_paths.push(image_path);
    }
    let input_to_witness = Embedding::new(
        input,
        &witness.graph,
        None,
        second
            .paths
            .iter()
            .cloned()
            .collect::<Option<Vec<Vec<EdgeId>>>>()
            .ok_or(Error::UnembeddedWitness)?,
    )
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

fn input_edge(input: &Graph, index: usize) -> Result<super::super::model::Edge, Error> {
    input.edge(EdgeId(index)).ok_or(Error::InvalidWitness)
}

/// Removes closed subwalks while preserving the exact endpoints of a composed
/// Algorithm 4 embedding path.
fn loop_erase(
    input: &Graph,
    start: crate::FlowNodeId,
    walk: Vec<EdgeId>,
) -> Result<Vec<EdgeId>, Error> {
    let mut positions = BTreeMap::from([(start, 0_usize)]);
    let mut vertices = vec![start];
    let mut result = Vec::new();
    let mut current = start;
    for edge_id in walk {
        let edge = input.edge(edge_id).ok_or(Error::InvalidWitness)?;
        let next = if edge.first == current {
            edge.second
        } else if edge.second == current {
            edge.first
        } else {
            return Err(Error::InvalidWitness);
        };
        if let Some(position) = positions.get(&next).copied() {
            for vertex in vertices.drain(position + 1..) {
                positions.remove(&vertex);
            }
            result.truncate(position);
        } else {
            result.push(edge_id);
            vertices.push(next);
            positions.insert(next, result.len());
        }
        current = next;
    }
    if result.is_empty() {
        return Err(Error::InvalidWitness);
    }
    Ok(result)
}

/// Task 3 cannot produce a valid finite Algorithm 4 output.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("Algorithm 4 Task 3 has an unembedded witness edge")]
    UnembeddedWitness,
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
                first_embedding::{Parameters as FirstParameters, embed as first_embed},
                second_embedding::{Parameters as SecondParameters, embed as second_embed},
                witness,
            },
            experiment::{decomposition::single_level, domain::ExhaustiveDomain},
            model::{Edge, EdgeId, Embedding, Graph},
        },
    };

    fn complete() -> Graph {
        Graph::new(
            5,
            (0..5)
                .flat_map(|first| {
                    ((first + 1)..5).map(move |second| Edge {
                        first: FlowNodeId(first),
                        second: FlowNodeId(second),
                    })
                })
                .collect(),
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
    fn builds_a_sparse_image_and_composed_embedding() {
        let domain = ExhaustiveDomain { maximum_nodes: 8 };
        let input = complete();
        let direct = identity(&input);
        let decomposition = single_level(&input, ExactRatio::new(1, 2).unwrap(), domain).unwrap();
        let witness = witness::build(&input, &decomposition, domain).unwrap();
        let first = first_embed(
            &input,
            &input,
            &direct,
            &witness,
            FirstParameters {
                maximum_hops: 4,
                maximum_vertex_congestion: 100,
                maximum_rounds: 1,
            },
        )
        .unwrap();
        let second = second_embed(
            &input,
            &witness,
            SecondParameters {
                maximum_hops: 4,
                maximum_edge_congestion: 100,
                maximum_rounds: 1,
            },
        )
        .unwrap();
        let output = finish(&input, &input, &direct, &witness, &first, &second).unwrap();
        assert!(output.image.len() < input.edge_count());
        assert!(output.audit.composed.maximum_path_length > 1);
    }
}
