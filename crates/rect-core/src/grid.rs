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
}

#[cfg(test)]
mod tests {
    use super::ColorGrid;

    #[test]
    fn corner_contact_is_not_connectivity() {
        let grid = ColorGrid::new(2, 2, vec![true, false, false, true]).unwrap();
        let components = grid.four_connected_components();
        assert_eq!(components.len(), 4);
    }
}
