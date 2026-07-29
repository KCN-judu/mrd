use std::collections::{BTreeSet, VecDeque};

use thiserror::Error;

use crate::{ExactRatio, FlowNodeId};

pub mod bucket;
pub mod level;
pub mod oracle;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceEdgeId(pub usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceWeightedEdge {
    pub first: FlowNodeId,
    pub second: FlowNodeId,
    pub length: ExactRatio,
    pub weight: ExactRatio,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceGraphUpdate {
    Insert(SourceWeightedEdge),
    Delete(SourceEdgeId),
    SplitVertex {
        vertex: FlowNodeId,
        moved_edges: Vec<SourceEdgeId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceUpdateBatch {
    pub updates: Vec<SourceGraphUpdate>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceGraphMetrics {
    pub update_batches: u64,
    pub encoded_updates: u64,
    pub encoded_update_size: u64,
    pub edge_insertions: u64,
    pub edge_deletions: u64,
    pub vertex_splits: u64,
    pub initial_edges: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceEdgeState {
    edge: SourceWeightedEdge,
    active: bool,
    inserted_after_initialization: bool,
}

/// Checked dynamic graph domain shared by Lemma 5.4 and Theorem 1.2 audits.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDynamicGraph {
    node_count: usize,
    edges: Vec<SourceEdgeState>,
    maximum_abs_coordinate: i128,
    metrics: SourceGraphMetrics,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LsfPiece {
    pub vertices: BTreeSet<FlowNodeId>,
    pub forest_edges: BTreeSet<SourceEdgeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LsfStructuralCertificate {
    pub forest_edges: BTreeSet<SourceEdgeId>,
    pub roots: BTreeSet<FlowNodeId>,
    pub pieces: Vec<LsfPiece>,
    pub stretch_overestimates: Vec<ExactRatio>,
    pub piece_volume_limit: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LsfContractAudit {
    pub component_count: usize,
    pub piece_count: usize,
    pub maximum_piece_volume: u64,
    pub weighted_initial_stretch: ExactRatio,
    pub maximum_stretch: ExactRatio,
    pub inserted_edge_checks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSpannerCertificate {
    pub spanner_edges: BTreeSet<SourceEdgeId>,
    pub embedding_paths: Vec<Vec<SourceEdgeId>>,
    pub reembedded_spanner_edges: BTreeSet<SourceEdgeId>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceSpannerAudit {
    pub spanner_edge_count: u64,
    pub maximum_path_length: u64,
    pub maximum_vertex_congestion: u64,
    pub encoded_embedding_length: u64,
    pub reembedded_spanner_edges: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceStructureParameters {
    pub reduction_k: usize,
    pub spanner_l: usize,
    pub update_budget: u64,
    pub encoding_budget: u64,
    pub vertex_split_budget: u64,
    pub coordinate_log_exponent: u32,
    pub observed_coordinate_bits: u32,
    pub allowed_coordinate_bits: u32,
}

impl SourceDynamicGraph {
    /// Creates the exact positive-length/positive-weight source domain.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid endpoints, loops, nonpositive coordinates,
    /// or coordinates outside the supplied encoding bound.
    pub fn new(
        node_count: usize,
        edges: Vec<SourceWeightedEdge>,
        maximum_abs_coordinate: i128,
    ) -> Result<Self, SourceLsstError> {
        if node_count == 0 || maximum_abs_coordinate <= 0 {
            return Err(SourceLsstError::InvalidDomain);
        }
        for edge in &edges {
            validate_edge(node_count, edge, maximum_abs_coordinate)?;
        }
        let initial_edges = u64::try_from(edges.len()).map_err(|_| SourceLsstError::Overflow)?;
        Ok(Self {
            node_count,
            edges: edges
                .into_iter()
                .map(|edge| SourceEdgeState {
                    edge,
                    active: true,
                    inserted_after_initialization: false,
                })
                .collect(),
            maximum_abs_coordinate,
            metrics: SourceGraphMetrics {
                initial_edges,
                ..SourceGraphMetrics::default()
            },
        })
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.node_count
    }

    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub const fn metrics(&self) -> SourceGraphMetrics {
        self.metrics
    }

    #[must_use]
    pub const fn maximum_abs_coordinate(&self) -> i128 {
        self.maximum_abs_coordinate
    }

    #[must_use]
    pub fn edge(&self, id: SourceEdgeId) -> Option<&SourceWeightedEdge> {
        self.edges
            .get(id.0)
            .filter(|edge| edge.active)
            .map(|edge| &edge.edge)
    }

    pub(crate) fn split_projection_edge(
        &mut self,
        id: SourceEdgeId,
        from: FlowNodeId,
        offset: ExactRatio,
    ) -> Result<(FlowNodeId, SourceEdgeId, SourceEdgeId), SourceLsstError> {
        let original = self
            .edges
            .get(id.0)
            .filter(|edge| edge.active)
            .cloned()
            .ok_or(SourceLsstError::EdgeOutOfBounds)?;
        let toward = if original.edge.first == from {
            original.edge.second
        } else if original.edge.second == from {
            original.edge.first
        } else {
            return Err(SourceLsstError::InvalidDomain);
        };
        let remainder = original
            .edge
            .length
            .checked_sub(offset)
            .map_err(|_| SourceLsstError::Overflow)?;
        if !offset.is_positive() || !remainder.is_positive() {
            return Err(SourceLsstError::InvalidDomain);
        }
        let portal = FlowNodeId(self.node_count);
        let next_node_count = self
            .node_count
            .checked_add(1)
            .ok_or(SourceLsstError::Overflow)?;
        let first = SourceWeightedEdge {
            first: from,
            second: portal,
            length: offset,
            weight: original.edge.weight,
        };
        let second = SourceWeightedEdge {
            first: portal,
            second: toward,
            length: remainder,
            weight: original.edge.weight,
        };
        let next_bound = [first.length, first.weight, second.length, second.weight]
            .into_iter()
            .try_fold(self.maximum_abs_coordinate, |bound, value| {
                Ok::<_, SourceLsstError>(
                    bound
                        .max(
                            value
                                .numerator()
                                .checked_abs()
                                .ok_or(SourceLsstError::Overflow)?,
                        )
                        .max(value.denominator()),
                )
            })?;
        validate_edge(next_node_count, &first, next_bound)?;
        validate_edge(next_node_count, &second, next_bound)?;
        let next_initial_edges = self
            .metrics
            .initial_edges
            .checked_add(1)
            .ok_or(SourceLsstError::Overflow)?;

        self.edges[id.0].edge = first;
        let first_id = id;
        let second_id = SourceEdgeId(self.edges.len());
        self.edges.push(SourceEdgeState {
            edge: second,
            active: true,
            inserted_after_initialization: false,
        });
        self.node_count = next_node_count;
        self.maximum_abs_coordinate = next_bound;
        self.metrics.initial_edges = next_initial_edges;
        Ok((portal, first_id, second_id))
    }

    /// Applies one batch atomically after checking every source operation.
    ///
    /// # Errors
    ///
    /// Returns an error for duplicate/invalid deletions, invalid insertions or
    /// splits, coordinate-bound violations, or counter overflow.
    pub fn apply_batch(&mut self, batch: &SourceUpdateBatch) -> Result<(), SourceLsstError> {
        if batch.updates.is_empty() {
            return Err(SourceLsstError::InvalidUpdate);
        }
        let mut candidate = self.clone();
        let mut touched = BTreeSet::new();
        for update in &batch.updates {
            match update {
                SourceGraphUpdate::Insert(edge) => {
                    validate_edge(candidate.node_count, edge, candidate.maximum_abs_coordinate)?;
                    candidate.edges.push(SourceEdgeState {
                        edge: edge.clone(),
                        active: true,
                        inserted_after_initialization: true,
                    });
                    candidate.metrics.edge_insertions = candidate
                        .metrics
                        .edge_insertions
                        .checked_add(1)
                        .ok_or(SourceLsstError::Overflow)?;
                }
                SourceGraphUpdate::Delete(id) => {
                    if !touched.insert(*id) {
                        return Err(SourceLsstError::InvalidUpdate);
                    }
                    let edge = candidate
                        .edges
                        .get_mut(id.0)
                        .ok_or(SourceLsstError::EdgeOutOfBounds)?;
                    if !edge.active {
                        return Err(SourceLsstError::InvalidUpdate);
                    }
                    edge.active = false;
                    candidate.metrics.edge_deletions = candidate
                        .metrics
                        .edge_deletions
                        .checked_add(1)
                        .ok_or(SourceLsstError::Overflow)?;
                }
                SourceGraphUpdate::SplitVertex {
                    vertex,
                    moved_edges,
                } => {
                    candidate.apply_split(*vertex, moved_edges, &mut touched)?;
                }
            }
        }
        candidate.metrics.update_batches = candidate
            .metrics
            .update_batches
            .checked_add(1)
            .ok_or(SourceLsstError::Overflow)?;
        candidate.metrics.encoded_updates = candidate
            .metrics
            .encoded_updates
            .checked_add(u64::try_from(batch.updates.len()).map_err(|_| SourceLsstError::Overflow)?)
            .ok_or(SourceLsstError::Overflow)?;
        let encoded_size = batch.updates.iter().try_fold(0_u64, |sum, update| {
            let size = match update {
                SourceGraphUpdate::Insert(_) | SourceGraphUpdate::Delete(_) => 1,
                SourceGraphUpdate::SplitVertex { moved_edges, .. } => {
                    u64::try_from(moved_edges.len().max(1))
                        .map_err(|_| SourceLsstError::Overflow)?
                }
            };
            sum.checked_add(size).ok_or(SourceLsstError::Overflow)
        })?;
        candidate.metrics.encoded_update_size = candidate
            .metrics
            .encoded_update_size
            .checked_add(encoded_size)
            .ok_or(SourceLsstError::Overflow)?;
        *self = candidate;
        Ok(())
    }

    /// Verifies the exact structural portion of a Lemma 5.4 LSF certificate.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-forest, invalid root assignment, incomplete
    /// piece partition, excessive piece volume, or incorrect stretch bound.
    pub fn audit_lsf(
        &self,
        certificate: &LsfStructuralCertificate,
    ) -> Result<LsfContractAudit, SourceLsstError> {
        self.validate_certificate_dimensions(certificate)?;
        let adjacency = self.forest_adjacency(&certificate.forest_edges)?;
        let (component_of, components) = forest_components(self.node_count, &adjacency);
        for component in &components {
            if component
                .iter()
                .filter(|node| certificate.roots.contains(&FlowNodeId(**node)))
                .count()
                != 1
            {
                return Err(SourceLsstError::InvalidRoots);
            }
        }
        if certificate.forest_edges.len() + components.len() != self.node_count {
            return Err(SourceLsstError::InvalidForest);
        }
        let maximum_piece_volume = self.verify_pieces(certificate)?;
        let zero = ratio(0)?;
        let mut weighted_initial_stretch = zero;
        let mut maximum_stretch = zero;
        let mut inserted_edge_checks = 0_u64;
        for (index, state) in self.edges.iter().enumerate() {
            if !state.active {
                continue;
            }
            let exact = self.exact_stretch(
                SourceEdgeId(index),
                &adjacency,
                &component_of,
                &certificate.roots,
            )?;
            let bound = certificate.stretch_overestimates[index];
            if !bound.at_least(exact).map_err(map_ratio)? {
                return Err(SourceLsstError::InvalidStretch);
            }
            if state.inserted_after_initialization {
                if bound != ratio(1)? {
                    return Err(SourceLsstError::InvalidStretch);
                }
                inserted_edge_checks = inserted_edge_checks
                    .checked_add(1)
                    .ok_or(SourceLsstError::Overflow)?;
            } else {
                weighted_initial_stretch = weighted_initial_stretch
                    .checked_add(state.edge.weight.checked_mul(bound).map_err(map_ratio)?)
                    .map_err(map_ratio)?;
            }
            if bound.at_least(maximum_stretch).map_err(map_ratio)? {
                maximum_stretch = bound;
            }
        }
        Ok(LsfContractAudit {
            component_count: components.len(),
            piece_count: certificate.pieces.len(),
            maximum_piece_volume,
            weighted_initial_stretch,
            maximum_stretch,
            inserted_edge_checks,
        })
    }

    /// Verifies an explicit subgraph/path embedding and exact congestion/work
    /// counters used by Theorem 8.2's source contract.
    ///
    /// # Errors
    ///
    /// Returns an error when a spanner edge is inactive, an embedding path is
    /// missing or noncontiguous, or a re-embedding identifier is invalid.
    pub fn audit_spanner(
        &self,
        certificate: &SourceSpannerCertificate,
    ) -> Result<SourceSpannerAudit, SourceLsstError> {
        if certificate.embedding_paths.len() != self.edges.len()
            || certificate
                .spanner_edges
                .iter()
                .any(|id| self.edge(*id).is_none())
            || !certificate
                .reembedded_spanner_edges
                .is_subset(&certificate.spanner_edges)
        {
            return Err(SourceLsstError::InvalidEmbedding);
        }
        let mut congestion = vec![0_u64; self.node_count];
        let mut maximum_path_length = 0_u64;
        let mut encoded_embedding_length = 0_u64;
        for (index, state) in self.edges.iter().enumerate() {
            let path = &certificate.embedding_paths[index];
            if !state.active {
                if !path.is_empty() {
                    return Err(SourceLsstError::InvalidEmbedding);
                }
                continue;
            }
            let vertices = embedding_vertices(&state.edge, path, &certificate.spanner_edges, self)?;
            let path_length = u64::try_from(path.len()).map_err(|_| SourceLsstError::Overflow)?;
            maximum_path_length = maximum_path_length.max(path_length);
            encoded_embedding_length = encoded_embedding_length
                .checked_add(path_length)
                .ok_or(SourceLsstError::Overflow)?;
            for vertex in vertices {
                congestion[vertex] = congestion[vertex]
                    .checked_add(1)
                    .ok_or(SourceLsstError::Overflow)?;
            }
        }
        Ok(SourceSpannerAudit {
            spanner_edge_count: u64::try_from(certificate.spanner_edges.len())
                .map_err(|_| SourceLsstError::Overflow)?,
            maximum_path_length,
            maximum_vertex_congestion: congestion.into_iter().max().unwrap_or(0),
            encoded_embedding_length,
            reembedded_spanner_edges: u64::try_from(certificate.reembedded_spanner_edges.len())
                .map_err(|_| SourceLsstError::Overflow)?,
        })
    }

    /// Checks explicit reduction, update-domain, and quasipolynomial
    /// coordinate bounds before a source runtime statement may consume them.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid `k`/`L`, exhausted update/split budgets, or
    /// a coordinate bit width above `ceil(log2(n+1))^exponent`.
    pub fn audit_source_parameters(
        &self,
        reduction_k: usize,
        spanner_l: usize,
        update_budget: u64,
        encoding_budget: u64,
        vertex_split_budget: u64,
        coordinate_log_exponent: u32,
    ) -> Result<SourceStructureParameters, SourceLsstError> {
        if reduction_k == 0
            || reduction_k > self.edges.len().max(1)
            || spanner_l == 0
            || coordinate_log_exponent == 0
            || self.metrics.encoded_updates > update_budget
            || self.metrics.encoded_update_size > encoding_budget
            || self.metrics.vertex_splits > vertex_split_budget
        {
            return Err(SourceLsstError::InvalidParameters);
        }
        let logarithmic_base = ceil_log2_usize(
            self.node_count
                .checked_add(1)
                .ok_or(SourceLsstError::Overflow)?,
        )
        .max(2);
        let allowed_coordinate_bits = logarithmic_base
            .checked_pow(coordinate_log_exponent)
            .ok_or(SourceLsstError::Overflow)?;
        let observed_coordinate_bits = bit_length_i128(self.maximum_abs_coordinate)?;
        if observed_coordinate_bits > allowed_coordinate_bits {
            return Err(SourceLsstError::InvalidParameters);
        }
        Ok(SourceStructureParameters {
            reduction_k,
            spanner_l,
            update_budget,
            encoding_budget,
            vertex_split_budget,
            coordinate_log_exponent,
            observed_coordinate_bits,
            allowed_coordinate_bits,
        })
    }

    fn apply_split(
        &mut self,
        vertex: FlowNodeId,
        moved_edges: &[SourceEdgeId],
        touched: &mut BTreeSet<SourceEdgeId>,
    ) -> Result<(), SourceLsstError> {
        if vertex.0 >= self.node_count {
            return Err(SourceLsstError::InvalidUpdate);
        }
        let incident_count = self
            .edges
            .iter()
            .filter(|state| {
                state.active && (state.edge.first == vertex || state.edge.second == vertex)
            })
            .count();
        let split = FlowNodeId(self.node_count);
        for id in moved_edges {
            if !touched.insert(*id) {
                return Err(SourceLsstError::InvalidUpdate);
            }
            let state = self
                .edges
                .get_mut(id.0)
                .ok_or(SourceLsstError::EdgeOutOfBounds)?;
            if !state.active {
                return Err(SourceLsstError::InvalidUpdate);
            }
            if state.edge.first == vertex {
                state.edge.first = split;
            } else if state.edge.second == vertex {
                state.edge.second = split;
            } else {
                return Err(SourceLsstError::InvalidUpdate);
            }
        }
        let remaining = incident_count
            .checked_sub(moved_edges.len())
            .ok_or(SourceLsstError::InvalidUpdate)?;
        if moved_edges.len() > remaining {
            return Err(SourceLsstError::InvalidUpdate);
        }
        self.node_count = self
            .node_count
            .checked_add(1)
            .ok_or(SourceLsstError::Overflow)?;
        self.metrics.vertex_splits = self
            .metrics
            .vertex_splits
            .checked_add(1)
            .ok_or(SourceLsstError::Overflow)?;
        Ok(())
    }

    fn validate_certificate_dimensions(
        &self,
        certificate: &LsfStructuralCertificate,
    ) -> Result<(), SourceLsstError> {
        if certificate.stretch_overestimates.len() != self.edges.len()
            || certificate
                .forest_edges
                .iter()
                .any(|id| self.edge(*id).is_none())
            || certificate
                .roots
                .iter()
                .any(|root| root.0 >= self.node_count)
        {
            return Err(SourceLsstError::InvalidCertificate);
        }
        Ok(())
    }

    fn forest_adjacency(
        &self,
        forest_edges: &BTreeSet<SourceEdgeId>,
    ) -> Result<Vec<Vec<(usize, SourceEdgeId)>>, SourceLsstError> {
        let mut adjacency = vec![Vec::new(); self.node_count];
        for id in forest_edges {
            let edge = self.edge(*id).ok_or(SourceLsstError::InvalidForest)?;
            adjacency[edge.first.0].push((edge.second.0, *id));
            adjacency[edge.second.0].push((edge.first.0, *id));
        }
        Ok(adjacency)
    }

    fn verify_pieces(
        &self,
        certificate: &LsfStructuralCertificate,
    ) -> Result<u64, SourceLsstError> {
        let mut assigned = BTreeSet::new();
        let mut membership = vec![0_u64; self.node_count];
        for piece in &certificate.pieces {
            if piece.vertices.is_empty()
                || piece
                    .vertices
                    .iter()
                    .any(|vertex| vertex.0 >= self.node_count)
            {
                return Err(SourceLsstError::InvalidPieces);
            }
            for vertex in &piece.vertices {
                membership[vertex.0] = membership[vertex.0]
                    .checked_add(1)
                    .ok_or(SourceLsstError::Overflow)?;
            }
            for id in &piece.forest_edges {
                let edge = self.edge(*id).ok_or(SourceLsstError::InvalidPieces)?;
                if !certificate.forest_edges.contains(id)
                    || !piece.vertices.contains(&edge.first)
                    || !piece.vertices.contains(&edge.second)
                    || !assigned.insert(*id)
                {
                    return Err(SourceLsstError::InvalidPieces);
                }
            }
            verify_piece_tree(piece, self)?;
        }
        if assigned != certificate.forest_edges || membership.contains(&0) {
            return Err(SourceLsstError::InvalidPieces);
        }
        let mut maximum = 0_u64;
        for piece in &certificate.pieces {
            let boundary_count = piece
                .vertices
                .iter()
                .filter(|vertex| membership[vertex.0] > 1)
                .count();
            if boundary_count > 1 {
                return Err(SourceLsstError::InvalidPieces);
            }
            let volume = self.piece_volume(piece, &certificate.roots)?;
            if volume > certificate.piece_volume_limit {
                return Err(SourceLsstError::PieceVolumeExceeded);
            }
            maximum = maximum.max(volume);
        }
        Ok(maximum)
    }

    fn piece_volume(
        &self,
        piece: &LsfPiece,
        roots: &BTreeSet<FlowNodeId>,
    ) -> Result<u64, SourceLsstError> {
        let mut volume = 0_u64;
        for state in &self.edges {
            if !state.active {
                continue;
            }
            for endpoint in [state.edge.first, state.edge.second] {
                if piece.vertices.contains(&endpoint) && !roots.contains(&endpoint) {
                    volume = volume.checked_add(1).ok_or(SourceLsstError::Overflow)?;
                }
            }
        }
        Ok(volume)
    }

    fn exact_stretch(
        &self,
        id: SourceEdgeId,
        adjacency: &[Vec<(usize, SourceEdgeId)>],
        component_of: &[usize],
        roots: &BTreeSet<FlowNodeId>,
    ) -> Result<ExactRatio, SourceLsstError> {
        let edge = self.edge(id).ok_or(SourceLsstError::EdgeOutOfBounds)?;
        let route = if component_of[edge.first.0] == component_of[edge.second.0] {
            forest_distance(self, adjacency, edge.first.0, edge.second.0)?
        } else {
            let first_root = component_root(component_of, edge.first.0, roots)?;
            let second_root = component_root(component_of, edge.second.0, roots)?;
            forest_distance(self, adjacency, edge.first.0, first_root)?
                .checked_add(forest_distance(
                    self,
                    adjacency,
                    edge.second.0,
                    second_root,
                )?)
                .map_err(map_ratio)?
        };
        edge.length
            .checked_add(route)
            .and_then(|value| value.checked_mul(edge.length.reciprocal()?))
            .map_err(map_ratio)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceLsstError {
    #[error("source graph domain or coordinate bound is invalid")]
    InvalidDomain,
    #[error("dynamic update batch is invalid")]
    InvalidUpdate,
    #[error("edge is outside the graph")]
    EdgeOutOfBounds,
    #[error("LSF certificate dimensions or identifiers are invalid")]
    InvalidCertificate,
    #[error("certificate edges do not form a forest")]
    InvalidForest,
    #[error("each forest component must have exactly one root")]
    InvalidRoots,
    #[error("forest pieces do not form the required edge partition")]
    InvalidPieces,
    #[error("a forest piece exceeds its explicit volume bound")]
    PieceVolumeExceeded,
    #[error("a stretch overestimate is invalid")]
    InvalidStretch,
    #[error("spanner subgraph or explicit path embedding is invalid")]
    InvalidEmbedding,
    #[error("source reduction parameters or bounded-coordinate assumptions are invalid")]
    InvalidParameters,
    #[error("checked source accounting overflowed")]
    Overflow,
}

fn validate_edge(
    node_count: usize,
    edge: &SourceWeightedEdge,
    maximum_abs_coordinate: i128,
) -> Result<(), SourceLsstError> {
    if edge.first.0 >= node_count
        || edge.second.0 >= node_count
        || edge.first == edge.second
        || !edge.length.is_positive()
        || !edge.weight.is_positive()
        || !ratio_within(edge.length, maximum_abs_coordinate)?
        || !ratio_within(edge.weight, maximum_abs_coordinate)?
    {
        return Err(SourceLsstError::InvalidDomain);
    }
    Ok(())
}

fn ratio_within(value: ExactRatio, bound: i128) -> Result<bool, SourceLsstError> {
    let numerator = value
        .numerator()
        .checked_abs()
        .ok_or(SourceLsstError::Overflow)?;
    Ok(numerator <= bound && value.denominator() <= bound)
}

fn forest_components(
    node_count: usize,
    adjacency: &[Vec<(usize, SourceEdgeId)>],
) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut component_of = vec![usize::MAX; node_count];
    let mut components = Vec::new();
    for start in 0..node_count {
        if component_of[start] != usize::MAX {
            continue;
        }
        let id = components.len();
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        component_of[start] = id;
        while let Some(node) = queue.pop_front() {
            component.push(node);
            for (next, _) in &adjacency[node] {
                if component_of[*next] == usize::MAX {
                    component_of[*next] = id;
                    queue.push_back(*next);
                }
            }
        }
        components.push(component);
    }
    (component_of, components)
}

fn verify_piece_tree(piece: &LsfPiece, graph: &SourceDynamicGraph) -> Result<(), SourceLsstError> {
    if piece.forest_edges.len().checked_add(1) != Some(piece.vertices.len()) {
        return Err(SourceLsstError::InvalidPieces);
    }
    let start = piece
        .vertices
        .first()
        .ok_or(SourceLsstError::InvalidPieces)?
        .0;
    let mut adjacency = vec![Vec::new(); graph.node_count];
    for id in &piece.forest_edges {
        let edge = graph.edge(*id).ok_or(SourceLsstError::InvalidPieces)?;
        adjacency[edge.first.0].push(edge.second.0);
        adjacency[edge.second.0].push(edge.first.0);
    }
    let mut seen = BTreeSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        for next in &adjacency[node] {
            if seen.insert(*next) {
                queue.push_back(*next);
            }
        }
    }
    if seen != piece.vertices.iter().map(|vertex| vertex.0).collect() {
        return Err(SourceLsstError::InvalidPieces);
    }
    Ok(())
}

fn component_root(
    component_of: &[usize],
    node: usize,
    roots: &BTreeSet<FlowNodeId>,
) -> Result<usize, SourceLsstError> {
    roots
        .iter()
        .find(|root| component_of[root.0] == component_of[node])
        .map(|root| root.0)
        .ok_or(SourceLsstError::InvalidRoots)
}

fn forest_distance(
    graph: &SourceDynamicGraph,
    adjacency: &[Vec<(usize, SourceEdgeId)>],
    start: usize,
    target: usize,
) -> Result<ExactRatio, SourceLsstError> {
    let zero = ratio(0)?;
    let mut queue = VecDeque::from([(start, zero)]);
    let mut seen = vec![false; graph.node_count];
    seen[start] = true;
    while let Some((node, distance)) = queue.pop_front() {
        if node == target {
            return Ok(distance);
        }
        for (next, id) in &adjacency[node] {
            if !seen[*next] {
                seen[*next] = true;
                let length = graph
                    .edge(*id)
                    .ok_or(SourceLsstError::InvalidForest)?
                    .length;
                queue.push_back((*next, distance.checked_add(length).map_err(map_ratio)?));
            }
        }
    }
    Err(SourceLsstError::InvalidForest)
}

fn embedding_vertices(
    input: &SourceWeightedEdge,
    path: &[SourceEdgeId],
    spanner_edges: &BTreeSet<SourceEdgeId>,
    graph: &SourceDynamicGraph,
) -> Result<Vec<usize>, SourceLsstError> {
    if path.is_empty() {
        return Err(SourceLsstError::InvalidEmbedding);
    }
    for start in [input.first.0, input.second.0] {
        let mut current = start;
        let mut vertices = vec![start];
        let mut seen = BTreeSet::from([start]);
        let mut valid = true;
        for id in path {
            if !spanner_edges.contains(id) {
                valid = false;
                break;
            }
            let edge = graph.edge(*id).ok_or(SourceLsstError::InvalidEmbedding)?;
            let next = if edge.first.0 == current {
                edge.second.0
            } else if edge.second.0 == current {
                edge.first.0
            } else {
                valid = false;
                break;
            };
            if !seen.insert(next) {
                valid = false;
                break;
            }
            vertices.push(next);
            current = next;
        }
        let target = if start == input.first.0 {
            input.second.0
        } else {
            input.first.0
        };
        if valid && current == target {
            return Ok(vertices);
        }
    }
    Err(SourceLsstError::InvalidEmbedding)
}

fn ratio(value: i128) -> Result<ExactRatio, SourceLsstError> {
    ExactRatio::new(value, 1).map_err(map_ratio)
}

fn map_ratio(_: crate::StableMinRatioError) -> SourceLsstError {
    SourceLsstError::Overflow
}

fn ceil_log2_usize(value: usize) -> u32 {
    usize::BITS - value.saturating_sub(1).leading_zeros()
}

fn bit_length_i128(value: i128) -> Result<u32, SourceLsstError> {
    let magnitude = value
        .checked_abs()
        .ok_or(SourceLsstError::Overflow)?
        .unsigned_abs();
    Ok((i128::BITS - magnitude.leading_zeros()).max(1))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        LsfPiece, LsfStructuralCertificate, SourceDynamicGraph, SourceEdgeId, SourceGraphUpdate,
        SourceLsstError, SourceSpannerCertificate, SourceUpdateBatch, SourceWeightedEdge,
    };
    use crate::{ExactRatio, FlowNodeId};

    fn edge(first: usize, second: usize) -> SourceWeightedEdge {
        SourceWeightedEdge {
            first: FlowNodeId(first),
            second: FlowNodeId(second),
            length: ExactRatio::new(1, 1).unwrap(),
            weight: ExactRatio::new(1, 1).unwrap(),
        }
    }

    #[test]
    fn audits_definition_five_three_and_piece_contracts() {
        let graph =
            SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap();
        let certificate = LsfStructuralCertificate {
            forest_edges: BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)]),
            roots: BTreeSet::from([FlowNodeId(0)]),
            pieces: vec![
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]),
                    forest_edges: BTreeSet::from([SourceEdgeId(0)]),
                },
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(1), FlowNodeId(2)]),
                    forest_edges: BTreeSet::from([SourceEdgeId(1)]),
                },
            ],
            stretch_overestimates: vec![
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(2, 1).unwrap(),
                ExactRatio::new(3, 1).unwrap(),
            ],
            piece_volume_limit: 4,
        };
        let audit = graph.audit_lsf(&certificate).unwrap();
        assert_eq!(audit.component_count, 1);
        assert_eq!(audit.piece_count, 2);
        assert_eq!(audit.maximum_piece_volume, 4);
        assert_eq!(
            audit.weighted_initial_stretch,
            ExactRatio::new(7, 1).unwrap()
        );
        assert_eq!(audit.maximum_stretch, ExactRatio::new(3, 1).unwrap());
    }

    #[test]
    fn applies_update_batches_atomically_with_split_accounting() {
        let mut graph = SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2)], 8).unwrap();
        graph
            .apply_batch(&SourceUpdateBatch {
                updates: vec![
                    SourceGraphUpdate::SplitVertex {
                        vertex: FlowNodeId(1),
                        moved_edges: vec![SourceEdgeId(1)],
                    },
                    SourceGraphUpdate::Insert(edge(1, 2)),
                ],
            })
            .unwrap();
        assert_eq!(graph.node_count(), 4);
        assert_eq!(graph.edge_count(), 3);
        assert_eq!(graph.metrics().update_batches, 1);
        assert_eq!(graph.metrics().encoded_updates, 2);
        assert_eq!(graph.metrics().encoded_update_size, 2);
        assert_eq!(graph.metrics().vertex_splits, 1);
        assert_eq!(graph.metrics().edge_insertions, 1);

        let before = graph.clone();
        assert_eq!(
            graph.apply_batch(&SourceUpdateBatch {
                updates: vec![
                    SourceGraphUpdate::Delete(SourceEdgeId(0)),
                    SourceGraphUpdate::Delete(SourceEdgeId(0)),
                ],
            }),
            Err(SourceLsstError::InvalidUpdate)
        );
        assert_eq!(graph, before);
    }

    #[test]
    fn requires_unit_stretch_for_inserted_root_edge() {
        let mut graph = SourceDynamicGraph::new(2, Vec::new(), 8).unwrap();
        graph
            .apply_batch(&SourceUpdateBatch {
                updates: vec![SourceGraphUpdate::Insert(edge(0, 1))],
            })
            .unwrap();
        let certificate = LsfStructuralCertificate {
            forest_edges: BTreeSet::new(),
            roots: BTreeSet::from([FlowNodeId(0), FlowNodeId(1)]),
            pieces: vec![
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(0)]),
                    forest_edges: BTreeSet::new(),
                },
                LsfPiece {
                    vertices: BTreeSet::from([FlowNodeId(1)]),
                    forest_edges: BTreeSet::new(),
                },
            ],
            stretch_overestimates: vec![ExactRatio::new(1, 1).unwrap()],
            piece_volume_limit: 0,
        };
        let audit = graph.audit_lsf(&certificate).unwrap();
        assert_eq!(audit.inserted_edge_checks, 1);
    }

    #[test]
    fn audits_explicit_spanner_paths_and_vertex_congestion() {
        let graph =
            SourceDynamicGraph::new(3, vec![edge(0, 1), edge(1, 2), edge(0, 2)], 8).unwrap();
        let audit = graph
            .audit_spanner(&SourceSpannerCertificate {
                spanner_edges: BTreeSet::from([SourceEdgeId(0), SourceEdgeId(1)]),
                embedding_paths: vec![
                    vec![SourceEdgeId(0)],
                    vec![SourceEdgeId(1)],
                    vec![SourceEdgeId(0), SourceEdgeId(1)],
                ],
                reembedded_spanner_edges: BTreeSet::from([SourceEdgeId(1)]),
            })
            .unwrap();
        assert_eq!(audit.spanner_edge_count, 2);
        assert_eq!(audit.maximum_path_length, 2);
        assert_eq!(audit.maximum_vertex_congestion, 3);
        assert_eq!(audit.encoded_embedding_length, 4);
        assert_eq!(audit.reembedded_spanner_edges, 1);
        let parameters = graph.audit_source_parameters(2, 1, 0, 0, 0, 2).unwrap();
        assert_eq!(parameters.observed_coordinate_bits, 4);
        assert_eq!(parameters.allowed_coordinate_bits, 4);
    }

    #[test]
    fn split_encoding_requires_the_smaller_side() {
        let mut graph =
            SourceDynamicGraph::new(4, vec![edge(0, 1), edge(0, 2), edge(0, 3)], 8).unwrap();
        assert_eq!(
            graph.apply_batch(&SourceUpdateBatch {
                updates: vec![SourceGraphUpdate::SplitVertex {
                    vertex: FlowNodeId(0),
                    moved_edges: vec![SourceEdgeId(0), SourceEdgeId(1)],
                }],
            }),
            Err(SourceLsstError::InvalidUpdate)
        );
        graph
            .apply_batch(&SourceUpdateBatch {
                updates: vec![SourceGraphUpdate::SplitVertex {
                    vertex: FlowNodeId(0),
                    moved_edges: vec![SourceEdgeId(0)],
                }],
            })
            .unwrap();
        assert_eq!(graph.metrics().encoded_updates, 1);
        assert_eq!(graph.metrics().encoded_update_size, 1);
    }
}
