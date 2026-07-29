use dominance::experiment::{Mode, Verification};
use mrd_domain::{ColorGrid, DissectionResult, GridComponent};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ComponentVerification {
    pub component_id: usize,
    pub cell_count: usize,
    pub exact_cover: Option<DissectionResult>,
    pub sg_explicit: DissectionResult,
    pub dominance_c0: DissectionResult,
    pub dominance_compact: DissectionResult,
    pub dominance_compact_only: DissectionResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub component_count: usize,
    pub verified_components: Vec<ComponentVerification>,
    pub exact_cover_cell_limit: usize,
}

/// Runs every solver on every four-connected monochromatic component.
///
/// # Errors
///
/// Returns [`VerificationError`] on a solver failure or optimum mismatch.
pub fn verify_grid<C: Clone + Eq>(
    grid: &ColorGrid<C>,
    exact_cover_cell_limit: usize,
) -> Result<VerificationReport, VerificationError> {
    let components = grid.four_connected_components();
    let mut verified_components = Vec::with_capacity(components.len());
    for component in &components {
        verified_components.push(verify_component(component, exact_cover_cell_limit)?);
    }
    Ok(VerificationReport {
        component_count: components.len(),
        verified_components,
        exact_cover_cell_limit,
    })
}

/// Differentially verifies one component, optionally skipping the small oracle.
///
/// # Errors
///
/// Returns [`VerificationError`] on a solver failure or optimum mismatch.
pub fn verify_component<C>(
    component: &GridComponent<C>,
    exact_cover_cell_limit: usize,
) -> Result<ComponentVerification, VerificationError> {
    let exact_cover = (component.cell_count() <= exact_cover_cell_limit)
        .then(|| exact_cover_oracle::solve(component))
        .transpose()
        .map_err(|error| VerificationError::Solver {
            solver: "exact-cover",
            message: error.to_string(),
        })?;
    let sg_explicit =
        sg_oracle::grid::solve(component).map_err(|error| VerificationError::Solver {
            solver: "sg-explicit",
            message: error.to_string(),
        })?;
    let dominance_c0 =
        dominance::experiment::solve(component, Mode::ExplicitEdges).map_err(|error| {
            VerificationError::Solver {
                solver: "dominance-c0",
                message: error.to_string(),
            }
        })?;
    let dominance_compact =
        dominance::experiment::solve(component, Mode::Compact).map_err(|error| {
            VerificationError::Solver {
                solver: "dominance-compact",
                message: error.to_string(),
            }
        })?;
    let dominance_compact_only =
        dominance::experiment::solve_with_verification_mode(component, Verification::CompactOnly)
            .map_err(|error| VerificationError::Solver {
            solver: "dominance-compact-only",
            message: error.to_string(),
        })?;

    let expected = exact_cover
        .as_ref()
        .map_or(sg_explicit.optimum_rectangle_count, |result| {
            result.optimum_rectangle_count
        });
    for (solver, actual) in [
        ("sg-explicit", sg_explicit.optimum_rectangle_count),
        ("dominance-c0", dominance_c0.optimum_rectangle_count),
        (
            "dominance-compact",
            dominance_compact.optimum_rectangle_count,
        ),
        (
            "dominance-compact-only",
            dominance_compact_only.optimum_rectangle_count,
        ),
    ] {
        if actual != expected {
            return Err(VerificationError::OptimumMismatch {
                component: component.id.0,
                expected,
                solver,
                actual,
            });
        }
    }
    Ok(ComponentVerification {
        component_id: component.id.0,
        cell_count: component.cell_count(),
        exact_cover,
        sg_explicit,
        dominance_c0,
        dominance_compact,
        dominance_compact_only,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GridFixture {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<bool>,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExhaustiveReport {
    pub width: usize,
    pub height: usize,
    pub grid_count: u64,
    pub component_count: u64,
    pub exact_cover_comparison_count: u64,
    pub sg_comparison_count: u64,
    pub c0_comparison_count: u64,
    pub compressed_comparison_count: u64,
    pub counterexample_count: u64,
}

/// Enumerates all binary grids of the requested dimensions.
///
/// # Errors
///
/// Returns [`VerificationError`] if the grid exceeds 20 cells or the first
/// differential counterexample is found.
pub fn exhaustive_binary(
    width: usize,
    height: usize,
) -> Result<ExhaustiveReport, VerificationError> {
    let cell_count = width
        .checked_mul(height)
        .ok_or(VerificationError::EnumerationTooLarge)?;
    if cell_count > 20 {
        return Err(VerificationError::EnumerationTooLarge);
    }
    let grid_count = 1_u64
        .checked_shl(u32::try_from(cell_count).map_err(|_| VerificationError::EnumerationTooLarge)?)
        .ok_or(VerificationError::EnumerationTooLarge)?;
    let mut component_count = 0_u64;
    for mask in 0..grid_count {
        let cells = (0..cell_count)
            .map(|index| mask & (1_u64 << index) != 0)
            .collect::<Vec<_>>();
        let grid = ColorGrid::new(width, height, cells.clone()).map_err(|error| {
            VerificationError::Fixture {
                fixture: GridFixture {
                    width,
                    height,
                    cells: cells.clone(),
                    reason: error.to_string(),
                },
            }
        })?;
        let components = grid.four_connected_components();
        component_count += components.len() as u64;
        for component in components {
            if let Err(error) = verify_component(&component, 40) {
                return Err(VerificationError::Fixture {
                    fixture: GridFixture {
                        width,
                        height,
                        cells,
                        reason: error.to_string(),
                    },
                });
            }
        }
    }
    Ok(ExhaustiveReport {
        width,
        height,
        grid_count,
        component_count,
        exact_cover_comparison_count: component_count,
        sg_comparison_count: component_count,
        c0_comparison_count: component_count,
        compressed_comparison_count: component_count,
        counterexample_count: 0,
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RandomReport {
    pub width: usize,
    pub height: usize,
    pub cases: usize,
    pub seed: u64,
    pub component_count: usize,
    pub exact_cover_comparison_count: usize,
    pub sg_comparison_count: usize,
    pub c0_comparison_count: usize,
    pub compressed_comparison_count: usize,
    pub counterexample_count: usize,
}

/// Verifies deterministic cases from six shape families.
///
/// # Errors
///
/// Returns [`VerificationError`] on dimension overflow or the first
/// differential counterexample.
pub fn random_binary(
    width: usize,
    height: usize,
    cases: usize,
    seed: u64,
) -> Result<RandomReport, VerificationError> {
    let cell_count = width
        .checked_mul(height)
        .ok_or(VerificationError::EnumerationTooLarge)?;
    let mut random = SplitMix64::new(seed);
    let mut component_count = 0;
    let mut exact_cover_comparison_count = 0;
    for case_index in 0..cases {
        let family = case_index % 6;
        let cells = random_case(width, height, cell_count, family, &mut random);
        let grid = ColorGrid::new(width, height, cells.clone()).map_err(|error| {
            VerificationError::Fixture {
                fixture: GridFixture {
                    width,
                    height,
                    cells: cells.clone(),
                    reason: error.to_string(),
                },
            }
        })?;
        let components = grid.four_connected_components();
        component_count += components.len();
        for component in components {
            let limit = if component.cell_count() <= 40 { 40 } else { 0 };
            exact_cover_comparison_count += usize::from(limit != 0);
            if let Err(error) = verify_component(&component, limit) {
                return Err(VerificationError::Fixture {
                    fixture: GridFixture {
                        width,
                        height,
                        cells,
                        reason: format!("random case {case_index}, family {family}: {error}"),
                    },
                });
            }
        }
    }
    Ok(RandomReport {
        width,
        height,
        cases,
        seed,
        component_count,
        exact_cover_comparison_count,
        sg_comparison_count: component_count,
        c0_comparison_count: component_count,
        compressed_comparison_count: component_count,
        counterexample_count: 0,
    })
}

fn random_case(
    width: usize,
    height: usize,
    cell_count: usize,
    family: usize,
    random: &mut SplitMix64,
) -> Vec<bool> {
    match family {
        0 => (0..cell_count).map(|_| random.next() & 1 == 1).collect(),
        1 => random_walk(width, height, random),
        2 => rectangle_union(width, height, random),
        3 => (0..cell_count)
            .map(|index| {
                let x = index % width;
                x.is_multiple_of(2) || index / width == height / 2
            })
            .collect(),
        4 => (0..cell_count)
            .map(|index| (index % width + index / width).is_multiple_of(2))
            .collect(),
        _ => ring_with_corridor(width, height),
    }
}

fn random_walk(width: usize, height: usize, random: &mut SplitMix64) -> Vec<bool> {
    let mut cells = vec![false; width * height];
    if cells.is_empty() {
        return cells;
    }
    let mut x = random_index(random, width);
    let mut y = random_index(random, height);
    for _ in 0..(width * height * 2).max(1) {
        cells[y * width + x] = true;
        match random.next() % 4 {
            0 if x > 0 => x -= 1,
            1 if x + 1 < width => x += 1,
            2 if y > 0 => y -= 1,
            3 if y + 1 < height => y += 1,
            _ => {}
        }
    }
    cells
}

fn rectangle_union(width: usize, height: usize, random: &mut SplitMix64) -> Vec<bool> {
    let mut cells = vec![false; width * height];
    if width == 0 || height == 0 {
        return cells;
    }
    for _ in 0..4 {
        let xa = random_index(random, width);
        let xb = random_index(random, width);
        let ya = random_index(random, height);
        let yb = random_index(random, height);
        let (x0, x1) = (xa.min(xb), xa.max(xb) + 1);
        let (y0, y1) = (ya.min(yb), ya.max(yb) + 1);
        for y in y0..y1 {
            for x in x0..x1 {
                cells[y * width + x] = true;
            }
        }
    }
    cells
}

fn ring_with_corridor(width: usize, height: usize) -> Vec<bool> {
    (0..width * height)
        .map(|index| {
            let x = index % width;
            let y = index / width;
            x == 0 || y == 0 || x + 1 == width || y + 1 == height || x == width / 2
        })
        .collect()
}

fn random_index(random: &mut SplitMix64, upper: usize) -> usize {
    let upper = u64::try_from(upper).expect("usize fits u64 on supported Rust targets");
    usize::try_from(random.next() % upper).expect("sample is strictly less than usize upper bound")
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
}

#[derive(Debug, Error)]
pub enum VerificationError {
    #[error("{solver} solver failed: {message}")]
    Solver {
        solver: &'static str,
        message: String,
    },
    #[error("component {component}: expected optimum {expected}, {solver} returned {actual}")]
    OptimumMismatch {
        component: usize,
        expected: usize,
        solver: &'static str,
        actual: usize,
    },
    #[error("exhaustive enumeration is limited to at most 20 cells")]
    EnumerationTooLarge,
    #[error("verification counterexample: {fixture:?}")]
    Fixture { fixture: GridFixture },
}

impl VerificationError {
    #[must_use]
    pub const fn fixture(&self) -> Option<&GridFixture> {
        match self {
            Self::Fixture { fixture } => Some(fixture),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use mrd_domain::{ColorGrid, GridComponent};

    use super::{exhaustive_binary, verify_component, verify_grid};

    #[test]
    fn all_solvers_agree_exhaustively_on_three_by_three() {
        let report = exhaustive_binary(3, 3).unwrap();
        assert_eq!(report.grid_count, 512);
    }

    #[test]
    fn all_solvers_agree_on_hole() {
        let grid = ColorGrid::new(
            3,
            3,
            vec![true, true, true, true, false, true, true, true, true],
        )
        .unwrap();
        verify_grid(&grid, 40).unwrap();
    }

    #[test]
    fn optimum_is_invariant_under_grid_isometries_translation_and_scaling() {
        let width = 4;
        let height = 3;
        let cells = vec![
            true, true, true, false, true, false, true, true, true, true, true, false,
        ];
        let base = foreground_optimum(width, height, cells.clone());

        let reflected_x = transform(width, height, &cells, width, height, |x, y| {
            (width - 1 - x, y)
        });
        assert_eq!(foreground_optimum(width, height, reflected_x), base);
        let reflected_y = transform(width, height, &cells, width, height, |x, y| {
            (x, height - 1 - y)
        });
        assert_eq!(foreground_optimum(width, height, reflected_y), base);

        let rotated_90 = transform(width, height, &cells, height, width, |x, y| {
            (height - 1 - y, x)
        });
        assert_eq!(foreground_optimum(height, width, rotated_90.clone()), base);
        let rotated_180 = transform(height, width, &rotated_90, width, height, |x, y| {
            (width - 1 - y, x)
        });
        assert_eq!(foreground_optimum(width, height, rotated_180), base);
        let rotated_270 = transform(height, width, &rotated_90, height, width, |x, y| {
            (height - 1 - x, width - 1 - y)
        });
        assert_eq!(foreground_optimum(height, width, rotated_270), base);

        let translated = transform(width, height, &cells, width + 3, height + 2, |x, y| {
            (x + 2, y + 1)
        });
        assert_eq!(foreground_optimum(width + 3, height + 2, translated), base);

        let scaled_width = width * 2;
        let scaled_height = height * 2;
        let mut scaled = vec![false; scaled_width * scaled_height];
        for y in 0..height {
            for x in 0..width {
                for dy in 0..2 {
                    for dx in 0..2 {
                        scaled[(y * 2 + dy) * scaled_width + x * 2 + dx] = cells[y * width + x];
                    }
                }
            }
        }
        assert_eq!(
            foreground_optimum(scaled_width, scaled_height, scaled),
            base
        );
    }

    fn transform(
        source_width: usize,
        source_height: usize,
        source: &[bool],
        target_width: usize,
        target_height: usize,
        map: impl Fn(usize, usize) -> (usize, usize),
    ) -> Vec<bool> {
        let mut target = vec![false; target_width * target_height];
        for y in 0..source_height {
            for x in 0..source_width {
                if source[y * source_width + x] {
                    let (target_x, target_y) = map(x, y);
                    target[target_y * target_width + target_x] = true;
                }
            }
        }
        target
    }

    fn foreground_optimum(width: usize, height: usize, cells: Vec<bool>) -> usize {
        let grid = ColorGrid::new(width, height, cells).unwrap();
        let component = grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
            .max_by_key(GridComponent::cell_count)
            .unwrap();
        verify_component(&component, 40)
            .unwrap()
            .sg_explicit
            .optimum_rectangle_count
    }
}
