use std::collections::{BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{Coord, GridComponent, OrthogonalLoop, Point, PolygonError, RectilinearPolygon};

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
        if component.cells.is_empty() {
            return Err(BoundaryError::EmptyComponent);
        }

        let mut edges = HashSet::new();
        for cell in &component.cells {
            let x = Coord::try_from(cell.x).map_err(|_| BoundaryError::CoordinateOverflow)?;
            let y = Coord::try_from(cell.y).map_err(|_| BoundaryError::CoordinateOverflow)?;
            let x1 = x.checked_add(1).ok_or(BoundaryError::CoordinateOverflow)?;
            let y1 = y.checked_add(1).ok_or(BoundaryError::CoordinateOverflow)?;
            for edge in [
                (Point::new(x, y), Point::new(x1, y)),
                (Point::new(x1, y), Point::new(x1, y1)),
                (Point::new(x1, y1), Point::new(x, y1)),
                (Point::new(x, y1), Point::new(x, y)),
            ] {
                if !edges.remove(&(edge.1, edge.0)) {
                    edges.insert(edge);
                }
            }
        }

        let mut outgoing: HashMap<Point, Vec<Point>> = HashMap::new();
        for &(start, end) in &edges {
            outgoing.entry(start).or_default().push(end);
        }
        for destinations in outgoing.values_mut() {
            destinations.sort_unstable();
        }

        let mut unused = edges.iter().copied().collect::<BTreeSet<_>>();
        let mut loops = Vec::new();
        while let Some(&(start, first_end)) = unused.first() {
            let mut raw = vec![start];
            let mut previous = start;
            let mut current = first_end;
            unused.remove(&(start, first_end));

            while current != start {
                raw.push(current);
                let candidates = outgoing
                    .get(&current)
                    .ok_or(BoundaryError::OpenLoop { point: current })?;
                let next = choose_successor(previous, current, candidates, &unused)
                    .ok_or(BoundaryError::OpenLoop { point: current })?;
                unused.remove(&(current, next));
                previous = current;
                current = next;
                if raw.len() > edges.len() + 1 {
                    return Err(BoundaryError::NonTerminatingTrace);
                }
            }

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

        let expected_twice_area = i128::try_from(component.cell_count())
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

        let reflex_vertices = loops
            .iter()
            .flat_map(|boundary_loop| reflex_points(&boundary_loop.vertices))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|point| ReflexVertex { point })
            .collect();
        let mut unit_edges = edges.into_iter().collect::<Vec<_>>();
        unit_edges.sort_unstable();

        Ok(Self {
            loops,
            reflex_vertices,
            unit_edges,
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
    use crate::ColorGrid;

    use super::{Boundary, BoundaryIndex, BoundaryIndexError};

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
}
