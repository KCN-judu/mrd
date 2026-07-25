use std::collections::BTreeMap;
use std::time::Instant;

use thiserror::Error;

use crate::{Boundary, BoundaryError, Coord, GridComponent, GridError, PreparedGridComponent};

#[derive(Clone, Debug)]
pub struct PreparedComponentContext<'a, C> {
    pub component: &'a GridComponent<C>,
    pub prepared: PreparedGridComponent,
    pub boundary: Boundary,
    pub reflex_by_row: BTreeMap<Coord, Vec<Coord>>,
    pub reflex_by_column: BTreeMap<Coord, Vec<Coord>>,
    pub prepared_component_build_microseconds: u128,
    pub boundary_extraction_microseconds: u128,
    pub reflex_grouping_microseconds: u128,
}

impl<'a, C> PreparedComponentContext<'a, C> {
    /// Builds all shared grid geometry for one component solve.
    ///
    /// # Errors
    ///
    /// Returns [`PreparedContextError`] when preparation or boundary extraction fails.
    pub fn new(component: &'a GridComponent<C>) -> Result<Self, PreparedContextError> {
        let started = Instant::now();
        let prepared = PreparedGridComponent::from_component(component)?;
        let prepared_at = Instant::now();
        let boundary = Boundary::from_component(component)?;
        let boundary_at = Instant::now();
        let mut reflex_by_row = BTreeMap::<Coord, Vec<Coord>>::new();
        let mut reflex_by_column = BTreeMap::<Coord, Vec<Coord>>::new();
        for vertex in &boundary.reflex_vertices {
            reflex_by_row
                .entry(vertex.point.y)
                .or_default()
                .push(vertex.point.x);
            reflex_by_column
                .entry(vertex.point.x)
                .or_default()
                .push(vertex.point.y);
        }
        for coordinates in reflex_by_row.values_mut() {
            coordinates.sort_unstable();
        }
        for coordinates in reflex_by_column.values_mut() {
            coordinates.sort_unstable();
        }
        let grouped_at = Instant::now();
        Ok(Self {
            component,
            prepared,
            boundary,
            reflex_by_row,
            reflex_by_column,
            prepared_component_build_microseconds: prepared_at.duration_since(started).as_micros(),
            boundary_extraction_microseconds: boundary_at.duration_since(prepared_at).as_micros(),
            reflex_grouping_microseconds: grouped_at.duration_since(boundary_at).as_micros(),
        })
    }
}

#[derive(Debug, Error)]
pub enum PreparedContextError {
    #[error(transparent)]
    Grid(#[from] GridError),
    #[error(transparent)]
    Boundary(#[from] BoundaryError),
}
