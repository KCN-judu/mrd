//! Difference-array validation over a prepared coordinate arrangement.

use mrd_domain::{CoordinateRect, DoubledPoint, RectilinearPolygon};

use crate::polygon::PolygonValidationError;

use super::Arrangement;

#[derive(Clone, Copy, Debug, Default)]
pub struct Validator;

impl Validator {
    /// # Errors
    ///
    /// Returns the first exact coverage or rectangle-geometry failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate(
        self,
        arrangement: &Arrangement,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<(), PolygonValidationError> {
        let mut difference = vec![0_i64; (arrangement.width + 1) * (arrangement.height + 1)];
        let mut area = 0_i128;
        for (index, rectangle) in rectangles.iter().copied().enumerate() {
            if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
                return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
            }
            let center = DoubledPoint::new(
                i128::from(rectangle.x0) + i128::from(rectangle.x1),
                i128::from(rectangle.y0) + i128::from(rectangle.y1),
            );
            let Ok(x0) = arrangement.xs.binary_search(&rectangle.x0) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: center,
                });
            };
            let Ok(x1) = arrangement.xs.binary_search(&rectangle.x1) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: center,
                });
            };
            let Ok(y0) = arrangement.ys.binary_search(&rectangle.y0) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: center,
                });
            };
            let Ok(y1) = arrangement.ys.binary_search(&rectangle.y1) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: center,
                });
            };
            let stride = arrangement.width + 1;
            difference[y0 * stride + x0] += 1;
            difference[y0 * stride + x1] -= 1;
            difference[y1 * stride + x0] -= 1;
            difference[y1 * stride + x1] += 1;
            area = area
                .checked_add(rectangle.area())
                .ok_or(PolygonValidationError::AreaOverflow)?;
        }
        let polygon_area = polygon
            .twice_signed_area()
            .map_err(PolygonValidationError::Polygon)?;
        if area
            .checked_mul(2)
            .ok_or(PolygonValidationError::AreaOverflow)?
            != polygon_area
        {
            return Err(PolygonValidationError::AreaMismatch {
                polygon_area_twice: polygon_area,
                rectangle_area_twice: area * 2,
            });
        }
        let stride = arrangement.width + 1;
        for y in 0..=arrangement.height {
            for x in 0..=arrangement.width {
                let index = y * stride + x;
                let left = x.checked_sub(1).map_or(0, |_| difference[index - 1]);
                let above = y.checked_sub(1).map_or(0, |_| difference[index - stride]);
                let diagonal = if x > 0 && y > 0 {
                    difference[index - stride - 1]
                } else {
                    0
                };
                difference[index] += left + above - diagonal;
            }
        }
        for y in 0..arrangement.height {
            for x in 0..arrangement.width {
                let coverage = difference[y * stride + x];
                let point = DoubledPoint::new(
                    i128::from(arrangement.xs[x]) + i128::from(arrangement.xs[x + 1]),
                    i128::from(arrangement.ys[y]) + i128::from(arrangement.ys[y + 1]),
                );
                if coverage > 1 {
                    let mut covering =
                        rectangles
                            .iter()
                            .enumerate()
                            .filter_map(|(index, rectangle)| {
                                rectangle
                                    .contains_doubled_point_strict(point)
                                    .then_some(index)
                            });
                    let first = covering.next().unwrap_or(0);
                    let second = covering.next().unwrap_or(first);
                    return Err(PolygonValidationError::Overlap {
                        first,
                        second,
                        point,
                    });
                }
                if arrangement.occupied[y * arrangement.width + x] && coverage == 0 {
                    return Err(PolygonValidationError::UncoveredInterior { point });
                }
                if !arrangement.occupied[y * arrangement.width + x] && coverage != 0 {
                    let rectangle = rectangles
                        .iter()
                        .position(|rectangle| rectangle.contains_doubled_point_strict(point))
                        .unwrap_or(0);
                    return Err(PolygonValidationError::OutsidePolygon { rectangle, point });
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "indexed-difference-array"
    }
}
