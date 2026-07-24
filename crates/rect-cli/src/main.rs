use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use rect_core::{ColorGrid, DissectionResult, GridComponent, SvgOverlay, render_dissection_svg};
use rect_dominance::DominanceMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Parser)]
#[command(
    name = "rect-cli",
    version,
    about = "Exact rectangular-dissection verification"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Solve {
        #[arg(long, value_enum)]
        solver: SolverArg,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
        #[arg(long)]
        svg: Option<PathBuf>,
    },
    Verify {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, default_value_t = false)]
        all_solvers: bool,
        #[arg(long, default_value_t = 40)]
        exact_cover_cell_limit: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Exhaustive {
        #[arg(long)]
        width: usize,
        #[arg(long)]
        height: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Random {
        #[arg(long)]
        width: usize,
        #[arg(long)]
        height: usize,
        #[arg(long)]
        cases: usize,
        #[arg(long)]
        seed: u64,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Polyomino {
        #[arg(long)]
        max_cells: usize,
        #[arg(long, default_value_t = false)]
        all_solvers: bool,
        #[arg(long, default_value_t = 40)]
        oracle_cell_limit: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    CompareExternal {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        external_result: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SolverArg {
    ExactCover,
    SgExplicit,
    DominanceC0,
    DominanceCompressed,
}

#[derive(Clone, Debug, Deserialize)]
struct JsonGrid {
    width: usize,
    height: usize,
    cells: Vec<Value>,
}

#[derive(Serialize)]
struct ComponentSolution {
    component_id: usize,
    color: Value,
    cells: Vec<rect_core::Cell>,
    result: DissectionResult,
}

#[derive(Serialize)]
struct SolveOutput {
    solver: String,
    width: usize,
    height: usize,
    components: Vec<ComponentSolution>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Solve {
            solver,
            input,
            output,
            svg,
        } => solve_command(solver, &input, output.as_deref(), svg.as_deref()),
        Command::Verify {
            input,
            all_solvers: _,
            exact_cover_cell_limit,
            output,
        } => {
            let grid = load_grid(&input)?;
            let report = rect_verify::verify_grid(&grid, exact_cover_cell_limit)
                .map_err(|error| CliError::Verification(error.to_string()))?;
            write_json(&report, output.as_deref())
        }
        Command::Exhaustive {
            width,
            height,
            output,
        } => match rect_verify::exhaustive_binary(width, height) {
            Ok(report) => write_json(&report, output.as_deref()),
            Err(error) => {
                persist_counterexample(&error)?;
                Err(CliError::Verification(error.to_string()))
            }
        },
        Command::Random {
            width,
            height,
            cases,
            seed,
            output,
        } => match rect_verify::random_binary(width, height, cases, seed) {
            Ok(report) => write_json(&report, output.as_deref()),
            Err(error) => {
                persist_counterexample(&error)?;
                Err(CliError::Verification(error.to_string()))
            }
        },
        Command::Polyomino {
            max_cells,
            all_solvers,
            oracle_cell_limit,
            output,
        } => {
            if !all_solvers {
                return Err(CliError::Input(
                    "polyomino verification requires --all-solvers".to_owned(),
                ));
            }
            let summary = rect_verify::polyomino::verify_polyominoes(max_cells, oracle_cell_limit);
            write_json(&summary, output.as_deref())
        }
        Command::CompareExternal {
            input,
            external_result,
            output,
        } => {
            let input_bytes = fs::read(&input)?;
            let input_hash = format!("{:x}", Sha256::digest(&input_bytes));
            let input_grid: JsonGrid = serde_json::from_slice(&input_bytes)?;
            let grid = ColorGrid::new(input_grid.width, input_grid.height, input_grid.cells)
                .map_err(|error| CliError::Input(error.to_string()))?;
            let external_bytes = fs::read(external_result)?;
            let external: rect_verify::external::ExternalOracleResult =
                serde_json::from_slice(&external_bytes)?;
            let report = rect_verify::external::compare_external(&grid, &input_hash, &external)
                .map_err(|error| CliError::Verification(error.to_string()))?;
            if !report.all_agree {
                return Err(CliError::Verification(
                    "external oracle disagrees with at least one Rust solver".to_owned(),
                ));
            }
            write_json(&report, output.as_deref())
        }
    }
}

fn solve_command(
    solver: SolverArg,
    input: &Path,
    output: Option<&Path>,
    svg: Option<&Path>,
) -> Result<(), CliError> {
    let grid = load_grid(input)?;
    let components = grid.four_connected_components();
    let mut solutions = Vec::with_capacity(components.len());
    for component in &components {
        let result = solve_component(component, solver)?;
        solutions.push(ComponentSolution {
            component_id: component.id.0,
            color: component.color.clone(),
            cells: component.cells.clone(),
            result,
        });
    }
    if let Some(svg_path) = svg {
        write_svg_files(svg_path, &components, &solutions, solver)?;
    }
    write_json(
        &SolveOutput {
            solver: format!("{solver:?}"),
            width: grid.width,
            height: grid.height,
            components: solutions,
        },
        output,
    )
}

fn solve_component<C>(
    component: &GridComponent<C>,
    solver: SolverArg,
) -> Result<DissectionResult, CliError> {
    match solver {
        SolverArg::ExactCover => rect_oracle_exact_cover::solve(component)
            .map_err(|error| CliError::Solver(error.to_string())),
        SolverArg::SgExplicit => {
            rect_oracle_sg::solve(component).map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::DominanceC0 => rect_dominance::solve(component, DominanceMode::ExplicitEdges)
            .map_err(|error| CliError::Solver(error.to_string())),
        SolverArg::DominanceCompressed => rect_dominance::solve(component, DominanceMode::Compact)
            .map_err(|error| CliError::Solver(error.to_string())),
    }
}

fn load_grid(path: &Path) -> Result<ColorGrid<Value>, CliError> {
    let bytes = fs::read(path)?;
    let input: JsonGrid = serde_json::from_slice(&bytes)?;
    ColorGrid::new(input.width, input.height, input.cells)
        .map_err(|error| CliError::Input(error.to_string()))
}

fn write_json(value: &impl Serialize, path: Option<&Path>) -> Result<(), CliError> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    if let Some(path) = path {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)?;
    } else {
        io::stdout().write_all(&bytes)?;
    }
    Ok(())
}

fn write_svg_files(
    requested_path: &Path,
    components: &[GridComponent<Value>],
    solutions: &[ComponentSolution],
    solver: SolverArg,
) -> Result<(), CliError> {
    for (index, (component, solution)) in components.iter().zip(solutions).enumerate() {
        let actual_path = if components.len() == 1 {
            requested_path.to_path_buf()
        } else {
            component_svg_path(requested_path, index)
        };
        let svg = if solver == SolverArg::ExactCover {
            render_dissection_svg(component, &solution.result, &SvgOverlay::empty())?
        } else {
            let analysis = rect_oracle_sg::analyze(component)
                .map_err(|error| CliError::Solver(error.to_string()))?;
            let (selected_horizontal, selected_vertical) =
                selected_chords(&solution.result, &analysis)?;
            render_dissection_svg(
                component,
                &solution.result,
                &SvgOverlay {
                    horizontal_chords: &analysis.horizontal_chords,
                    vertical_chords: &analysis.vertical_chords,
                    selected_horizontal: &selected_horizontal,
                    selected_vertical: &selected_vertical,
                },
            )?
        };
        fs::write(actual_path, svg)?;
    }
    Ok(())
}

fn selected_chords(
    result: &DissectionResult,
    analysis: &rect_oracle_sg::SgAnalysis,
) -> Result<(Vec<bool>, Vec<bool>), CliError> {
    let payload = &result
        .certificate
        .as_ref()
        .ok_or_else(|| CliError::Certificate("missing certificate".to_owned()))?
        .payload;
    let horizontal_indices = payload
        .get("selected_horizontal")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::Certificate("missing selected_horizontal".to_owned()))?;
    let vertical_indices = payload
        .get("selected_vertical")
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::Certificate("missing selected_vertical".to_owned()))?;
    let mut horizontal = vec![false; analysis.horizontal_chords.len()];
    let mut vertical = vec![false; analysis.vertical_chords.len()];
    for value in horizontal_indices {
        let index = value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| CliError::Certificate("invalid horizontal chord index".to_owned()))?;
        let selected = horizontal.get_mut(index).ok_or_else(|| {
            CliError::Certificate("horizontal chord index out of bounds".to_owned())
        })?;
        *selected = true;
    }
    for value in vertical_indices {
        let index = value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| CliError::Certificate("invalid vertical chord index".to_owned()))?;
        let selected = vertical.get_mut(index).ok_or_else(|| {
            CliError::Certificate("vertical chord index out of bounds".to_owned())
        })?;
        *selected = true;
    }
    Ok((horizontal, vertical))
}

fn component_svg_path(path: &Path, index: usize) -> PathBuf {
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("dissection");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("svg");
    path.with_file_name(format!("{stem}-component-{index}.{extension}"))
}

fn persist_counterexample(error: &rect_verify::VerificationError) -> Result<(), CliError> {
    let Some(fixture) = error.fixture() else {
        return Ok(());
    };
    let directory = Path::new("test-data/counterexamples");
    fs::create_dir_all(directory)?;
    let bytes = serde_json::to_vec_pretty(fixture)?;
    fs::write(directory.join("first.json"), bytes)?;
    Ok(())
}

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Format(#[from] rect_core::FormatError),
    #[error("invalid input: {0}")]
    Input(String),
    #[error("solver failed: {0}")]
    Solver(String),
    #[error("verification failed: {0}")]
    Verification(String),
    #[error("invalid certificate: {0}")]
    Certificate(String),
}
