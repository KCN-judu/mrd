use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    Coord, GridComponent, OrthogonalLoop, Point, PolygonError, PreparedGridComponent,
    RectilinearPolygon,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundaryLoop {
    pub vertices: Vec<Point>,
    pub twice_signed_area: i128,
    pub is_hole: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BoundaryLoopId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct BoundaryVertexId {
    pub loop_id: BoundaryLoopId,
    pub cyclic_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ReflexVertex {
    pub point: Point,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Boundary {
    pub loops: Vec<BoundaryLoop>,
    pub reflex_vertices: Vec<ReflexVertex>,
    pub unit_edges: Vec<(Point, Point)>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundaryBuildMetrics {
    pub total_build_nanoseconds: u128,
    pub edge_discovery_nanoseconds: u128,
    pub adjacency_build_nanoseconds: u128,
    pub loop_tracing_nanoseconds: u128,
    pub loop_normalization_nanoseconds: u128,
    pub reflex_detection_nanoseconds: u128,
    pub unit_edge_sort_nanoseconds: u128,
    pub candidate_edge_probe_count: usize,
    pub exposed_unit_edge_count: usize,
    pub trace_edge_visit_count: usize,
    pub loop_count: usize,
    pub normalized_vertex_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BoundaryBuild {
    pub boundary: Boundary,
    pub metrics: BoundaryBuildMetrics,
}

type DirectedUnitEdge = (Point, Point);

struct EdgeDiscovery {
    edges: HashSet<DirectedUnitEdge>,
    candidate_edge_probe_count: usize,
}

struct LoopTrace {
    raw_loops: Vec<Vec<Point>>,
    edge_visit_count: usize,
}

/// Exact normalized-vertex lookup built once for a boundary.
///
/// `Boundary::vertex_id` remains the small reference convenience API.  This
/// index is the production lookup path for repeated endpoint metadata queries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundaryIndex {
    vertex_ids: HashMap<Point, BoundaryVertexId>,
    loop_lengths: Vec<usize>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BoundaryIndexError {
    #[error("normalized boundary vertex {point:?} occurs in loops {first:?} and {second:?}")]
    DuplicateVertex {
        point: Point,
        first: BoundaryVertexId,
        second: BoundaryVertexId,
    },
}

impl BoundaryIndex {
    /// Builds an exact index over every normalized vertex in every loop.
    ///
    /// # Errors
    ///
    /// Returns [`BoundaryIndexError::DuplicateVertex`] when normalization
    /// leaves the same point at more than one stable boundary identity.
    pub fn new(boundary: &Boundary) -> Result<Self, BoundaryIndexError> {
        let capacity = boundary
            .loops
            .iter()
            .map(|boundary_loop| boundary_loop.vertices.len())
            .sum();
        let mut vertex_ids = HashMap::with_capacity(capacity);
        let mut loop_lengths = Vec::with_capacity(boundary.loops.len());
        for (loop_index, boundary_loop) in boundary.loops.iter().enumerate() {
            loop_lengths.push(boundary_loop.vertices.len());
            for (cyclic_index, &point) in boundary_loop.vertices.iter().enumerate() {
                let id = BoundaryVertexId {
                    loop_id: BoundaryLoopId(loop_index),
                    cyclic_index,
                };
                if let Some(first) = vertex_ids.insert(point, id) {
                    return Err(BoundaryIndexError::DuplicateVertex {
                        point,
                        first,
                        second: id,
                    });
                }
            }
        }
        Ok(Self {
            vertex_ids,
            loop_lengths,
        })
    }

    /// Builds the historical first-occurrence view used only for malformed or
    /// point-contact reference inputs. Callers that require a strict index
    /// must use [`Self::new`] and handle its structured duplicate error.
    pub(crate) fn from_boundary_first_occurrence(boundary: &Boundary) -> Self {
        let mut vertex_ids = HashMap::new();
        let mut loop_lengths = Vec::with_capacity(boundary.loops.len());
        for (loop_index, boundary_loop) in boundary.loops.iter().enumerate() {
            loop_lengths.push(boundary_loop.vertices.len());
            for (cyclic_index, &point) in boundary_loop.vertices.iter().enumerate() {
                vertex_ids.entry(point).or_insert(BoundaryVertexId {
                    loop_id: BoundaryLoopId(loop_index),
                    cyclic_index,
                });
            }
        }
        Self {
            vertex_ids,
            loop_lengths,
        }
    }

    #[must_use]
    pub fn vertex_id(&self, point: Point) -> Option<BoundaryVertexId> {
        self.vertex_ids.get(&point).copied()
    }

    #[must_use]
    pub fn loop_len(&self, loop_id: BoundaryLoopId) -> Option<usize> {
        self.loop_lengths.get(loop_id.0).copied()
    }

    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.vertex_ids.len()
    }

    #[must_use]
    pub fn owned_bytes_estimate(&self) -> usize {
        self.vertex_ids.len()
            * (std::mem::size_of::<Point>()
                + std::mem::size_of::<BoundaryVertexId>()
                + 2 * std::mem::size_of::<usize>())
            + self.loop_lengths.len() * std::mem::size_of::<usize>()
    }
}

impl Boundary {
    /// Builds compact normalized boundary metadata from an ordinary polygon.
    ///
    /// Long polygon edges remain compact; `unit_edges` is intentionally empty
    /// because it is a grid-source diagnostic rather than boundary semantics.
    ///
    /// # Panics
    ///
    /// Panics only if a polygon constructed outside the validated constructor
    /// contains an area that cannot be represented in `i128`.
    #[must_use]
    pub fn from_polygon(polygon: &RectilinearPolygon) -> Self {
        let loops = std::iter::once((&polygon.outer, false))
            .chain(
                polygon
                    .holes
                    .iter()
                    .map(|boundary_loop| (boundary_loop, true)),
            )
            .map(|(boundary_loop, is_hole)| BoundaryLoop {
                vertices: boundary_loop.vertices.clone(),
                twice_signed_area: boundary_loop
                    .twice_signed_area()
                    .expect("validated polygon area fits i128"),
                is_hole,
            })
            .collect::<Vec<_>>();
        let reflex_vertices = loops
            .iter()
            .flat_map(|boundary_loop| reflex_points(&boundary_loop.vertices))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|point| ReflexVertex { point })
            .collect();
        Self {
            loops,
            reflex_vertices,
            unit_edges: Vec::new(),
        }
    }

    /// Converts an ordinary one-outer-loop boundary into the normalized
    /// boundary-native polygon model.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonError`] when the boundary contains a self-contact,
    /// degenerate hole, multiple outer loops, or another unsupported topology.
    pub fn to_polygon(&self) -> Result<RectilinearPolygon, PolygonError> {
        let mut outer = self
            .loops
            .iter()
            .filter(|boundary_loop| !boundary_loop.is_hole);
        let first = outer.next().ok_or(PolygonError::DisconnectedInterior)?;
        if outer.next().is_some() {
            return Err(PolygonError::DisconnectedInterior);
        }
        RectilinearPolygon::new(
            OrthogonalLoop::new(first.vertices.clone()),
            self.loops
                .iter()
                .filter(|boundary_loop| boundary_loop.is_hole)
                .map(|boundary_loop| OrthogonalLoop::new(boundary_loop.vertices.clone()))
                .collect(),
        )
    }

    /// Extracts normalized directed loops and reflex vertices from a grid component.
    ///
    /// # Errors
    ///
    /// Returns [`BoundaryError`] when coordinates overflow or the boundary/area
    /// invariants cannot be established.
    pub fn from_component<C>(component: &GridComponent<C>) -> Result<Self, BoundaryError> {
        Self::from_component_with_metrics(component).map(|build| build.boundary)
    }

    /// Extracts a boundary through the reference directed-edge toggle path and
    /// reports timings for each deterministic reduction stage.
    ///
    /// The reference discovery stage probes all four candidate edges of every
    /// occupied cell. Shared edges cancel as oppositely directed pairs.
    ///
    /// # Errors
    ///
    /// Returns [`BoundaryError`] under the same conditions as [`Self::from_component`].
    pub fn from_component_with_metrics<C>(
        component: &GridComponent<C>,
    ) -> Result<BoundaryBuild, BoundaryError> {
        build_boundary(component.cell_count(), || {
            discover_edges_by_reference_toggle(component)
        })
    }

    /// Extracts a boundary by probing prepared occupancy and emitting only
    /// exposed directed edges.
    ///
    /// It shares tracing, normalization, area validation, reflex detection, and
    /// ordering with the reference path so differential tests isolate edge
    /// discovery alone.
    ///
    /// # Errors
    ///
    /// Returns [`BoundaryError`] when coordinates overflow or the resulting
    /// boundary/area invariants cannot be established.
    pub fn from_prepared_component<C>(
        component: &GridComponent<C>,
        prepared: &PreparedGridComponent,
    ) -> Result<BoundaryBuild, BoundaryError> {
        build_boundary(component.cell_count(), || {
            discover_exposed_edges_from_prepared(component, prepared)
        })
    }

    #[must_use]
    pub fn outer_loop_count(&self) -> usize {
        self.loops.iter().filter(|item| !item.is_hole).count()
    }

    #[must_use]
    pub fn hole_count(&self) -> usize {
        self.loops.iter().filter(|item| item.is_hole).count()
    }

    #[must_use]
    pub fn boundary_complexity(&self) -> usize {
        self.loops.iter().map(|item| item.vertices.len()).sum()
    }

    /// Returns the stable loop-local identity of a normalized boundary point.
    #[must_use]
    pub fn vertex_id(&self, point: Point) -> Option<BoundaryVertexId> {
        self.loops
            .iter()
            .enumerate()
            .find_map(|(loop_index, boundary_loop)| {
                boundary_loop
                    .vertices
                    .iter()
                    .position(|&vertex| vertex == point)
                    .map(|cyclic_index| BoundaryVertexId {
                        loop_id: BoundaryLoopId(loop_index),
                        cyclic_index,
                    })
            })
    }

    #[must_use]
    pub fn loop_len(&self, loop_id: BoundaryLoopId) -> Option<usize> {
        self.loops
            .get(loop_id.0)
            .map(|boundary_loop| boundary_loop.vertices.len())
    }

    #[must_use]
    pub fn vertex(&self, id: BoundaryVertexId) -> Option<Point> {
        self.loops
            .get(id.loop_id.0)
            .and_then(|boundary_loop| boundary_loop.vertices.get(id.cyclic_index))
            .copied()
    }
}

fn build_boundary(
    cell_count: usize,
    discover_edges: impl FnOnce() -> Result<EdgeDiscovery, BoundaryError>,
) -> Result<BoundaryBuild, BoundaryError> {
    if cell_count == 0 {
        return Err(BoundaryError::EmptyComponent);
    }

    let total_started = Instant::now();

    let phase_started = Instant::now();
    let discovery = discover_edges()?;
    let edge_discovery_nanoseconds = phase_started.elapsed().as_nanos();
    let exposed_unit_edge_count = discovery.edges.len();

    let phase_started = Instant::now();
    let outgoing = build_outgoing_adjacency(&discovery.edges);
    let adjacency_build_nanoseconds = phase_started.elapsed().as_nanos();

    let phase_started = Instant::now();
    let trace = trace_boundary_loops(&discovery.edges, &outgoing)?;
    let loop_tracing_nanoseconds = phase_started.elapsed().as_nanos();

    let phase_started = Instant::now();
    let loops = normalize_boundary_loops(trace.raw_loops, cell_count)?;
    let loop_normalization_nanoseconds = phase_started.elapsed().as_nanos();
    let loop_count = loops.len();
    let normalized_vertex_count = loops
        .iter()
        .map(|boundary_loop| boundary_loop.vertices.len())
        .sum();

    let phase_started = Instant::now();
    let reflex_vertices = collect_reflex_vertices(&loops);
    let reflex_detection_nanoseconds = phase_started.elapsed().as_nanos();

    let phase_started = Instant::now();
    let mut unit_edges = discovery.edges.into_iter().collect::<Vec<_>>();
    unit_edges.sort_unstable();
    let unit_edge_sort_nanoseconds = phase_started.elapsed().as_nanos();

    let boundary = Boundary {
        loops,
        reflex_vertices,
        unit_edges,
    };
    let metrics = BoundaryBuildMetrics {
        total_build_nanoseconds: total_started.elapsed().as_nanos(),
        edge_discovery_nanoseconds,
        adjacency_build_nanoseconds,
        loop_tracing_nanoseconds,
        loop_normalization_nanoseconds,
        reflex_detection_nanoseconds,
        unit_edge_sort_nanoseconds,
        candidate_edge_probe_count: discovery.candidate_edge_probe_count,
        exposed_unit_edge_count,
        trace_edge_visit_count: trace.edge_visit_count,
        loop_count,
        normalized_vertex_count,
    };
    Ok(BoundaryBuild { boundary, metrics })
}

fn discover_edges_by_reference_toggle<C>(
    component: &GridComponent<C>,
) -> Result<EdgeDiscovery, BoundaryError> {
    let mut edges = HashSet::new();
    let mut candidate_edge_probe_count = 0;
    for cell in &component.cells {
        let corners = cell_corners(cell.x, cell.y)?;
        for edge in directed_cell_edges(corners) {
            candidate_edge_probe_count += 1;
            if !edges.remove(&(edge.1, edge.0)) {
                edges.insert(edge);
            }
        }
    }
    Ok(EdgeDiscovery {
        edges,
        candidate_edge_probe_count,
    })
}

fn discover_exposed_edges_from_prepared<C>(
    component: &GridComponent<C>,
    prepared: &PreparedGridComponent,
) -> Result<EdgeDiscovery, BoundaryError> {
    let mut edges = HashSet::new();
    let mut candidate_edge_probe_count = 0;
    for cell in &component.cells {
        let corners = cell_corners(cell.x, cell.y)?;
        let [bottom, right, top, left] = directed_cell_edges(corners);

        candidate_edge_probe_count += 1;
        if cell.y == 0 || !prepared.contains_cell(cell.x, cell.y - 1) {
            edges.insert(bottom);
        }

        candidate_edge_probe_count += 1;
        if !cell
            .x
            .checked_add(1)
            .is_some_and(|x| prepared.contains_cell(x, cell.y))
        {
            edges.insert(right);
        }

        candidate_edge_probe_count += 1;
        if !cell
            .y
            .checked_add(1)
            .is_some_and(|y| prepared.contains_cell(cell.x, y))
        {
            edges.insert(top);
        }

        candidate_edge_probe_count += 1;
        if cell.x == 0 || !prepared.contains_cell(cell.x - 1, cell.y) {
            edges.insert(left);
        }
    }
    Ok(EdgeDiscovery {
        edges,
        candidate_edge_probe_count,
    })
}

fn cell_corners(x: usize, y: usize) -> Result<[Point; 4], BoundaryError> {
    let x = Coord::try_from(x).map_err(|_| BoundaryError::CoordinateOverflow)?;
    let y = Coord::try_from(y).map_err(|_| BoundaryError::CoordinateOverflow)?;
    let x1 = x.checked_add(1).ok_or(BoundaryError::CoordinateOverflow)?;
    let y1 = y.checked_add(1).ok_or(BoundaryError::CoordinateOverflow)?;
    Ok([
        Point::new(x, y),
        Point::new(x1, y),
        Point::new(x1, y1),
        Point::new(x, y1),
    ])
}

const fn directed_cell_edges(corners: [Point; 4]) -> [DirectedUnitEdge; 4] {
    [
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ]
}

fn build_outgoing_adjacency(edges: &HashSet<DirectedUnitEdge>) -> HashMap<Point, Vec<Point>> {
    let mut outgoing: HashMap<Point, Vec<Point>> = HashMap::new();
    for &(start, end) in edges {
        outgoing.entry(start).or_default().push(end);
    }
    for destinations in outgoing.values_mut() {
        destinations.sort_unstable();
    }
    outgoing
}

fn trace_boundary_loops(
    edges: &HashSet<DirectedUnitEdge>,
    outgoing: &HashMap<Point, Vec<Point>>,
) -> Result<LoopTrace, BoundaryError> {
    let mut unused = edges.iter().copied().collect::<BTreeSet<_>>();
    let mut raw_loops = Vec::new();
    let mut edge_visit_count = 0;
    while let Some(&(start, first_end)) = unused.first() {
        let mut raw = vec![start];
        let mut previous = start;
        let mut current = first_end;
        unused.remove(&(start, first_end));
        edge_visit_count += 1;

        while current != start {
            raw.push(current);
            let candidates = outgoing
                .get(&current)
                .ok_or(BoundaryError::OpenLoop { point: current })?;
            let next = choose_successor(previous, current, candidates, &unused)
                .ok_or(BoundaryError::OpenLoop { point: current })?;
            unused.remove(&(current, next));
            edge_visit_count += 1;
            previous = current;
            current = next;
            if raw.len() > edges.len() + 1 {
                return Err(BoundaryError::NonTerminatingTrace);
            }
        }
        raw_loops.push(raw);
    }
    Ok(LoopTrace {
        raw_loops,
        edge_visit_count,
    })
}

fn normalize_boundary_loops(
    raw_loops: Vec<Vec<Point>>,
    cell_count: usize,
) -> Result<Vec<BoundaryLoop>, BoundaryError> {
    let mut loops = Vec::with_capacity(raw_loops.len());
    for raw in raw_loops {
        let vertices = simplify_collinear(&raw);
        if vertices.len() < 4 {
            return Err(BoundaryError::DegenerateLoop);
        }
        let twice_signed_area = twice_signed_area(&vertices);
        if twice_signed_area == 0 {
            return Err(BoundaryError::DegenerateLoop);
        }
        loops.push(BoundaryLoop {
            vertices,
            twice_signed_area,
            is_hole: twice_signed_area < 0,
        });
    }

    let expected_twice_area = i128::try_from(cell_count)
        .map_err(|_| BoundaryError::CoordinateOverflow)?
        .checked_mul(2)
        .ok_or(BoundaryError::CoordinateOverflow)?;
    let actual_twice_area = loops.iter().try_fold(0_i128, |sum, boundary_loop| {
        sum.checked_add(boundary_loop.twice_signed_area)
            .ok_or(BoundaryError::CoordinateOverflow)
    })?;
    if actual_twice_area != expected_twice_area {
        return Err(BoundaryError::AreaMismatch {
            expected_twice_area,
            actual_twice_area,
        });
    }
    Ok(loops)
}

fn collect_reflex_vertices(loops: &[BoundaryLoop]) -> Vec<ReflexVertex> {
    loops
        .iter()
        .flat_map(|boundary_loop| reflex_points(&boundary_loop.vertices))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|point| ReflexVertex { point })
        .collect()
}

fn choose_successor(
    previous: Point,
    current: Point,
    candidates: &[Point],
    unused: &BTreeSet<(Point, Point)>,
) -> Option<Point> {
    let incoming = (current.x - previous.x, current.y - previous.y);
    candidates
        .iter()
        .copied()
        .filter(|&candidate| unused.contains(&(current, candidate)))
        .max_by_key(|candidate| {
            let outgoing = (candidate.x - current.x, candidate.y - current.y);
            turn_priority(incoming, outgoing)
        })
}

const fn turn_priority(incoming: (Coord, Coord), outgoing: (Coord, Coord)) -> i8 {
    let cross = incoming.0 * outgoing.1 - incoming.1 * outgoing.0;
    let dot = incoming.0 * outgoing.0 + incoming.1 * outgoing.1;
    if cross > 0 {
        3
    } else if dot > 0 {
        2
    } else if cross < 0 {
        1
    } else {
        0
    }
}

fn simplify_collinear(points: &[Point]) -> Vec<Point> {
    let mut result = Vec::new();
    for index in 0..points.len() {
        let previous = points[(index + points.len() - 1) % points.len()];
        let current = points[index];
        let next = points[(index + 1) % points.len()];
        let incoming = (current.x - previous.x, current.y - previous.y);
        let outgoing = (next.x - current.x, next.y - current.y);
        if incoming.0 * outgoing.1 != incoming.1 * outgoing.0 {
            result.push(current);
        }
    }
    result
}

fn twice_signed_area(vertices: &[Point]) -> i128 {
    let mut area = 0_i128;
    for index in 0..vertices.len() {
        let first = vertices[index];
        let second = vertices[(index + 1) % vertices.len()];
        area +=
            i128::from(first.x) * i128::from(second.y) - i128::from(second.x) * i128::from(first.y);
    }
    area
}

fn reflex_points(vertices: &[Point]) -> impl Iterator<Item = Point> + '_ {
    (0..vertices.len()).filter_map(|index| {
        let previous = vertices[(index + vertices.len() - 1) % vertices.len()];
        let current = vertices[index];
        let next = vertices[(index + 1) % vertices.len()];
        let incoming = (current.x - previous.x, current.y - previous.y);
        let outgoing = (next.x - current.x, next.y - current.y);
        let cross = i128::from(incoming.0) * i128::from(outgoing.1)
            - i128::from(incoming.1) * i128::from(outgoing.0);
        (cross < 0).then_some(current)
    })
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum BoundaryError {
    #[error("cannot extract a boundary from an empty component")]
    EmptyComponent,
    #[error("grid coordinate does not fit the exact coordinate type")]
    CoordinateOverflow,
    #[error("boundary trace is open at {point:?}")]
    OpenLoop { point: Point },
    #[error("boundary trace did not terminate")]
    NonTerminatingTrace,
    #[error("boundary loop is degenerate")]
    DegenerateLoop,
    #[error(
        "boundary area mismatch: expected doubled area {expected_twice_area}, got {actual_twice_area}"
    )]
    AreaMismatch {
        expected_twice_area: i128,
        actual_twice_area: i128,
    },
}

#[cfg(test)]
mod tests {
    use crate::{Cell, ColorGrid, ComponentId, GridComponent, PreparedGridComponent};

    use super::{Boundary, BoundaryBuild, BoundaryError, BoundaryIndex, BoundaryIndexError};

    fn foreground_components_from_mask(
        width: usize,
        height: usize,
        mask: u64,
    ) -> Vec<GridComponent<bool>> {
        let cells = (0..width * height)
            .map(|index| mask & (1_u64 << index) != 0)
            .collect();
        ColorGrid::new(width, height, cells)
            .unwrap()
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
            .collect()
    }

    fn occupied_cells_from_rows(rows: &[&str]) -> (usize, usize, Vec<Cell>) {
        let width = rows.first().map_or(0, |row| row.len());
        assert!(rows.iter().all(|row| row.len() == width));
        let cells = rows
            .iter()
            .enumerate()
            .flat_map(|(y, row)| {
                row.bytes()
                    .enumerate()
                    .filter(|(_, value)| *value == b'#')
                    .map(move |(x, _)| Cell { x, y })
            })
            .collect();
        (width, rows.len(), cells)
    }

    fn foreground_components_from_cells(
        width: usize,
        height: usize,
        occupied: &[Cell],
    ) -> Vec<GridComponent<bool>> {
        let mut cells = vec![false; width * height];
        for cell in occupied {
            cells[cell.y * width + cell.x] = true;
        }
        ColorGrid::new(width, height, cells)
            .unwrap()
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
            .collect()
    }

    fn assert_reference_and_experimental_match<C>(component: &GridComponent<C>) -> BoundaryBuild {
        let prepared = PreparedGridComponent::from_component(component).unwrap();
        let reference = Boundary::from_component_with_metrics(component).unwrap();
        let experimental = Boundary::from_prepared_component(component, &prepared).unwrap();

        assert_eq!(reference.boundary, experimental.boundary);
        assert_eq!(
            reference.metrics.candidate_edge_probe_count,
            component.cell_count() * 4
        );
        assert_eq!(
            experimental.metrics.candidate_edge_probe_count,
            component.cell_count() * 4
        );
        assert_eq!(
            reference.metrics.exposed_unit_edge_count,
            experimental.metrics.exposed_unit_edge_count
        );
        assert_eq!(
            reference.metrics.trace_edge_visit_count,
            experimental.metrics.trace_edge_visit_count
        );
        assert_eq!(
            reference.metrics.loop_count,
            experimental.metrics.loop_count
        );
        assert_eq!(
            reference.metrics.normalized_vertex_count,
            experimental.metrics.normalized_vertex_count
        );
        assert_eq!(
            reference.metrics.exposed_unit_edge_count,
            reference.boundary.unit_edges.len()
        );
        assert_eq!(
            reference.metrics.trace_edge_visit_count,
            reference.metrics.exposed_unit_edge_count
        );
        let measured_phase_nanoseconds = reference.metrics.edge_discovery_nanoseconds
            + reference.metrics.adjacency_build_nanoseconds
            + reference.metrics.loop_tracing_nanoseconds
            + reference.metrics.loop_normalization_nanoseconds
            + reference.metrics.reflex_detection_nanoseconds
            + reference.metrics.unit_edge_sort_nanoseconds;
        assert!(reference.metrics.total_build_nanoseconds >= measured_phase_nanoseconds);
        reference
    }

    fn structural_signature(build: &BoundaryBuild) -> (usize, usize, usize, usize) {
        (
            build.boundary.outer_loop_count(),
            build.boundary.hole_count(),
            build.boundary.reflex_vertices.len(),
            build.boundary.unit_edges.len(),
        )
    }

    #[test]
    fn extracts_outer_and_hole_loops() {
        let grid = ColorGrid::new(
            3,
            3,
            vec![true, true, true, true, false, true, true, true, true],
        )
        .unwrap();
        let component = grid
            .four_connected_components()
            .into_iter()
            .find(|component| component.color)
            .unwrap();
        let boundary = Boundary::from_component(&component).unwrap();
        assert_eq!(boundary.outer_loop_count(), 1);
        assert_eq!(boundary.hole_count(), 1);
        assert_eq!(boundary.reflex_vertices.len(), 4);
    }

    #[test]
    fn boundary_index_rejects_duplicate_normalized_vertices() {
        let grid = ColorGrid::new(
            3,
            3,
            vec![true, true, true, true, false, true, true, true, false],
        )
        .unwrap();
        let component = grid
            .four_connected_components()
            .into_iter()
            .find(|component| component.color)
            .unwrap();
        let boundary = Boundary::from_component(&component).unwrap();
        assert!(matches!(
            BoundaryIndex::new(&boundary),
            Err(BoundaryIndexError::DuplicateVertex { .. })
        ));
    }

    #[test]
    fn reference_and_prepared_discovery_match_for_every_nonempty_three_by_three_mask() {
        for mask in 1..(1_u64 << 9) {
            for component in foreground_components_from_mask(3, 3, mask) {
                assert_reference_and_experimental_match(&component);
            }
        }
    }

    #[test]
    fn reference_and_prepared_discovery_match_on_deterministic_four_by_four_samples() {
        let mut state = 0x6a09_e667_f3bc_c909_u64;
        for _ in 0..2_048 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mask = (state & 0xffff).max(1);
            for component in foreground_components_from_mask(4, 4, mask) {
                assert_reference_and_experimental_match(&component);
            }
        }
    }

    #[test]
    fn reference_and_prepared_discovery_match_topology_fixtures() {
        let fixtures: &[&[&str]] = &[
            &["#"],
            &["####", "####"],
            &["####", "...#", "####", "#...", "####"],
            &["#....", "##...", "###..", "####.", "#####"],
            &["#####", "#.#.#", "#####"],
            &["###", "#.#", "###"],
            &["###", "#.#", "##."],
            &["#.#", "...", "#.#"],
        ];
        for rows in fixtures {
            let (width, height, occupied) = occupied_cells_from_rows(rows);
            let components = foreground_components_from_cells(width, height, &occupied);
            assert!(!components.is_empty());
            for component in components {
                assert_reference_and_experimental_match(&component);
            }
        }
    }

    #[test]
    fn transformed_components_preserve_structure_and_path_equality() {
        let (width, height, occupied) =
            occupied_cells_from_rows(&["#####", "#.#.#", "#####", "..###"]);

        let baseline_component =
            foreground_components_from_cells(width, height, &occupied).remove(0);
        let baseline = structural_signature(&assert_reference_and_experimental_match(
            &baseline_component,
        ));

        let translated = occupied
            .iter()
            .map(|cell| Cell {
                x: cell.x + 3,
                y: cell.y + 2,
            })
            .collect::<Vec<_>>();
        let translated_component =
            foreground_components_from_cells(width + 5, height + 4, &translated).remove(0);
        assert_eq!(
            structural_signature(&assert_reference_and_experimental_match(
                &translated_component
            )),
            baseline
        );

        let reflected = occupied
            .iter()
            .map(|cell| Cell {
                x: width - 1 - cell.x,
                y: cell.y,
            })
            .collect::<Vec<_>>();
        let reflected_component =
            foreground_components_from_cells(width, height, &reflected).remove(0);
        assert_eq!(
            structural_signature(&assert_reference_and_experimental_match(
                &reflected_component
            )),
            baseline
        );

        let rotated = occupied
            .iter()
            .map(|cell| Cell {
                x: height - 1 - cell.y,
                y: cell.x,
            })
            .collect::<Vec<_>>();
        let rotated_component = foreground_components_from_cells(height, width, &rotated).remove(0);
        assert_eq!(
            structural_signature(&assert_reference_and_experimental_match(&rotated_component)),
            baseline
        );
    }

    #[test]
    fn both_discovery_paths_reject_an_empty_component() {
        let component = GridComponent {
            id: ComponentId(0),
            color: true,
            grid_width: 0,
            grid_height: 0,
            cells: Vec::new(),
        };
        let prepared = PreparedGridComponent {
            x0: 0,
            y0: 0,
            x1: 0,
            y1: 0,
            occupancy: Vec::new(),
            occupancy_prefix_sums: Vec::new(),
            horizontal_interior_runs: Vec::new(),
            vertical_interior_runs: Vec::new(),
        };
        assert_eq!(
            Boundary::from_component_with_metrics(&component),
            Err(BoundaryError::EmptyComponent)
        );
        assert_eq!(
            Boundary::from_prepared_component(&component, &prepared),
            Err(BoundaryError::EmptyComponent)
        );
    }

    #[test]
    fn both_discovery_paths_support_the_largest_unit_cell_coordinate() {
        let coordinate = usize::try_from(i64::MAX - 1).unwrap();
        let component = GridComponent {
            id: ComponentId(0),
            color: true,
            grid_width: coordinate + 1,
            grid_height: 1,
            cells: vec![Cell {
                x: coordinate,
                y: 0,
            }],
        };
        assert_reference_and_experimental_match(&component);
    }
}
