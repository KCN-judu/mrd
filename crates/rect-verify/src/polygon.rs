//! Independent bounded raster Oracle and grid/polygon differential tests.

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
    use std::collections::BTreeMap;

    use rect_core::{
        Boundary, ColorGrid, CoordinateRect, GridComponent, OrthogonalLoop, Point,
        PolygonDissectionResult, RectilinearPolygon,
    };
    use rect_dominance::{
        ChordEnumerator, VerificationMode, solve_polygon,
        solve_with_verification_mode_and_chord_enumerator_and_completion_backend,
    };
    use rect_oracle_sg::{
        CoordinateCompressedCompletion, GridInteriorRunEnumerator, HorizontalCutSegment,
        HorizontalUnitCut, IndexedFrontierCompletion, VerticalCutSegment, VerticalUnitCut,
        analyze_geometry_with, complete_with_prepared_backend,
    };

    use super::{RasterLimits, RasterOracleError, bounded_rasterize_polygon};

    #[derive(Debug, Default)]
    struct DifferentialCounts {
        inputs: usize,
        components: usize,
        supported_components: usize,
        rejected_components: usize,
    }

    #[allow(clippy::too_many_lines)]
    fn compare_component(
        component: &GridComponent<bool>,
        counts: &mut DifferentialCounts,
    ) -> Result<(), String> {
        counts.components += 1;
        let boundary = Boundary::from_component(component).map_err(|error| error.to_string())?;
        let Ok(polygon) = boundary.to_polygon() else {
            counts.rejected_components += 1;
            return Ok(());
        };
        counts.supported_components += 1;
        let geometry = analyze_geometry_with(component, &GridInteriorRunEnumerator)
            .map_err(|error| error.to_string())?;
        let polygon_families = rect_oracle_sg::GeneralPolygonPairwiseEnumerator
            .enumerate(&polygon)
            .map_err(|error| error.to_string())?;
        if geometry.horizontal_chords != polygon_families.horizontal
            || geometry.vertical_chords != polygon_families.vertical
        {
            return Err("effective chord families differ".to_owned());
        }

        let grid_result = solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
            component,
            VerificationMode::CompactOnly,
            ChordEnumerator::GridInteriorRuns,
            rect_oracle_sg::CompletionBackendKind::IndexedFrontier,
        )
        .map_err(|error| error.to_string())?;
        let polygon_result = solve_polygon(&polygon).map_err(|error| error.to_string())?;
        if grid_result.optimum_rectangle_count != polygon_result.optimum_rectangle_count {
            return Err("minimum rectangle counts differ".to_owned());
        }

        let selected_horizontal = selected_grid_flags(
            &grid_result,
            "selected_horizontal",
            geometry.horizontal_chords.len(),
        );
        let selected_vertical = selected_grid_flags(
            &grid_result,
            "selected_vertical",
            geometry.vertical_chords.len(),
        );
        let polygon_selected_horizontal = selected_polygon_flags(
            &polygon_result,
            "selected_horizontal",
            geometry.horizontal_chords.len(),
        );
        let polygon_selected_vertical = selected_polygon_flags(
            &polygon_result,
            "selected_vertical",
            geometry.vertical_chords.len(),
        );
        if selected_horizontal != polygon_selected_horizontal
            || selected_vertical != polygon_selected_vertical
        {
            return Err("minimum-cover selections differ".to_owned());
        }

        let grid_completion = complete_with_prepared_backend(
            component,
            &geometry.prepared,
            &geometry.horizontal_chords,
            &geometry.vertical_chords,
            &selected_horizontal,
            &selected_vertical,
            &IndexedFrontierCompletion,
        )
        .map_err(|error| error.to_string())?;
        let polygon_completion = CoordinateCompressedCompletion
            .complete(
                &polygon,
                &polygon_families.horizontal,
                &polygon_families.vertical,
                &polygon_selected_horizontal,
                &polygon_selected_vertical,
            )
            .map_err(|error| error.to_string())?;
        if merge_horizontal(&grid_completion.selected_horizontal_unit_cuts)
            != polygon_completion.selected_horizontal_cuts
            || merge_vertical(&grid_completion.selected_vertical_unit_cuts)
                != polygon_completion.selected_vertical_cuts
            || merge_horizontal(&grid_completion.added_horizontal_unit_cuts)
                != polygon_completion.added_horizontal_cuts
            || merge_vertical(&grid_completion.added_vertical_unit_cuts)
                != polygon_completion.added_vertical_cuts
        {
            return Err("selected or added cuts differ".to_owned());
        }

        let grid_rectangles = grid_result
            .rectangles
            .iter()
            .map(|rectangle| {
                CoordinateRect::new(
                    i64::try_from(rectangle.x0).map_err(|error| error.to_string())?,
                    i64::try_from(rectangle.y0).map_err(|error| error.to_string())?,
                    i64::try_from(rectangle.x1).map_err(|error| error.to_string())?,
                    i64::try_from(rectangle.y1).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if grid_rectangles != polygon_result.rectangles
            || polygon_result.rectangles != polygon_completion.rectangles
        {
            return Err("canonical rectangles differ".to_owned());
        }
        Ok(())
    }

    fn selected_grid_flags(
        result: &rect_core::DissectionResult,
        key: &str,
        len: usize,
    ) -> Vec<bool> {
        let mut flags = vec![false; len];
        for index in result.certificate.as_ref().unwrap().payload[key]
            .as_array()
            .unwrap()
        {
            flags[usize::try_from(index.as_u64().unwrap()).unwrap()] = true;
        }
        flags
    }

    fn selected_polygon_flags(
        result: &PolygonDissectionResult,
        key: &str,
        len: usize,
    ) -> Vec<bool> {
        let flags = result.certificate.as_ref().unwrap().payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(flags.len(), len);
        flags
    }

    fn merge_horizontal(cuts: &[HorizontalUnitCut]) -> Vec<HorizontalCutSegment> {
        let mut cuts = cuts.to_vec();
        cuts.sort_unstable_by_key(|cut| (cut.y, cut.x));
        let mut result = Vec::<HorizontalCutSegment>::new();
        for cut in cuts {
            let x = i64::try_from(cut.x).unwrap();
            let y = i64::try_from(cut.y).unwrap();
            if let Some(last) = result.last_mut()
                && last.y == y
                && last.right == x
            {
                last.right += 1;
            } else {
                result.push(HorizontalCutSegment {
                    left: x,
                    right: x + 1,
                    y,
                });
            }
        }
        result.sort_unstable();
        result
    }

    fn merge_vertical(cuts: &[VerticalUnitCut]) -> Vec<VerticalCutSegment> {
        let mut cuts = cuts.to_vec();
        cuts.sort_unstable_by_key(|cut| (cut.x, cut.y));
        let mut result = Vec::<VerticalCutSegment>::new();
        for cut in cuts {
            let x = i64::try_from(cut.x).unwrap();
            let y = i64::try_from(cut.y).unwrap();
            if let Some(last) = result.last_mut()
                && last.x == x
                && last.top == y
            {
                last.top += 1;
            } else {
                result.push(VerticalCutSegment {
                    x,
                    bottom: y,
                    top: y + 1,
                });
            }
        }
        result.sort_unstable();
        result
    }

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

    #[test]
    #[ignore = "release-mode extended v0.9 grid/polygon differential populations"]
    fn extended_grid_polygon_differential_populations_match() {
        use crate::adversarial::{
            clean_complete_bipartite_grid, external_oracle_adversarial_instances,
        };
        use crate::polyomino::enumerate_free_polyominoes;

        let mut counts = DifferentialCounts::default();
        for level in enumerate_free_polyominoes(10) {
            for polyomino in level {
                let instance = polyomino.to_instance(
                    format!("polyomino-{}", polyomino.canonical_key()),
                    "free-polyomino",
                );
                counts.inputs += 1;
                compare_instance(&instance, &mut counts);
            }
        }
        for instance in external_oracle_adversarial_instances() {
            counts.inputs += 1;
            compare_instance(&instance, &mut counts);
        }
        for t in 1..=4 {
            let instance = clean_complete_bipartite_grid(t).unwrap();
            counts.inputs += 1;
            compare_instance(&instance, &mut counts);
        }
        for case in 0..1_000 {
            let instance = random_connected_instance(case);
            counts.inputs += 1;
            compare_instance(&instance, &mut counts);
        }
        assert!(counts.supported_components > 1_000);
        println!("{counts:?}");
    }

    fn compare_instance(
        instance: &crate::adversarial::AdversarialInstance,
        counts: &mut DifferentialCounts,
    ) {
        let grid = ColorGrid::new(instance.width, instance.height, instance.cells.clone()).unwrap();
        for component in grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
        {
            compare_component(&component, counts)
                .unwrap_or_else(|error| panic!("{}: {error}", instance.name));
        }
    }

    fn random_connected_instance(case: usize) -> crate::adversarial::AdversarialInstance {
        let width = 8;
        let height = 8;
        let mut state = (case as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut cells = vec![false; width * height];
        let mut x = case % width;
        let mut y = (case / width) % height;
        for _ in 0..24 {
            cells[y * width + x] = true;
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            match (state >> 62) as usize {
                0 if x > 0 => x -= 1,
                1 if x + 1 < width => x += 1,
                2 if y > 0 => y -= 1,
                _ if y + 1 < height => y += 1,
                _ => {}
            }
        }
        crate::adversarial::AdversarialInstance {
            name: format!("polygon-random-{case:04}"),
            family: "polygon-random-connected".to_owned(),
            width,
            height,
            cells,
            parameters: BTreeMap::from([("seed".to_owned(), case)]),
        }
    }
}
