//! Immutable identifiers and source-tree snapshots for a dynamic tree chain.

use std::collections::BTreeSet;

use crate::{FlowNodeId, SourceEdgeId};

/// Stable identifier of one logical tree-chain level.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LevelId(pub usize);

/// Stable identifier of a branch across all levels of one tree chain.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BranchId(pub usize);

/// Immutable source-edge snapshot of one spanning tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tree {
    root: FlowNodeId,
    source_edges: BTreeSet<SourceEdgeId>,
}

impl Tree {
    /// Creates a source-edge tree snapshot whose graph validity is checked by
    /// [`super::chain::Chain::new`].
    #[must_use]
    pub const fn new(root: FlowNodeId, source_edges: BTreeSet<SourceEdgeId>) -> Self {
        Self { root, source_edges }
    }

    /// Returns the fixed root used by this tree snapshot.
    #[must_use]
    pub const fn root(&self) -> FlowNodeId {
        self.root
    }

    /// Returns the stable source-edge IDs of this immutable tree snapshot.
    #[must_use]
    pub const fn source_edges(&self) -> &BTreeSet<SourceEdgeId> {
        &self.source_edges
    }
}

/// One ordered branch of a logical tree-chain level.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    id: BranchId,
    slot: usize,
    tree: Tree,
}

impl Branch {
    /// Creates one branch with an explicit stable ID and shift slot.
    #[must_use]
    pub const fn new(id: BranchId, slot: usize, tree: Tree) -> Self {
        Self { id, slot, tree }
    }

    /// Returns the stable branch ID.
    #[must_use]
    pub const fn id(&self) -> BranchId {
        self.id
    }

    /// Returns the branch's deterministic shift slot within its level.
    #[must_use]
    pub const fn slot(&self) -> usize {
        self.slot
    }

    /// Returns this branch's immutable source-tree snapshot.
    #[must_use]
    pub const fn tree(&self) -> &Tree {
        &self.tree
    }
}

/// Ordered collection of shifted branches at one logical depth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Level {
    id: LevelId,
    branches: Vec<Branch>,
}

impl Level {
    /// Creates one logical level whose slots and tree certificates are checked
    /// by [`super::chain::Chain::new`].
    #[must_use]
    pub const fn new(id: LevelId, branches: Vec<Branch>) -> Self {
        Self { id, branches }
    }

    /// Returns the stable logical-level ID.
    #[must_use]
    pub const fn id(&self) -> LevelId {
        self.id
    }

    /// Returns the explicitly slotted branches in this level.
    #[must_use]
    pub fn branches(&self) -> &[Branch] {
        &self.branches
    }
}
