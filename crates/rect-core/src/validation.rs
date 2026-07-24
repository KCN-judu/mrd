use thiserror::Error;

use crate::{DissectionResult, GridComponent, GridRect};

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
    if result.rectangles.len() != result.optimum_rectangle_count {
        return Err(ValidationError::DeclaredCount {
            declared: result.optimum_rectangle_count,
            actual: result.rectangles.len(),
        });
    }

    let grid_len = component
        .grid_width
        .checked_mul(component.grid_height)
        .ok_or(ValidationError::DimensionOverflow)?;
    let mut coverage = vec![0_u8; grid_len];
    for (rectangle_index, rectangle) in result.rectangles.iter().copied().enumerate() {
        validate_rectangle_bounds(component, rectangle, rectangle_index)?;
        for y in rectangle.y0..rectangle.y1 {
            for x in rectangle.x0..rectangle.x1 {
                if !component.contains_cell(x, y) {
                    return Err(ValidationError::OutsideCell {
                        rectangle: rectangle_index,
                        x,
                        y,
                    });
                }
                let index = y * component.grid_width + x;
                coverage[index] = coverage[index]
                    .checked_add(1)
                    .ok_or(ValidationError::CoverageOverflow { x, y })?;
                if coverage[index] > 1 {
                    return Err(ValidationError::Overlap { x, y });
                }
            }
        }
    }

    for cell in &component.cells {
        let count = coverage[cell.y * component.grid_width + cell.x];
        if count != 1 {
            return Err(ValidationError::Coverage {
                x: cell.x,
                y: cell.y,
                count,
            });
        }
    }
    Ok(())
}

fn validate_rectangle_bounds<C>(
    component: &GridComponent<C>,
    rectangle: GridRect,
    rectangle_index: usize,
) -> Result<(), ValidationError> {
    if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
        return Err(ValidationError::NonPositiveRectangle {
            rectangle: rectangle_index,
        });
    }
    if rectangle.x1 > component.grid_width || rectangle.y1 > component.grid_height {
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
