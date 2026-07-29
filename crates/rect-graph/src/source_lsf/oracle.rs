//! Exhaustive bounded static low-stretch spanning-tree oracle.

use std::collections::BTreeSet;

use crate::{ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId};

use super::experiment::{Error, Tree, exact_forest_stretches, map_ratio, ratio};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lsst {
    pub edges: BTreeSet<SourceEdgeId>,
    pub weighted_stretch: ExactRatio,
    pub total_weight: ExactRatio,
    pub candidates_checked: u64,
}

impl Lsst {
    /// Exhaustively finds the minimum exact weighted-stretch spanning tree on
    /// a bounded small graph. This is a differential Oracle, not AN19.
    ///
    /// # Errors
    ///
    /// Returns an error beyond the explicit small-instance limit, for a
    /// disconnected graph, or when exact enumeration arithmetic overflows.
    pub fn solve(graph: &SourceDynamicGraph) -> Result<Self, Error> {
        if graph.node_count() == 0 || graph.node_count() > 12 || graph.edge_count() > 24 {
            return Err(Error::OracleLimitExceeded);
        }
        let active = (0..graph.edge_count())
            .filter(|index| graph.edge(SourceEdgeId(*index)).is_some())
            .map(SourceEdgeId)
            .collect::<Vec<_>>();
        let required = graph
            .node_count()
            .checked_sub(1)
            .ok_or(Error::InvalidTree)?;
        if active.len() < required {
            return Err(Error::InvalidTree);
        }
        let mut state = OracleEnumeration {
            graph,
            active: &active,
            required,
            best: None,
            candidates_checked: 0,
        };
        state.enumerate(0, &mut Vec::new())?;
        let (edges, weighted_stretch, total_weight) = state.best.ok_or(Error::InvalidTree)?;
        Ok(Self {
            edges,
            weighted_stretch,
            total_weight,
            candidates_checked: state.candidates_checked,
        })
    }
}

struct OracleEnumeration<'a> {
    graph: &'a SourceDynamicGraph,
    active: &'a [SourceEdgeId],
    required: usize,
    best: Option<(BTreeSet<SourceEdgeId>, ExactRatio, ExactRatio)>,
    candidates_checked: u64,
}

impl OracleEnumeration<'_> {
    fn enumerate(&mut self, cursor: usize, chosen: &mut Vec<SourceEdgeId>) -> Result<(), Error> {
        if chosen.len() == self.required {
            self.evaluate(chosen)?;
            return Ok(());
        }
        let needed = self.required - chosen.len();
        if self.active.len().saturating_sub(cursor) < needed {
            return Ok(());
        }
        for index in cursor..self.active.len() {
            chosen.push(self.active[index]);
            self.enumerate(index + 1, chosen)?;
            chosen.pop();
        }
        Ok(())
    }

    fn evaluate(&mut self, chosen: &[SourceEdgeId]) -> Result<(), Error> {
        let tree = match Tree::new(self.graph, chosen.iter().copied(), FlowNodeId(0)) {
            Ok(tree) => tree,
            Err(Error::InvalidTree) => return Ok(()),
            Err(error) => return Err(error),
        };
        self.candidates_checked = self
            .candidates_checked
            .checked_add(1)
            .ok_or(Error::Overflow)?;
        let roots = BTreeSet::from([FlowNodeId(0)]);
        let stretches =
            exact_forest_stretches(&tree, self.graph, &chosen.iter().copied().collect(), &roots)?;
        let mut weighted_stretch = ratio(0)?;
        let mut total_weight = ratio(0)?;
        for (index, stretch) in stretches.into_iter().enumerate() {
            let Some(edge) = self.graph.edge(SourceEdgeId(index)) else {
                continue;
            };
            total_weight = total_weight.checked_add(edge.weight).map_err(map_ratio)?;
            weighted_stretch = weighted_stretch
                .checked_add(edge.weight.checked_mul(stretch).map_err(map_ratio)?)
                .map_err(map_ratio)?;
        }
        let improves = match &self.best {
            None => true,
            Some((_, old, _)) => {
                old.at_least(weighted_stretch).map_err(map_ratio)? && *old != weighted_stretch
            }
        };
        let ties_with_lower_ids = self.best.as_ref().is_some_and(|(old_edges, old, _)| {
            *old == weighted_stretch && chosen.iter().copied().collect::<BTreeSet<_>>() < *old_edges
        });
        if improves || ties_with_lower_ids {
            self.best = Some((
                chosen.iter().copied().collect(),
                weighted_stretch,
                total_weight,
            ));
        }
        Ok(())
    }
}
