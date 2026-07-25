use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct ComponentId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Cell {
    pub x: usize,
    pub y: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ColorGrid<C> {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<C>,
}

impl<C> ColorGrid<C> {
    /// # Errors
    ///
    /// Returns [`GridError`] when dimensions overflow or `cells` has the wrong length.
    pub fn new(width: usize, height: usize, cells: Vec<C>) -> Result<Self, GridError> {
        let expected = width
            .checked_mul(height)
            .ok_or(GridError::DimensionOverflow)?;
        if cells.len() != expected {
            return Err(GridError::CellCount {
                expected,
                actual: cells.len(),
            });
        }
        Ok(Self {
            width,
            height,
            cells,
        })
    }

    #[must_use]
    pub fn get(&self, x: usize, y: usize) -> Option<&C> {
        (x < self.width && y < self.height).then(|| &self.cells[y * self.width + x])
    }
}

impl<C: Clone + Eq> ColorGrid<C> {
    #[must_use]
    pub fn four_connected_components(&self) -> Vec<GridComponent<C>> {
        let mut visited = vec![false; self.cells.len()];
        let mut components = Vec::new();

        for seed_index in 0..self.cells.len() {
            if visited[seed_index] {
                continue;
            }
            visited[seed_index] = true;
            let seed = Cell {
                x: seed_index % self.width,
                y: seed_index / self.width,
            };
            let color = self.cells[seed_index].clone();
            let mut queue = VecDeque::from([seed]);
            let mut cells = Vec::new();

            while let Some(cell) = queue.pop_front() {
                cells.push(cell);
                for neighbor in neighbors(cell, self.width, self.height) {
                    let index = neighbor.y * self.width + neighbor.x;
                    if !visited[index] && self.cells[index] == color {
                        visited[index] = true;
                        queue.push_back(neighbor);
                    }
                }
            }

            cells.sort_unstable();
            components.push(GridComponent {
                id: ComponentId(components.len()),
                color,
                grid_width: self.width,
                grid_height: self.height,
                cells,
            });
        }
        components
    }
}

fn neighbors(cell: Cell, width: usize, height: usize) -> impl Iterator<Item = Cell> {
    let mut result = [None; 4];
    let mut count = 0;
    if cell.x > 0 {
        result[count] = Some(Cell {
            x: cell.x - 1,
            y: cell.y,
        });
        count += 1;
    }
    if cell.x + 1 < width {
        result[count] = Some(Cell {
            x: cell.x + 1,
            y: cell.y,
        });
        count += 1;
    }
    if cell.y > 0 {
        result[count] = Some(Cell {
            x: cell.x,
            y: cell.y - 1,
        });
        count += 1;
    }
    if cell.y + 1 < height {
        result[count] = Some(Cell {
            x: cell.x,
            y: cell.y + 1,
        });
        count += 1;
    }
    result.into_iter().take(count).flatten()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GridComponent<C> {
    pub id: ComponentId,
    pub color: C,
    pub grid_width: usize,
    pub grid_height: usize,
    pub cells: Vec<Cell>,
}

/// Component-local geometry prepared for repeated grid algorithms.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PreparedGridComponent {
    pub x0: usize,
    pub y0: usize,
    pub x1: usize,
    pub y1: usize,
    pub occupancy: Vec<bool>,
    pub horizontal_interior_runs: Vec<Vec<(usize, usize)>>,
    pub vertical_interior_runs: Vec<Vec<(usize, usize)>>,
}

impl PreparedGridComponent {
    /// Builds a component-local occupancy mask and maximal two-sided runs.
    ///
    /// # Errors
    ///
    /// Returns [`GridError`] for an empty component or dimension overflow.
    pub fn from_component<C>(component: &GridComponent<C>) -> Result<Self, GridError> {
        let (x0, y0, x1, y1) = component.bounds().ok_or(GridError::EmptyComponent)?;
        let width = x1 - x0;
        let height = y1 - y0;
        let mut occupancy = vec![
            false;
            width
                .checked_mul(height)
                .ok_or(GridError::DimensionOverflow)?
        ];
        for cell in &component.cells {
            occupancy[(cell.y - y0) * width + cell.x - x0] = true;
        }
        let mut horizontal_interior_runs = vec![Vec::new(); height + 1];
        for y in 0..=height {
            let mut x = 0;
            while x < width {
                if y > 0 && y < height && occupancy[(y - 1) * width + x] && occupancy[y * width + x]
                {
                    let start = x;
                    x += 1;
                    while x < width && occupancy[(y - 1) * width + x] && occupancy[y * width + x] {
                        x += 1;
                    }
                    horizontal_interior_runs[y].push((start + x0, x + x0));
                } else {
                    x += 1;
                }
            }
        }
        let mut vertical_interior_runs = vec![Vec::new(); width + 1];
        for x in 0..=width {
            let mut y = 0;
            while y < height {
                if x > 0 && x < width && occupancy[y * width + x - 1] && occupancy[y * width + x] {
                    let start = y;
                    y += 1;
                    while y < height && occupancy[y * width + x - 1] && occupancy[y * width + x] {
                        y += 1;
                    }
                    vertical_interior_runs[x].push((start + y0, y + y0));
                } else {
                    y += 1;
                }
            }
        }
        Ok(Self {
            x0,
            y0,
            x1,
            y1,
            occupancy,
            horizontal_interior_runs,
            vertical_interior_runs,
        })
    }

    #[must_use]
    pub fn contains_cell(&self, x: usize, y: usize) -> bool {
        x >= self.x0
            && x < self.x1
            && y >= self.y0
            && y < self.y1
            && self.occupancy[(y - self.y0) * (self.x1 - self.x0) + x - self.x0]
    }
}

impl<C> GridComponent<C> {
    #[must_use]
    pub fn contains_cell(&self, x: usize, y: usize) -> bool {
        self.cells.binary_search(&Cell { x, y }).is_ok()
    }

    #[must_use]
    pub const fn cell_count(&self) -> usize {
        self.cells.len()
    }

    #[must_use]
    pub fn bounds(&self) -> Option<(usize, usize, usize, usize)> {
        let first = self.cells.first()?;
        let mut x0 = first.x;
        let mut y0 = first.y;
        let mut x1 = first.x + 1;
        let mut y1 = first.y + 1;
        for cell in &self.cells[1..] {
            x0 = x0.min(cell.x);
            y0 = y0.min(cell.y);
            x1 = x1.max(cell.x + 1);
            y1 = y1.max(cell.y + 1);
        }
        Some((x0, y0, x1, y1))
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum GridError {
    #[error("grid dimensions overflow usize")]
    DimensionOverflow,
    #[error("expected {expected} cells but received {actual}")]
    CellCount { expected: usize, actual: usize },
    #[error("cannot prepare an empty component")]
    EmptyComponent,
}

#[cfg(test)]
mod tests {
    use super::{ColorGrid, PreparedGridComponent};

    #[test]
    fn corner_contact_is_not_connectivity() {
        let grid = ColorGrid::new(2, 2, vec![true, false, false, true]).unwrap();
        let components = grid.four_connected_components();
        assert_eq!(components.len(), 4);
    }

    #[test]
    fn prepared_component_builds_local_runs() {
        let grid = ColorGrid::new(3, 2, vec![true, true, false, true, true, false]).unwrap();
        let component = grid
            .four_connected_components()
            .into_iter()
            .find(|component| component.color)
            .unwrap();
        let prepared = PreparedGridComponent::from_component(&component).unwrap();
        assert_eq!(
            (prepared.x0, prepared.y0, prepared.x1, prepared.y1),
            (0, 0, 2, 2)
        );
        assert!(prepared.contains_cell(1, 1));
        assert_eq!(prepared.horizontal_interior_runs[1], vec![(0, 2)]);
    }
}
