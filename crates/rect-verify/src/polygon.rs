//! Independent bounded raster Oracle for small integer-coordinate polygons.

use rect_core::{ColorGrid, DoubledPoint, RectilinearPolygon};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RasterLimits {
    pub max_width: usize,
    pub max_height: usize,
    pub max_cells: usize,
}

impl Default for RasterLimits {
    fn default() -> Self {
        Self {
            max_width: 256,
            max_height: 256,
            max_cells: 65_536,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RasterizedPolygon {
    pub origin_x: i64,
    pub origin_y: i64,
    pub grid: ColorGrid<bool>,
}

/// Rasterizes a small integer-coordinate polygon for differential testing.
///
/// This function is an optional bounded Oracle. Production polygon solving
/// never calls it.
///
/// # Errors
///
/// Returns [`RasterOracleError`] if the coordinate bounding box exceeds an
/// explicit width, height, or cell limit.
pub fn bounded_rasterize_polygon(
    polygon: &RectilinearPolygon,
    limits: RasterLimits,
) -> Result<RasterizedPolygon, RasterOracleError> {
    let mut points = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().copied());
    let first = points.next().ok_or(RasterOracleError::EmptyPolygon)?;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    let width = usize::try_from(i128::from(max_x) - i128::from(min_x))
        .map_err(|_| RasterOracleError::DimensionOverflow)?;
    let height = usize::try_from(i128::from(max_y) - i128::from(min_y))
        .map_err(|_| RasterOracleError::DimensionOverflow)?;
    if width > limits.max_width {
        return Err(RasterOracleError::WidthLimit {
            actual: width,
            limit: limits.max_width,
        });
    }
    if height > limits.max_height {
        return Err(RasterOracleError::HeightLimit {
            actual: height,
            limit: limits.max_height,
        });
    }
    let cell_count = width
        .checked_mul(height)
        .ok_or(RasterOracleError::DimensionOverflow)?;
    if cell_count > limits.max_cells {
        return Err(RasterOracleError::CellLimit {
            actual: cell_count,
            limit: limits.max_cells,
        });
    }
    let mut cells = Vec::with_capacity(cell_count);
    for local_y in 0..height {
        let y = i128::from(min_y)
            .checked_add(i128::try_from(local_y).map_err(|_| RasterOracleError::DimensionOverflow)?)
            .ok_or(RasterOracleError::DimensionOverflow)?;
        for local_x in 0..width {
            let x = i128::from(min_x)
                .checked_add(
                    i128::try_from(local_x).map_err(|_| RasterOracleError::DimensionOverflow)?,
                )
                .ok_or(RasterOracleError::DimensionOverflow)?;
            cells.push(
                polygon.contains_doubled_point_strict(DoubledPoint::new(
                    x.checked_mul(2)
                        .and_then(|value| value.checked_add(1))
                        .ok_or(RasterOracleError::DimensionOverflow)?,
                    y.checked_mul(2)
                        .and_then(|value| value.checked_add(1))
                        .ok_or(RasterOracleError::DimensionOverflow)?,
                )),
            );
        }
    }
    Ok(RasterizedPolygon {
        origin_x: min_x,
        origin_y: min_y,
        grid: ColorGrid::new(width, height, cells)
            .map_err(|_| RasterOracleError::DimensionOverflow)?,
    })
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RasterOracleError {
    #[error("cannot rasterize an empty polygon")]
    EmptyPolygon,
    #[error("polygon raster dimensions overflow usize")]
    DimensionOverflow,
    #[error("polygon raster width {actual} exceeds limit {limit}")]
    WidthLimit { actual: usize, limit: usize },
    #[error("polygon raster height {actual} exceeds limit {limit}")]
    HeightLimit { actual: usize, limit: usize },
    #[error("polygon raster cell count {actual} exceeds limit {limit}")]
    CellLimit { actual: usize, limit: usize },
}

#[cfg(test)]
mod tests {
    use rect_core::{OrthogonalLoop, Point, RectilinearPolygon};

    use super::{RasterLimits, RasterOracleError, bounded_rasterize_polygon};

    fn loop_from(points: &[(i64, i64)]) -> OrthogonalLoop {
        OrthogonalLoop::new(points.iter().map(|&(x, y)| Point::new(x, y)).collect())
    }

    #[test]
    fn rasterizes_small_translated_polygon_with_explicit_origin() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[(-3, 5), (1, 5), (1, 6), (-2, 6), (-2, 9), (-3, 9)]),
            vec![],
        )
        .unwrap();
        let raster = bounded_rasterize_polygon(&polygon, RasterLimits::default()).unwrap();
        assert_eq!((raster.origin_x, raster.origin_y), (-3, 5));
        assert_eq!((raster.grid.width, raster.grid.height), (4, 4));
        assert_eq!(raster.grid.cells.iter().filter(|&&cell| cell).count(), 7);
    }

    #[test]
    fn rejects_large_coordinate_gap_before_allocating_cells() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[(0, 0), (1_000_000_000, 0), (1_000_000_000, 1), (0, 1)]),
            vec![],
        )
        .unwrap();
        assert!(matches!(
            bounded_rasterize_polygon(&polygon, RasterLimits::default()),
            Err(RasterOracleError::WidthLimit { .. })
        ));
    }
}
