//! Shared exact coordinate arrangement for indexed polygon completion.

use std::collections::{BTreeSet, VecDeque};

use rect_core::{CoordinateRect, Point, PreparedPolygonContext, RectilinearPolygon};
use serde::{Deserialize, Serialize};

use crate::polygon::{
    HorizontalCutSegment, PolygonSgError, PolygonValidationError, VerticalCutSegment,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArrangementMetrics {
    pub arrangement_x_count: usize,
    pub arrangement_y_count: usize,
    pub arrangement_atomic_cells: usize,
    pub arrangement_point_location_queries: usize,
    pub arrangement_boundary_edge_visits: usize,
    pub arrangement_span_writes: usize,
    pub arrangement_rectangle_recovery_visits: usize,
}

/// One exact coordinate-compressed polygon arrangement shared by recovery and
/// indexed validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCoordinateArrangement {
    xs: Vec<i64>,
    ys: Vec<i64>,
    occupied: Vec<bool>,
    horizontal_barriers: Vec<bool>,
    vertical_barriers: Vec<bool>,
    width: usize,
    height: usize,
    metrics: ArrangementMetrics,
    owned_bytes: usize,
}

impl PreparedCoordinateArrangement {
    /// Builds occupancy by a scanline parity sweep and indexes all barriers.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError::CoordinateOverflow`] when arrangement sizes
    /// cannot be represented safely.
    #[allow(clippy::missing_panics_doc, clippy::too_many_lines)]
    pub fn new(
        prepared: &PreparedPolygonContext,
        horizontal_cuts: &BTreeSet<HorizontalCutSegment>,
        vertical_cuts: &BTreeSet<VerticalCutSegment>,
    ) -> Result<Self, PolygonSgError> {
        let polygon = prepared.polygon();
        let mut xs = polygon
            .loops()
            .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.x))
            .collect::<BTreeSet<_>>();
        let mut ys = polygon
            .loops()
            .flat_map(|boundary_loop| boundary_loop.vertices.iter().map(|point| point.y))
            .collect::<BTreeSet<_>>();
        for cut in horizontal_cuts {
            xs.extend([cut.left, cut.right]);
            ys.insert(cut.y);
        }
        for cut in vertical_cuts {
            xs.insert(cut.x);
            ys.extend([cut.bottom, cut.top]);
        }
        let xs = xs.into_iter().collect::<Vec<_>>();
        let ys = ys.into_iter().collect::<Vec<_>>();
        let width = xs.len().saturating_sub(1);
        let height = ys.len().saturating_sub(1);
        let atomic_cells = width
            .checked_mul(height)
            .ok_or(PolygonSgError::CoordinateOverflow)?;
        let mut occupied = vec![false; atomic_cells];
        let mut metrics = ArrangementMetrics {
            arrangement_x_count: xs.len(),
            arrangement_y_count: ys.len(),
            arrangement_atomic_cells: atomic_cells,
            ..ArrangementMetrics::default()
        };

        for y_index in 0..height {
            let doubled_y = i128::from(ys[y_index]) + i128::from(ys[y_index + 1]);
            let mut crossings = prepared
                .edge_index()
                .active_vertical_edge_ids(doubled_y)
                .into_iter()
                .filter_map(|edge_id| prepared.edge_index().edge(edge_id).map(|edge| edge.first.x))
                .collect::<Vec<_>>();
            crossings.sort_unstable();
            crossings.dedup();
            metrics.arrangement_boundary_edge_visits += crossings.len();
            for pair in crossings.chunks_exact(2) {
                let left = xs
                    .binary_search(&pair[0])
                    .expect("boundary crossing coordinate is in arrangement");
                let right = xs
                    .binary_search(&pair[1])
                    .expect("boundary crossing coordinate is in arrangement");
                for x_index in left..right {
                    occupied[y_index * width + x_index] = true;
                    metrics.arrangement_span_writes += 1;
                }
            }
        }

        let mut horizontal_barriers = vec![false; (height + 1) * width];
        let mut vertical_barriers = vec![false; (width + 1) * height];
        for boundary_loop in polygon.loops() {
            for (first, second) in boundary_loop.edges() {
                index_boundary_segment(
                    &xs,
                    &ys,
                    &mut horizontal_barriers,
                    &mut vertical_barriers,
                    first,
                    second,
                );
            }
        }
        for cut in horizontal_cuts {
            let left = xs
                .binary_search(&cut.left)
                .expect("cut endpoint is indexed");
            let right = xs
                .binary_search(&cut.right)
                .expect("cut endpoint is indexed");
            let y = ys.binary_search(&cut.y).expect("cut ordinate is indexed");
            for x in left..right {
                horizontal_barriers[y * width + x] = true;
            }
        }
        for cut in vertical_cuts {
            let x = xs.binary_search(&cut.x).expect("cut abscissa is indexed");
            let bottom = ys
                .binary_search(&cut.bottom)
                .expect("cut endpoint is indexed");
            let top = ys.binary_search(&cut.top).expect("cut endpoint is indexed");
            for y in bottom..top {
                vertical_barriers[y * (width + 1) + x] = true;
            }
        }

        let owned_bytes = xs.len() * std::mem::size_of::<i64>()
            + ys.len() * std::mem::size_of::<i64>()
            + (occupied.len() + horizontal_barriers.len() + vertical_barriers.len())
                * std::mem::size_of::<bool>();
        Ok(Self {
            xs,
            ys,
            occupied,
            horizontal_barriers,
            vertical_barriers,
            width,
            height,
            metrics,
            owned_bytes,
        })
    }

    #[must_use]
    pub fn xs(&self) -> &[i64] {
        &self.xs
    }

    #[must_use]
    pub fn ys(&self) -> &[i64] {
        &self.ys
    }

    #[must_use]
    pub const fn metrics(&self) -> &ArrangementMetrics {
        &self.metrics
    }

    #[must_use]
    pub const fn owned_bytes_estimate(&self) -> usize {
        self.owned_bytes
    }

    #[must_use]
    pub fn occupied(&self, x: usize, y: usize) -> bool {
        self.occupied[y * self.width + x]
    }

    fn vertical_barrier(&self, x: usize, y: usize) -> bool {
        self.vertical_barriers[y * (self.width + 1) + x]
    }

    fn horizontal_barrier(&self, x: usize, y: usize) -> bool {
        self.horizontal_barriers[y * self.width + x]
    }

    /// Recovers canonical coordinate rectangles from arrangement regions.
    ///
    /// # Errors
    ///
    /// Returns [`PolygonSgError::NonRectangularCompletionRegion`] when a
    /// barrier-connected region is not a rectangle.
    pub fn recover_rectangles(&mut self) -> Result<Vec<CoordinateRect>, PolygonSgError> {
        let cell_count = self.occupied.len();
        let mut region_ids = vec![usize::MAX; cell_count];
        let mut queue = VecDeque::new();
        let mut rectangles = Vec::new();
        for seed in 0..cell_count {
            if !self.occupied[seed] || region_ids[seed] != usize::MAX {
                continue;
            }
            let region_id = rectangles.len();
            region_ids[seed] = region_id;
            queue.push_back(seed);
            let (mut left, mut right) = (seed % self.width, seed % self.width + 1);
            let (mut bottom, mut top) = (seed / self.width, seed / self.width + 1);
            while let Some(index) = queue.pop_front() {
                self.metrics.arrangement_rectangle_recovery_visits += 1;
                let x = index % self.width;
                let y = index / self.width;
                left = left.min(x);
                right = right.max(x + 1);
                bottom = bottom.min(y);
                top = top.max(y + 1);
                let mut visit = |neighbor: usize| {
                    if self.occupied[neighbor] && region_ids[neighbor] == usize::MAX {
                        region_ids[neighbor] = region_id;
                        queue.push_back(neighbor);
                    }
                };
                if x > 0 && !self.vertical_barrier(x, y) {
                    visit(index - 1);
                }
                if x + 1 < self.width && !self.vertical_barrier(x + 1, y) {
                    visit(index + 1);
                }
                if y > 0 && !self.horizontal_barrier(x, y) {
                    visit(index - self.width);
                }
                if y + 1 < self.height && !self.horizontal_barrier(x, y + 1) {
                    visit(index + self.width);
                }
            }
            if !(bottom..top)
                .all(|y| (left..right).all(|x| region_ids[y * self.width + x] == region_id))
            {
                return Err(PolygonSgError::NonRectangularCompletionRegion {
                    point: Point::new(self.xs[seed % self.width], self.ys[seed / self.width]),
                });
            }
            rectangles.push(CoordinateRect::new(
                self.xs[left],
                self.ys[bottom],
                self.xs[right],
                self.ys[top],
            )?);
        }
        rectangles.sort_unstable();
        Ok(rectangles)
    }

    /// Validates coverage using a two-dimensional difference array.
    ///
    /// # Errors
    ///
    /// Returns the first exact coverage or rectangle-geometry failure.
    #[allow(clippy::too_many_lines)]
    pub fn validate_rectangles(
        &self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<(), PolygonValidationError> {
        let mut difference = vec![0_i64; (self.width + 1) * (self.height + 1)];
        let mut area = 0_i128;
        for (index, rectangle) in rectangles.iter().copied().enumerate() {
            if rectangle.x0 >= rectangle.x1 || rectangle.y0 >= rectangle.y1 {
                return Err(PolygonValidationError::NonPositiveRectangle { rectangle: index });
            }
            let Ok(x0) = self.xs.binary_search(&rectangle.x0) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: rect_core::DoubledPoint::new(
                        i128::from(rectangle.x0) + i128::from(rectangle.x1),
                        i128::from(rectangle.y0) + i128::from(rectangle.y1),
                    ),
                });
            };
            let Ok(x1) = self.xs.binary_search(&rectangle.x1) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: rect_core::DoubledPoint::new(
                        i128::from(rectangle.x0) + i128::from(rectangle.x1),
                        i128::from(rectangle.y0) + i128::from(rectangle.y1),
                    ),
                });
            };
            let Ok(y0) = self.ys.binary_search(&rectangle.y0) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: rect_core::DoubledPoint::new(
                        i128::from(rectangle.x0) + i128::from(rectangle.x1),
                        i128::from(rectangle.y0) + i128::from(rectangle.y1),
                    ),
                });
            };
            let Ok(y1) = self.ys.binary_search(&rectangle.y1) else {
                return Err(PolygonValidationError::OutsidePolygon {
                    rectangle: index,
                    point: rect_core::DoubledPoint::new(
                        i128::from(rectangle.x0) + i128::from(rectangle.x1),
                        i128::from(rectangle.y0) + i128::from(rectangle.y1),
                    ),
                });
            };
            difference[y0 * (self.width + 1) + x0] += 1;
            difference[y0 * (self.width + 1) + x1] -= 1;
            difference[y1 * (self.width + 1) + x0] -= 1;
            difference[y1 * (self.width + 1) + x1] += 1;
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
        let stride = self.width + 1;
        for y in 0..=self.height {
            for x in 0..=self.width {
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
        for y in 0..self.height {
            for x in 0..self.width {
                let coverage = difference[y * stride + x];
                let point = rect_core::DoubledPoint::new(
                    i128::from(self.xs[x]) + i128::from(self.xs[x + 1]),
                    i128::from(self.ys[y]) + i128::from(self.ys[y + 1]),
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
                if self.occupied[y * self.width + x] && coverage == 0 {
                    return Err(PolygonValidationError::UncoveredInterior { point });
                }
                if !self.occupied[y * self.width + x] && coverage != 0 {
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
}

fn index_boundary_segment(
    xs: &[i64],
    ys: &[i64],
    horizontal: &mut [bool],
    vertical: &mut [bool],
    first: Point,
    second: Point,
) {
    let width = xs.len().saturating_sub(1);
    if first.y == second.y {
        let left = xs
            .binary_search(&first.x.min(second.x))
            .expect("boundary x indexed");
        let right = xs
            .binary_search(&first.x.max(second.x))
            .expect("boundary x indexed");
        let y = ys.binary_search(&first.y).expect("boundary y indexed");
        for x in left..right {
            horizontal[y * width + x] = true;
        }
    } else {
        let x = xs.binary_search(&first.x).expect("boundary x indexed");
        let bottom = ys
            .binary_search(&first.y.min(second.y))
            .expect("boundary y indexed");
        let top = ys
            .binary_search(&first.y.max(second.y))
            .expect("boundary y indexed");
        for y in bottom..top {
            vertical[y * (width + 1) + x] = true;
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct IndexedArrangementValidator;

#[derive(Clone, Copy, Debug, Default)]
pub struct ReferenceArrangementValidator;

impl IndexedArrangementValidator {
    /// # Errors
    ///
    /// Returns the first exact coverage or rectangle-geometry failure.
    pub fn validate(
        &self,
        arrangement: &PreparedCoordinateArrangement,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<(), PolygonValidationError> {
        arrangement.validate_rectangles(polygon, rectangles)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "indexed-difference-array"
    }
}

impl ReferenceArrangementValidator {
    /// # Errors
    ///
    /// Returns the reference validator's first exact coverage failure.
    pub fn validate(
        &self,
        polygon: &RectilinearPolygon,
        rectangles: &[CoordinateRect],
    ) -> Result<(), PolygonValidationError> {
        crate::polygon::validate_polygon_dissection(polygon, rectangles)
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        "reference-arrangement-scan"
    }
}
