use rect_core::{DissectionResult, GridComponent, GridRect, ValidationError, validate_dissection};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GridTransform {
    Translate { dx: usize, dy: usize },
    ReflectHorizontal,
    ReflectVertical,
    Rotate90,
    Rotate180,
    Rotate270,
    ReflectMainDiagonal,
    ReflectAntiDiagonal,
    Scale { factor: usize },
}

#[derive(Clone, Debug)]
pub struct TransformedComponent<C> {
    pub component: GridComponent<C>,
    pub transform: GridTransform,
    original_width: usize,
    original_height: usize,
}

impl<C: Clone> TransformedComponent<C> {
    /// Applies an exact cell-grid transform while preserving component identity and color.
    ///
    /// # Errors
    ///
    /// Returns [`TransformError`] for zero scaling or overflowing dimensions/coordinates.
    pub fn new(
        original: &GridComponent<C>,
        transform: GridTransform,
    ) -> Result<Self, TransformError> {
        let (target_width, target_height) =
            transformed_dimensions(original.grid_width, original.grid_height, transform)?;
        let mut cells = Vec::new();
        for cell in &original.cells {
            match transform {
                GridTransform::Scale { factor } => {
                    for dy in 0..factor {
                        for dx in 0..factor {
                            cells.push(rect_core::Cell {
                                x: cell
                                    .x
                                    .checked_mul(factor)
                                    .and_then(|value| value.checked_add(dx))
                                    .ok_or(TransformError::Overflow)?,
                                y: cell
                                    .y
                                    .checked_mul(factor)
                                    .and_then(|value| value.checked_add(dy))
                                    .ok_or(TransformError::Overflow)?,
                            });
                        }
                    }
                }
                _ => cells.push(transform_cell(
                    *cell,
                    original.grid_width,
                    original.grid_height,
                    transform,
                )?),
            }
        }
        cells.sort_unstable();
        cells.dedup();
        Ok(Self {
            component: GridComponent {
                id: original.id,
                color: original.color.clone(),
                grid_width: target_width,
                grid_height: target_height,
                cells,
            },
            transform,
            original_width: original.grid_width,
            original_height: original.grid_height,
        })
    }

    /// Maps a transformed solver result back and validates it on the original component.
    ///
    /// # Errors
    ///
    /// Returns [`TransformError`] when inverse coordinates are invalid, scaled
    /// cuts are not aligned, or the mapped dissection fails exact validation.
    pub fn map_result_back(
        &self,
        original: &GridComponent<C>,
        transformed_result: &DissectionResult,
    ) -> Result<DissectionResult, TransformError> {
        let mut mapped = transformed_result.clone();
        mapped.rectangles = transformed_result
            .rectangles
            .iter()
            .copied()
            .map(|rectangle| self.inverse_rectangle(rectangle))
            .collect::<Result<Vec<_>, _>>()?;
        mapped.rectangles.sort_unstable();
        mapped.rectangles.dedup();
        mapped.optimum_rectangle_count = mapped.rectangles.len();
        mapped.diagnostics.output_rectangle_count = mapped.rectangles.len();
        validate_dissection(original, &mapped)?;
        Ok(mapped)
    }

    fn inverse_rectangle(&self, rectangle: GridRect) -> Result<GridRect, TransformError> {
        let (x0, y0, x1, y1) = match self.transform {
            GridTransform::Translate { dx, dy } => (
                rectangle.x0.checked_sub(dx),
                rectangle.y0.checked_sub(dy),
                rectangle.x1.checked_sub(dx),
                rectangle.y1.checked_sub(dy),
            ),
            GridTransform::ReflectHorizontal => (
                self.original_width.checked_sub(rectangle.x1),
                Some(rectangle.y0),
                self.original_width.checked_sub(rectangle.x0),
                Some(rectangle.y1),
            ),
            GridTransform::ReflectVertical => (
                Some(rectangle.x0),
                self.original_height.checked_sub(rectangle.y1),
                Some(rectangle.x1),
                self.original_height.checked_sub(rectangle.y0),
            ),
            GridTransform::Rotate90 => (
                Some(rectangle.y0),
                self.original_height.checked_sub(rectangle.x1),
                Some(rectangle.y1),
                self.original_height.checked_sub(rectangle.x0),
            ),
            GridTransform::Rotate180 => (
                self.original_width.checked_sub(rectangle.x1),
                self.original_height.checked_sub(rectangle.y1),
                self.original_width.checked_sub(rectangle.x0),
                self.original_height.checked_sub(rectangle.y0),
            ),
            GridTransform::Rotate270 => (
                self.original_width.checked_sub(rectangle.y1),
                Some(rectangle.x0),
                self.original_width.checked_sub(rectangle.y0),
                Some(rectangle.x1),
            ),
            GridTransform::ReflectMainDiagonal => (
                Some(rectangle.y0),
                Some(rectangle.x0),
                Some(rectangle.y1),
                Some(rectangle.x1),
            ),
            GridTransform::ReflectAntiDiagonal => (
                self.original_width.checked_sub(rectangle.y1),
                self.original_height.checked_sub(rectangle.x1),
                self.original_width.checked_sub(rectangle.y0),
                self.original_height.checked_sub(rectangle.x0),
            ),
            GridTransform::Scale { factor } => {
                if factor == 0
                    || !rectangle.x0.is_multiple_of(factor)
                    || !rectangle.y0.is_multiple_of(factor)
                    || !rectangle.x1.is_multiple_of(factor)
                    || !rectangle.y1.is_multiple_of(factor)
                {
                    return Err(TransformError::UnalignedScaledRectangle { rectangle, factor });
                }
                (
                    Some(rectangle.x0 / factor),
                    Some(rectangle.y0 / factor),
                    Some(rectangle.x1 / factor),
                    Some(rectangle.y1 / factor),
                )
            }
        };
        GridRect::new(
            x0.ok_or(TransformError::InvalidInverse)?,
            y0.ok_or(TransformError::InvalidInverse)?,
            x1.ok_or(TransformError::InvalidInverse)?,
            y1.ok_or(TransformError::InvalidInverse)?,
        )
        .map_err(|_| TransformError::InvalidInverse)
    }
}

fn transformed_dimensions(
    width: usize,
    height: usize,
    transform: GridTransform,
) -> Result<(usize, usize), TransformError> {
    match transform {
        GridTransform::Translate { dx, dy } => Ok((
            width.checked_add(dx).ok_or(TransformError::Overflow)?,
            height.checked_add(dy).ok_or(TransformError::Overflow)?,
        )),
        GridTransform::Rotate90
        | GridTransform::Rotate270
        | GridTransform::ReflectMainDiagonal
        | GridTransform::ReflectAntiDiagonal => Ok((height, width)),
        GridTransform::Scale { factor } => {
            if factor == 0 {
                return Err(TransformError::ZeroScale);
            }
            Ok((
                width.checked_mul(factor).ok_or(TransformError::Overflow)?,
                height.checked_mul(factor).ok_or(TransformError::Overflow)?,
            ))
        }
        GridTransform::ReflectHorizontal
        | GridTransform::ReflectVertical
        | GridTransform::Rotate180 => Ok((width, height)),
    }
}

fn transform_cell(
    cell: rect_core::Cell,
    width: usize,
    height: usize,
    transform: GridTransform,
) -> Result<rect_core::Cell, TransformError> {
    let transformed = match transform {
        GridTransform::Translate { dx, dy } => rect_core::Cell {
            x: cell.x.checked_add(dx).ok_or(TransformError::Overflow)?,
            y: cell.y.checked_add(dy).ok_or(TransformError::Overflow)?,
        },
        GridTransform::ReflectHorizontal => rect_core::Cell {
            x: width - 1 - cell.x,
            y: cell.y,
        },
        GridTransform::ReflectVertical => rect_core::Cell {
            x: cell.x,
            y: height - 1 - cell.y,
        },
        GridTransform::Rotate90 => rect_core::Cell {
            x: height - 1 - cell.y,
            y: cell.x,
        },
        GridTransform::Rotate180 => rect_core::Cell {
            x: width - 1 - cell.x,
            y: height - 1 - cell.y,
        },
        GridTransform::Rotate270 => rect_core::Cell {
            x: cell.y,
            y: width - 1 - cell.x,
        },
        GridTransform::ReflectMainDiagonal => rect_core::Cell {
            x: cell.y,
            y: cell.x,
        },
        GridTransform::ReflectAntiDiagonal => rect_core::Cell {
            x: height - 1 - cell.y,
            y: width - 1 - cell.x,
        },
        GridTransform::Scale { .. } => return Err(TransformError::InvalidInverse),
    };
    Ok(transformed)
}

#[derive(Debug, Error)]
pub enum TransformError {
    #[error("uniform scale factor must be positive")]
    ZeroScale,
    #[error("grid transformation overflowed usize")]
    Overflow,
    #[error("rectangle cannot be mapped through the inverse transform")]
    InvalidInverse,
    #[error("scaled rectangle {rectangle:?} is not aligned to factor {factor}")]
    UnalignedScaledRectangle { rectangle: GridRect, factor: usize },
    #[error("mapped-back dissection is invalid: {0}")]
    InvalidMappedOutput(#[from] ValidationError),
}

#[cfg(test)]
mod tests {
    use rect_core::{ColorGrid, GridComponent};
    use rect_dominance::{DominanceMode, VerificationMode};

    use super::{GridTransform, TransformedComponent};

    #[test]
    fn every_transform_maps_every_solver_output_back_to_valid_geometry() {
        let grid = ColorGrid::new(
            4,
            3,
            vec![
                true, true, true, false, true, false, true, true, true, true, true, false,
            ],
        )
        .unwrap();
        let original = grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
            .max_by_key(GridComponent::cell_count)
            .unwrap();
        let expected = rect_oracle_exact_cover::solve(&original)
            .unwrap()
            .optimum_rectangle_count;
        let transforms = [
            GridTransform::Translate { dx: 3, dy: 2 },
            GridTransform::ReflectHorizontal,
            GridTransform::ReflectVertical,
            GridTransform::Rotate90,
            GridTransform::Rotate180,
            GridTransform::Rotate270,
            GridTransform::ReflectMainDiagonal,
            GridTransform::ReflectAntiDiagonal,
            GridTransform::Scale { factor: 2 },
        ];
        for transform in transforms {
            let transformed = TransformedComponent::new(&original, transform).unwrap();
            let results = [
                rect_oracle_exact_cover::solve(&transformed.component).unwrap(),
                rect_oracle_sg::solve(&transformed.component).unwrap(),
                rect_dominance::solve(&transformed.component, DominanceMode::ExplicitEdges)
                    .unwrap(),
                rect_dominance::solve(&transformed.component, DominanceMode::Compact).unwrap(),
                rect_dominance::solve_with_verification_mode(
                    &transformed.component,
                    VerificationMode::CompactOnly,
                )
                .unwrap(),
            ];
            for result in results {
                assert_eq!(result.optimum_rectangle_count, expected, "{transform:?}");
                transformed.map_result_back(&original, &result).unwrap();
            }
        }
    }
}
