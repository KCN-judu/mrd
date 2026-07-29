//! Definition-level pairwise chord enumerators.

#[allow(clippy::wildcard_imports)]
use super::super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Pairwise;

#[derive(Clone, Copy, Debug, Default)]
pub struct Indexed;

impl Pairwise {
    /// Enumerates every Definition 7 effective chord for an ordinary polygon.
    ///
    /// This is an exact `O(r^2 n)` reference implementation, not the general
    /// Soltan--Gorpinevich sweep-line algorithm.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] when normalization, boundary indexing, or
    /// exact chord construction fails.
    pub fn enumerate(&self, polygon: &RectilinearPolygon) -> Result<Families, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            mrd_domain::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            mrd_domain::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        self.enumerate_prepared(&prepared)
    }

    /// Runs the preserved full-pair reference algorithm on shared metadata.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for endpoint metadata or chord geometry
    /// failures.
    pub fn enumerate_prepared(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<Families, PolygonSgError> {
        Ok(self.enumerate_prepared_with_metrics(prepared)?.families)
    }

    /// Runs the preserved pairwise reference algorithm and returns its scan
    /// counters as well as the exact chord families.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid endpoint metadata, coordinate
    /// overflow, or effective-chord construction failure.
    pub fn enumerate_prepared_with_metrics(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<PolygonChordEnumerationResult, PolygonSgError> {
        let polygon = prepared.polygon();
        let boundary = prepared.boundary();
        let boundary_index = prepared.boundary_index();
        let points = boundary
            .reflex_vertices
            .iter()
            .map(|vertex| vertex.point)
            .collect::<Vec<_>>();
        let mut horizontal = BTreeSet::new();
        let mut vertical = BTreeSet::new();
        let total_pairs = points.len().saturating_sub(1) * points.len() / 2;
        let aligned_pairs = prepared.metrics().polygon_aligned_reflex_candidate_pairs;
        let mut metrics = PolygonChordEnumerationMetrics {
            polygon_aligned_reflex_candidate_pairs: aligned_pairs,
            polygon_unaligned_reflex_pair_checks: total_pairs.saturating_sub(aligned_pairs),
            ..PolygonChordEnumerationMetrics::default()
        };

        for first_index in 0..points.len() {
            for second_index in first_index + 1..points.len() {
                let first = points[first_index];
                let second = points[second_index];
                if first.y == second.y {
                    let left = first.x.min(second.x);
                    let right = first.x.max(second.x);
                    if endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(left, first.y),
                        true,
                    )? && endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(right, first.y),
                        true,
                    )? && horizontal_satisfies_definition_7(
                        polygon,
                        boundary,
                        boundary_index,
                        left,
                        right,
                        first.y,
                        &mut metrics,
                    )? {
                        horizontal.insert((first.y, left, right));
                    }
                }
                if first.x == second.x {
                    let bottom = first.y.min(second.y);
                    let top = first.y.max(second.y);
                    if endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(first.x, bottom),
                        false,
                    )? && endpoint_has_collinear_edge(
                        boundary,
                        boundary_index,
                        Point::new(first.x, top),
                        false,
                    )? && vertical_satisfies_definition_7(
                        polygon,
                        boundary,
                        boundary_index,
                        first.x,
                        bottom,
                        top,
                        &mut metrics,
                    )? {
                        vertical.insert((first.x, bottom, top));
                    }
                }
            }
        }

        let families = Families {
            horizontal: horizontal
                .into_iter()
                .enumerate()
                .map(|(index, (y, left, right))| {
                    HorizontalChord::new(HorizontalChordId(index), left, right, y)
                })
                .collect::<Result<Vec<_>, _>>()?,
            vertical: vertical
                .into_iter()
                .enumerate()
                .map(|(index, (x, bottom, top))| {
                    VerticalChord::new(VerticalChordId(index), x, bottom, top)
                })
                .collect::<Result<Vec<_>, _>>()?,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(total_pairs),
        };
        Ok(PolygonChordEnumerationResult {
            families,
            metrics,
            sweep_certificate: None,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "general-polygon-pairwise"
    }
}

impl Indexed {
    /// Convenience API that prepares the polygon once before indexed enumeration.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] for invalid polygon metadata or chord geometry.
    pub fn enumerate(&self, polygon: &RectilinearPolygon) -> Result<Families, PolygonSgError> {
        let prepared = PreparedPolygonContext::new(polygon).map_err(|error| match error {
            mrd_domain::PreparedPolygonError::Polygon(error) => PolygonSgError::Polygon(error),
            mrd_domain::PreparedPolygonError::BoundaryIndex(error) => {
                PolygonSgError::BoundaryIndex(error)
            }
        })?;
        Ok(self.enumerate_prepared(&prepared)?.families)
    }

    /// Enumerates only coordinate-aligned reflex pairs using prepared indexes.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError`] when endpoint metadata or exact chord
    /// construction fails.
    pub fn enumerate_prepared(
        &self,
        prepared: &PreparedPolygonContext,
    ) -> Result<PolygonChordEnumerationResult, PolygonSgError> {
        let mut horizontal = BTreeSet::new();
        let mut vertical = BTreeSet::new();
        let mut metrics = PolygonChordEnumerationMetrics {
            polygon_aligned_reflex_candidate_pairs: prepared
                .metrics()
                .polygon_aligned_reflex_candidate_pairs,
            ..PolygonChordEnumerationMetrics::default()
        };

        for points in prepared.reflex_by_y().values() {
            for first in 0..points.len() {
                for second in first + 1..points.len() {
                    let left = points[first].x.min(points[second].x);
                    let right = points[first].x.max(points[second].x);
                    let y = points[first].y;
                    if endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(left, y),
                        true,
                    )? && endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(right, y),
                        true,
                    )? && horizontal_satisfies_definition_7_indexed(
                        prepared,
                        left,
                        right,
                        y,
                        &mut metrics,
                    )? {
                        horizontal.insert((y, left, right));
                    }
                }
            }
        }
        for points in prepared.reflex_by_x().values() {
            for first in 0..points.len() {
                for second in first + 1..points.len() {
                    let x = points[first].x;
                    let bottom = points[first].y.min(points[second].y);
                    let top = points[first].y.max(points[second].y);
                    if endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(x, bottom),
                        false,
                    )? && endpoint_has_collinear_edge(
                        prepared.boundary(),
                        prepared.boundary_index(),
                        Point::new(x, top),
                        false,
                    )? && vertical_satisfies_definition_7_indexed(
                        prepared,
                        x,
                        bottom,
                        top,
                        &mut metrics,
                    )? {
                        vertical.insert((x, bottom, top));
                    }
                }
            }
        }

        let families = Families {
            horizontal: horizontal
                .into_iter()
                .enumerate()
                .map(|(index, (y, left, right))| {
                    HorizontalChord::new(HorizontalChordId(index), left, right, y)
                })
                .collect::<Result<Vec<_>, _>>()?,
            vertical: vertical
                .into_iter()
                .enumerate()
                .map(|(index, (x, bottom, top))| {
                    VerticalChord::new(VerticalChordId(index), x, bottom, top)
                })
                .collect::<Result<Vec<_>, _>>()?,
            horizontal_interior_run_count: None,
            vertical_interior_run_count: None,
            candidate_reflex_pair_count: Some(metrics.polygon_aligned_reflex_candidate_pairs),
        };
        Ok(PolygonChordEnumerationResult {
            families,
            metrics,
            sweep_certificate: None,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "indexed-polygon-pairwise"
    }
}
