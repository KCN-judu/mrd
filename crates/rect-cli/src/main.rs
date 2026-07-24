use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

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
    about = "Exact rectangular-dissection verification for finite colored grids",
    long_about = "Exact rectangular-dissection verification for finite colored grids built from unit cells.\n\nSupported inputs are ordinary nondegenerate finite grid regions. Ornaments, isolated formal-boundary points, line-segment holes, point holes, arbitrary degenerate formal holes, and general polygon inputs are outside the supported model."
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
        #[arg(long, default_value_t = 40)]
        exact_cover_cell_limit: usize,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Benchmark {
        #[arg(long, value_enum)]
        suite: BenchmarkSuiteArg,
        #[arg(long, default_value_t = 12)]
        max_cells: usize,
        #[arg(long, default_value_t = 40)]
        oracle_cell_limit: usize,
        #[arg(long, default_value = "4,8,16,32,64,128")]
        sizes: String,
        #[arg(long)]
        output: PathBuf,
    },
    Generate {
        #[arg(long, value_enum)]
        family: GenerateFamilyArg,
        #[arg(long)]
        horizontal: usize,
        #[arg(long)]
        vertical: usize,
        #[arg(long)]
        json: PathBuf,
        #[arg(long)]
        svg: PathBuf,
    },
    ExportAdversarial {
        #[arg(long)]
        output_dir: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SolverArg {
    ExactCover,
    SgExplicit,
    DominanceC0,
    DominanceCompressed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BenchmarkSuiteArg {
    Adversarial,
    DenseConflict,
    Polyomino,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GenerateFamilyArg {
    DenseConflict,
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
        } => exhaustive_command(width, height, output.as_deref()),
        Command::Random {
            width,
            height,
            cases,
            seed,
            output,
        } => random_command(width, height, cases, seed, output.as_deref()),
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
            write_experiment_json(
                &summary,
                None,
                summary.records.len(),
                summary.records.len(),
                output.as_deref(),
            )
        }
        Command::CompareExternal {
            input,
            external_result,
            exact_cover_cell_limit,
            output,
        } => compare_external_command(
            &input,
            &external_result,
            exact_cover_cell_limit,
            output.as_deref(),
        ),
        Command::Benchmark {
            suite,
            max_cells,
            oracle_cell_limit,
            sizes,
            output,
        } => {
            let sizes = parse_sizes(&sizes)?;
            benchmark_command(suite, max_cells, oracle_cell_limit, &sizes, &output)
        }
        Command::Generate {
            family,
            horizontal,
            vertical,
            json,
            svg,
        } => generate_command(family, horizontal, vertical, &json, &svg),
        Command::ExportAdversarial { output_dir } => export_adversarial(&output_dir),
    }
}

fn exhaustive_command(width: usize, height: usize, output: Option<&Path>) -> Result<(), CliError> {
    match rect_verify::exhaustive_binary(width, height) {
        Ok(report) => {
            let input_count = usize::try_from(report.grid_count)
                .map_err(|_| CliError::Output("grid count exceeds usize".to_owned()))?;
            let component_count = usize::try_from(report.component_count)
                .map_err(|_| CliError::Output("component count exceeds usize".to_owned()))?;
            write_experiment_json(&report, None, input_count, component_count, output)
        }
        Err(error) => {
            persist_counterexample(&error)?;
            Err(CliError::Verification(error.to_string()))
        }
    }
}

fn random_command(
    width: usize,
    height: usize,
    cases: usize,
    seed: u64,
    output: Option<&Path>,
) -> Result<(), CliError> {
    match rect_verify::random_binary(width, height, cases, seed) {
        Ok(report) => {
            write_experiment_json(&report, Some(seed), cases, report.component_count, output)
        }
        Err(error) => {
            persist_counterexample(&error)?;
            Err(CliError::Verification(error.to_string()))
        }
    }
}

fn export_adversarial(output_dir: &Path) -> Result<(), CliError> {
    fs::create_dir_all(output_dir)?;
    let instances = rect_verify::adversarial::endpoint_contact_instances()
        .into_iter()
        .chain(rect_verify::adversarial::topological_stress_instances())
        .chain(rect_verify::adversarial::external_oracle_adversarial_instances())
        .chain([
            rect_verify::adversarial::dense_conflict_grid(4, 5),
            rect_verify::adversarial::dense_conflict_grid(8, 8),
        ])
        .collect::<Vec<_>>();
    for (index, instance) in instances.iter().enumerate() {
        let path = output_dir.join(format!("{index:03}-{}.json", instance.name));
        instance
            .write_json(&path)
            .map_err(|error| CliError::Output(error.to_string()))?;
    }
    write_json(&instances, Some(&output_dir.join("index.json")))
}

fn benchmark_command(
    suite: BenchmarkSuiteArg,
    max_cells: usize,
    oracle_cell_limit: usize,
    sizes: &[usize],
    output: &Path,
) -> Result<(), CliError> {
    if suite == BenchmarkSuiteArg::Polyomino && max_cells == 0 {
        return Err(CliError::Input(
            "polyomino benchmark requires --max-cells greater than zero".to_owned(),
        ));
    }
    let context = benchmark_context()?;
    let report = match suite {
        BenchmarkSuiteArg::Adversarial => rect_verify::benchmark::benchmark_adversarial(context),
        BenchmarkSuiteArg::DenseConflict => {
            rect_verify::benchmark::benchmark_dense_conflict(context, sizes)
        }
        BenchmarkSuiteArg::Polyomino => {
            rect_verify::benchmark::benchmark_polyomino(context, max_cells, oracle_cell_limit)
        }
    };
    let csv = report
        .to_csv()
        .map_err(|error| CliError::Output(error.to_string()))?;
    write_text(output, &csv)?;
    let manifest_path = output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("manifest.json");
    update_manifest(&manifest_path, report.metadata.clone())?;
    if report.counterexample_count != 0 || report.solver_error_count != 0 {
        return Err(CliError::Verification(format!(
            "benchmark recorded {} counterexamples and {} solver errors",
            report.counterexample_count, report.solver_error_count
        )));
    }
    Ok(())
}

fn parse_sizes(value: &str) -> Result<Vec<usize>, CliError> {
    let sizes = value
        .split(',')
        .map(str::trim)
        .map(|item| {
            item.parse::<usize>().map_err(|_| {
                CliError::Input(format!("dense-conflict size `{item}` is not an integer"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if sizes.is_empty() || sizes.contains(&0) {
        return Err(CliError::Input(
            "dense-conflict sizes must be nonempty positive integers".to_owned(),
        ));
    }
    Ok(sizes)
}

fn compare_external_command(
    input: &Path,
    external_result: &Path,
    exact_cover_cell_limit: usize,
    output: Option<&Path>,
) -> Result<(), CliError> {
    let input_bytes = fs::read(input)?;
    let input_hash = format!("{:x}", Sha256::digest(&input_bytes));
    let input_grid: JsonGrid = serde_json::from_slice(&input_bytes)?;
    let grid = ColorGrid::new(input_grid.width, input_grid.height, input_grid.cells)
        .map_err(|error| CliError::Input(error.to_string()))?;
    let external_bytes = fs::read(external_result)?;
    let external: rect_verify::external::ExternalOracleResult =
        serde_json::from_slice(&external_bytes)?;
    let report = rect_verify::external::compare_external(
        &grid,
        &input_hash,
        &external,
        exact_cover_cell_limit,
    )
    .map_err(|error| CliError::Verification(error.to_string()))?;
    write_json(&report, output)?;
    if !report.all_agree {
        return Err(CliError::Verification(
            "external oracle disagrees with at least one Rust solver".to_owned(),
        ));
    }
    Ok(())
}

fn generate_command(
    family: GenerateFamilyArg,
    horizontal: usize,
    vertical: usize,
    json_path: &Path,
    svg_path: &Path,
) -> Result<(), CliError> {
    if horizontal == 0 || vertical == 0 {
        return Err(CliError::Input(
            "dense-conflict chord targets must be positive".to_owned(),
        ));
    }
    let instance = match family {
        GenerateFamilyArg::DenseConflict => {
            rect_verify::adversarial::dense_conflict_grid(horizontal, vertical)
        }
    };
    if let Some(parent) = json_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    instance
        .write_json(json_path)
        .map_err(|error| CliError::Output(error.to_string()))?;
    let components = instance
        .foreground_components()
        .map_err(|error| CliError::Input(error.to_string()))?;
    let [component] = components.as_slice() else {
        return Err(CliError::Input(format!(
            "dense-conflict generator produced {} foreground components",
            components.len()
        )));
    };
    let analysis =
        rect_oracle_sg::analyze(component).map_err(|error| CliError::Solver(error.to_string()))?;
    let result = rect_dominance::solve(component, DominanceMode::Compact)
        .map_err(|error| CliError::Solver(error.to_string()))?;
    let (selected_horizontal, selected_vertical) = selected_chords(&result, &analysis)?;
    let svg = render_dissection_svg(
        component,
        &result,
        &SvgOverlay {
            horizontal_chords: &analysis.horizontal_chords,
            vertical_chords: &analysis.vertical_chords,
            selected_horizontal: &selected_horizontal,
            selected_vertical: &selected_vertical,
        },
    )?;
    if let Some(parent) = svg_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(svg_path, svg)?;
    Ok(())
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

fn write_text(path: &Path, value: &str) -> Result<(), CliError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, value)?;
    Ok(())
}

fn write_experiment_json(
    report: &impl Serialize,
    seed: Option<u64>,
    input_count: usize,
    component_count: usize,
    output: Option<&Path>,
) -> Result<(), CliError> {
    let context = benchmark_context()?;
    let mut value = serde_json::to_value(report)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| CliError::Output("experiment report is not a JSON object".to_owned()))?;
    object.insert(
        "metadata".to_owned(),
        serde_json::to_value(rect_verify::benchmark::BenchmarkMetadata {
            git_commit: context.git_commit,
            rustc_version: context.rustc_version,
            command: context.command,
            seed,
            timestamp: context.timestamp,
            input_count,
            component_count,
            input_model: "finite-colored-unit-cell-grid".to_owned(),
            unsupported_input_features: vec![
                "ornaments".to_owned(),
                "isolated-formal-boundary-points".to_owned(),
                "line-segment-holes".to_owned(),
                "point-holes".to_owned(),
                "degenerate-formal-holes".to_owned(),
                "general-polygon-input".to_owned(),
            ],
        })?,
    );
    write_json(&value, output)
}

fn benchmark_context() -> Result<rect_verify::benchmark::BenchmarkContext, CliError> {
    let git_output = ProcessCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !git_output.status.success() {
        return Err(CliError::Output(
            "cannot resolve the current Git commit".to_owned(),
        ));
    }
    let rustc_output = ProcessCommand::new("rustc").arg("--version").output()?;
    if !rustc_output.status.success() {
        return Err(CliError::Output("cannot resolve rustc version".to_owned()));
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| CliError::Output(error.to_string()))?
        .as_secs();
    Ok(rect_verify::benchmark::BenchmarkContext {
        git_commit: String::from_utf8(git_output.stdout)
            .map_err(|error| CliError::Output(error.to_string()))?
            .trim()
            .to_owned(),
        rustc_version: String::from_utf8(rustc_output.stdout)
            .map_err(|error| CliError::Output(error.to_string()))?
            .trim()
            .to_owned(),
        command: std::env::args().collect::<Vec<_>>().join(" "),
        seed: None,
        timestamp,
    })
}

fn update_manifest(
    path: &Path,
    metadata: rect_verify::benchmark::BenchmarkMetadata,
) -> Result<(), CliError> {
    let mut manifest = if path.exists() {
        serde_json::from_slice::<rect_verify::benchmark::ExperimentManifest>(&fs::read(path)?)?
    } else {
        rect_verify::benchmark::ExperimentManifest::default()
    };
    manifest.runs.push(metadata);
    write_json(&manifest, Some(path))
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
    rect_verify::minimize::write_regression_bundle(Path::new("test-data/regressions"), fixture)
        .map_err(|error| CliError::Output(error.to_string()))?;
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
    #[error("cannot produce requested output: {0}")]
    Output(String),
}
