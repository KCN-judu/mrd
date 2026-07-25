use std::collections::{BTreeMap, BTreeSet};

use rect_core::GridComponent;
use rect_dominance::{DominanceMode, VerificationMode};
use serde::{Deserialize, Serialize};

use crate::adversarial::{AdversarialInstance, one_hole_ring};

type SignedCell = (i32, i32);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Polyomino {
    cells: Vec<SignedCell>,
}

impl Polyomino {
    #[must_use]
    pub fn singleton() -> Self {
        Self {
            cells: vec![(0, 0)],
        }
    }

    #[must_use]
    pub fn canonical(cells: impl IntoIterator<Item = SignedCell>) -> Self {
        let cells = cells.into_iter().collect::<Vec<_>>();
        let variants = (0..8).map(|symmetry| {
            let transformed = cells
                .iter()
                .copied()
                .map(|(x, y)| apply_symmetry(x, y, symmetry))
                .collect::<Vec<_>>();
            normalize(transformed)
        });
        Self {
            cells: variants.min().unwrap_or_default(),
        }
    }

    #[must_use]
    pub const fn cell_count(&self) -> usize {
        self.cells.len()
    }

    #[must_use]
    pub fn canonical_key(&self) -> String {
        self.cells
            .iter()
            .map(|(x, y)| format!("{x}:{y}"))
            .collect::<Vec<_>>()
            .join(";")
    }

    /// # Panics
    ///
    /// Panics only if the internal canonical representation contains a negative
    /// coordinate, which would violate [`Polyomino::canonical`].
    #[must_use]
    pub fn to_instance(&self, name: String, source: &str) -> AdversarialInstance {
        let width = self
            .cells
            .iter()
            .map(|(x, _)| usize::try_from(*x).expect("canonical x is nonnegative") + 1)
            .max()
            .unwrap_or(0);
        let height = self
            .cells
            .iter()
            .map(|(_, y)| usize::try_from(*y).expect("canonical y is nonnegative") + 1)
            .max()
            .unwrap_or(0);
        let mut cells = vec![false; width * height];
        for &(x, y) in &self.cells {
            let x = usize::try_from(x).expect("canonical x is nonnegative");
            let y = usize::try_from(y).expect("canonical y is nonnegative");
            cells[y * width + x] = true;
        }
        AdversarialInstance {
            name,
            family: source.to_owned(),
            width,
            height,
            cells,
            parameters: [("cell_count".to_owned(), self.cell_count())]
                .into_iter()
                .collect(),
        }
    }

    fn children(&self) -> BTreeSet<Self> {
        let occupied = self.cells.iter().copied().collect::<BTreeSet<_>>();
        let mut children = BTreeSet::new();
        for &(x, y) in &self.cells {
            for candidate in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if occupied.contains(&candidate) {
                    continue;
                }
                let mut cells = self.cells.clone();
                cells.push(candidate);
                children.insert(Self::canonical(cells));
            }
        }
        children
    }
}

#[must_use]
pub fn enumerate_free_polyominoes(max_cells: usize) -> Vec<Vec<Polyomino>> {
    if max_cells == 0 {
        return Vec::new();
    }
    let mut levels = vec![vec![Polyomino::singleton()]];
    for _ in 2..=max_cells {
        let mut next = BTreeSet::new();
        let Some(previous) = levels.last() else {
            break;
        };
        for polyomino in previous {
            next.extend(polyomino.children());
        }
        levels.push(next.into_iter().collect());
    }
    levels
}

#[must_use]
pub fn explicit_hole_polyominoes(max_cells: usize) -> Vec<AdversarialInstance> {
    [(3, 3), (4, 4), (5, 5)]
        .into_iter()
        .map(|(width, height)| one_hole_ring(width, height))
        .filter(|instance| instance.cells.iter().filter(|&&cell| cell).count() <= max_cells)
        .enumerate()
        .map(|(index, mut instance)| {
            instance.name = format!("explicit-hole-polyomino-{}", index + 1);
            "explicit-hole-polyomino".clone_into(&mut instance.family);
            instance
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolyominoStatus {
    Verified,
    Unsupported,
    OracleTimeout,
    SolverError,
    Counterexample,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolyominoRecord {
    pub source: String,
    pub canonical_key: String,
    pub cell_count: usize,
    pub width: usize,
    pub height: usize,
    pub hole_count: Option<usize>,
    pub status: PolyominoStatus,
    pub solver_optima: BTreeMap<String, usize>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PolyominoSummary {
    pub max_cells: usize,
    pub free_count_by_size: BTreeMap<usize, usize>,
    pub explicit_hole_count: usize,
    pub status_counts: BTreeMap<String, usize>,
    pub records: Vec<PolyominoRecord>,
}

/// Enumerates and differentially verifies every canonical free shape and the
/// separate ordinary-hole fixtures within the configured cell bound.
///
/// Instances above `oracle_cell_limit` are still run through every graph solver
/// and are explicitly recorded as `oracle-timeout` rather than silently skipped.
#[must_use]
pub fn verify_polyominoes(max_cells: usize, oracle_cell_limit: usize) -> PolyominoSummary {
    let levels = enumerate_free_polyominoes(max_cells);
    let free_count_by_size = levels
        .iter()
        .enumerate()
        .map(|(index, level)| (index + 1, level.len()))
        .collect();
    let hole_instances = explicit_hole_polyominoes(max_cells);
    let explicit_hole_count = hole_instances.len();
    let free_instances = levels.into_iter().enumerate().flat_map(|(level, shapes)| {
        shapes.into_iter().enumerate().map(move |(index, shape)| {
            let name = format!("free-polyomino-{}-{}", level + 1, index + 1);
            let key = shape.canonical_key();
            (shape.to_instance(name, "free-polyomino"), key)
        })
    });
    let hole_instances = hole_instances.into_iter().map(|instance| {
        let key = instance_key(&instance);
        (instance, key)
    });
    let records = free_instances
        .chain(hole_instances)
        .map(|(instance, key)| verify_instance(&instance, key, oracle_cell_limit))
        .collect::<Vec<_>>();
    let mut status_counts = BTreeMap::new();
    for record in &records {
        *status_counts
            .entry(status_name(record.status).to_owned())
            .or_default() += 1;
    }
    PolyominoSummary {
        max_cells,
        free_count_by_size,
        explicit_hole_count,
        status_counts,
        records,
    }
}

fn verify_instance(
    instance: &AdversarialInstance,
    canonical_key: String,
    oracle_cell_limit: usize,
) -> PolyominoRecord {
    let foreground = match instance.foreground_components() {
        Ok(components) if components.len() == 1 => components.into_iter().next().unwrap(),
        Ok(components) => {
            return record_error(
                instance,
                canonical_key,
                PolyominoStatus::Unsupported,
                format!(
                    "expected one foreground component, found {}",
                    components.len()
                ),
            );
        }
        Err(error) => {
            return record_error(
                instance,
                canonical_key,
                PolyominoStatus::SolverError,
                error.to_string(),
            );
        }
    };
    solve_component(instance, canonical_key, &foreground, oracle_cell_limit)
}

fn solve_component(
    instance: &AdversarialInstance,
    canonical_key: String,
    component: &GridComponent<bool>,
    oracle_cell_limit: usize,
) -> PolyominoRecord {
    let mut solver_optima = BTreeMap::new();
    let mut oracle_was_skipped = false;
    if component.cell_count() <= oracle_cell_limit {
        match rect_oracle_exact_cover::solve(component) {
            Ok(result) => {
                solver_optima.insert("exact-cover".to_owned(), result.optimum_rectangle_count);
            }
            Err(error) => {
                return record_error(
                    instance,
                    canonical_key,
                    PolyominoStatus::SolverError,
                    format!("exact-cover: {error}"),
                );
            }
        }
    } else {
        oracle_was_skipped = true;
    }
    for (name, result) in [
        (
            "sg-explicit",
            rect_oracle_sg::solve(component).map_err(|error| error.to_string()),
        ),
        (
            "dominance-c0",
            rect_dominance::solve(component, DominanceMode::ExplicitEdges)
                .map_err(|error| error.to_string()),
        ),
        (
            "dominance-compressed",
            rect_dominance::solve(component, DominanceMode::Compact)
                .map_err(|error| error.to_string()),
        ),
        (
            "dominance-compact-only",
            rect_dominance::solve_with_verification_mode(component, VerificationMode::CompactOnly)
                .map_err(|error| error.to_string()),
        ),
    ] {
        match result {
            Ok(result) => {
                solver_optima.insert(name.to_owned(), result.optimum_rectangle_count);
            }
            Err(error) => {
                return record_error(
                    instance,
                    canonical_key,
                    PolyominoStatus::SolverError,
                    format!("{name}: {error}"),
                );
            }
        }
    }
    let distinct = solver_optima.values().copied().collect::<BTreeSet<_>>();
    let status = if distinct.len() != 1 {
        PolyominoStatus::Counterexample
    } else if oracle_was_skipped {
        PolyominoStatus::OracleTimeout
    } else {
        PolyominoStatus::Verified
    };
    let hole_count = rect_oracle_sg::analyze(component)
        .ok()
        .map(|analysis| analysis.boundary.hole_count());
    PolyominoRecord {
        source: instance.family.clone(),
        canonical_key,
        cell_count: component.cell_count(),
        width: instance.width,
        height: instance.height,
        hole_count,
        status,
        solver_optima,
        message: (status == PolyominoStatus::Counterexample)
            .then_some("solver optimum values differ".to_owned()),
    }
}

fn record_error(
    instance: &AdversarialInstance,
    canonical_key: String,
    status: PolyominoStatus,
    message: String,
) -> PolyominoRecord {
    PolyominoRecord {
        source: instance.family.clone(),
        canonical_key,
        cell_count: instance.cells.iter().filter(|&&cell| cell).count(),
        width: instance.width,
        height: instance.height,
        hole_count: None,
        status,
        solver_optima: BTreeMap::new(),
        message: Some(message),
    }
}

fn instance_key(instance: &AdversarialInstance) -> String {
    instance
        .cells
        .iter()
        .enumerate()
        .filter_map(|(index, &cell)| {
            cell.then_some(format!(
                "{}:{}",
                index % instance.width,
                index / instance.width
            ))
        })
        .collect::<Vec<_>>()
        .join(";")
}

const fn status_name(status: PolyominoStatus) -> &'static str {
    match status {
        PolyominoStatus::Verified => "verified",
        PolyominoStatus::Unsupported => "unsupported",
        PolyominoStatus::OracleTimeout => "oracle-timeout",
        PolyominoStatus::SolverError => "solver-error",
        PolyominoStatus::Counterexample => "counterexample",
    }
}

fn apply_symmetry(x: i32, y: i32, symmetry: usize) -> SignedCell {
    match symmetry {
        0 => (x, y),
        1 => (-x, y),
        2 => (x, -y),
        3 => (-x, -y),
        4 => (y, x),
        5 => (-y, x),
        6 => (y, -x),
        7 => (-y, -x),
        _ => unreachable!("D4 has exactly eight elements"),
    }
}

fn normalize(mut cells: Vec<SignedCell>) -> Vec<SignedCell> {
    let min_x = cells.iter().map(|(x, _)| *x).min().unwrap_or(0);
    let min_y = cells.iter().map(|(_, y)| *y).min().unwrap_or(0);
    for (x, y) in &mut cells {
        *x -= min_x;
        *y -= min_y;
    }
    cells.sort_unstable();
    cells.dedup();
    cells
}

#[cfg(test)]
mod tests {
    use super::{
        Polyomino, PolyominoStatus, enumerate_free_polyominoes, explicit_hole_polyominoes,
        verify_polyominoes,
    };

    #[test]
    fn free_counts_match_the_known_sequence_through_eight_cells() {
        let counts = enumerate_free_polyominoes(8)
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>();
        assert_eq!(counts, vec![1, 1, 2, 5, 12, 35, 108, 369]);
    }

    #[test]
    fn canonical_key_identifies_all_dihedral_images() {
        let shape = [(0, 0), (1, 0), (2, 0), (0, 1)];
        let reflected = [(0, 0), (-1, 0), (-2, 0), (0, 1)];
        let rotated = [(0, 0), (0, 1), (0, 2), (-1, 0)];
        assert_eq!(Polyomino::canonical(shape), Polyomino::canonical(reflected));
        assert_eq!(Polyomino::canonical(shape), Polyomino::canonical(rotated));
    }

    #[test]
    fn hole_population_is_explicit_and_verified() {
        let holes = explicit_hole_polyominoes(12);
        assert_eq!(holes.len(), 2);
        let summary = verify_polyominoes(8, 20);
        assert_eq!(summary.explicit_hole_count, 1);
        assert!(
            summary
                .records
                .iter()
                .all(|record| record.status == PolyominoStatus::Verified)
        );
    }
}
