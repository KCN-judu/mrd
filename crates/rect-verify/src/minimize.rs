use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rect_core::{ColorGrid, Diagnostics, DissectionResult, SvgOverlay, render_dissection_svg};
use rect_dominance::{
    DominanceMode, VerificationMode, biclique::BicliquePartition, embedding::DominanceEmbedding,
};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{GridFixture, verify_grid};

#[must_use]
pub fn minimize_counterexample(fixture: &GridFixture) -> GridFixture {
    if !fixture_fails(fixture) {
        return fixture.clone();
    }
    let mut current = fixture.clone();
    loop {
        let mut reduced = false;
        for row in 0..current.height {
            let candidate = remove_row(&current, row);
            if fixture_fails(&candidate) {
                current = candidate;
                reduced = true;
                break;
            }
        }
        if reduced {
            continue;
        }
        for column in 0..current.width {
            let candidate = remove_column(&current, column);
            if fixture_fails(&candidate) {
                current = candidate;
                reduced = true;
                break;
            }
        }
        if reduced {
            continue;
        }
        for index in 0..current.cells.len() {
            if !current.cells[index] {
                continue;
            }
            let mut candidate = current.clone();
            candidate.cells[index] = false;
            if fixture_fails(&candidate) {
                current = candidate;
                reduced = true;
                break;
            }
        }
        if !reduced {
            break;
        }
    }
    canonicalize_failing(current)
}

/// Writes a minimized input, all Rust solver outputs, and replay explanation.
///
/// # Errors
///
/// Returns [`MinimizeError`] for malformed fixture data, serialization, or I/O failures.
pub fn write_regression_bundle(
    root: &Path,
    fixture: &GridFixture,
) -> Result<PathBuf, MinimizeError> {
    let minimized = minimize_counterexample(fixture);
    let identifier = format!(
        "{}x{}-{:016x}",
        minimized.width,
        minimized.height,
        stable_fixture_hash(&minimized)
    );
    let directory = root.join(identifier);
    fs::create_dir_all(&directory)?;
    fs::write(
        directory.join("input.json"),
        serde_json::to_vec_pretty(&minimized)?,
    )?;
    let outputs = collect_solver_outputs(&minimized)?;
    fs::write(
        directory.join("solver-outputs.json"),
        serde_json::to_vec_pretty(&outputs)?,
    )?;
    fs::write(
        directory.join("biclique-audit.json"),
        serde_json::to_vec_pretty(&collect_biclique_audits(&minimized)?)?,
    )?;
    write_input_svgs(&directory, &minimized)?;
    let explanation = format!(
        "# Minimized differential regression\n\nExpected behavior: every supported solver must return the same optimum and a cell-exact valid dissection.\n\nOriginal failure: {}\n\nThe workspace test suite replays this input automatically.\n",
        minimized.reason
    );
    fs::write(directory.join("README.md"), explanation)?;
    Ok(directory)
}

fn collect_biclique_audits(fixture: &GridFixture) -> Result<Value, MinimizeError> {
    let grid = ColorGrid::new(fixture.width, fixture.height, fixture.cells.clone())
        .map_err(|error| MinimizeError::InvalidGrid(error.to_string()))?;
    let mut audits = BTreeMap::new();
    for component in grid.four_connected_components() {
        let value = (|| {
            let analysis =
                rect_oracle_sg::analyze(&component).map_err(|error| error.to_string())?;
            let embedding =
                DominanceEmbedding::new(&analysis.horizontal_chords, &analysis.vertical_chords)
                    .map_err(|error| error.to_string())?;
            let graph = embedding
                .explicit_graph()
                .map_err(|error| error.to_string())?;
            let partition = BicliquePartition::comparability_theorem_8_audited(&embedding)
                .map_err(|error| error.to_string())?
                .partition;
            Ok::<Value, String>(json!(partition.audit(&graph, 256)))
        })()
        .unwrap_or_else(|error| json!({"error": error}));
        audits.insert(component.id.0.to_string(), value);
    }
    Ok(json!(audits))
}

fn write_input_svgs(directory: &Path, fixture: &GridFixture) -> Result<(), MinimizeError> {
    let grid = ColorGrid::new(fixture.width, fixture.height, fixture.cells.clone())
        .map_err(|error| MinimizeError::InvalidGrid(error.to_string()))?;
    for component in grid.four_connected_components() {
        let input_only = DissectionResult {
            optimum_rectangle_count: 0,
            rectangles: Vec::new(),
            diagnostics: Diagnostics::default(),
            certificate: None,
        };
        let svg = render_dissection_svg(&component, &input_only, &SvgOverlay::empty())
            .map_err(|error| MinimizeError::Svg(error.to_string()))?;
        fs::write(
            directory.join(format!("component-{}.svg", component.id.0)),
            svg,
        )?;
    }
    Ok(())
}

fn collect_solver_outputs(fixture: &GridFixture) -> Result<Value, MinimizeError> {
    let grid = ColorGrid::new(fixture.width, fixture.height, fixture.cells.clone())
        .map_err(|error| MinimizeError::InvalidGrid(error.to_string()))?;
    let mut components = BTreeMap::new();
    for component in grid.four_connected_components() {
        let mut solvers = BTreeMap::new();
        solvers.insert(
            "exact-cover",
            result_value(rect_oracle_exact_cover::solve(&component)),
        );
        solvers.insert(
            "sg-explicit",
            result_value(rect_oracle_sg::solve(&component)),
        );
        solvers.insert(
            "dominance-c0",
            result_value(rect_dominance::solve(
                &component,
                DominanceMode::ExplicitEdges,
            )),
        );
        solvers.insert(
            "dominance-compressed",
            result_value(rect_dominance::solve(&component, DominanceMode::Compact)),
        );
        solvers.insert(
            "dominance-compact-only",
            result_value(rect_dominance::solve_with_verification_mode(
                &component,
                VerificationMode::CompactOnly,
            )),
        );
        components.insert(component.id.0.to_string(), json!(solvers));
    }
    Ok(json!({
        "reason": fixture.reason,
        "components": components,
    }))
}

fn result_value<T, E>(result: Result<T, E>) -> Value
where
    T: serde::Serialize,
    E: std::fmt::Display,
{
    match result {
        Ok(value) => serde_json::to_value(value)
            .unwrap_or_else(|error| json!({"serialization_error": error.to_string()})),
        Err(error) => json!({"error": error.to_string()}),
    }
}

fn fixture_fails(fixture: &GridFixture) -> bool {
    ColorGrid::new(fixture.width, fixture.height, fixture.cells.clone())
        .ok()
        .is_some_and(|grid| verify_grid(&grid, 40).is_err())
}

fn remove_row(fixture: &GridFixture, row: usize) -> GridFixture {
    let cells = fixture
        .cells
        .chunks(fixture.width)
        .enumerate()
        .filter(|(index, _)| *index != row)
        .flat_map(|(_, row_cells)| row_cells.iter().copied())
        .collect();
    GridFixture {
        width: fixture.width,
        height: fixture.height - 1,
        cells,
        reason: fixture.reason.clone(),
    }
}

fn remove_column(fixture: &GridFixture, column: usize) -> GridFixture {
    let cells = fixture
        .cells
        .iter()
        .enumerate()
        .filter_map(|(index, &cell)| (index % fixture.width != column).then_some(cell))
        .collect();
    GridFixture {
        width: fixture.width - 1,
        height: fixture.height,
        cells,
        reason: fixture.reason.clone(),
    }
}

fn canonicalize_failing(fixture: GridFixture) -> GridFixture {
    let variants = dihedral_variants(&fixture);
    variants
        .into_iter()
        .filter(fixture_fails)
        .min_by_key(|candidate| {
            (
                candidate.cells.len(),
                candidate.width,
                candidate.height,
                candidate.cells.clone(),
            )
        })
        .unwrap_or(fixture)
}

fn dihedral_variants(fixture: &GridFixture) -> Vec<GridFixture> {
    (0..8)
        .map(|symmetry| {
            let swaps_dimensions = symmetry >= 4;
            let (width, height) = if swaps_dimensions {
                (fixture.height, fixture.width)
            } else {
                (fixture.width, fixture.height)
            };
            let mut cells = vec![false; width * height];
            for y in 0..fixture.height {
                for x in 0..fixture.width {
                    let (target_x, target_y) = match symmetry {
                        0 => (x, y),
                        1 => (fixture.width - 1 - x, y),
                        2 => (x, fixture.height - 1 - y),
                        3 => (fixture.width - 1 - x, fixture.height - 1 - y),
                        4 => (y, x),
                        5 => (fixture.height - 1 - y, x),
                        6 => (y, fixture.width - 1 - x),
                        7 => (fixture.height - 1 - y, fixture.width - 1 - x),
                        _ => unreachable!(),
                    };
                    cells[target_y * width + target_x] = fixture.cells[y * fixture.width + x];
                }
            }
            GridFixture {
                width,
                height,
                cells,
                reason: fixture.reason.clone(),
            }
        })
        .collect()
}

fn stable_fixture_hash(fixture: &GridFixture) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for value in [fixture.width as u64, fixture.height as u64]
        .into_iter()
        .chain(fixture.cells.iter().map(|&cell| u64::from(cell)))
    {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Debug, Error)]
pub enum MinimizeError {
    #[error("invalid regression grid: {0}")]
    InvalidGrid(String),
    #[error("cannot render regression SVG: {0}")]
    Svg(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use rect_core::ColorGrid;

    use crate::{GridFixture, verify_grid};

    use super::dihedral_variants;

    #[test]
    fn dihedral_canonicalization_generates_eight_valid_views() {
        let fixture = GridFixture {
            width: 3,
            height: 2,
            cells: vec![true, true, false, true, false, false],
            reason: "test".to_owned(),
        };
        let variants = dihedral_variants(&fixture);
        assert_eq!(variants.len(), 8);
        assert!(variants.iter().all(|variant| {
            variant.cells.len() == variant.width * variant.height
                && variant.cells.iter().filter(|&&cell| cell).count() == 3
        }));
    }

    #[test]
    fn every_stored_regression_is_replayed() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../test-data/regressions");
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten().filter(|entry| entry.path().is_dir()) {
            let input = entry.path().join("input.json");
            if !input.exists() {
                continue;
            }
            let fixture: GridFixture = serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
            let grid = ColorGrid::new(fixture.width, fixture.height, fixture.cells).unwrap();
            verify_grid(&grid, 40)
                .unwrap_or_else(|error| panic!("regression {} failed: {error}", input.display()));
        }
    }
}
