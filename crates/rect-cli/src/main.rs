use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use rect_core::{ColorGrid, DissectionResult, GridComponent, SvgOverlay, render_dissection_svg};
use rect_dominance::{
    ChordEnumerator, ConflictRepresentationBackend, DominanceMode, RegionDualBackend,
    VerificationMode, solve_with_representation_and_region_dual,
};
use rect_oracle_sg::CompletionBackendKind;
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
        #[arg(long, value_enum)]
        chord_enumerator: Option<ChordEnumeratorArg>,
        #[arg(long, value_enum)]
        completion_backend: Option<CompletionBackendArg>,
        #[arg(long, value_enum)]
        representation: Option<RepresentationArg>,
        #[arg(long, value_enum)]
        region_dual: Option<RegionDualArg>,
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
        #[arg(
            long,
            default_value = "staircase,alternating-notch-corridor,comb,double-comb,spiral"
        )]
        families: String,
        #[arg(long)]
        output: PathBuf,
    },
    Generate {
        #[arg(long, value_enum)]
        family: GenerateFamilyArg,
        #[arg(long)]
        t: Option<usize>,
        #[arg(long, default_value_t = 1)]
        horizontal: usize,
        #[arg(long, default_value_t = 1)]
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
    DominanceCompactOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ChordEnumeratorArg {
    ReferencePairwise,
    GridInteriorRuns,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CompletionBackendArg {
    ReferenceRescan,
    IndexedFrontier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum RepresentationArg {
    Dominance4d,
    PathTree,
    Auto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum RegionDualArg {
    ReferenceArea,
    BoundaryLaminar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BenchmarkSuiteArg {
    Adversarial,
    CleanCensus,
    CleanCompleteBipartite,
    DenseConflict,
    DenseCompactOnly,
    DenseCompletion,
    CompletionHeavy,
    AreaHeavy,
    PathTreeComparison,
    Polyomino,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GenerateFamilyArg {
    CleanCompleteBipartite,
    DenseConflict,
}

#[derive(Clone, Debug, Deserialize)]
struct JsonGrid {
    width: usize,
    height: usize,
    cells: Vec<Value>,
}

#[derive(Deserialize, Serialize)]
struct PreservedExperimentManifest {
    schema_version: usize,
    runs: Vec<rect_verify::benchmark::BenchmarkMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_metadata: Option<ReleaseMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_summaries: Option<Vec<ReleaseSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated_tables: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize)]
struct ReleaseMetadata {
    git_commit: String,
    rustc_version: String,
    operating_system: String,
    cpu: String,
    build_profile: String,
    random_seed: u64,
    cp_sat_seed: u64,
    cp_sat_timeout_seconds_per_component: f64,
    commands: Vec<String>,
}

#[derive(Deserialize, Serialize)]
struct ReleaseSummary {
    version: String,
    tag: String,
    peeled_commit: String,
    evidence: String,
    result_commits: Vec<String>,
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

#[allow(clippy::too_many_lines)]
fn run() -> Result<(), CliError> {
    let cli = Cli::parse();
    match cli.command {
        Command::Solve {
            solver,
            input,
            output,
            svg,
            chord_enumerator,
            completion_backend,
            representation,
            region_dual,
        } => solve_command(
            solver,
            chord_enumerator,
            completion_backend,
            representation,
            region_dual,
            &input,
            output.as_deref(),
            svg.as_deref(),
        ),
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
            families,
            output,
        } => {
            let sizes = parse_sizes(&sizes)?;
            let families = parse_families(&families);
            benchmark_command(
                suite,
                max_cells,
                oracle_cell_limit,
                &sizes,
                &families,
                &output,
            )
        }
        Command::Generate {
            family,
            t,
            horizontal,
            vertical,
            json,
            svg,
        } => generate_command(family, t, horizontal, vertical, &json, &svg),
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
    families: &[String],
    output: &Path,
) -> Result<(), CliError> {
    if suite == BenchmarkSuiteArg::Polyomino && max_cells == 0 {
        return Err(CliError::Input(
            "polyomino benchmark requires --max-cells greater than zero".to_owned(),
        ));
    }
    let context = benchmark_context()?;
    if suite == BenchmarkSuiteArg::CleanCensus {
        let census = rect_verify::benchmark::clean_census_4x4(context);
        write_text(output, &census.to_csv())?;
        let json_path = output.with_extension("json");
        write_json(&census, Some(&json_path))?;
        write_text(&output.with_extension("md"), &census.to_markdown())?;
        let manifest_path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("manifest.json");
        update_manifest(&manifest_path, census.metadata)?;
        return Ok(());
    }
    let report = match suite {
        BenchmarkSuiteArg::Adversarial => rect_verify::benchmark::benchmark_adversarial(context),
        BenchmarkSuiteArg::CleanCompleteBipartite => {
            rect_verify::benchmark::benchmark_clean_complete_bipartite(context, sizes)
        }
        BenchmarkSuiteArg::PathTreeComparison => {
            rect_verify::benchmark::benchmark_path_tree_comparison(context, sizes)
        }
        BenchmarkSuiteArg::DenseConflict => {
            rect_verify::benchmark::benchmark_dense_conflict(context, sizes)
        }
        BenchmarkSuiteArg::DenseCompactOnly => {
            rect_verify::benchmark::benchmark_dense_compact_only(context, sizes)
        }
        BenchmarkSuiteArg::DenseCompletion => {
            rect_verify::benchmark::benchmark_dense_completion(context, sizes)
        }
        BenchmarkSuiteArg::CompletionHeavy => {
            rect_verify::benchmark::benchmark_completion_heavy(context, sizes, families)
        }
        BenchmarkSuiteArg::AreaHeavy => {
            rect_verify::benchmark::benchmark_area_heavy(context, sizes)
        }
        BenchmarkSuiteArg::Polyomino => {
            rect_verify::benchmark::benchmark_polyomino(context, max_cells, oracle_cell_limit)
        }
        BenchmarkSuiteArg::CleanCensus => unreachable!(),
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
    for fixture in &report.failure_fixtures {
        rect_verify::minimize::write_regression_bundle(Path::new("test-data/regressions"), fixture)
            .map_err(|error| CliError::Output(error.to_string()))?;
    }
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

fn parse_families(value: &str) -> Vec<String> {
    value
        .split(',')
        .filter(|family| !family.is_empty())
        .map(str::to_owned)
        .collect()
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
    t: Option<usize>,
    horizontal: usize,
    vertical: usize,
    json_path: &Path,
    svg_path: &Path,
) -> Result<(), CliError> {
    if matches!(family, GenerateFamilyArg::DenseConflict) && (horizontal == 0 || vertical == 0) {
        return Err(CliError::Input(
            "dense-conflict chord targets must be positive".to_owned(),
        ));
    }
    let instance = match family {
        GenerateFamilyArg::CleanCompleteBipartite => {
            rect_verify::adversarial::clean_complete_bipartite_grid(t.ok_or_else(|| {
                CliError::Input("clean-complete-bipartite requires --t".to_owned())
            })?)
            .map_err(|error| CliError::Input(error.to_string()))?
        }
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
            "generator produced {} foreground components",
            components.len()
        )));
    };
    let geometry = rect_oracle_sg::analyze_geometry_with(
        component,
        &rect_oracle_sg::GridInteriorRunEnumerator,
    )
    .map_err(|error| CliError::Solver(error.to_string()))?;
    let result =
        rect_dominance::solve_with_verification_mode(component, VerificationMode::CompactOnly)
            .map_err(|error| CliError::Solver(error.to_string()))?;
    let (selected_horizontal, selected_vertical) = selected_chords(
        &result,
        geometry.horizontal_chords.len(),
        geometry.vertical_chords.len(),
    )?;
    let svg = render_dissection_svg(
        component,
        &result,
        &SvgOverlay {
            horizontal_chords: &geometry.horizontal_chords,
            vertical_chords: &geometry.vertical_chords,
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

#[allow(clippy::too_many_arguments)]
fn solve_command(
    solver: SolverArg,
    chord_enumerator: Option<ChordEnumeratorArg>,
    completion_backend: Option<CompletionBackendArg>,
    representation: Option<RepresentationArg>,
    region_dual: Option<RegionDualArg>,
    input: &Path,
    output: Option<&Path>,
    svg: Option<&Path>,
) -> Result<(), CliError> {
    let grid = load_grid(input)?;
    let components = grid.four_connected_components();
    let mut solutions = Vec::with_capacity(components.len());
    for component in &components {
        let result = solve_component(
            component,
            solver,
            chord_enumerator,
            completion_backend,
            representation,
            region_dual,
        )?;
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
    chord_enumerator: Option<ChordEnumeratorArg>,
    completion_backend: Option<CompletionBackendArg>,
    representation: Option<RepresentationArg>,
    region_dual: Option<RegionDualArg>,
) -> Result<DissectionResult, CliError> {
    let completion_backend = completion_backend.map(completion_backend_kind);
    match solver {
        SolverArg::ExactCover => {
            if completion_backend.is_some() || representation.is_some() || region_dual.is_some() {
                return Err(CliError::Input(
                    "completion and representation options apply only to dominance solvers"
                        .to_owned(),
                ));
            }
            rect_oracle_exact_cover::solve(component)
                .map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::SgExplicit => {
            if completion_backend.is_some() || representation.is_some() || region_dual.is_some() {
                return Err(CliError::Input(
                    "completion and representation options apply only to dominance solvers"
                        .to_owned(),
                ));
            }
            rect_oracle_sg::solve(component).map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::DominanceC0 => {
            if completion_backend.is_some() || representation.is_some() || region_dual.is_some() {
                return Err(CliError::Input(
                    "completion and representation options apply only to dominance solvers"
                        .to_owned(),
                ));
            }
            rect_dominance::solve(component, DominanceMode::ExplicitEdges)
                .map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::DominanceCompressed => solve_with_representation_and_region_dual(
            component,
            VerificationMode::FullyAudited,
            representation_kind(representation.unwrap_or(RepresentationArg::Dominance4d)),
            dominance_enumerator(chord_enumerator.unwrap_or(ChordEnumeratorArg::ReferencePairwise)),
            completion_backend.unwrap_or(CompletionBackendKind::ReferenceRescan),
            region_dual.map_or(
                RegionDualBackend::ReferenceAreaFloodFill,
                region_dual_kind,
            ),
        )
        .map_err(|error| CliError::Solver(error.to_string())),
        SolverArg::DominanceCompactOnly => solve_with_representation_and_region_dual(
            component,
            VerificationMode::CompactOnly,
            representation_kind(representation.unwrap_or(RepresentationArg::Dominance4d)),
            dominance_enumerator(chord_enumerator.unwrap_or(ChordEnumeratorArg::GridInteriorRuns)),
            completion_backend.unwrap_or(CompletionBackendKind::IndexedFrontier),
            region_dual.map_or(RegionDualBackend::BoundaryLaminar, region_dual_kind),
        )
        .map_err(|error| CliError::Solver(error.to_string())),
    }
}

const fn completion_backend_kind(backend: CompletionBackendArg) -> CompletionBackendKind {
    match backend {
        CompletionBackendArg::ReferenceRescan => CompletionBackendKind::ReferenceRescan,
        CompletionBackendArg::IndexedFrontier => CompletionBackendKind::IndexedFrontier,
    }
}

const fn dominance_enumerator(enumerator: ChordEnumeratorArg) -> ChordEnumerator {
    match enumerator {
        ChordEnumeratorArg::ReferencePairwise => ChordEnumerator::ReferencePairwise,
        ChordEnumeratorArg::GridInteriorRuns => ChordEnumerator::GridInteriorRuns,
    }
}

const fn representation_kind(representation: RepresentationArg) -> ConflictRepresentationBackend {
    match representation {
        RepresentationArg::Dominance4d => ConflictRepresentationBackend::GeneralDominance4D,
        RepresentationArg::PathTree => ConflictRepresentationBackend::CleanHoleFreePathTree,
        RepresentationArg::Auto => ConflictRepresentationBackend::Auto,
    }
}

const fn region_dual_kind(backend: RegionDualArg) -> RegionDualBackend {
    match backend {
        RegionDualArg::ReferenceArea => RegionDualBackend::ReferenceAreaFloodFill,
        RegionDualArg::BoundaryLaminar => RegionDualBackend::BoundaryLaminar,
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
        serde_json::from_slice::<PreservedExperimentManifest>(&fs::read(path)?)?
    } else {
        PreservedExperimentManifest {
            schema_version: 1,
            runs: Vec::new(),
            release_metadata: None,
            release_summaries: None,
            generated_tables: None,
        }
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
        } else if solver == SolverArg::DominanceCompactOnly {
            let geometry = rect_oracle_sg::analyze_geometry_with(
                component,
                &rect_oracle_sg::GridInteriorRunEnumerator,
            )
            .map_err(|error| CliError::Solver(error.to_string()))?;
            let (selected_horizontal, selected_vertical) = selected_chords(
                &solution.result,
                geometry.horizontal_chords.len(),
                geometry.vertical_chords.len(),
            )?;
            render_dissection_svg(
                component,
                &solution.result,
                &SvgOverlay {
                    horizontal_chords: &geometry.horizontal_chords,
                    vertical_chords: &geometry.vertical_chords,
                    selected_horizontal: &selected_horizontal,
                    selected_vertical: &selected_vertical,
                },
            )?
        } else {
            let analysis = rect_oracle_sg::analyze(component)
                .map_err(|error| CliError::Solver(error.to_string()))?;
            let (selected_horizontal, selected_vertical) = selected_chords(
                &solution.result,
                analysis.horizontal_chords.len(),
                analysis.vertical_chords.len(),
            )?;
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
    horizontal_count: usize,
    vertical_count: usize,
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
    let mut horizontal = vec![false; horizontal_count];
    let mut vertical = vec![false; vertical_count];
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

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Value;

    use super::{
        ChordEnumeratorArg, CompletionBackendArg, RepresentationArg, SolverArg, solve_command,
    };

    #[test]
    fn compact_only_svg_keeps_forbidden_execution_trace_false() {
        let root =
            std::env::temp_dir().join(format!("mrd-compact-svg-regression-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("input.json");
        let output = root.join("output.json");
        let svg = root.join("output.svg");
        fs::write(
            &input,
            br#"{"width":2,"height":2,"cells":["a","a","a","a"]}"#,
        )
        .unwrap();
        solve_command(
            SolverArg::DominanceCompactOnly,
            Some(ChordEnumeratorArg::GridInteriorRuns),
            Some(CompletionBackendArg::IndexedFrontier),
            Some(RepresentationArg::Dominance4d),
            None,
            input.as_path(),
            Some(output.as_path()),
            Some(svg.as_path()),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        let diagnostics = &value["components"][0]["result"]["diagnostics"];
        assert!(diagnostics["explicit_conflict_edge_count"].is_null());
        let trace = &diagnostics["execution_trace"];
        for key in [
            "pairwise_embedding_audit_called",
            "explicit_conflict_graph_built",
            "hopcroft_karp_called",
            "c0_partition_built",
            "full_edge_partition_audit_called",
        ] {
            assert_eq!(trace[key], false, "forbidden trace flag {key}");
        }
        assert_eq!(trace["compact_structure_check_called"], true);
        assert!(!fs::read(&svg).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }
}
