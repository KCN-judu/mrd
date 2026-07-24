use std::collections::HashMap;
use std::time::Instant;

use rect_core::{
    Certificate, Diagnostics, DissectionResult, GridComponent, GridRect, ValidationError,
    validate_dissection,
};
use rect_graph::BitSet;
use serde_json::json;
use thiserror::Error;

#[derive(Clone, Debug)]
struct Candidate {
    rectangle: GridRect,
    cells: BitSet,
    area: usize,
}

pub fn enumerate_valid_rectangles<C>(component: &GridComponent<C>) -> Vec<GridRect> {
    let Some((min_x, min_y, max_x, max_y)) = component.bounds() else {
        return Vec::new();
    };
    let mut rectangles = Vec::new();
    for y0 in min_y..max_y {
        for y1 in (y0 + 1)..=max_y {
            for x0 in min_x..max_x {
                for x1 in (x0 + 1)..=max_x {
                    let rectangle = GridRect { x0, y0, x1, y1 };
                    if (y0..y1).all(|y| (x0..x1).all(|x| component.contains_cell(x, y))) {
                        rectangles.push(rectangle);
                    }
                }
            }
        }
    }
    rectangles.sort_by_key(|rectangle| {
        (
            std::cmp::Reverse(rectangle.area()),
            rectangle.y0,
            rectangle.x0,
            rectangle.y1,
            rectangle.x1,
        )
    });
    rectangles
}

/// Solves a component with independent bitset Algorithm X branch-and-bound.
///
/// # Errors
///
/// Returns [`ExactCoverError`] for an empty component or an invalid produced cover.
pub fn solve<C>(component: &GridComponent<C>) -> Result<DissectionResult, ExactCoverError> {
    if component.cells.is_empty() {
        return Err(ExactCoverError::EmptyComponent);
    }
    let started = Instant::now();
    let cell_index = component
        .cells
        .iter()
        .enumerate()
        .map(|(index, cell)| (*cell, index))
        .collect::<HashMap<_, _>>();
    let rectangles = enumerate_valid_rectangles(component);
    let mut candidates = Vec::with_capacity(rectangles.len());
    let mut candidates_by_cell = vec![Vec::new(); component.cell_count()];

    for rectangle in rectangles {
        let mut cells = BitSet::new(component.cell_count());
        for y in rectangle.y0..rectangle.y1 {
            for x in rectangle.x0..rectangle.x1 {
                let index = cell_index[&rect_core::Cell { x, y }];
                cells.insert(index);
            }
        }
        let index = candidates.len();
        for cell in cells.ones() {
            candidates_by_cell[cell].push(index);
        }
        candidates.push(Candidate {
            rectangle,
            cells,
            area: rectangle.area(),
        });
    }

    let mut search = Search {
        candidates: &candidates,
        candidates_by_cell: &candidates_by_cell,
        cell_count: component.cell_count(),
        best: singleton_upper_bound(component, &cell_index, &candidates),
        nodes_visited: 0,
        lower_bound_prunes: 0,
    };
    let mut chosen = Vec::new();
    search.recurse(&BitSet::new(component.cell_count()), &mut chosen);
    let selected = search.best;
    let output_rectangles = selected
        .iter()
        .map(|&index| candidates[index].rectangle)
        .collect::<Vec<_>>();
    let elapsed = started.elapsed();
    let result = DissectionResult {
        optimum_rectangle_count: output_rectangles.len(),
        rectangles: output_rectangles,
        diagnostics: Diagnostics {
            cell_count: component.cell_count(),
            final_rectangle_count: selected.len(),
            phase_microseconds: [("exact_cover".to_owned(), elapsed.as_micros())]
                .into_iter()
                .collect(),
            ..Diagnostics::default()
        },
        certificate: Some(Certificate {
            kind: "exact-cover".to_owned(),
            payload: json!({
                "candidate_rectangle_count": candidates.len(),
                "selected_candidate_indices": selected,
                "search_nodes": search.nodes_visited,
                "lower_bound_prunes": search.lower_bound_prunes,
            }),
        }),
    };
    validate_dissection(component, &result)?;
    Ok(result)
}

fn singleton_upper_bound<C>(
    component: &GridComponent<C>,
    cell_index: &HashMap<rect_core::Cell, usize>,
    candidates: &[Candidate],
) -> Vec<usize> {
    let by_rectangle = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| (candidate.rectangle, index))
        .collect::<HashMap<_, _>>();
    component
        .cells
        .iter()
        .map(|cell| {
            let rectangle = GridRect {
                x0: cell.x,
                y0: cell.y,
                x1: cell.x + 1,
                y1: cell.y + 1,
            };
            let _ = cell_index[cell];
            by_rectangle[&rectangle]
        })
        .collect()
}

struct Search<'a> {
    candidates: &'a [Candidate],
    candidates_by_cell: &'a [Vec<usize>],
    cell_count: usize,
    best: Vec<usize>,
    nodes_visited: u64,
    lower_bound_prunes: u64,
}

impl Search<'_> {
    fn recurse(&mut self, covered: &BitSet, chosen: &mut Vec<usize>) {
        self.nodes_visited += 1;
        let covered_count = covered.count_ones();
        if covered_count == self.cell_count {
            if chosen.len() < self.best.len() {
                self.best.clone_from(chosen);
            }
            return;
        }
        if chosen.len() >= self.best.len() {
            return;
        }

        let Some(max_area) = self
            .candidates
            .iter()
            .filter(|candidate| !candidate.cells.intersects(covered))
            .map(|candidate| candidate.area)
            .max()
        else {
            return;
        };
        let uncovered = self.cell_count - covered_count;
        let lower_bound = uncovered.div_ceil(max_area);
        if chosen.len() + lower_bound >= self.best.len() {
            self.lower_bound_prunes += 1;
            return;
        }

        let branch_cell = (0..self.cell_count)
            .filter(|&cell| !covered.contains(cell))
            .min_by_key(|&cell| {
                self.candidates_by_cell[cell]
                    .iter()
                    .filter(|&&candidate| !self.candidates[candidate].cells.intersects(covered))
                    .count()
            })
            .expect("an uncovered cell exists");
        let branches = self.candidates_by_cell[branch_cell].clone();
        for candidate_index in branches {
            let candidate = &self.candidates[candidate_index];
            if candidate.cells.intersects(covered) {
                continue;
            }
            let mut next_covered = covered.clone();
            next_covered.union_with(&candidate.cells);
            chosen.push(candidate_index);
            self.recurse(&next_covered, chosen);
            chosen.pop();
        }
    }
}

#[derive(Debug, Error)]
pub enum ExactCoverError {
    #[error("cannot solve an empty component")]
    EmptyComponent,
    #[error("solver produced an invalid dissection: {0}")]
    InvalidOutput(#[from] ValidationError),
}

#[cfg(test)]
mod tests {
    use rect_core::{ColorGrid, validate_dissection};

    use super::solve;

    fn foreground_component(
        width: usize,
        height: usize,
        cells: Vec<bool>,
    ) -> rect_core::GridComponent<bool> {
        ColorGrid::new(width, height, cells)
            .unwrap()
            .four_connected_components()
            .into_iter()
            .max_by_key(rect_core::GridComponent::cell_count)
            .unwrap()
    }

    #[test]
    fn solves_l_tromino() {
        let component = foreground_component(2, 2, vec![true, true, true, false]);
        let result = solve(&component).unwrap();
        assert_eq!(result.optimum_rectangle_count, 2);
        validate_dissection(&component, &result).unwrap();
    }

    #[test]
    fn solves_ring() {
        let component = foreground_component(
            3,
            3,
            vec![true, true, true, true, false, true, true, true, true],
        );
        let result = solve(&component).unwrap();
        assert_eq!(result.optimum_rectangle_count, 4);
        validate_dissection(&component, &result).unwrap();
    }
}
