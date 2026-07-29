//! Slow independent greedy-rebuild Oracle for finite dynamic spanner checks.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    super::{
        model::{EdgeId, Embedding, Error as ModelError},
        oracle::simple_paths,
    },
    batch::{Error as BatchError, State as BatchState},
};

/// A canonical greedy spanner certificate expressed in stable source edge IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Snapshot {
    pub selected: BTreeSet<EdgeId>,
    pub embeddings: BTreeMap<EdgeId, Vec<EdgeId>>,
}

impl Snapshot {
    /// Independently validates this Oracle certificate on the active source graph.
    ///
    /// # Errors
    ///
    /// Returns an error when a stable ID is no longer active or a mapped path is
    /// not a valid selected-subgraph embedding.
    pub fn verify(&self, input: &BatchState) -> Result<(), Error> {
        let (graph, stable_ids) = input.active_graph().map_err(Error::Batch)?;
        if self.embeddings.len() != stable_ids.len()
            || self.embeddings.keys().copied().collect::<BTreeSet<_>>()
                != stable_ids.iter().copied().collect()
        {
            return Err(Error::InvalidSnapshot);
        }
        let relative = stable_ids
            .iter()
            .enumerate()
            .map(|(index, stable)| (*stable, EdgeId(index)))
            .collect::<BTreeMap<_, _>>();
        let selected = self
            .selected
            .iter()
            .map(|stable| relative.get(stable).copied().ok_or(Error::InvalidSnapshot))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let paths = stable_ids
            .iter()
            .map(|stable| {
                self.embeddings
                    .get(stable)
                    .ok_or(Error::InvalidSnapshot)?
                    .iter()
                    .map(|edge| relative.get(edge).copied().ok_or(Error::InvalidSnapshot))
                    .collect()
            })
            .collect::<Result<Vec<Vec<_>>, _>>()?;
        Embedding::new(&graph, &graph, Some(&selected), paths).map_err(Error::Model)?;
        Ok(())
    }
}

/// Rebuilds a deterministic greedy certificate by exhaustively enumerating
/// selected-subgraph paths for every active input edge.
///
/// # Errors
///
/// Returns an error for a zero hop bound or an invalid active source graph.
/// This is deliberately slow and remains an Oracle, not a Theorem 8.2 backend.
pub fn greedy(input: &BatchState, maximum_hops: usize) -> Result<Snapshot, Error> {
    if maximum_hops == 0 {
        return Err(Error::InvalidHopBound);
    }
    let (graph, stable_ids) = input.active_graph().map_err(Error::Batch)?;
    let mut selected = BTreeSet::new();
    let mut paths = Vec::with_capacity(graph.edge_count());
    for index in 0..graph.edge_count() {
        let edge = EdgeId(index);
        let source = graph.edge(edge).ok_or(Error::InvalidSnapshot)?;
        let path = simple_paths(
            &graph,
            source.first,
            source.second,
            Some(&selected),
            maximum_hops,
        )
        .map_err(Error::Model)?
        .into_iter()
        .next();
        if let Some(path) = path {
            paths.push(path);
        } else {
            selected.insert(edge);
            paths.push(vec![edge]);
        }
    }
    let stable_selected = selected
        .iter()
        .map(|edge| {
            stable_ids
                .get(edge.0)
                .copied()
                .ok_or(Error::InvalidSnapshot)
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let embeddings = stable_ids
        .iter()
        .zip(paths)
        .map(|(source, path)| {
            let stable_path = path
                .iter()
                .map(|edge| {
                    stable_ids
                        .get(edge.0)
                        .copied()
                        .ok_or(Error::InvalidSnapshot)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((*source, stable_path))
        })
        .collect::<Result<BTreeMap<_, _>, Error>>()?;
    let result = Snapshot {
        selected: stable_selected,
        embeddings,
    };
    result.verify(input)?;
    Ok(result)
}

/// The independent greedy-rebuild Oracle cannot produce a certificate.
#[derive(Clone, Debug, Eq, thiserror::Error, PartialEq)]
pub enum Error {
    #[error("source spanner batch is invalid: {0}")]
    Batch(#[source] BatchError),
    #[error("Oracle greedy spanner hop bound must be positive")]
    InvalidHopBound,
    #[error("Oracle stable dynamic snapshot is invalid")]
    InvalidSnapshot,
    #[error("Oracle embedding verification failed: {0}")]
    Model(#[source] ModelError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::greedy;
    use crate::{
        FlowNodeId,
        source_spanner::{
            dynamic::batch::State,
            model::{Edge, EdgeId, Graph},
        },
    };

    fn triangle() -> Graph {
        Graph::new(
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
        .unwrap()
    }

    #[test]
    fn exhaustively_rebuilds_the_canonical_greedy_certificate() {
        let input = State::new(&triangle()).unwrap();
        let snapshot = greedy(&input, 2).unwrap();

        assert_eq!(snapshot.selected, BTreeSet::from([EdgeId(0), EdgeId(1)]));
        assert_eq!(snapshot.embeddings[&EdgeId(2)], vec![EdgeId(0), EdgeId(1)]);
        snapshot.verify(&input).unwrap();
    }
}
