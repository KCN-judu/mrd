use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Parser, Subcommand, ValueEnum};
use rect_core::{
    ColorGrid, DissectionResult, FormalBoundaryIncidence, FormalRectilinearPolygon, GridComponent,
    Ornament, OrnamentSegment, OrthogonalLoop, Point, PolygonDissectionResult,
    PolygonGeometryBackend, PolygonValidationBackend, RectilinearPolygon, SvgOverlay,
    render_dissection_svg, render_polygon_dissection_svg,
};
use rect_dominance::{
    ChordEnumerator, ConflictRepresentationBackend, DominanceMode, PathTreeOrientationPolicy,
    PolygonArrangementBackend, PolygonChordBackend, PolygonCompletionBackend, PolygonSolveOptions,
    RegionDualBackend, VerificationMode, complete_formal_polygon, solve_polygon_with_options,
    solve_with_representation_and_region_dual_and_orientation_policy,
};
use rect_graph::{An19AdversarialCampaign, An19AdversarialFamily};
use rect_oracle_sg::{
    CompletionBackendKind, PolygonCutIndexBackend, PolygonDissectionValidatorBackend,
    PolygonRecoveryBackend, SparseValidatorBackend, SubdivisionBuilderBackend,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Parser)]
#[command(
    name = "rect-cli",
    version,
    about = "Exact rectangular-dissection verification for grids and ordinary polygons",
    long_about = "Exact rectangular-dissection verification for finite colored unit-cell grids and boundary-native rectilinear polygons.\n\nOrdinary polygon solving supports one nondegenerate outer loop and ordinary two-dimensional holes. Formal-boundary JSON additionally supports source-validated ornaments, isolated points, and point/segment formal holes for canonical inspection; solver integration begins in phase P3."
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
        #[arg(long, value_enum, default_value_t = InputFormatArg::Auto)]
        input_format: InputFormatArg,
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
        #[arg(long, value_enum)]
        path_tree_orientation: Option<PathTreeOrientationArg>,
        #[arg(long, value_enum)]
        polygon_geometry: Option<PolygonGeometryArg>,
        #[arg(long, value_enum)]
        polygon_validator: Option<PolygonValidatorArg>,
        #[arg(long, value_enum)]
        polygon_chords: Option<PolygonChordsArg>,
        #[arg(long, value_enum)]
        polygon_completion: Option<PolygonCompletionArg>,
        #[arg(long, value_enum)]
        polygon_arrangement: Option<PolygonArrangementArg>,
        #[arg(long, value_enum)]
        polygon_cut_index: Option<PolygonCutIndexArg>,
        #[arg(long, value_enum)]
        polygon_recovery: Option<PolygonRecoveryArg>,
        #[arg(long, value_enum)]
        polygon_dissection_validator: Option<PolygonDissectionValidatorArg>,
        #[arg(long, value_enum)]
        subdivision_builder: Option<SubdivisionBuilderArg>,
        #[arg(long, value_enum)]
        sparse_validator: Option<SparseValidatorArg>,
    },
    Verify {
        #[arg(long)]
        input: PathBuf,
        #[arg(long, value_enum, default_value_t = InputFormatArg::Auto)]
        input_format: InputFormatArg,
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
        #[arg(long, default_value_t = 100_000)]
        random_cases: usize,
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
    SearchPathTreeWitness {
        #[arg(long, default_value_t = 12)]
        max_width: usize,
        #[arg(long, default_value_t = 12)]
        max_height: usize,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        #[arg(long, default_value_t = false)]
        require_clean: bool,
        #[arg(long, default_value_t = 2)]
        min_horizontal_chords: usize,
        #[arg(long, default_value_t = 2)]
        min_vertical_chords: usize,
        #[arg(long, default_value_t = 3)]
        min_dual_branching: usize,
        #[arg(long, default_value_t = 3)]
        min_path_count: usize,
        #[arg(long, default_value_t = 4)]
        min_heavy_chain_intervals: usize,
        #[arg(long, default_value_t = 2)]
        min_canonical_nodes: usize,
        #[arg(long)]
        output_dir: PathBuf,
    },
    An19Events {
        #[arg(long, value_enum, default_value_t = An19EventEngineArg::ReducedExact)]
        an19_event_engine: An19EventEngineArg,
        #[arg(long)]
        an19_event_trace: Option<PathBuf>,
        #[arg(long)]
        an19_charge_analysis: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = An19AdversarialFamilyArg::All)]
        an19_adversarial_family: An19AdversarialFamilyArg,
        #[arg(long, default_value = "16,32,64")]
        an19_adversarial_size: String,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        markdown: PathBuf,
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
enum InputFormatArg {
    Auto,
    Grid,
    Polygon,
    FormalPolygon,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum An19EventEngineArg {
    ExactOracle,
    ReducedExact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum An19AdversarialFamilyArg {
    All,
    ManyReducedCostsFewSourceLengths,
    RepeatedPortalSplitting,
    FullDepthPersistence,
    AllEqualReducedKeys,
    AllDistinctReducedKeys,
    AlternatingPartitionContraction,
    HighwayHalvingReorder,
    VirtualRealMixedSegments,
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
enum PathTreeOrientationArg {
    BuildBoth,
    BoundEstimate,
    VerticalTree,
    HorizontalTree,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonGeometryArg {
    ReferenceScan,
    Indexed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonValidatorArg {
    ReferenceQuadratic,
    OrthogonalSweep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonChordsArg {
    ReferencePairwise,
    IndexedPairwise,
    SgSweep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonCompletionArg {
    CoordinateReference,
    IndexedFrontier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonArrangementArg {
    Reference,
    Indexed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonCutIndexArg {
    LineMapReference,
    DynamicStabbing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonRecoveryArg {
    DenseArrangement,
    SparseSubdivision,
    Auto,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PolygonDissectionValidatorArg {
    DenseArrangement,
    SparseSlab,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SubdivisionBuilderArg {
    ReferenceRangeScan,
    OrthogonalSweep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SparseValidatorArg {
    ReferenceSlabRescan,
    EventSegmentTree,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum BenchmarkSuiteArg {
    Adversarial,
    BicliqueConstruction,
    CleanCensus,
    CleanBoundaryDifferential,
    CleanCompleteBipartite,
    CleanCompleteBipartiteCompact,
    DenseConflict,
    DenseCompactOnly,
    DenseCompletion,
    CompletionHeavy,
    AreaHeavy,
    PathTreeComparison,
    PathTreeFamilies,
    PathTreeScaling,
    PathTreeVs4d,
    PathTreeAdvantage,
    PathTreeOrientationAudit,
    PathTreeDualDifferential,
    PathTreeGapDifferential,
    AutoFallback,
    Polyomino,
    PolygonDifferential,
    PolygonBackendDifferential,
    PolygonNegative,
    PolygonNativeFixtures,
    PolygonScaling,
    FormalFixtures,
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

#[derive(Clone, Debug, Deserialize)]
struct JsonPolygon {
    #[serde(rename = "type")]
    kind: String,
    outer: Vec<[i64; 2]>,
    #[serde(default)]
    holes: Vec<Vec<[i64; 2]>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct JsonFormalOrnament {
    #[serde(default)]
    isolated_points: Vec<[i64; 2]>,
    #[serde(default)]
    segments: Vec<[[i64; 2]; 2]>,
}

#[derive(Clone, Debug, Deserialize)]
struct JsonFormalPolygon {
    #[serde(rename = "type")]
    kind: String,
    outer: Vec<[i64; 2]>,
    #[serde(default)]
    holes: Vec<Vec<[i64; 2]>>,
    #[serde(default)]
    ornament: JsonFormalOrnament,
}

#[derive(Clone, Debug, Serialize)]
struct FormalBoundaryValidationOutput {
    input_model: &'static str,
    polygon: FormalRectilinearPolygon,
    incidence: FormalBoundaryIncidence,
}

#[derive(Clone, Debug, Serialize)]
struct FormalSolveOutput {
    solver: String,
    input_model: &'static str,
    polygon: FormalRectilinearPolygon,
    local_nonconvexity_measure: usize,
    interior_component_count: usize,
    formal_hole_count: usize,
    effective_number: usize,
    optimum_rectangle_count: usize,
    effective_chords: rect_core::FormalEffectiveChordFamilies,
    step_two_transformation: rect_dominance::FormalStep2Transformation,
    explicit_matching: rect_graph::Matching,
    explicit_vertex_cover: rect_graph::VertexCover,
    compact_vertex_cover: rect_graph::VertexCover,
    selected_horizontal: Vec<bool>,
    selected_vertical: Vec<bool>,
    completion: rect_oracle_sg::PolygonCompletionResult,
}

#[derive(Deserialize, Serialize)]
struct PreservedExperimentManifest {
    schema_version: usize,
    runs: Vec<rect_verify::benchmark::BenchmarkMetadata>,
    #[serde(
        default,
        alias = "release_metadata",
        skip_serializing_if = "Option::is_none"
    )]
    historical_release_metadata: Option<ReleaseMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_summaries: Option<Vec<ReleaseSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_release: Option<CurrentRelease>,
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

#[derive(Deserialize, Serialize)]
struct CurrentRelease {
    version: String,
    tag: String,
    peeled_commit: String,
    defaults: BTreeMap<String, String>,
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

#[derive(Serialize)]
struct PolygonSolveOutput {
    solver: String,
    input_model: &'static str,
    polygon: RectilinearPolygon,
    result: PolygonDissectionResult,
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
            input_format,
            input,
            output,
            svg,
            chord_enumerator,
            completion_backend,
            representation,
            region_dual,
            path_tree_orientation,
            polygon_geometry,
            polygon_validator,
            polygon_chords,
            polygon_completion,
            polygon_arrangement,
            polygon_cut_index,
            polygon_recovery,
            polygon_dissection_validator,
            subdivision_builder,
            sparse_validator,
        } => solve_command(
            solver,
            input_format,
            chord_enumerator,
            completion_backend,
            representation,
            region_dual,
            path_tree_orientation,
            polygon_geometry,
            polygon_validator,
            polygon_chords,
            polygon_completion,
            polygon_arrangement,
            polygon_cut_index,
            polygon_recovery,
            polygon_dissection_validator,
            subdivision_builder,
            sparse_validator,
            &input,
            output.as_deref(),
            svg.as_deref(),
        ),
        Command::Verify {
            input,
            input_format,
            all_solvers: _,
            exact_cover_cell_limit,
            output,
        } => match load_input(&input, input_format)? {
            LoadedInput::Grid(grid) => {
                let report = rect_verify::verify_grid(&grid, exact_cover_cell_limit)
                    .map_err(|error| CliError::Verification(error.to_string()))?;
                write_json(&report, output.as_deref())
            }
            LoadedInput::Polygon(polygon) => {
                let report = rect_verify::polygon::verify_polygon(
                    &polygon,
                    Some(rect_verify::polygon::RasterLimits {
                        max_width: exact_cover_cell_limit,
                        max_height: exact_cover_cell_limit,
                        max_cells: exact_cover_cell_limit,
                    }),
                )
                .map_err(|error| CliError::Verification(error.to_string()))?;
                write_json(&report, output.as_deref())?;
                if report.verified() {
                    Ok(())
                } else {
                    Err(CliError::Verification(format!(
                        "polygon backend disagreements: {}",
                        report.disagreements.join("; ")
                    )))
                }
            }
            LoadedInput::FormalPolygon(polygon) => {
                let incidence = polygon
                    .incidence()
                    .map_err(|error| CliError::Verification(error.to_string()))?;
                write_json(
                    &FormalBoundaryValidationOutput {
                        input_model: "formal-rectilinear-polygon",
                        polygon,
                        incidence,
                    },
                    output.as_deref(),
                )
            }
        },
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
            random_cases,
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
                random_cases,
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
        Command::SearchPathTreeWitness {
            max_width,
            max_height,
            seed,
            require_clean,
            min_horizontal_chords,
            min_vertical_chords,
            min_dual_branching,
            min_path_count,
            min_heavy_chain_intervals,
            min_canonical_nodes,
            output_dir,
        } => search_path_tree_witness_command(
            max_width,
            max_height,
            seed,
            require_clean,
            min_horizontal_chords,
            min_vertical_chords,
            min_dual_branching,
            min_path_count,
            min_heavy_chain_intervals,
            min_canonical_nodes,
            &output_dir,
        ),
        Command::An19Events {
            an19_event_engine,
            an19_event_trace,
            an19_charge_analysis,
            an19_adversarial_family,
            an19_adversarial_size,
            output,
            markdown,
        } => an19_events_command(
            an19_event_engine,
            an19_adversarial_family,
            &an19_adversarial_size,
            &output,
            &markdown,
            an19_event_trace.as_deref(),
            an19_charge_analysis.as_deref(),
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn an19_events_command(
    engine: An19EventEngineArg,
    family: An19AdversarialFamilyArg,
    sizes: &str,
    output: &Path,
    markdown: &Path,
    trace_output: Option<&Path>,
    charge_output: Option<&Path>,
) -> Result<(), CliError> {
    let context = benchmark_context()?;
    let sizes = parse_an19_sizes(sizes)?;
    let families = family.families();
    let campaign =
        An19AdversarialCampaign::run(&families, &sizes, context.git_commit, context.command)
            .map_err(|error| CliError::Verification(error.to_string()))?;
    write_json(&campaign, Some(output))?;
    write_text(markdown, &campaign.to_markdown())?;
    if let Some(path) = trace_output {
        let traces = campaign
            .cases
            .iter()
            .map(|case| match engine {
                An19EventEngineArg::ExactOracle => &case.oracle_run,
                An19EventEngineArg::ReducedExact => &case.reduced_run,
            })
            .collect::<Vec<_>>();
        write_json(&traces, Some(path))?;
    }
    if let Some(path) = charge_output {
        let analyses = campaign
            .cases
            .iter()
            .map(|case| {
                (
                    case.input_family,
                    case.size_parameter,
                    case.logical_call_index,
                    &case.charge_analyses,
                )
            })
            .collect::<Vec<_>>();
        write_json(&analyses, Some(path))?;
    }
    Ok(())
}

impl An19AdversarialFamilyArg {
    fn families(self) -> Vec<An19AdversarialFamily> {
        match self {
            Self::All => An19AdversarialFamily::ALL.to_vec(),
            Self::ManyReducedCostsFewSourceLengths => {
                vec![An19AdversarialFamily::ManyReducedCostsFewSourceLengths]
            }
            Self::RepeatedPortalSplitting => {
                vec![An19AdversarialFamily::RepeatedPortalSplitting]
            }
            Self::FullDepthPersistence => vec![An19AdversarialFamily::FullDepthPersistence],
            Self::AllEqualReducedKeys => vec![An19AdversarialFamily::AllEqualReducedKeys],
            Self::AllDistinctReducedKeys => vec![An19AdversarialFamily::AllDistinctReducedKeys],
            Self::AlternatingPartitionContraction => {
                vec![An19AdversarialFamily::AlternatingPartitionContraction]
            }
            Self::HighwayHalvingReorder => vec![An19AdversarialFamily::HighwayHalvingReorder],
            Self::VirtualRealMixedSegments => {
                vec![An19AdversarialFamily::VirtualRealMixedSegments]
            }
        }
    }
}

fn parse_an19_sizes(value: &str) -> Result<Vec<usize>, CliError> {
    let sizes = value
        .split(',')
        .map(str::trim)
        .map(|item| {
            item.parse::<usize>().map_err(|_| {
                CliError::Input(format!("AN19 adversarial size `{item}` is not an integer"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if sizes.is_empty() || sizes.contains(&0) {
        return Err(CliError::Input(
            "AN19 adversarial sizes must be nonempty positive integers".to_owned(),
        ));
    }
    Ok(sizes)
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

#[allow(clippy::too_many_arguments)]
fn search_path_tree_witness_command(
    max_width: usize,
    max_height: usize,
    seed: u64,
    require_clean: bool,
    min_horizontal_chords: usize,
    min_vertical_chords: usize,
    min_dual_branching: usize,
    min_path_count: usize,
    min_heavy_chain_intervals: usize,
    min_canonical_nodes: usize,
    output_dir: &Path,
) -> Result<(), CliError> {
    let report = rect_verify::witness::search_path_tree_witnesses(
        max_width,
        max_height,
        seed,
        require_clean,
        min_horizontal_chords,
        min_vertical_chords,
        min_dual_branching,
        min_path_count,
        min_heavy_chain_intervals,
        min_canonical_nodes,
        output_dir,
    )
    .map_err(|error| CliError::Output(error.to_string()))?;
    write_json(&report, Some(&output_dir.join("report.json")))
}

#[allow(clippy::too_many_lines)]
fn benchmark_command(
    suite: BenchmarkSuiteArg,
    max_cells: usize,
    oracle_cell_limit: usize,
    random_cases: usize,
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
    if suite == BenchmarkSuiteArg::PolygonDifferential {
        let report =
            rect_verify::polygon_campaign::exhaustive_grid_polygon_campaign(context, sizes);
        write_json(&report, Some(output))?;
        write_json(
            &report.minimized_counterexamples,
            Some(&output.with_extension("counterexamples.json")),
        )?;
        update_manifest(
            &output.with_file_name("manifest.json"),
            report.metadata.clone(),
        )?;
        return if report.verified() {
            Ok(())
        } else {
            Err(CliError::Verification(format!(
                "polygon differential failures: {} disagreements, {} solver errors",
                report.disagreements, report.solver_errors
            )))
        };
    }
    if suite == BenchmarkSuiteArg::PolygonBackendDifferential {
        let report = rect_verify::polygon_campaign::extended_polygon_backend_campaign(
            context,
            max_cells,
            random_cases,
            sizes,
        );
        write_json(&report, Some(output))?;
        write_json(
            &report.minimized_counterexamples,
            Some(&output.with_extension("counterexamples.json")),
        )?;
        update_manifest(
            &output.with_file_name("manifest.json"),
            report.metadata.clone(),
        )?;
        return if report.verified() {
            Ok(())
        } else {
            Err(CliError::Verification(format!(
                "polygon backend differential failures: {} disagreements, {} solver errors",
                report.disagreements, report.solver_errors
            )))
        };
    }
    if suite == BenchmarkSuiteArg::PolygonNegative {
        let report = rect_verify::polygon_campaign::polygon_negative_campaign(context);
        write_json(&report, Some(output))?;
        update_manifest(
            &output.with_file_name("manifest.json"),
            report.metadata.clone(),
        )?;
        return if report.verified() {
            Ok(())
        } else {
            Err(CliError::Verification(format!(
                "polygon negative validation failures: {} disagreements",
                report.disagreements
            )))
        };
    }
    if suite == BenchmarkSuiteArg::PolygonNativeFixtures {
        let report = rect_verify::polygon_campaign::native_polygon_fixture_campaign(context, sizes);
        write_json(&report, Some(output))?;
        write_json(
            &report.minimized_counterexamples,
            Some(&output.with_extension("counterexamples.json")),
        )?;
        update_manifest(
            &output.with_file_name("manifest.json"),
            report.metadata.clone(),
        )?;
        return if report.verified() {
            Ok(())
        } else {
            Err(CliError::Verification(format!(
                "polygon native fixture failures: {} disagreements, {} solver errors",
                report.disagreements, report.solver_errors
            )))
        };
    }
    if suite == BenchmarkSuiteArg::PolygonScaling {
        let report = rect_verify::polygon_campaign::polygon_scaling_campaign(context, sizes);
        write_text(output, &report.to_csv())?;
        write_json(&report, Some(&output.with_extension("json")))?;
        update_manifest(
            &output.with_file_name("manifest.json"),
            report.metadata.clone(),
        )?;
        return if report.verified() {
            Ok(())
        } else {
            Err(CliError::Verification(format!(
                "polygon scaling failures: {} disagreements, {} solver errors",
                report.disagreements, report.solver_errors
            )))
        };
    }
    if suite == BenchmarkSuiteArg::FormalFixtures {
        let report = rect_verify::formal_campaign::formal_fixture_campaign(context);
        write_json(&report, Some(output))?;
        update_manifest(
            &output.with_file_name("manifest.json"),
            report.metadata.clone(),
        )?;
        return if report.verified() {
            Ok(())
        } else {
            Err(CliError::Verification(format!(
                "formal fixture failures: {} disagreements, {} solver errors",
                report.disagreements, report.solver_errors
            )))
        };
    }
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
    if suite == BenchmarkSuiteArg::CleanBoundaryDifferential {
        let report = rect_verify::benchmark::clean_boundary_differential_4x4(context);
        write_text(output, &report.to_csv())?;
        write_json(&report, Some(&output.with_extension("json")))?;
        write_text(&output.with_extension("md"), &report.to_markdown())?;
        let manifest_path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("manifest.json");
        update_manifest(&manifest_path, report.metadata)?;
        return Ok(());
    }
    if suite == BenchmarkSuiteArg::PathTreeGapDifferential {
        let maximum_t = sizes.iter().copied().max().unwrap_or(128).max(1);
        let report = rect_verify::gap_differential::verify_gap_backends(
            context,
            rect_verify::gap_differential::GapDifferentialConfig {
                polyomino_max_cells: max_cells,
                random_cases,
                complete_bipartite_max_t: maximum_t,
                family_scales: sizes.to_vec(),
                ..rect_verify::gap_differential::GapDifferentialConfig::default()
            },
        );
        write_text(output, &report.to_csv())?;
        write_json(&report, Some(&output.with_extension("json")))?;
        write_text(&output.with_extension("md"), &report.to_markdown())?;
        let manifest_path = output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("manifest.json");
        update_manifest(&manifest_path, report.metadata.clone())?;
        return if report.verified() {
            Ok(())
        } else {
            Err(CliError::Verification(format!(
                "path-tree gap differential failures: {} mismatches, {} solver errors",
                report.total_mismatch_count, report.total_solver_error_count
            )))
        };
    }
    let report = match suite {
        BenchmarkSuiteArg::Adversarial => rect_verify::benchmark::benchmark_adversarial(context),
        BenchmarkSuiteArg::BicliqueConstruction => {
            let report = rect_verify::benchmark::benchmark_biclique_construction(context, sizes);
            write_text(output, &report.to_csv())?;
            write_json(&report, Some(&output.with_extension("json")))?;
            let manifest_path = output
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("manifest.json");
            update_manifest(&manifest_path, report.metadata.clone())?;
            return if report.verified() {
                Ok(())
            } else {
                Err(CliError::Verification(format!(
                    "biclique construction recorded {} counterexamples and {} solver errors",
                    report.counterexample_count, report.solver_error_count
                )))
            };
        }
        BenchmarkSuiteArg::CleanCompleteBipartite => {
            rect_verify::benchmark::benchmark_clean_complete_bipartite(context, sizes)
        }
        BenchmarkSuiteArg::CleanCompleteBipartiteCompact => {
            rect_verify::benchmark::benchmark_clean_complete_bipartite_compact(context, sizes)
        }
        BenchmarkSuiteArg::PathTreeComparison => {
            rect_verify::benchmark::benchmark_path_tree_comparison(context, sizes)
        }
        BenchmarkSuiteArg::PathTreeFamilies => {
            rect_verify::benchmark::benchmark_path_tree_geometry_families(
                context,
                sizes.iter().copied().max().unwrap_or(5),
            )
        }
        BenchmarkSuiteArg::PathTreeScaling => {
            rect_verify::benchmark::benchmark_path_tree_geometry_scaling(context, sizes)
        }
        BenchmarkSuiteArg::PathTreeVs4d => {
            let report = rect_verify::benchmark::benchmark_path_tree_vs_4d(context, sizes);
            write_text(output, &report.to_csv())?;
            write_json(&report, Some(&output.with_extension("json")))?;
            let manifest_path = output
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("manifest.json");
            update_manifest(&manifest_path, report.metadata.clone())?;
            return if report.counterexamples == 0 {
                Ok(())
            } else {
                Err(CliError::Solver(format!(
                    "path-tree-vs-4d counterexamples: {}",
                    report.counterexamples
                )))
            };
        }
        BenchmarkSuiteArg::PathTreeAdvantage => {
            let report = rect_verify::benchmark::benchmark_path_tree_advantage(&context, sizes, 16);
            write_text(output, &report.to_csv())?;
            write_json(&report, Some(&output.with_extension("json")))?;
            write_text(&output.with_extension("md"), &report.to_markdown())?;
            let manifest_path = output
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("manifest.json");
            update_manifest(&manifest_path, report.metadata.clone())?;
            return Ok(());
        }
        BenchmarkSuiteArg::PathTreeOrientationAudit => {
            let report =
                rect_verify::benchmark::benchmark_path_tree_orientation_audit(context, sizes);
            write_text(output, &report.to_csv())?;
            write_json(
                &serde_json::json!({
                    "metadata": report.metadata.clone(),
                    "rows": report.rows.len(),
                    "exact_matches": report.exact_matches,
                    "mismatches": report.mismatches,
                    "tie_orientation_differences": report.tie_orientation_differences,
                    "maximum_absolute_regret": report.maximum_absolute_regret,
                    "maximum_regret_ratio": report.maximum_regret_ratio,
                }),
                Some(&output.with_extension("json")),
            )?;
            write_text(&output.with_extension("md"), &report.to_markdown())?;
            let manifest_path = output
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("manifest.json");
            update_manifest(&manifest_path, report.metadata.clone())?;
            // Positive regret is selector evidence, not a solver failure.
            // CompactOnly therefore keeps exact BuildBothExact in production.
            return Ok(());
        }
        BenchmarkSuiteArg::PathTreeDualDifferential => {
            let report =
                rect_verify::benchmark::benchmark_path_tree_dual_differential(context, sizes);
            write_text(output, &report.to_csv())?;
            write_json(
                &serde_json::json!({
                    "metadata": report.metadata.clone(),
                    "rows": report.rows.len(),
                    "verified": report.verified,
                    "counterexamples": report.counterexamples,
                    "solver_errors": report.solver_errors,
                }),
                Some(&output.with_extension("json")),
            )?;
            let manifest_path = output
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("manifest.json");
            update_manifest(&manifest_path, report.metadata.clone())?;
            if report.counterexamples == 0 && report.solver_errors == 0 {
                return Ok(());
            }
            return Err(CliError::Verification(format!(
                "path-tree dual differential failures: {} counterexamples, {} solver errors",
                report.counterexamples, report.solver_errors
            )));
        }
        BenchmarkSuiteArg::AutoFallback => rect_verify::benchmark::benchmark_auto_fallback(context),
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
        BenchmarkSuiteArg::CleanCensus | BenchmarkSuiteArg::CleanBoundaryDifferential => {
            unreachable!()
        }
        BenchmarkSuiteArg::PathTreeGapDifferential
        | BenchmarkSuiteArg::PolygonDifferential
        | BenchmarkSuiteArg::PolygonBackendDifferential
        | BenchmarkSuiteArg::PolygonNegative
        | BenchmarkSuiteArg::PolygonNativeFixtures
        | BenchmarkSuiteArg::PolygonScaling
        | BenchmarkSuiteArg::FormalFixtures => unreachable!(),
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
#[allow(clippy::too_many_lines)]
fn solve_command(
    solver: SolverArg,
    input_format: InputFormatArg,
    chord_enumerator: Option<ChordEnumeratorArg>,
    completion_backend: Option<CompletionBackendArg>,
    representation: Option<RepresentationArg>,
    region_dual: Option<RegionDualArg>,
    path_tree_orientation: Option<PathTreeOrientationArg>,
    polygon_geometry: Option<PolygonGeometryArg>,
    polygon_validator: Option<PolygonValidatorArg>,
    polygon_chords: Option<PolygonChordsArg>,
    polygon_completion: Option<PolygonCompletionArg>,
    polygon_arrangement: Option<PolygonArrangementArg>,
    polygon_cut_index: Option<PolygonCutIndexArg>,
    polygon_recovery: Option<PolygonRecoveryArg>,
    polygon_dissection_validator: Option<PolygonDissectionValidatorArg>,
    subdivision_builder: Option<SubdivisionBuilderArg>,
    sparse_validator: Option<SparseValidatorArg>,
    input: &Path,
    output: Option<&Path>,
    svg: Option<&Path>,
) -> Result<(), CliError> {
    match load_input(input, input_format)? {
        LoadedInput::Grid(grid) => {
            if polygon_geometry.is_some()
                || polygon_validator.is_some()
                || polygon_chords.is_some()
                || polygon_completion.is_some()
                || polygon_arrangement.is_some()
                || polygon_cut_index.is_some()
                || polygon_recovery.is_some()
                || polygon_dissection_validator.is_some()
                || subdivision_builder.is_some()
                || sparse_validator.is_some()
            {
                return Err(CliError::Input(
                    "polygon backend options require boundary-native polygon input".to_owned(),
                ));
            }
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
                    path_tree_orientation,
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
        LoadedInput::Polygon(polygon) => {
            if !matches!(
                solver,
                SolverArg::DominanceCompressed | SolverArg::DominanceCompactOnly
            ) {
                return Err(CliError::UnsupportedSolverForPolygon { solver });
            }
            if chord_enumerator.is_some() || completion_backend.is_some() {
                return Err(CliError::Input(
                    "polygon input uses general-polygon-pairwise chords and coordinate-compressed completion"
                        .to_owned(),
                ));
            }
            if region_dual.is_some_and(|backend| backend != RegionDualArg::BoundaryLaminar) {
                return Err(CliError::Input(
                    "polygon path-tree input supports only boundary-laminar region duals"
                        .to_owned(),
                ));
            }
            if path_tree_orientation
                .is_some_and(|policy| policy != PathTreeOrientationArg::BuildBoth)
            {
                return Err(CliError::Input(
                    "polygon path-tree orientation is fixed to exact build-both in v0.9".to_owned(),
                ));
            }
            let result = solve_polygon_with_options(
                &polygon,
                PolygonSolveOptions {
                    verification_mode: match solver {
                        SolverArg::DominanceCompressed => VerificationMode::FullyAudited,
                        SolverArg::DominanceCompactOnly => VerificationMode::CompactOnly,
                        _ => unreachable!("polygon solver was checked above"),
                    },
                    geometry_backend: polygon_geometry
                        .map_or(PolygonGeometryBackend::Indexed, polygon_geometry_kind),
                    validation_backend: polygon_validator.map_or(
                        PolygonValidationBackend::OrthogonalSweep,
                        polygon_validator_kind,
                    ),
                    chord_backend: polygon_chords.map_or(
                        PolygonChordBackend::SoltanGorpinevichSweep,
                        polygon_chords_kind,
                    ),
                    completion_backend: polygon_completion.map_or(
                        PolygonCompletionBackend::IndexedFrontier,
                        polygon_completion_kind,
                    ),
                    cut_index_backend: polygon_cut_index.map_or(
                        PolygonCutIndexBackend::DynamicStabbing,
                        polygon_cut_index_kind,
                    ),
                    recovery_backend: polygon_recovery.map_or_else(
                        || {
                            polygon_arrangement
                                .map_or(PolygonRecoveryBackend::SparseSubdivision, |_| {
                                    PolygonRecoveryBackend::DenseCoordinateArrangement
                                })
                        },
                        polygon_recovery_kind,
                    ),
                    dissection_validator_backend: polygon_dissection_validator.map_or_else(
                        || {
                            polygon_arrangement
                                .map_or(PolygonDissectionValidatorBackend::SparseSlab, |_| {
                                    PolygonDissectionValidatorBackend::DenseArrangement
                                })
                        },
                        polygon_dissection_validator_kind,
                    ),
                    subdivision_builder_backend: subdivision_builder.map_or(
                        SubdivisionBuilderBackend::OrthogonalSweep,
                        subdivision_builder_kind,
                    ),
                    sparse_validator_backend: sparse_validator.map_or(
                        SparseValidatorBackend::EventSegmentTree,
                        sparse_validator_kind,
                    ),
                    arrangement_backend: polygon_arrangement
                        .map_or(PolygonArrangementBackend::Indexed, polygon_arrangement_kind),
                    representation: representation_kind(
                        representation.unwrap_or(RepresentationArg::Dominance4d),
                    ),
                },
            )
            .map_err(|error| CliError::Solver(error.to_string()))?;
            if let Some(svg_path) = svg {
                write_text(svg_path, &render_polygon_dissection_svg(&polygon, &result)?)?;
            }
            write_json(
                &PolygonSolveOutput {
                    solver: format!("{solver:?}"),
                    input_model: "rectilinear-polygon",
                    polygon,
                    result,
                },
                output,
            )
        }
        LoadedInput::FormalPolygon(polygon) => {
            if !matches!(
                solver,
                SolverArg::DominanceCompressed | SolverArg::DominanceCompactOnly
            ) {
                return Err(CliError::UnsupportedSolverForFormalPolygon { solver });
            }
            if chord_enumerator.is_some()
                || completion_backend.is_some()
                || representation.is_some()
                || region_dual.is_some()
                || path_tree_orientation.is_some()
                || polygon_geometry.is_some()
                || polygon_validator.is_some()
                || polygon_chords.is_some()
                || polygon_completion.is_some()
                || polygon_arrangement.is_some()
                || polygon_cut_index.is_some()
                || polygon_recovery.is_some()
                || polygon_dissection_validator.is_some()
                || subdivision_builder.is_some()
                || sparse_validator.is_some()
            {
                return Err(CliError::Input(
                    "formal polygon solving uses the source-fixed formal chord, matching, completion, recovery, and validation pipeline"
                        .to_owned(),
                ));
            }
            let analysis = complete_formal_polygon(&polygon)
                .map_err(|error| CliError::Solver(error.to_string()))?;
            if let Some(svg_path) = svg {
                let rendered = PolygonDissectionResult {
                    optimum_rectangle_count: analysis.admissible.optimum_rectangle_count,
                    rectangles: analysis.completion.rectangles.clone(),
                    diagnostics: rect_core::Diagnostics::default(),
                    certificate: None,
                };
                write_text(
                    svg_path,
                    &render_polygon_dissection_svg(polygon.region(), &rendered)?,
                )?;
            }
            let admissible = analysis.admissible;
            write_json(
                &FormalSolveOutput {
                    solver: format!("{solver:?}"),
                    input_model: "formal-rectilinear-polygon",
                    polygon,
                    local_nonconvexity_measure: admissible.local_nonconvexity_measure,
                    interior_component_count: admissible.interior_component_count,
                    formal_hole_count: admissible.formal_hole_count,
                    effective_number: admissible.effective_number,
                    optimum_rectangle_count: admissible.optimum_rectangle_count,
                    effective_chords: admissible.families,
                    step_two_transformation: admissible.transformation,
                    explicit_matching: admissible.explicit_matching,
                    explicit_vertex_cover: admissible.explicit_vertex_cover,
                    compact_vertex_cover: admissible.compact_vertex_cover,
                    selected_horizontal: admissible.selected_horizontal,
                    selected_vertical: admissible.selected_vertical,
                    completion: analysis.completion,
                },
                output,
            )
        }
    }
}

fn solve_component<C>(
    component: &GridComponent<C>,
    solver: SolverArg,
    chord_enumerator: Option<ChordEnumeratorArg>,
    completion_backend: Option<CompletionBackendArg>,
    representation: Option<RepresentationArg>,
    region_dual: Option<RegionDualArg>,
    path_tree_orientation: Option<PathTreeOrientationArg>,
) -> Result<DissectionResult, CliError> {
    let completion_backend = completion_backend.map(completion_backend_kind);
    match solver {
        SolverArg::ExactCover => {
            if chord_enumerator.is_some()
                || completion_backend.is_some()
                || representation.is_some()
                || region_dual.is_some()
                || path_tree_orientation.is_some()
            {
                return Err(CliError::Input(
                    "chord, completion, and representation options apply only to dominance solvers"
                        .to_owned(),
                ));
            }
            rect_oracle_exact_cover::solve(component)
                .map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::SgExplicit => {
            if chord_enumerator.is_some()
                || completion_backend.is_some()
                || representation.is_some()
                || region_dual.is_some()
                || path_tree_orientation.is_some()
            {
                return Err(CliError::Input(
                    "chord, completion, and representation options apply only to dominance solvers"
                        .to_owned(),
                ));
            }
            rect_oracle_sg::solve(component).map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::DominanceC0 => {
            if chord_enumerator.is_some()
                || completion_backend.is_some()
                || representation.is_some()
                || region_dual.is_some()
                || path_tree_orientation.is_some()
            {
                return Err(CliError::Input(
                    "chord, completion, and representation options apply only to dominance solvers"
                        .to_owned(),
                ));
            }
            rect_dominance::solve(component, DominanceMode::ExplicitEdges)
                .map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::DominanceCompressed => {
            solve_with_representation_and_region_dual_and_orientation_policy(
                component,
                VerificationMode::FullyAudited,
                representation_kind(representation.unwrap_or(RepresentationArg::Dominance4d)),
                dominance_enumerator(
                    chord_enumerator.unwrap_or(ChordEnumeratorArg::ReferencePairwise),
                ),
                completion_backend.unwrap_or(CompletionBackendKind::ReferenceRescan),
                region_dual.map_or(RegionDualBackend::ReferenceAreaFloodFill, region_dual_kind),
                path_tree_orientation.map_or(
                    PathTreeOrientationPolicy::BuildBothExact,
                    path_tree_orientation_kind,
                ),
            )
            .map_err(|error| CliError::Solver(error.to_string()))
        }
        SolverArg::DominanceCompactOnly => {
            solve_with_representation_and_region_dual_and_orientation_policy(
                component,
                VerificationMode::CompactOnly,
                representation_kind(representation.unwrap_or(RepresentationArg::Dominance4d)),
                dominance_enumerator(
                    chord_enumerator.unwrap_or(ChordEnumeratorArg::GridInteriorRuns),
                ),
                completion_backend.unwrap_or(CompletionBackendKind::IndexedFrontier),
                region_dual.map_or(RegionDualBackend::BoundaryLaminar, region_dual_kind),
                path_tree_orientation.map_or(
                    PathTreeOrientationPolicy::BuildBothExact,
                    path_tree_orientation_kind,
                ),
            )
            .map_err(|error| CliError::Solver(error.to_string()))
        }
    }
}

const fn completion_backend_kind(backend: CompletionBackendArg) -> CompletionBackendKind {
    match backend {
        CompletionBackendArg::ReferenceRescan => CompletionBackendKind::ReferenceRescan,
        CompletionBackendArg::IndexedFrontier => CompletionBackendKind::IndexedFrontier,
    }
}

const fn polygon_geometry_kind(backend: PolygonGeometryArg) -> PolygonGeometryBackend {
    match backend {
        PolygonGeometryArg::ReferenceScan => PolygonGeometryBackend::ReferenceScan,
        PolygonGeometryArg::Indexed => PolygonGeometryBackend::Indexed,
    }
}

const fn polygon_validator_kind(backend: PolygonValidatorArg) -> PolygonValidationBackend {
    match backend {
        PolygonValidatorArg::ReferenceQuadratic => PolygonValidationBackend::ReferenceQuadratic,
        PolygonValidatorArg::OrthogonalSweep => PolygonValidationBackend::OrthogonalSweep,
    }
}

const fn polygon_chords_kind(backend: PolygonChordsArg) -> PolygonChordBackend {
    match backend {
        PolygonChordsArg::ReferencePairwise => PolygonChordBackend::ReferencePairwise,
        PolygonChordsArg::IndexedPairwise => PolygonChordBackend::IndexedPairwise,
        PolygonChordsArg::SgSweep => PolygonChordBackend::SoltanGorpinevichSweep,
    }
}

const fn polygon_completion_kind(backend: PolygonCompletionArg) -> PolygonCompletionBackend {
    match backend {
        PolygonCompletionArg::CoordinateReference => PolygonCompletionBackend::CoordinateReference,
        PolygonCompletionArg::IndexedFrontier => PolygonCompletionBackend::IndexedFrontier,
    }
}

const fn polygon_arrangement_kind(backend: PolygonArrangementArg) -> PolygonArrangementBackend {
    match backend {
        PolygonArrangementArg::Reference => PolygonArrangementBackend::Reference,
        PolygonArrangementArg::Indexed => PolygonArrangementBackend::Indexed,
    }
}

const fn polygon_cut_index_kind(backend: PolygonCutIndexArg) -> PolygonCutIndexBackend {
    match backend {
        PolygonCutIndexArg::LineMapReference => PolygonCutIndexBackend::ReferenceLineMaps,
        PolygonCutIndexArg::DynamicStabbing => PolygonCutIndexBackend::DynamicStabbing,
    }
}

const fn polygon_recovery_kind(backend: PolygonRecoveryArg) -> PolygonRecoveryBackend {
    match backend {
        PolygonRecoveryArg::DenseArrangement => PolygonRecoveryBackend::DenseCoordinateArrangement,
        PolygonRecoveryArg::SparseSubdivision => PolygonRecoveryBackend::SparseSubdivision,
        PolygonRecoveryArg::Auto => PolygonRecoveryBackend::Auto,
    }
}

const fn polygon_dissection_validator_kind(
    backend: PolygonDissectionValidatorArg,
) -> PolygonDissectionValidatorBackend {
    match backend {
        PolygonDissectionValidatorArg::DenseArrangement => {
            PolygonDissectionValidatorBackend::DenseArrangement
        }
        PolygonDissectionValidatorArg::SparseSlab => PolygonDissectionValidatorBackend::SparseSlab,
    }
}

const fn subdivision_builder_kind(backend: SubdivisionBuilderArg) -> SubdivisionBuilderBackend {
    match backend {
        SubdivisionBuilderArg::ReferenceRangeScan => SubdivisionBuilderBackend::ReferenceRangeScan,
        SubdivisionBuilderArg::OrthogonalSweep => SubdivisionBuilderBackend::OrthogonalSweep,
    }
}

const fn sparse_validator_kind(backend: SparseValidatorArg) -> SparseValidatorBackend {
    match backend {
        SparseValidatorArg::ReferenceSlabRescan => SparseValidatorBackend::ReferenceSlabRescan,
        SparseValidatorArg::EventSegmentTree => SparseValidatorBackend::EventSegmentTree,
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

const fn path_tree_orientation_kind(
    orientation: PathTreeOrientationArg,
) -> PathTreeOrientationPolicy {
    match orientation {
        PathTreeOrientationArg::BuildBoth => PathTreeOrientationPolicy::BuildBothExact,
        PathTreeOrientationArg::BoundEstimate => PathTreeOrientationPolicy::BoundEstimate,
        PathTreeOrientationArg::VerticalTree => PathTreeOrientationPolicy::VerticalTree,
        PathTreeOrientationArg::HorizontalTree => PathTreeOrientationPolicy::HorizontalTree,
    }
}

enum LoadedInput {
    Grid(ColorGrid<Value>),
    Polygon(RectilinearPolygon),
    FormalPolygon(FormalRectilinearPolygon),
}

fn load_input(path: &Path, format: InputFormatArg) -> Result<LoadedInput, CliError> {
    let bytes = fs::read(path)?;
    let value: Value = serde_json::from_slice(&bytes)?;
    let detected_format = match value.get("type").and_then(Value::as_str) {
        None => InputFormatArg::Grid,
        Some("rectilinear-polygon") => InputFormatArg::Polygon,
        Some("formal-rectilinear-polygon") => InputFormatArg::FormalPolygon,
        Some(kind) => return Err(CliError::Input(format!("unsupported input type {kind}"))),
    };
    if format != InputFormatArg::Auto && format != detected_format {
        return Err(CliError::Input(format!(
            "input detected as {detected_format:?}, but --input-format requires {format:?}"
        )));
    }
    match detected_format {
        InputFormatArg::Grid => {
            let input: JsonGrid = serde_json::from_value(value)?;
            ColorGrid::new(input.width, input.height, input.cells)
                .map(LoadedInput::Grid)
                .map_err(|error| CliError::Input(error.to_string()))
        }
        InputFormatArg::Polygon => {
            let input: JsonPolygon = serde_json::from_value(value)?;
            debug_assert_eq!(input.kind, "rectilinear-polygon");
            polygon_from_coordinates(input.outer, input.holes)
                .map(LoadedInput::Polygon)
                .map_err(|error| CliError::Input(error.to_string()))
        }
        InputFormatArg::FormalPolygon => {
            let input: JsonFormalPolygon = serde_json::from_value(value)?;
            debug_assert_eq!(input.kind, "formal-rectilinear-polygon");
            let region = polygon_from_coordinates(input.outer, input.holes)
                .map_err(|error| CliError::Input(error.to_string()))?;
            let isolated_points = input
                .ornament
                .isolated_points
                .into_iter()
                .map(|[x, y]| Point::new(x, y))
                .collect();
            let segments = input
                .ornament
                .segments
                .into_iter()
                .map(|[[x0, y0], [x1, y1]]| OrnamentSegment {
                    start: Point::new(x0, y0),
                    end: Point::new(x1, y1),
                })
                .collect();
            FormalRectilinearPolygon::new(
                region,
                Ornament {
                    isolated_points,
                    segments,
                },
            )
            .map(LoadedInput::FormalPolygon)
            .map_err(|error| CliError::Input(error.to_string()))
        }
        InputFormatArg::Auto => unreachable!("auto input format is resolved before parsing"),
    }
}

fn polygon_from_coordinates(
    outer: Vec<[i64; 2]>,
    holes: Vec<Vec<[i64; 2]>>,
) -> Result<RectilinearPolygon, rect_core::PolygonError> {
    let outer = OrthogonalLoop::new(outer.into_iter().map(|[x, y]| Point::new(x, y)).collect());
    let holes = holes
        .into_iter()
        .map(|vertices| {
            OrthogonalLoop::new(
                vertices
                    .into_iter()
                    .map(|[x, y]| Point::new(x, y))
                    .collect(),
            )
        })
        .collect();
    RectilinearPolygon::new(outer, holes)
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
        command: std::iter::once("rect-cli".to_owned())
            .chain(std::env::args().skip(1))
            .collect::<Vec<_>>()
            .join(" "),
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
            schema_version: 3,
            runs: Vec::new(),
            historical_release_metadata: None,
            release_summaries: None,
            current_release: None,
            generated_tables: None,
        }
    };
    manifest.schema_version = 3;
    manifest.runs.retain(|run| run.command != metadata.command);
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
    #[error("solver {solver:?} is unavailable for boundary-native polygon input")]
    UnsupportedSolverForPolygon { solver: SolverArg },
    #[error("solver {solver:?} is unavailable for formal polygon input")]
    UnsupportedSolverForFormalPolygon { solver: SolverArg },
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

    use clap::Parser;
    use serde_json::Value;

    use super::{
        An19AdversarialFamilyArg, An19EventEngineArg, ChordEnumeratorArg, Cli, Command,
        CompletionBackendArg, InputFormatArg, LoadedInput, PathTreeOrientationArg,
        PolygonArrangementArg, PolygonChordsArg, PolygonCompletionArg, PolygonGeometryArg,
        PolygonValidatorArg, RegionDualArg, RepresentationArg, SolverArg, load_input,
        solve_command,
    };

    #[test]
    fn an19_event_cli_exposes_exact_backends_and_rejects_unproved_backend() {
        let cli = Cli::try_parse_from([
            "rect-cli",
            "an19-events",
            "--an19-event-engine",
            "exact-oracle",
            "--an19-adversarial-family",
            "highway-halving-reorder",
            "--an19-adversarial-size",
            "16,32",
            "--output",
            "campaign.json",
            "--markdown",
            "campaign.md",
        ])
        .unwrap();
        let Command::An19Events {
            an19_event_engine,
            an19_adversarial_family,
            an19_adversarial_size,
            ..
        } = cli.command
        else {
            panic!("wrong command parsed");
        };
        assert_eq!(an19_event_engine, An19EventEngineArg::ExactOracle);
        assert_eq!(
            an19_adversarial_family,
            An19AdversarialFamilyArg::HighwayHalvingReorder
        );
        assert_eq!(an19_adversarial_size, "16,32");
        assert!(
            Cli::try_parse_from([
                "rect-cli",
                "an19-events",
                "--an19-event-engine",
                "proved-unavailable",
                "--output",
                "campaign.json",
                "--markdown",
                "campaign.md",
            ])
            .is_err()
        );
    }

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
            InputFormatArg::Grid,
            Some(ChordEnumeratorArg::GridInteriorRuns),
            Some(CompletionBackendArg::IndexedFrontier),
            Some(RepresentationArg::Dominance4d),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
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

    #[test]
    fn compact_path_tree_svg_uses_axis_view_without_transpose() {
        let root = std::env::temp_dir().join(format!(
            "mrd-compact-path-tree-svg-regression-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("input.json");
        let output = root.join("output.json");
        let svg = root.join("output.svg");
        fs::write(
            &input,
            br#"{"width":3,"height":3,"cells":["a","a","a","a","a","a","a","a","a"]}"#,
        )
        .unwrap();
        solve_command(
            SolverArg::DominanceCompactOnly,
            InputFormatArg::Grid,
            Some(ChordEnumeratorArg::GridInteriorRuns),
            Some(CompletionBackendArg::IndexedFrontier),
            Some(RepresentationArg::PathTree),
            Some(RegionDualArg::BoundaryLaminar),
            Some(PathTreeOrientationArg::HorizontalTree),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            input.as_path(),
            Some(output.as_path()),
            Some(svg.as_path()),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        let diagnostics = &value["components"][0]["result"]["diagnostics"];
        assert_eq!(diagnostics["region_dual_backend"], "boundary-laminar");
        assert_eq!(
            diagnostics["path_tree_orientation_policy"],
            "horizontal-tree"
        );
        assert_eq!(
            diagnostics["execution_trace"]["prepared_occupancy_transposed"],
            false
        );
        assert_eq!(
            diagnostics["execution_trace"]["area_flood_fill_dual_built"],
            false
        );
        assert!(!fs::read(&svg).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn polygon_json_auto_detection_solves_and_renders_without_rasterization() {
        let root =
            std::env::temp_dir().join(format!("mrd-polygon-cli-regression-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("polygon.json");
        let output = root.join("output.json");
        let svg = root.join("output.svg");
        fs::write(
            &input,
            br#"{"type":"rectilinear-polygon","outer":[[0,0],[1000000000,0],[1000000000,1],[1,1],[1,4],[0,4]],"holes":[]}"#,
        )
        .unwrap();
        solve_command(
            SolverArg::DominanceCompactOnly,
            InputFormatArg::Auto,
            None,
            None,
            Some(RepresentationArg::Dominance4d),
            None,
            None,
            Some(PolygonGeometryArg::Indexed),
            Some(PolygonValidatorArg::OrthogonalSweep),
            Some(PolygonChordsArg::IndexedPairwise),
            Some(PolygonCompletionArg::IndexedFrontier),
            None,
            None,
            None,
            None,
            None,
            None,
            input.as_path(),
            Some(output.as_path()),
            Some(svg.as_path()),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        assert_eq!(value["input_model"], "rectilinear-polygon");
        assert_eq!(value["result"]["optimum_rectangle_count"], 2);
        assert_eq!(value["result"]["diagnostics"]["raster_oracle_used"], false);
        let diagnostics = &value["result"]["diagnostics"];
        assert_eq!(diagnostics["atomic_cell_count"], 0);
        assert_eq!(
            diagnostics["execution_trace"]["dense_atomic_cells_materialized"],
            false
        );
        assert!(
            diagnostics["sparse_subdivision_vertex_count"]
                .as_u64()
                .is_some_and(|count| count > 0)
        );
        assert!(!fs::read(&svg).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn formal_polygon_json_auto_detection_builds_canonical_incidence() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let path = workspace
            .join("test-data")
            .join("polygons")
            .join("formal-boundary.json");
        let LoadedInput::FormalPolygon(polygon) = load_input(&path, InputFormatArg::Auto).unwrap()
        else {
            panic!("formal fixture did not parse as a formal polygon");
        };
        let incidence = polygon.incidence().unwrap();
        assert_eq!(incidence.formal_holes().count(), 4);
        assert_eq!(
            polygon.ornament().isolated_points,
            [rect_core::Point::new(6, 6)]
        );
        assert!(
            polygon
                .ornament()
                .segments
                .iter()
                .all(|segment| segment.start < segment.end)
        );
        let serialized = serde_json::to_string(&polygon).unwrap();
        let round_trip: rect_core::FormalRectilinearPolygon =
            serde_json::from_str(&serialized).unwrap();
        assert_eq!(round_trip, polygon);
    }

    #[test]
    fn formal_fixtures_solve_through_the_production_cli() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let fixture_dir = workspace.join("test-data").join("polygons").join("formal");
        let root =
            std::env::temp_dir().join(format!("mrd-formal-cli-regression-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for name in [
            "point-hole",
            "segment-hole",
            "attached-hole",
            "shared-endpoint",
            "source-figure-three",
        ] {
            let output = root.join(format!("{name}.json"));
            let svg = root.join(format!("{name}.svg"));
            solve_command(
                SolverArg::DominanceCompactOnly,
                InputFormatArg::Auto,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                &fixture_dir.join(format!("{name}.json")),
                Some(&output),
                Some(&svg),
            )
            .unwrap_or_else(|error| panic!("formal fixture {name} failed: {error}"));
            let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
            assert_eq!(value["input_model"], "formal-rectilinear-polygon");
            let m = value["local_nonconvexity_measure"].as_u64().unwrap();
            let c = value["interior_component_count"].as_u64().unwrap();
            let h = value["formal_hole_count"].as_u64().unwrap();
            let e = value["effective_number"].as_u64().unwrap();
            let optimum = value["optimum_rectangle_count"].as_u64().unwrap();
            assert_eq!(optimum, m + c - h - e, "fixture {name}");
            assert_eq!(
                value["completion"]["rectangles"].as_array().unwrap().len() as u64,
                optimum,
                "fixture {name}"
            );
            assert_eq!(
                value["explicit_vertex_cover"], value["compact_vertex_cover"],
                "fixture {name}"
            );
            assert!(!fs::read(&svg).unwrap().is_empty());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn polygon_sg_sweep_compact_svg_keeps_pairwise_work_disabled() {
        let root = std::env::temp_dir().join(format!(
            "mrd-polygon-sg-sweep-svg-regression-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let input = root.join("polygon.json");
        let output = root.join("output.json");
        let svg = root.join("output.svg");
        fs::write(
            &input,
            br#"{"type":"rectilinear-polygon","outer":[[0,0],[8,0],[8,8],[5,8],[5,3],[3,3],[3,8],[0,8]],"holes":[]}"#,
        )
        .unwrap();
        solve_command(
            SolverArg::DominanceCompactOnly,
            InputFormatArg::Polygon,
            None,
            None,
            Some(RepresentationArg::Dominance4d),
            None,
            None,
            Some(PolygonGeometryArg::Indexed),
            Some(PolygonValidatorArg::OrthogonalSweep),
            Some(PolygonChordsArg::SgSweep),
            Some(PolygonCompletionArg::IndexedFrontier),
            Some(PolygonArrangementArg::Indexed),
            None,
            None,
            None,
            None,
            None,
            input.as_path(),
            Some(output.as_path()),
            Some(svg.as_path()),
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
        let result = &value["result"];
        let diagnostics = &result["diagnostics"];
        assert_eq!(diagnostics["polygon_chord_enumerator"], "sg-sweep");
        for key in [
            "sweep_aligned_pair_iterations",
            "sweep_all_pair_iterations",
            "sweep_definition7_fallback_checks",
            "sweep_full_boundary_scans",
            "sweep_duplicate_output_count",
        ] {
            assert_eq!(diagnostics[key], 0, "sweep contract field {key}");
        }
        assert!(result["certificate"]["payload"]["sweep_certificate"].is_object());
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

    #[test]
    fn native_polygon_fixture_corpus_validates_and_solves() {
        let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        for name in [
            "nonuniform-l.json",
            "large-gap.json",
            "two-holes.json",
            "comb.json",
            "spiral-corridor.json",
            "scaled-complete-bipartite.json",
            "reflex-heavy-stretched.json",
        ] {
            let path = workspace.join("test-data/polygons").join(name);
            let LoadedInput::Polygon(polygon) = load_input(&path, InputFormatArg::Polygon).unwrap()
            else {
                panic!("fixture {name} did not parse as a polygon");
            };
            let formal = rect_core::FormalRectilinearPolygon::new(
                polygon.clone(),
                rect_core::Ornament::default(),
            )
            .unwrap();
            assert_eq!(formal.region(), &polygon);
            assert_eq!(
                formal.incidence().unwrap().formal_holes().count(),
                polygon.holes.len()
            );
            assert_eq!(
                formal.incidence().unwrap().elementary_segments.len(),
                polygon.boundary_complexity()
            );
            let result = rect_dominance::solve_polygon(&polygon)
                .unwrap_or_else(|error| panic!("fixture {name} failed: {error}"));
            assert_eq!(
                result.diagnostics.polygon_chord_enumerator.as_deref(),
                Some("sg-sweep")
            );
            if name == "scaled-complete-bipartite.json" {
                let families = rect_oracle_sg::GeneralPolygonPairwiseEnumerator
                    .enumerate(&polygon)
                    .unwrap();
                assert_eq!(families.horizontal.len(), 4);
                assert_eq!(families.vertical.len(), 4);
                assert!(families.horizontal.iter().all(|&horizontal| {
                    families
                        .vertical
                        .iter()
                        .all(|&vertical| rect_core::closed_chords_intersect(horizontal, vertical))
                }));
                let auto = rect_dominance::solve_polygon_with_representation(
                    &polygon,
                    rect_dominance::ConflictRepresentationBackend::Auto,
                )
                .unwrap();
                assert_eq!(
                    auto.diagnostics.conflict_representation.as_deref(),
                    Some("path-tree")
                );
            }
            if name == "reflex-heavy-stretched.json" {
                assert!(result.diagnostics.reflex_vertex_count >= 8);
                assert_eq!(result.diagnostics.atomic_cell_count, Some(0));
                assert!(
                    result
                        .diagnostics
                        .sparse_subdivision_vertex_count
                        .is_some_and(|count| count >= 10)
                );
                assert!(
                    !result
                        .diagnostics
                        .execution_trace
                        .dense_atomic_cells_materialized
                );
            }
        }
    }
}
