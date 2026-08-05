use std::collections::BTreeMap;
use std::time::Instant;

use thiserror::Error;

use crate::boundary::{BoundaryBuild, BoundaryBuildMetrics};
use crate::{
    Boundary, BoundaryError, BoundaryIndex, BoundaryIndexError, Coord, GridComponent, GridError,
    PreparedGridComponent,
};

#[derive(Clone, Debug)]
pub struct PreparedComponentContext<'a, C> {
    pub component: &'a GridComponent<C>,
    pub prepared: PreparedGridComponent,
    pub boundary: Boundary,
    pub boundary_index: BoundaryIndex,
    pub reflex_by_row: BTreeMap<Coord, Vec<Coord>>,
    pub reflex_by_column: BTreeMap<Coord, Vec<Coord>>,
    pub boundary_build_metrics: BoundaryBuildMetrics,
    pub boundary_discovery_backend: &'static str,
    pub prepared_component_build_nanoseconds: u128,
    pub boundary_index_build_nanoseconds: u128,
    pub reflex_grouping_nanoseconds: u128,
    pub prepared_component_build_microseconds: u128,
    pub boundary_extraction_microseconds: u128,
    pub boundary_index_build_microseconds: u128,
    pub reflex_grouping_microseconds: u128,
}

impl<'a, C> PreparedComponentContext<'a, C> {
    /// Builds all shared grid geometry for one component solve.
    ///
    /// # Errors
    ///
    /// Returns [`PreparedContextError`] when preparation or boundary extraction fails.
    pub fn new(component: &'a GridComponent<C>) -> Result<Self, PreparedContextError> {
        Self::build(component, BoundaryDiscoveryBackend::PreparedExposedEdges)
    }

    fn build(
        component: &'a GridComponent<C>,
        boundary_discovery_backend: BoundaryDiscoveryBackend,
    ) -> Result<Self, PreparedContextError> {
        let started = Instant::now();
        let prepared = PreparedGridComponent::from_component(component)?;
        let prepared_at = Instant::now();
        let BoundaryBuild {
            boundary,
            metrics: boundary_build_metrics,
        } = match boundary_discovery_backend {
            BoundaryDiscoveryBackend::ReferenceEdgeToggle => {
                Boundary::from_component_with_metrics(component)?
            }
            BoundaryDiscoveryBackend::PreparedExposedEdges => {
                Boundary::from_prepared_component(component, &prepared)?
            }
        };
        let boundary_at = Instant::now();
        let boundary_index = BoundaryIndex::new(&boundary)
            .unwrap_or_else(|_| BoundaryIndex::from_boundary_first_occurrence(&boundary));
        let boundary_index_at = Instant::now();
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
        let prepared_component_build = prepared_at.duration_since(started);
        let boundary_index_build = boundary_index_at.duration_since(boundary_at);
        let reflex_grouping = grouped_at.duration_since(boundary_index_at);
        Ok(Self {
            component,
            prepared,
            boundary,
            boundary_index,
            reflex_by_row,
            reflex_by_column,
            boundary_build_metrics,
            boundary_discovery_backend: boundary_discovery_backend.name(),
            prepared_component_build_nanoseconds: prepared_component_build.as_nanos(),
            boundary_index_build_nanoseconds: boundary_index_build.as_nanos(),
            reflex_grouping_nanoseconds: reflex_grouping.as_nanos(),
            prepared_component_build_microseconds: prepared_component_build.as_micros(),
            boundary_extraction_microseconds: boundary_at.duration_since(prepared_at).as_micros(),
            boundary_index_build_microseconds: boundary_index_build.as_micros(),
            reflex_grouping_microseconds: reflex_grouping.as_micros(),
        })
    }
}

/// Definition-level construction paths retained for differential verification.
pub mod oracle {
    use super::{BoundaryDiscoveryBackend, PreparedComponentContext, PreparedContextError};
    use crate::GridComponent;

    /// Builds the historical four-edge toggle representation.
    ///
    /// # Errors
    ///
    /// Returns [`PreparedContextError`] when preparation or boundary extraction fails.
    pub fn prepare_component<C>(
        component: &GridComponent<C>,
    ) -> Result<PreparedComponentContext<'_, C>, PreparedContextError> {
        PreparedComponentContext::build(component, BoundaryDiscoveryBackend::ReferenceEdgeToggle)
    }
}

#[derive(Clone, Copy)]
enum BoundaryDiscoveryBackend {
    ReferenceEdgeToggle,
    PreparedExposedEdges,
}

impl BoundaryDiscoveryBackend {
    const fn name(self) -> &'static str {
        match self {
            Self::ReferenceEdgeToggle => "reference-edge-toggle",
            Self::PreparedExposedEdges => "prepared-exposed-edges",
        }
    }
}

#[derive(Debug, Error)]
pub enum PreparedContextError {
    #[error(transparent)]
    Grid(#[from] GridError),
    #[error(transparent)]
    Boundary(#[from] BoundaryError),
    #[error(transparent)]
    BoundaryIndex(#[from] BoundaryIndexError),
}

#[cfg(test)]
mod tests {
    use crate::ColorGrid;

    use super::PreparedComponentContext;

    #[test]
    fn constructors_expose_stable_boundary_backend_identity() {
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

        let reference = super::oracle::prepare_component(&component).unwrap();
        let experimental = PreparedComponentContext::new(&component).unwrap();

        assert_eq!(reference.boundary, experimental.boundary);
        assert_eq!(
            reference.boundary_discovery_backend,
            "reference-edge-toggle"
        );
        assert_eq!(
            experimental.boundary_discovery_backend,
            "prepared-exposed-edges"
        );
        assert_eq!(
            reference.boundary_build_metrics.candidate_edge_probe_count,
            component.cell_count() * 4
        );
        assert_eq!(
            reference.boundary_build_metrics.exposed_unit_edge_count,
            experimental.boundary_build_metrics.exposed_unit_edge_count
        );
    }
}
