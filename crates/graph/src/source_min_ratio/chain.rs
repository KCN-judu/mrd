//! Checked tree-chain selection and pure shifted-branch transitions.

use std::collections::{BTreeSet, VecDeque};

use thiserror::Error;

use crate::{FlowNodeId, SourceDynamicGraph};

use super::model::{Branch, BranchId, Level, LevelId};

/// A checked finite tree chain with one deterministic branch choice per level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chain {
    levels: Vec<Level>,
}

/// Immutable selector state for every ordered tree-chain level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shifts {
    offsets: Vec<usize>,
}

/// One branch selected by a valid [`Shifts`] state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Selection {
    /// The logical level containing the selected branch.
    pub level: LevelId,
    /// The selected branch's stable source identity.
    pub branch: BranchId,
    /// The deterministic slot selected at this level.
    pub slot: usize,
}

impl Chain {
    /// Validates immutable branch trees against the current source graph.
    ///
    /// # Errors
    ///
    /// Returns an error for empty or duplicate levels, noncontiguous branch
    /// slots, duplicate branch IDs, or a branch that is not a current source
    /// spanning tree.
    pub fn new(graph: &SourceDynamicGraph, levels: Vec<Level>) -> Result<Self, Error> {
        if levels.is_empty() {
            return Err(Error::EmptyChain);
        }
        let mut level_ids = BTreeSet::new();
        let mut branch_ids = BTreeSet::new();
        for level in &levels {
            if !level_ids.insert(level.id()) {
                return Err(Error::DuplicateLevel(level.id()));
            }
            validate_level(graph, level, &mut branch_ids)?;
        }
        Ok(Self { levels })
    }

    /// Returns the ordered immutable levels of this tree chain.
    #[must_use]
    pub fn levels(&self) -> &[Level] {
        &self.levels
    }

    /// Creates the first deterministic selection, choosing slot zero at every
    /// level.
    #[must_use]
    pub fn initial_shifts(&self) -> Shifts {
        Shifts {
            offsets: vec![0; self.levels.len()],
        }
    }

    /// Restores a persisted selector state after validating every offset.
    ///
    /// # Errors
    ///
    /// Returns an error when the state has the wrong number of levels or an
    /// offset does not name a branch slot in its level.
    pub fn shifts(&self, offsets: Vec<usize>) -> Result<Shifts, Error> {
        let shifts = Shifts { offsets };
        self.validate_shifts(&shifts)?;
        Ok(shifts)
    }

    /// Returns a new state after advancing one level and resetting descendants.
    ///
    /// This is a pure transition: the chain and prior shift state remain
    /// unchanged, so replay and mutation tests can retain both snapshots.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid prior selector state or unknown level.
    pub fn shift(&self, current: &Shifts, level: LevelId) -> Result<Shifts, Error> {
        self.validate_shifts(current)?;
        let index = self
            .levels
            .iter()
            .position(|candidate| candidate.id() == level)
            .ok_or(Error::UnknownLevel(level))?;
        let count = self.levels[index].branches().len();
        let mut offsets = current.offsets.clone();
        offsets[index] = offsets[index].checked_add(1).ok_or(Error::Overflow)? % count;
        offsets[index + 1..].fill(0);
        Ok(Shifts { offsets })
    }

    /// Resolves exactly one stable branch from every level in source order.
    ///
    /// # Errors
    ///
    /// Returns an error when the selector state is invalid.
    pub fn select(&self, shifts: &Shifts) -> Result<Vec<Selection>, Error> {
        self.validate_shifts(shifts)?;
        self.levels
            .iter()
            .zip(&shifts.offsets)
            .map(|(level, slot)| {
                let branch = level
                    .branches()
                    .iter()
                    .find(|branch| branch.slot() == *slot)
                    .ok_or(Error::InvalidShifts)?;
                Ok(Selection {
                    level: level.id(),
                    branch: branch.id(),
                    slot: *slot,
                })
            })
            .collect()
    }

    /// Resolves one selected branch by its stable selection record.
    ///
    /// # Errors
    ///
    /// Returns an error when the selection record does not match this chain.
    pub fn branch(&self, selection: Selection) -> Result<&Branch, Error> {
        let level = self
            .levels
            .iter()
            .find(|level| level.id() == selection.level)
            .ok_or(Error::UnknownLevel(selection.level))?;
        let branch = level
            .branches()
            .iter()
            .find(|branch| branch.slot() == selection.slot)
            .ok_or(Error::InvalidSelection)?;
        if branch.id() != selection.branch {
            return Err(Error::InvalidSelection);
        }
        Ok(branch)
    }

    fn validate_shifts(&self, shifts: &Shifts) -> Result<(), Error> {
        if shifts.offsets.len() != self.levels.len()
            || self
                .levels
                .iter()
                .zip(&shifts.offsets)
                .any(|(level, offset)| *offset >= level.branches().len())
        {
            return Err(Error::InvalidShifts);
        }
        Ok(())
    }
}

impl Shifts {
    /// Returns the selected slots in logical-level order.
    #[must_use]
    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }
}

fn validate_level(
    graph: &SourceDynamicGraph,
    level: &Level,
    branch_ids: &mut BTreeSet<BranchId>,
) -> Result<(), Error> {
    if level.branches().is_empty() {
        return Err(Error::EmptyLevel(level.id()));
    }
    let mut slots = BTreeSet::new();
    for branch in level.branches() {
        if !branch_ids.insert(branch.id()) {
            return Err(Error::DuplicateBranch(branch.id()));
        }
        if !slots.insert(branch.slot()) {
            return Err(Error::DuplicateSlot {
                level: level.id(),
                slot: branch.slot(),
            });
        }
        validate_tree(graph, branch)?;
    }
    if slots.iter().copied().eq(0..level.branches().len()) {
        Ok(())
    } else {
        Err(Error::NoncontiguousSlots(level.id()))
    }
}

fn validate_tree(graph: &SourceDynamicGraph, branch: &Branch) -> Result<(), Error> {
    let tree = branch.tree();
    if tree.root().0 >= graph.node_count()
        || tree.source_edges().len().checked_add(1) != Some(graph.node_count())
    {
        return Err(Error::InvalidTree(branch.id()));
    }
    let mut adjacency = vec![Vec::<FlowNodeId>::new(); graph.node_count()];
    for source in tree.source_edges() {
        let edge = graph.edge(*source).ok_or(Error::InvalidTree(branch.id()))?;
        adjacency[edge.first.0].push(edge.second);
        adjacency[edge.second.0].push(edge.first);
    }
    let mut seen = BTreeSet::from([tree.root()]);
    let mut queue = VecDeque::from([tree.root()]);
    while let Some(vertex) = queue.pop_front() {
        for next in &adjacency[vertex.0] {
            if seen.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    if seen.len() == graph.node_count() {
        Ok(())
    } else {
        Err(Error::InvalidTree(branch.id()))
    }
}

/// A finite source tree-chain contract cannot be constructed or selected.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum Error {
    /// The chain must contain at least one level.
    #[error("tree chain has no levels")]
    EmptyChain,
    /// A logical level has no shifted branches.
    #[error("tree-chain level {0:?} has no branches")]
    EmptyLevel(LevelId),
    /// A logical level identity occurs more than once.
    #[error("tree-chain level ID {0:?} is duplicated")]
    DuplicateLevel(LevelId),
    /// A stable branch identity occurs more than once.
    #[error("tree-chain branch ID {0:?} is duplicated")]
    DuplicateBranch(BranchId),
    /// More than one branch was assigned one shift slot.
    #[error("tree-chain level {level:?} has duplicate branch slot {slot}")]
    DuplicateSlot {
        /// The malformed logical level.
        level: LevelId,
        /// The duplicated shift slot.
        slot: usize,
    },
    /// Branch slots must be exactly `0..branch_count`.
    #[error("tree-chain level {0:?} has noncontiguous branch slots")]
    NoncontiguousSlots(LevelId),
    /// A branch source-edge snapshot is not a current spanning tree.
    #[error("tree-chain branch {0:?} is not a current source spanning tree")]
    InvalidTree(BranchId),
    /// A persisted shift state does not match the chain's level and slot shape.
    #[error("tree-chain shift state is invalid")]
    InvalidShifts,
    /// The requested logical level is absent.
    #[error("tree-chain level {0:?} is unknown")]
    UnknownLevel(LevelId),
    /// A stable selection does not match this tree chain.
    #[error("tree-chain branch selection is invalid")]
    InvalidSelection,
    /// The finite transition counter overflowed.
    #[error("tree-chain shift counter overflowed")]
    Overflow,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Chain, Error};
    use crate::{
        ExactRatio, FlowNodeId, SourceDynamicGraph, SourceEdgeId, SourceWeightedEdge,
        source_min_ratio::model::{Branch, BranchId, Level, LevelId, Tree},
    };

    fn graph() -> SourceDynamicGraph {
        SourceDynamicGraph::new(
            3,
            vec![
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(1),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(1),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
                SourceWeightedEdge {
                    first: FlowNodeId(0),
                    second: FlowNodeId(2),
                    length: ExactRatio::new(1, 1).unwrap(),
                    weight: ExactRatio::new(1, 1).unwrap(),
                },
            ],
            8,
        )
        .unwrap()
    }

    fn branch(id: usize, slot: usize, edges: &[usize]) -> Branch {
        Branch::new(
            BranchId(id),
            slot,
            Tree::new(
                FlowNodeId(0),
                edges.iter().copied().map(SourceEdgeId).collect(),
            ),
        )
    }

    fn chain() -> Chain {
        Chain::new(
            &graph(),
            vec![
                Level::new(
                    LevelId(3),
                    vec![branch(10, 0, &[0, 1]), branch(11, 1, &[0, 2])],
                ),
                Level::new(
                    LevelId(7),
                    vec![branch(20, 0, &[0, 1]), branch(21, 1, &[1, 2])],
                ),
            ],
        )
        .unwrap()
    }

    #[test]
    fn selects_one_stable_branch_per_level_and_keeps_prior_state_immutable() {
        let chain = chain();
        let initial = chain.initial_shifts();
        assert_eq!(initial.offsets(), &[0, 0]);
        assert_eq!(
            chain.select(&initial).unwrap(),
            vec![
                super::Selection {
                    level: LevelId(3),
                    branch: BranchId(10),
                    slot: 0,
                },
                super::Selection {
                    level: LevelId(7),
                    branch: BranchId(20),
                    slot: 0,
                },
            ]
        );

        let shifted = chain.shift(&initial, LevelId(7)).unwrap();
        assert_eq!(initial.offsets(), &[0, 0]);
        assert_eq!(shifted.offsets(), &[0, 1]);
        assert_eq!(
            chain.select(&shifted).unwrap()[1],
            super::Selection {
                level: LevelId(7),
                branch: BranchId(21),
                slot: 1,
            }
        );

        let parent_shift = chain.shift(&shifted, LevelId(3)).unwrap();
        assert_eq!(parent_shift.offsets(), &[1, 0]);
        let selected = chain.select(&parent_shift).unwrap();
        assert_eq!(selected[0].branch, BranchId(11));
        assert_eq!(selected[1].branch, BranchId(20));
        assert_eq!(
            chain.branch(selected[0]).unwrap().tree().source_edges(),
            &BTreeSet::from([SourceEdgeId(0), SourceEdgeId(2)])
        );
    }

    #[test]
    fn rejects_malformed_levels_and_non_tree_branch_certificates() {
        let graph = graph();
        assert_eq!(Chain::new(&graph, Vec::new()), Err(Error::EmptyChain));
        assert_eq!(
            Chain::new(
                &graph,
                vec![
                    Level::new(LevelId(0), vec![branch(1, 0, &[0, 1])]),
                    Level::new(LevelId(0), vec![branch(2, 0, &[0, 2])]),
                ],
            ),
            Err(Error::DuplicateLevel(LevelId(0)))
        );
        assert_eq!(
            Chain::new(
                &graph,
                vec![
                    Level::new(LevelId(0), vec![branch(1, 0, &[0, 1])]),
                    Level::new(LevelId(1), vec![branch(1, 0, &[0, 2])]),
                ],
            ),
            Err(Error::DuplicateBranch(BranchId(1)))
        );
        assert_eq!(
            Chain::new(
                &graph,
                vec![Level::new(
                    LevelId(0),
                    vec![branch(1, 0, &[0, 1]), branch(2, 0, &[0, 2])],
                )],
            ),
            Err(Error::DuplicateSlot {
                level: LevelId(0),
                slot: 0,
            })
        );
        assert_eq!(
            Chain::new(
                &graph,
                vec![Level::new(LevelId(0), vec![branch(1, 1, &[0, 1])])],
            ),
            Err(Error::NoncontiguousSlots(LevelId(0)))
        );
        assert_eq!(
            Chain::new(
                &graph,
                vec![Level::new(LevelId(0), vec![branch(1, 0, &[0, 1, 2])])],
            ),
            Err(Error::InvalidTree(BranchId(1)))
        );
        assert_eq!(
            Chain::new(
                &graph,
                vec![Level::new(LevelId(0), vec![branch(1, 0, &[0, 99])])],
            ),
            Err(Error::InvalidTree(BranchId(1)))
        );
    }

    #[test]
    fn shift_slots_define_selection_independently_of_branch_storage_order() {
        let graph = graph();
        let chain = Chain::new(
            &graph,
            vec![Level::new(
                LevelId(0),
                vec![branch(2, 1, &[0, 2]), branch(1, 0, &[0, 1])],
            )],
        )
        .unwrap();
        assert_eq!(
            chain.select(&chain.initial_shifts()).unwrap()[0].branch,
            BranchId(1)
        );
        assert_eq!(
            chain
                .select(&chain.shift(&chain.initial_shifts(), LevelId(0)).unwrap())
                .unwrap()[0]
                .branch,
            BranchId(2)
        );
    }

    #[test]
    fn rejects_invalid_persisted_shifts_and_mismatched_stable_selections() {
        let chain = chain();
        assert_eq!(chain.shifts(vec![0]), Err(Error::InvalidShifts));
        assert_eq!(chain.shifts(vec![2, 0]), Err(Error::InvalidShifts));
        assert_eq!(
            chain.shift(&chain.initial_shifts(), LevelId(99)),
            Err(Error::UnknownLevel(LevelId(99)))
        );
        assert_eq!(
            chain.branch(super::Selection {
                level: LevelId(3),
                branch: BranchId(99),
                slot: 0,
            }),
            Err(Error::InvalidSelection)
        );
    }
}
