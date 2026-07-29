use thiserror::Error;

use crate::{DissectionResult, GridComponent, GridRect, PreparedGridComponent};

/// Validates exact, nonoverlapping cell coverage independently of any solver.
///
/// # Errors
///
/// Returns [`ValidationError`] for malformed rectangles, outside coverage,
/// overlap, omitted cells, or a mismatched declared count.
pub fn validate_dissection<C>(
    component: &GridComponent<C>,
    result: &DissectionResult,
) -> Result<(), ValidationError> {
    let prepared = PreparedGridComponent::from_component(component)
        .map_err(|_| ValidationError::DimensionOverflow)?;
    validate_dissection_prepared(&prepared, result)
}

/// Validates exact coverage against a shared component-local occupancy mask.
///
/// # Errors
///
/// Returns [`ValidationError`] for malformed rectangles or non-exact coverage.
pub fn validate_dissection_prepared(
    prepared: &PreparedGridComponent,
    result: &DissectionResult,
) -> Result<(), ValidationError> {
    if result.rectangles.len() != result.optimum_rectangle_count {
        return Err(ValidationError::DeclaredCount {
            declared: result.optimum_rectangle_count,
            actual: result.rectangles.len(),
        });
    }

    let grid_len = prepared
        .width()
        .checked_mul(prepared.height())
        .ok_or(ValidationError::DimensionOverflow)?;
    let mut coverage = vec![0_u8; grid_len];
    for (rectangle_index, rectangle) in result.rectangles.iter().copied().enumerate() {
        validate_rectangle_bounds(prepared, rectangle, rectangle_index)?;
        for y in rectangle.y0..rectangle.y1 {
            for x in rectangle.x0..rectangle.x1 {
                if !prepared.contains_cell(x, y) {
                    return Err(ValidationError::OutsideCell {
                        rectangle: rectangle_index,
                        x,
                        y,
                    });
                }
                let index = (y - prepared.y0) * prepared.width() + x - prepared.x0;
                coverage[index] = coverage[index]
                    .checked_add(1)
                    .ok_or(ValidationError::CoverageOverflow { x, y })?;
                if coverage[index] > 1 {
                    return Err(ValidationError::Overlap { x, y });
                }
            }
        }
    }

    for local_y in 0..prepared.height() {
        for local_x in 0..prepared.width() {
            if !prepared.occupancy[local_y * prepared.width() + local_x] {
                continue;
            }
            let count = coverage[local_y * prepared.width() + local_x];
            if count != 1 {
                return Err(ValidationError::Coverage {
                    x: prepared.x0 + local_x,
                    y: prepared.y0 + local_y,
                    count,
                });
            }
        }
    }
    Ok(())
}

fn validate_rectangle_bounds(
    prepared: &PreparedGridComponent,
    rectangle: GridRect,
    rectangle_index: usize,
) -> Result<(), ValidationError> {
    if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
        return Err(ValidationError::NonPositiveRectangle {
            rectangle: rectangle_index,
        });
    }
    if rectangle.x0 < prepared.x0
        || rectangle.y0 < prepared.y0
        || rectangle.x1 > prepared.x1
        || rectangle.y1 > prepared.y1
    {
        return Err(ValidationError::OutOfGrid {
            rectangle: rectangle_index,
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ValidationError {
    #[error("declared rectangle count {declared} differs from actual count {actual}")]
    DeclaredCount { declared: usize, actual: usize },
    #[error("rectangle {rectangle} has non-positive width or height")]
    NonPositiveRectangle { rectangle: usize },
    #[error("rectangle {rectangle} extends outside the source grid")]
    OutOfGrid { rectangle: usize },
    #[error("rectangle {rectangle} covers outside cell ({x}, {y})")]
    OutsideCell {
        rectangle: usize,
        x: usize,
        y: usize,
    },
    #[error("component cell ({x}, {y}) has coverage {count}, expected exactly one")]
    Coverage { x: usize, y: usize, count: u8 },
    #[error("positive-area overlap at cell ({x}, {y})")]
    Overlap { x: usize, y: usize },
    #[error("coverage counter overflow at cell ({x}, {y})")]
    CoverageOverflow { x: usize, y: usize },
    #[error("grid dimensions overflow usize")]
    DimensionOverflow,
}

#[cfg(test)]
mod tests {
    use crate::{ColorGrid, Diagnostics, DissectionResult, GridRect, PreparedGridComponent};

    use super::{ValidationError, validate_dissection, validate_dissection_prepared};

    fn result(rectangles: Vec<GridRect>) -> DissectionResult {
        DissectionResult {
            optimum_rectangle_count: rectangles.len(),
            rectangles,
            diagnostics: Diagnostics::default(),
            certificate: None,
        }
    }

    #[test]
    fn prepared_and_convenience_validators_agree() {
        let grid = ColorGrid::new(
            4,
            3,
            vec![
                false, true, true, false, false, true, true, false, false, false, true, false,
            ],
        )
        .unwrap();
        let component = grid
            .four_connected_components()
            .into_iter()
            .find(|component| component.color)
            .unwrap();
        let prepared = PreparedGridComponent::from_component(&component).unwrap();
        let valid = result(vec![
            GridRect::new(1, 0, 3, 2).unwrap(),
            GridRect::new(2, 2, 3, 3).unwrap(),
        ]);
        assert_eq!(validate_dissection(&component, &valid), Ok(()));
        assert_eq!(validate_dissection_prepared(&prepared, &valid), Ok(()));

        let outside = result(vec![GridRect::new(1, 0, 3, 3).unwrap()]);
        assert!(matches!(
            validate_dissection_prepared(&prepared, &outside),
            Err(ValidationError::OutsideCell { x: 1, y: 2, .. })
        ));
    }
}
