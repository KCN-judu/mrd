//! Reproducible, paired empirical scaling samples for the paper.
//!
//! This module deliberately contains no process orchestration or statistical
//! fitting. It builds one deterministic grid component, executes exactly one
//! named solver path, and emits a versioned sample with nanosecond phase
//! timings. The Python runner is responsible for fresh processes, pairing,
//! censoring, and aggregate analysis.

use std::collections::BTreeMap;
use std::time::Instant;

use dominance::biclique::Partition;
use dominance::compressed_flow::experiment as compressed_flow;
use dominance::embedding::{DominanceEmbedding, EmbeddingCoordinateBackend};
use graph::{DinicBackend, hopcroft_karp, minimum_vertex_cover};
use mrd_domain::{
    Boundary, Diagnostics, DissectionResult, GridComponent, GridRect, PreparedComponentContext,
    validate_dissection_prepared,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::adversarial::{
    AdversarialInstance, alternating_notch_corridor, clean_complete_bipartite_grid, comb,
    dense_conflict_grid, orthogonal_spiral, staircase,
};

const SCHEMA_VERSION: u32 = 1;
const GENERATOR_VERSION: &str = "paper-scaling-families-v1";

/// Named exact solver paths exposed by the paper campaign.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Algorithm {
    CompactMrd,
    ExplicitHopcroftKarp,
    ExplicitC0Flow,
    ExactCoverOracle,
}

impl Algorithm {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CompactMrd => "compact-mrd",
            Self::ExplicitHopcroftKarp => "explicit-hopcroft-karp",
            Self::ExplicitC0Flow => "explicit-c0-flow",
            Self::ExactCoverOracle => "exact-cover-oracle",
        }
    }
}

/// Predeclared instance families. The family-to-instance mapping is part of
/// the schema so a later result cannot silently change the population.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Family {
    RandomConnected,
    DenseConflict,
    SparseConflict,
    CombStaircase,
    SupportedHoles,
    Polyomino,
    RepresentationCrossover,
}

impl Family {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RandomConnected => "random-connected",
            Self::DenseConflict => "dense-conflict",
            Self::SparseConflict => "sparse-conflict",
            Self::CombStaircase => "comb-staircase",
            Self::SupportedHoles => "supported-holes",
            Self::Polyomino => "polyomino",
            Self::RepresentationCrossover => "representation-crossover",
        }
    }
}

fn default_oracle_cell_limit() -> usize {
    40
}

/// One deterministic request executed by the release CLI.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub schema_version: u32,
    pub family: Family,
    pub target_size: usize,
    pub seed: u64,
    pub algorithm: Algorithm,
    #[serde(default = "default_oracle_cell_limit")]
    pub oracle_cell_limit: usize,
}

/// Outcome generated inside the solver process. Python adds `timeout` when a
/// process is censored before it can emit this object.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Success,
    Unsupported,
    Error,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PhaseTimings {
    pub input_loading_ns: Option<u128>,
    pub instance_generation_ns: Option<u128>,
    pub geometry_preprocessing_ns: Option<u128>,
    pub chord_generation_ns: Option<u128>,
    pub embedding_ns: Option<u128>,
    pub explicit_conflict_graph_ns: Option<u128>,
    pub biclique_construction_ns: Option<u128>,
    pub network_construction_ns: Option<u128>,
    pub matching_or_flow_ns: Option<u128>,
    pub vertex_cover_recovery_ns: Option<u128>,
    pub chord_selection_ns: Option<u128>,
    pub geometric_completion_ns: Option<u128>,
    pub rectangle_recovery_ns: Option<u128>,
    pub verification_ns: Option<u128>,
    pub total_in_process_solve_ns: Option<u128>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct SizeMeasures {
    pub width: usize,
    pub height: usize,
    pub foreground_cells_n: usize,
    pub component_count: usize,
    pub boundary_size_b: Option<usize>,
    pub reflex_count: Option<usize>,
    pub horizontal_chord_count_h: Option<usize>,
    pub vertical_chord_count_v: Option<usize>,
    pub q: Option<usize>,
    pub explicit_conflict_edge_count_k: Option<usize>,
    pub biclique_count: Option<usize>,
    pub biclique_total_vertex_occurrences_sigma: Option<usize>,
    pub compressed_network_node_count: Option<usize>,
    pub compressed_network_arc_count: Option<usize>,
    pub optimum_rectangle_count: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralCounters {
    pub rank_sort_count: Option<usize>,
    pub rank_map_entry_count: Option<usize>,
    pub rank_map_owned_bytes: Option<usize>,
    pub matching_size: Option<usize>,
    pub vertex_cover_size: Option<usize>,
    pub c0_network_node_count: Option<usize>,
    pub c0_network_arc_count: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub schema_version: u32,
    pub generator_version: String,
    pub family: Family,
    pub algorithm: Algorithm,
    pub solver_provenance: String,
    pub seed: u64,
    pub target_size: usize,
    pub generation_attempts: usize,
    pub outcome: Outcome,
    pub sizes: SizeMeasures,
    pub structure: StructuralCounters,
    pub timings: PhaseTimings,
    pub optimum_rectangle_count: Option<usize>,
    pub canonical_rectangles: Option<Vec<GridRect>>,
    pub correctness: String,
    pub message: Option<String>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("paper-scaling request schema version {actual} is not {SCHEMA_VERSION}")]
    SchemaVersion { actual: u32 },
    #[error("paper-scaling target size must be positive")]
    ZeroTarget,
    #[error("paper-scaling oracle cell limit must be positive")]
    ZeroOracleLimit,
    #[error("instance generator failed: {0}")]
    Generator(String),
}

#[derive(Clone, Debug)]
struct Generated {
    instance: AdversarialInstance,
    attempts: usize,
}

#[derive(Clone, Debug)]
struct Solved {
    result: DissectionResult,
    phases: PhaseTimings,
    structure: StructuralCounters,
    provenance: &'static str,
}

/// Executes one release-process sample.
///
/// Solver errors are represented as `Outcome::Error` so the process runner can
/// preserve them in raw output. Invalid requests still return an error before
/// a sample exists.
///
/// # Errors
///
/// Returns [`enum@Error`] for an invalid schema, target, Oracle limit, or generated
/// instance.
#[allow(clippy::too_many_lines)]
pub fn run(request: &Request) -> Result<Sample, Error> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(Error::SchemaVersion {
            actual: request.schema_version,
        });
    }
    if request.target_size == 0 {
        return Err(Error::ZeroTarget);
    }
    if request.oracle_cell_limit == 0 {
        return Err(Error::ZeroOracleLimit);
    }

    let generation_started = Instant::now();
    let generated = generate(request.family, request.target_size, request.seed)?;
    let generation_ns = generation_started.elapsed().as_nanos();
    let grid = generated
        .instance
        .grid()
        .map_err(|error| Error::Generator(error.to_string()))?;
    let components = grid.four_connected_components();
    let foreground = components
        .iter()
        .filter(|component| component.color)
        .cloned()
        .collect::<Vec<_>>();
    let sizes = base_sizes(&generated.instance, &foreground);
    let Some(component) = foreground.first() else {
        return Ok(error_sample(
            request,
            &generated,
            generation_ns,
            sizes,
            "generator produced no foreground component",
        ));
    };
    if foreground.len() != 1 {
        return Ok(error_sample(
            request,
            &generated,
            generation_ns,
            sizes,
            "benchmark family must produce exactly one foreground component",
        ));
    }

    if request.algorithm == Algorithm::ExactCoverOracle
        && component.cell_count() > request.oracle_cell_limit
    {
        return Ok(Sample {
            schema_version: SCHEMA_VERSION,
            generator_version: GENERATOR_VERSION.to_owned(),
            family: request.family,
            algorithm: request.algorithm,
            solver_provenance:
                "exact-cover-oracle::solve (bitset branch-and-bound; predeclared cell limit)"
                    .to_owned(),
            seed: request.seed,
            target_size: request.target_size,
            generation_attempts: generated.attempts,
            outcome: Outcome::Unsupported,
            sizes,
            structure: StructuralCounters::default(),
            timings: PhaseTimings {
                input_loading_ns: Some(0),
                instance_generation_ns: Some(generation_ns),
                ..PhaseTimings::default()
            },
            optimum_rectangle_count: None,
            canonical_rectangles: None,
            correctness: "unsupported".to_owned(),
            message: Some(format!(
                "foreground cell count {} exceeds oracle limit {}",
                component.cell_count(),
                request.oracle_cell_limit
            )),
        });
    }

    let solve_started = Instant::now();
    let solved = match solve_algorithm(request.algorithm, component) {
        Ok(solved) => solved,
        Err(error) => {
            return Ok(error_sample(
                request,
                &generated,
                generation_ns,
                sizes,
                &error,
            ));
        }
    };
    let mut sample_sizes = sizes;
    sample_sizes.horizontal_chord_count_h = Some(solved.result.diagnostics.horizontal_chord_count);
    sample_sizes.vertical_chord_count_v = Some(solved.result.diagnostics.vertical_chord_count);
    sample_sizes.q = Some(solved.result.diagnostics.total_chord_count);
    sample_sizes.explicit_conflict_edge_count_k =
        solved.result.diagnostics.explicit_conflict_edge_count;
    sample_sizes.biclique_count = Some(solved.result.diagnostics.biclique_count);
    sample_sizes.biclique_total_vertex_occurrences_sigma =
        Some(solved.result.diagnostics.biclique_total_vertex_occurrences);
    sample_sizes.compressed_network_node_count =
        nonzero_or_none(solved.result.diagnostics.compressed_network_vertex_count);
    sample_sizes.compressed_network_arc_count =
        nonzero_or_none(solved.result.diagnostics.compressed_network_arc_count);
    sample_sizes.optimum_rectangle_count = Some(solved.result.optimum_rectangle_count);
    let mut phases = solved.phases;
    phases.instance_generation_ns = Some(generation_ns);
    phases.input_loading_ns = Some(0);
    phases.total_in_process_solve_ns = Some(solve_started.elapsed().as_nanos());
    Ok(Sample {
        schema_version: SCHEMA_VERSION,
        generator_version: GENERATOR_VERSION.to_owned(),
        family: request.family,
        algorithm: request.algorithm,
        solver_provenance: solved.provenance.to_owned(),
        seed: request.seed,
        target_size: request.target_size,
        generation_attempts: generated.attempts,
        outcome: Outcome::Success,
        sizes: sample_sizes,
        structure: solved.structure,
        timings: phases,
        optimum_rectangle_count: Some(solved.result.optimum_rectangle_count),
        canonical_rectangles: Some(canonical_rectangles(&solved.result.rectangles)),
        correctness: "valid".to_owned(),
        message: None,
    })
}

fn nonzero_or_none(value: usize) -> Option<usize> {
    (value != 0).then_some(value)
}

fn error_sample(
    request: &Request,
    generated: &Generated,
    generation_ns: u128,
    sizes: SizeMeasures,
    message: &str,
) -> Sample {
    Sample {
        schema_version: SCHEMA_VERSION,
        generator_version: GENERATOR_VERSION.to_owned(),
        family: request.family,
        algorithm: request.algorithm,
        solver_provenance: request.algorithm.name().to_owned(),
        seed: request.seed,
        target_size: request.target_size,
        generation_attempts: generated.attempts,
        outcome: Outcome::Error,
        sizes,
        structure: StructuralCounters::default(),
        timings: PhaseTimings {
            input_loading_ns: Some(0),
            instance_generation_ns: Some(generation_ns),
            ..PhaseTimings::default()
        },
        optimum_rectangle_count: None,
        canonical_rectangles: None,
        correctness: "error".to_owned(),
        message: Some(message.to_owned()),
    }
}

fn base_sizes(instance: &AdversarialInstance, components: &[GridComponent<bool>]) -> SizeMeasures {
    let foreground_cells_n = instance.cells.iter().filter(|&&cell| cell).count();
    let (boundary_size_b, reflex_count) = components
        .first()
        .and_then(|component| Boundary::from_component(component).ok())
        .map_or((None, None), |boundary| {
            (
                Some(boundary.boundary_complexity()),
                Some(boundary.reflex_vertices.len()),
            )
        });
    SizeMeasures {
        width: instance.width,
        height: instance.height,
        foreground_cells_n,
        component_count: components.len(),
        boundary_size_b,
        reflex_count,
        ..SizeMeasures::default()
    }
}

fn canonical_rectangles(rectangles: &[GridRect]) -> Vec<GridRect> {
    let mut canonical = rectangles.to_vec();
    canonical.sort_unstable();
    canonical
}

fn solve_algorithm(
    algorithm: Algorithm,
    component: &GridComponent<bool>,
) -> Result<Solved, String> {
    match algorithm {
        Algorithm::ExactCoverOracle => solve_exact_cover(component),
        Algorithm::ExplicitHopcroftKarp => solve_explicit_matching(component),
        Algorithm::ExplicitC0Flow => solve_c0_flow(component),
        Algorithm::CompactMrd => solve_compact(component),
    }
}

fn optimum(geometry: &sg_oracle::grid::Geometry, matching_size: usize) -> Result<usize, String> {
    let base = geometry
        .boundary
        .reflex_vertices
        .len()
        .checked_add(1)
        .and_then(|value| value.checked_sub(geometry.boundary.hole_count()))
        .ok_or_else(|| "MRD formula underflow while computing base count".to_owned())?;
    let independent = geometry
        .horizontal_chords
        .len()
        .checked_add(geometry.vertical_chords.len())
        .and_then(|value| value.checked_sub(matching_size))
        .ok_or_else(|| {
            "MRD formula underflow while computing independent chord count".to_owned()
        })?;
    base.checked_sub(independent)
        .ok_or_else(|| "MRD formula underflow while computing optimum".to_owned())
}

#[allow(clippy::too_many_arguments)]
fn finish(
    component: &GridComponent<bool>,
    geometry: &sg_oracle::grid::Geometry,
    selected_horizontal: &[bool],
    selected_vertical: &[bool],
    matching_size: usize,
    mut phases: PhaseTimings,
    structure: StructuralCounters,
    provenance: &'static str,
    backend: &impl sg_oracle::grid::GeometricCompletionBackend,
) -> Result<Solved, String> {
    let completion = sg_oracle::grid::complete_with_prepared_backend(
        component,
        &geometry.prepared,
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        selected_horizontal,
        selected_vertical,
        backend,
    )
    .map_err(|error| error.to_string())?;
    let completion_metrics = &completion.metrics;
    phases.geometric_completion_ns = Some(
        completion_metrics.selected_chord_cut_materialization_nanoseconds
            + completion_metrics.horizontal_simple_chord_completion_nanoseconds
            + completion_metrics.vertical_simple_chord_completion_nanoseconds,
    );
    phases.rectangle_recovery_ns = Some(completion_metrics.rectangle_recovery_nanoseconds);
    let declared = optimum(geometry, matching_size)?;
    if completion.rectangles.len() != declared {
        return Err(format!(
            "completion count {} differs from optimum {}",
            completion.rectangles.len(),
            declared
        ));
    }
    let result = DissectionResult {
        optimum_rectangle_count: declared,
        rectangles: completion.rectangles,
        diagnostics: Diagnostics::default(),
        certificate: None,
    };
    let validation_started = Instant::now();
    validate_dissection_prepared(&geometry.prepared, &result).map_err(|error| error.to_string())?;
    phases.verification_ns = Some(
        completion_metrics.final_output_validation_nanoseconds
            + validation_started.elapsed().as_nanos(),
    );
    Ok(Solved {
        result,
        phases,
        structure,
        provenance,
    })
}

fn solve_explicit_matching(component: &GridComponent<bool>) -> Result<Solved, String> {
    let started = Instant::now();
    let preprocessing_started = Instant::now();
    let context = PreparedComponentContext::new(component).map_err(|error| error.to_string())?;
    let preprocessing_ns = preprocessing_started.elapsed().as_nanos();
    let chord_started = Instant::now();
    let geometry = sg_oracle::grid::analyze_prepared_geometry(
        context,
        &sg_oracle::grid::experiment::InteriorRuns,
    )
    .map_err(|error| error.to_string())?;
    let chord_ns = chord_started.elapsed().as_nanos();
    let graph_started = Instant::now();
    let graph = sg_oracle::grid::build_conflict_graph(
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
    )
    .map_err(|error| error.to_string())?;
    let graph_ns = graph_started.elapsed().as_nanos();
    let matching_started = Instant::now();
    let matching = hopcroft_karp(&graph);
    let matching_ns = matching_started.elapsed().as_nanos();
    let cover_started = Instant::now();
    let cover = minimum_vertex_cover(&graph, &matching);
    let cover_ns = cover_started.elapsed().as_nanos();
    let selection_started = Instant::now();
    let selected_horizontal = cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selection_ns = selection_started.elapsed().as_nanos();
    let phases = PhaseTimings {
        geometry_preprocessing_ns: Some(preprocessing_ns),
        chord_generation_ns: Some(chord_ns),
        explicit_conflict_graph_ns: Some(graph_ns),
        matching_or_flow_ns: Some(matching_ns),
        vertex_cover_recovery_ns: Some(cover_ns),
        chord_selection_ns: Some(selection_ns),
        total_in_process_solve_ns: Some(started.elapsed().as_nanos()),
        ..PhaseTimings::default()
    };
    let structure = StructuralCounters {
        matching_size: Some(matching.size),
        vertex_cover_size: Some(cover.size),
        ..StructuralCounters::default()
    };
    let mut solved = finish(
        component,
        &geometry,
        &selected_horizontal,
        &selected_vertical,
        matching.size,
        phases,
        structure,
        "sg-oracle::grid::build_conflict_graph + Hopcroft-Karp + Konig cover + indexed completion",
        &sg_oracle::grid::IndexedFrontierCompletion,
    )?;
    solved.result.diagnostics.horizontal_chord_count = geometry.horizontal_chords.len();
    solved.result.diagnostics.vertical_chord_count = geometry.vertical_chords.len();
    solved.result.diagnostics.total_chord_count =
        geometry.horizontal_chords.len() + geometry.vertical_chords.len();
    solved.result.diagnostics.explicit_conflict_edge_count = Some(graph.edge_count());
    solved.result.diagnostics.maximum_matching_size = matching.size;
    solved.result.diagnostics.minimum_vertex_cover_size = cover.size;
    Ok(solved)
}

fn solve_c0_flow(component: &GridComponent<bool>) -> Result<Solved, String> {
    solve_dominance(component, false)
}

fn solve_compact(component: &GridComponent<bool>) -> Result<Solved, String> {
    solve_dominance(component, true)
}

#[allow(clippy::too_many_lines)]
fn solve_dominance(component: &GridComponent<bool>, compact: bool) -> Result<Solved, String> {
    let started = Instant::now();
    let preprocessing_started = Instant::now();
    let context = PreparedComponentContext::new(component).map_err(|error| error.to_string())?;
    let preprocessing_ns = preprocessing_started.elapsed().as_nanos();
    let chord_started = Instant::now();
    let geometry = sg_oracle::grid::analyze_prepared_geometry(
        context,
        &sg_oracle::grid::experiment::InteriorRuns,
    )
    .map_err(|error| error.to_string())?;
    let chord_ns = chord_started.elapsed().as_nanos();
    let embedding_started = Instant::now();
    let embedding = DominanceEmbedding::new_with_backend(
        &geometry.horizontal_chords,
        &geometry.vertical_chords,
        EmbeddingCoordinateBackend::DirectGridParity,
    )
    .map_err(|error| error.to_string())?;
    let embedding_ns = embedding_started.elapsed().as_nanos();
    let (partition, graph_ns, biclique_ns) = if compact {
        let partition_started = Instant::now();
        let partition = dominance::biclique::experiment::construct(&embedding)
            .map_err(|error| error.to_string())?
            .partition;
        partition
            .verify_dominance_blocks(&embedding)
            .map_err(|error| error.to_string())?;
        (
            partition,
            None,
            Some(partition_started.elapsed().as_nanos()),
        )
    } else {
        let graph_started = Instant::now();
        let graph = embedding
            .explicit_graph()
            .map_err(|error| error.to_string())?;
        let graph_ns = graph_started.elapsed().as_nanos();
        let partition_started = Instant::now();
        let partition = Partition::from_explicit_edges(&graph);
        partition
            .verify_exact_partition(&graph)
            .map_err(|error| error.to_string())?;
        (
            partition,
            Some(graph_ns),
            Some(partition_started.elapsed().as_nanos()),
        )
    };
    let network_started = Instant::now();
    let network = compressed_flow::construct_network(
        geometry.horizontal_chords.len(),
        geometry.vertical_chords.len(),
        &partition,
    )
    .map_err(|error| error.to_string())?;
    let network_ns = network_started.elapsed().as_nanos();
    let flow_started = Instant::now();
    let flow_result = compressed_flow::execute_flow(&network, &DinicBackend)
        .map_err(|error| error.to_string())?;
    let flow_ns = flow_started.elapsed().as_nanos();
    let cover_started = Instant::now();
    let flow = compressed_flow::recover_vertex_cover(&network, flow_result)
        .map_err(|error| error.to_string())?;
    let cover_ns = cover_started.elapsed().as_nanos();
    let selection_started = Instant::now();
    let selected_horizontal = flow
        .vertex_cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = flow
        .vertex_cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selection_ns = selection_started.elapsed().as_nanos();
    let explicit_edges = if compact {
        None
    } else {
        Some(partition.blocks.len())
    };
    let total_chords = geometry.horizontal_chords.len() + geometry.vertical_chords.len();
    let structure = StructuralCounters {
        rank_sort_count: Some(embedding.metrics.rank_sort_count),
        rank_map_entry_count: Some(embedding.metrics.rank_map_entry_count),
        rank_map_owned_bytes: Some(embedding.metrics.rank_map_owned_bytes),
        matching_size: Some(flow.vertex_cover.size),
        vertex_cover_size: Some(flow.vertex_cover.size),
        c0_network_node_count: (!compact)
            .then_some(2 + total_chords + explicit_edges.unwrap_or_default()),
        c0_network_arc_count: (!compact)
            .then_some(total_chords + explicit_edges.unwrap_or_default() * 2),
    };
    let phases = PhaseTimings {
        geometry_preprocessing_ns: Some(preprocessing_ns),
        chord_generation_ns: Some(chord_ns),
        embedding_ns: Some(embedding_ns),
        explicit_conflict_graph_ns: graph_ns,
        biclique_construction_ns: biclique_ns,
        network_construction_ns: Some(network_ns),
        matching_or_flow_ns: Some(flow_ns),
        vertex_cover_recovery_ns: Some(cover_ns),
        chord_selection_ns: Some(selection_ns),
        total_in_process_solve_ns: Some(started.elapsed().as_nanos()),
        ..PhaseTimings::default()
    };
    let provenance = if compact {
        "dominance::biclique::experiment::construct + compressed flow construction/execution/cover recovery + indexed completion"
    } else {
        "DominanceEmbedding::explicit_graph + C0 edge bicliques + explicit flow construction/execution/cover recovery + indexed completion"
    };
    let c0_network_node_count = structure.c0_network_node_count;
    let c0_network_arc_count = structure.c0_network_arc_count;
    let mut solved = finish(
        component,
        &geometry,
        &selected_horizontal,
        &selected_vertical,
        flow.vertex_cover.size,
        phases,
        structure,
        provenance,
        &sg_oracle::grid::IndexedFrontierCompletion,
    )?;
    solved.result.diagnostics.explicit_conflict_edge_count = if compact {
        None
    } else {
        Some(partition.blocks.len())
    };
    solved.result.diagnostics.total_chord_count = total_chords;
    solved.result.diagnostics.horizontal_chord_count = geometry.horizontal_chords.len();
    solved.result.diagnostics.vertical_chord_count = geometry.vertical_chords.len();
    solved.result.diagnostics.biclique_count = partition.blocks.len();
    solved.result.diagnostics.biclique_total_vertex_occurrences =
        partition.total_vertex_occurrences();
    solved.result.diagnostics.compressed_network_vertex_count = network.node_count();
    solved.result.diagnostics.compressed_network_arc_count = network.arc_count();
    solved.result.diagnostics.c0_network_vertex_count = c0_network_node_count.unwrap_or(0);
    solved.result.diagnostics.c0_network_arc_count = c0_network_arc_count.unwrap_or(0);
    Ok(solved)
}

fn solve_exact_cover(component: &GridComponent<bool>) -> Result<Solved, String> {
    let started = Instant::now();
    let result = exact_cover_oracle::solve(component).map_err(|error| error.to_string())?;
    let validation_started = Instant::now();
    let prepared = mrd_domain::PreparedGridComponent::from_component(component)
        .map_err(|error| error.to_string())?;
    validate_dissection_prepared(&prepared, &result).map_err(|error| error.to_string())?;
    let validation_ns = validation_started.elapsed().as_nanos();
    Ok(Solved {
        result,
        phases: PhaseTimings {
            matching_or_flow_ns: Some(started.elapsed().as_nanos()),
            verification_ns: Some(validation_ns),
            total_in_process_solve_ns: Some(started.elapsed().as_nanos()),
            ..PhaseTimings::default()
        },
        structure: StructuralCounters::default(),
        provenance: "exact-cover-oracle::solve (independent bitset branch-and-bound)",
    })
}

fn generate(family: Family, target: usize, seed: u64) -> Result<Generated, Error> {
    let instance = match family {
        Family::RandomConnected => random_connected(target, seed),
        Family::DenseConflict => Ok(dense_conflict_grid(target, target)),
        Family::SparseConflict => Ok(alternating_notch_corridor(target.max(2))),
        Family::CombStaircase => {
            if target % 2 == 0 {
                Ok(comb(target.max(2), target.max(2)))
            } else {
                Ok(staircase(target.max(2)))
            }
        }
        Family::SupportedHoles => Ok(many_holes(target)),
        Family::Polyomino => Ok(polyomino_spiral(target)),
        Family::RepresentationCrossover => clean_complete_bipartite_grid(target)
            .map_err(|error| Error::Generator(error.to_string())),
    }?;
    Ok(Generated {
        instance,
        attempts: 1,
    })
}

fn random_connected(target: usize, seed: u64) -> Result<AdversarialInstance, Error> {
    let side = ceil_sqrt(target.saturating_mul(4).max(16)).max(4);
    let width = side;
    let height = side;
    let capacity = width
        .checked_mul(height)
        .ok_or_else(|| Error::Generator("random grid dimensions overflow".to_owned()))?;
    let target = target.min(capacity);
    let mut cells = vec![false; capacity];
    let mut occupied = vec![(width / 2, height / 2)];
    cells[(height / 2) * width + width / 2] = true;
    let mut state = seed;
    while occupied.len() < target {
        let occupied_index = usize::try_from(
            next_u64(&mut state) % u64::try_from(occupied.len()).expect("length fits u64"),
        )
        .expect("modulo occupied length fits usize");
        let (x, y) = occupied[occupied_index];
        let mut candidates = Vec::with_capacity(4);
        for (nx, ny) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if nx < width && ny < height && !cells[ny * width + nx] {
                candidates.push((nx, ny));
            }
        }
        if candidates.is_empty() {
            continue;
        }
        let candidate_index = usize::try_from(
            next_u64(&mut state) % u64::try_from(candidates.len()).expect("length fits u64"),
        )
        .expect("modulo candidate length fits usize");
        let candidate = candidates[candidate_index];
        cells[candidate.1 * width + candidate.0] = true;
        occupied.push(candidate);
    }
    Ok(AdversarialInstance {
        name: format!("random-connected-n{target}-seed{seed}"),
        family: Family::RandomConnected.name().to_owned(),
        width,
        height,
        cells,
        parameters: [("target_cells".to_owned(), target)]
            .into_iter()
            .collect::<BTreeMap<_, _>>(),
    })
}

fn many_holes(hole_count: usize) -> AdversarialInstance {
    let hole_count = hole_count.max(1);
    let width = hole_count.saturating_mul(2).saturating_add(5);
    let height = 7;
    let mut cells = vec![true; width * height];
    for index in 0..hole_count {
        let x = 2 + index * 2;
        cells[3 * width + x] = false;
    }
    AdversarialInstance {
        name: format!("supported-holes-{hole_count}"),
        family: Family::SupportedHoles.name().to_owned(),
        width,
        height,
        cells,
        parameters: [("hole_count".to_owned(), hole_count)]
            .into_iter()
            .collect(),
    }
}

fn polyomino_spiral(size: usize) -> AdversarialInstance {
    let odd = size.saturating_mul(2).saturating_add(5) | 1;
    let mut instance = orthogonal_spiral(odd.max(5));
    instance.name = format!("polyomino-spiral-{size}");
    Family::Polyomino.name().clone_into(&mut instance.family);
    instance.parameters.insert("target_size".to_owned(), size);
    instance
}

fn ceil_sqrt(value: usize) -> usize {
    let mut root = 0_usize;
    while root.saturating_mul(root) < value {
        root += 1;
    }
    root
}

fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(family: Family, algorithm: Algorithm, target_size: usize) -> Request {
        Request {
            schema_version: SCHEMA_VERSION,
            family,
            target_size,
            seed: 42,
            algorithm,
            oracle_cell_limit: 40,
        }
    }

    #[test]
    fn family_generation_is_deterministic() {
        for family in [
            Family::RandomConnected,
            Family::DenseConflict,
            Family::SparseConflict,
            Family::CombStaircase,
            Family::SupportedHoles,
            Family::Polyomino,
            Family::RepresentationCrossover,
        ] {
            let first = generate(family, 4, 42).unwrap();
            let second = generate(family, 4, 42).unwrap();
            assert_eq!(first.instance.cells, second.instance.cells);
            assert_eq!(first.instance.width, second.instance.width);
            assert_eq!(first.instance.height, second.instance.height);
            assert_eq!(first.attempts, second.attempts);
        }
    }

    #[test]
    fn compact_and_explicit_match_optimum_on_small_fixture() {
        let compact = run(&request(Family::DenseConflict, Algorithm::CompactMrd, 2)).unwrap();
        let explicit = run(&request(
            Family::DenseConflict,
            Algorithm::ExplicitHopcroftKarp,
            2,
        ))
        .unwrap();
        assert_eq!(compact.outcome, Outcome::Success);
        assert_eq!(explicit.outcome, Outcome::Success);
        assert_eq!(
            compact.optimum_rectangle_count,
            explicit.optimum_rectangle_count
        );
        assert_eq!(compact.sizes.q, explicit.sizes.q);
    }

    #[test]
    fn oracle_limit_is_preserved_as_unsupported() {
        let mut request = request(Family::RandomConnected, Algorithm::ExactCoverOracle, 20);
        request.oracle_cell_limit = 1;
        let sample = run(&request).unwrap();
        assert_eq!(sample.outcome, Outcome::Unsupported);
        assert!(sample.message.unwrap().contains("exceeds oracle limit"));
    }

    #[test]
    fn sample_schema_serializes_without_absolute_paths() {
        let sample = run(&request(Family::SparseConflict, Algorithm::CompactMrd, 3)).unwrap();
        let json = serde_json::to_string(&sample).unwrap();
        assert!(!json.contains("/Users/"));
        assert!(json.contains("schema_version"));
        assert!(json.contains("total_in_process_solve_ns"));
    }

    #[test]
    fn c0_and_compressed_flow_have_network_counters() {
        let c0 = run(&request(
            Family::RepresentationCrossover,
            Algorithm::ExplicitC0Flow,
            2,
        ))
        .unwrap();
        let compact = run(&request(
            Family::RepresentationCrossover,
            Algorithm::CompactMrd,
            2,
        ))
        .unwrap();
        assert_eq!(c0.outcome, Outcome::Success);
        assert_eq!(compact.outcome, Outcome::Success);
        assert!(c0.structure.c0_network_node_count.is_some());
        assert!(compact.sizes.compressed_network_node_count.is_some());
    }
}
