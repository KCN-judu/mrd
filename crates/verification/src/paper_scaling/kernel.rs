//! In-process benchmark for the paper's representation and solver kernels.
//!
//! One request owns one family/size partition. The instance is generated once,
//! checked by every solver, and then measured repeatedly in counterbalanced
//! order without process-startup or serialization time in either scope.

use std::collections::{BTreeMap, BTreeSet};
use std::hint::black_box;
use std::time::Instant;

use dominance::biclique::Partition;
use dominance::compressed_flow::experiment as compressed_flow;
use dominance::embedding::{DominanceEmbedding, EmbeddingCoordinateBackend};
use graph::{DinicBackend, hopcroft_karp, minimum_vertex_cover};
use mrd_domain::{GridComponent, GridRect, PreparedComponentContext};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Algorithm, Family, Outcome, PhaseTimings, SizeMeasures};

const SCHEMA_VERSION: u32 = 1;
const CAMPAIGN: &str = "paper-kernel-scaling";
const GENERATOR_VERSION: &str = "paper-kernel-scaling-families-v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    SolveFromCanonicalInstance,
    RepresentationAndSolverKernel,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StopConditions {
    pub max_explicit_edges: usize,
    pub max_iteration_ns: u128,
    pub max_point_ns: u128,
    pub host_memory_budget_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RepetitionRule {
    pub target_measured_ns: u128,
    pub fast_threshold_ns: u128,
    pub medium_threshold_ns: u128,
    pub fast_minimum: usize,
    pub medium_minimum: usize,
    pub slow_minimum: usize,
    pub maximum: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WarmupRule {
    pub minimum: usize,
    pub maximum: usize,
    /// Maximum coefficient of variation in parts per million.
    pub cv_threshold_ppm: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub schema_version: u32,
    pub campaign: String,
    pub family: Family,
    pub target_size: usize,
    pub seed: u64,
    pub algorithms: Vec<Algorithm>,
    pub scopes: Vec<Scope>,
    pub oracle_cell_limit: usize,
    pub warmup: WarmupRule,
    pub repetitions: RepetitionRule,
    pub stop: StopConditions,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PointState {
    Complete,
    Stopped,
    Invalid,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Timings {
    pub geometry_preprocessing_ns: Option<u128>,
    pub chord_generation_ns: Option<u128>,
    pub embedding_ns: Option<u128>,
    pub explicit_conflict_construction_ns: Option<u128>,
    pub biclique_construction_ns: Option<u128>,
    pub explicit_network_construction_ns: Option<u128>,
    pub compressed_network_construction_ns: Option<u128>,
    pub matching_ns: Option<u128>,
    pub max_flow_ns: Option<u128>,
    pub vertex_cover_recovery_ns: Option<u128>,
    pub chord_selection_ns: Option<u128>,
    pub rectangle_completion_recovery_ns: Option<u128>,
    pub verification_ns: Option<u128>,
    pub scope_a_total_ns: Option<u128>,
    pub scope_b_total_ns: Option<u128>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructuralMeasures {
    pub explicit_graph_node_count: Option<usize>,
    pub explicit_graph_edge_count: Option<usize>,
    pub biclique_count: Option<usize>,
    pub biclique_incidence_sigma: Option<usize>,
    pub compact_node_count: Option<usize>,
    pub compact_arc_count: Option<usize>,
    pub explicit_c0_node_count: Option<usize>,
    pub explicit_c0_arc_count: Option<usize>,
    pub explicit_estimated_structural_bytes: Option<u128>,
    pub compact_estimated_structural_bytes: Option<u128>,
    pub max_rss_delta_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CorrectnessRecord {
    pub algorithm: Algorithm,
    pub outcome: Outcome,
    pub optimum_rectangle_count: Option<usize>,
    pub matching_size: Option<usize>,
    pub vertex_cover_size: Option<usize>,
    pub witness_checksum: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WarmupRecord {
    pub scope: Scope,
    pub algorithm: Algorithm,
    pub count: usize,
    pub converged: bool,
    pub last_five_cv_ppm: Option<u64>,
    pub preflight_ns: u128,
    pub measured_repetitions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IterationRecord {
    pub sample_identity: String,
    pub scope: Scope,
    pub algorithm: Algorithm,
    pub iteration: usize,
    pub order_position: usize,
    pub elapsed_ns: u128,
    pub timings: Timings,
    pub optimum_rectangle_count: usize,
    pub matching_size: usize,
    pub vertex_cover_size: usize,
    pub witness_checksum: String,
    pub consumed_checksum: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CampaignResult {
    pub schema_version: u32,
    pub campaign: String,
    pub generator_version: String,
    pub family: Family,
    pub target_size: usize,
    pub generator_parameter: usize,
    pub seed: u64,
    pub canonical_instance_identity: String,
    pub state: PointState,
    pub message: Option<String>,
    pub sizes: SizeMeasures,
    pub structure: StructuralMeasures,
    pub shared_scope_b_preprocessing: Timings,
    pub correctness: Vec<CorrectnessRecord>,
    pub oracle_optimum_rectangle_count: Option<usize>,
    pub warmups: Vec<WarmupRecord>,
    pub runs: Vec<IterationRecord>,
    pub exact_measured_order: Vec<String>,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("paper-kernel-scaling schema version {actual} is not {SCHEMA_VERSION}")]
    SchemaVersion { actual: u32 },
    #[error("campaign must be {CAMPAIGN}")]
    Campaign,
    #[error("target size and Oracle cell limit must be positive")]
    ZeroLimit,
    #[error("the request must contain each of the three timed algorithms exactly once")]
    Algorithms,
    #[error("the request must contain both timing scopes exactly once")]
    Scopes,
    #[error("invalid warm-up or repetition rule")]
    Rule,
    #[error("instance generator failed: {0}")]
    Generator(String),
}

#[derive(Clone, Debug)]
struct Observation {
    timings: Timings,
    optimum: usize,
    matching: usize,
    cover: usize,
    witness_checksum: u64,
    consumed_checksum: u64,
}

#[derive(Clone, Debug)]
struct KernelObservation {
    timings: Timings,
    matching: usize,
    cover: usize,
    cover_checksum: u64,
}

/// Runs one deterministic in-process benchmark partition.
///
/// # Errors
///
/// Returns an error for malformed requests or unsupported generated input.
#[allow(clippy::too_many_lines)]
pub fn run(request: &Request) -> std::result::Result<CampaignResult, Error> {
    validate_request(request)?;
    let generator_parameter = generator_parameter(request.family, request.target_size);
    let generated = super::generate(request.family, generator_parameter, request.seed)
        .map_err(|error| Error::Generator(error.to_string()))?;
    let grid = generated
        .instance
        .grid()
        .map_err(|error| Error::Generator(error.to_string()))?;
    let foreground = grid
        .four_connected_components()
        .into_iter()
        .filter(|component| component.color)
        .collect::<Vec<_>>();
    if foreground.len() != 1 {
        return Err(Error::Generator(format!(
            "expected one foreground component, found {}",
            foreground.len()
        )));
    }
    let component = &foreground[0];
    let identity = canonical_component_checksum(component);
    let mut sizes = super::base_sizes(&generated.instance, &foreground);
    let point_started = Instant::now();

    let mut correctness = Vec::new();
    let mut gate_solutions = BTreeMap::new();
    for &algorithm in &request.algorithms {
        match super::solve_algorithm(algorithm, component) {
            Ok(solved) => {
                let witness =
                    witness_checksum(&super::canonical_rectangles(&solved.result.rectangles));
                gate_solutions.insert(algorithm.name(), (solved.clone(), witness));
                correctness.push(CorrectnessRecord {
                    algorithm,
                    outcome: Outcome::Success,
                    optimum_rectangle_count: Some(solved.result.optimum_rectangle_count),
                    matching_size: solved.structure.matching_size,
                    vertex_cover_size: solved.structure.vertex_cover_size,
                    witness_checksum: Some(hex(witness)),
                    message: None,
                });
            }
            Err(message) => correctness.push(CorrectnessRecord {
                algorithm,
                outcome: Outcome::Error,
                optimum_rectangle_count: None,
                matching_size: None,
                vertex_cover_size: None,
                witness_checksum: None,
                message: Some(message),
            }),
        }
    }
    let correctness_error = correctness_failure(&correctness);
    if let Some(message) = correctness_error {
        return Ok(empty_result(
            request,
            generator_parameter,
            identity,
            sizes,
            correctness,
            PointState::Invalid,
            message,
        ));
    }

    let explicit = &gate_solutions[Algorithm::ExplicitHopcroftKarp.name()].0;
    let compact = &gate_solutions[Algorithm::CompactMrd.name()].0;
    let c0 = &gate_solutions[Algorithm::ExplicitC0Flow.name()].0;
    sizes.horizontal_chord_count_h = Some(explicit.result.diagnostics.horizontal_chord_count);
    sizes.vertical_chord_count_v = Some(explicit.result.diagnostics.vertical_chord_count);
    sizes.q = Some(explicit.result.diagnostics.total_chord_count);
    sizes.explicit_conflict_edge_count_k = explicit.result.diagnostics.explicit_conflict_edge_count;
    sizes.biclique_count = Some(compact.result.diagnostics.biclique_count);
    sizes.biclique_total_vertex_occurrences_sigma =
        Some(compact.result.diagnostics.biclique_total_vertex_occurrences);
    sizes.compressed_network_node_count =
        Some(compact.result.diagnostics.compressed_network_vertex_count);
    sizes.compressed_network_arc_count =
        Some(compact.result.diagnostics.compressed_network_arc_count);
    sizes.optimum_rectangle_count = Some(explicit.result.optimum_rectangle_count);
    let structure = structural_measures(&sizes, c0);
    if structure.explicit_graph_edge_count.unwrap_or_default() > request.stop.max_explicit_edges {
        return Ok(empty_result_with_structure(
            request,
            generator_parameter,
            identity,
            sizes,
            structure,
            correctness,
            PointState::Stopped,
            format!("explicit K exceeded {}", request.stop.max_explicit_edges),
        ));
    }

    let oracle_optimum = if component.cell_count() <= request.oracle_cell_limit {
        super::solve_exact_cover(component)
            .ok()
            .map(|solved| solved.result.optimum_rectangle_count)
    } else {
        None
    };
    if oracle_optimum.is_some_and(|value| value != explicit.result.optimum_rectangle_count) {
        return Ok(empty_result_with_structure(
            request,
            generator_parameter,
            identity,
            sizes,
            structure,
            correctness,
            PointState::Invalid,
            "exact-cover correctness gate disagreed".to_owned(),
        ));
    }

    let preprocessing_started = Instant::now();
    let prepared = PreparedComponentContext::new(component)
        .map_err(|error| Error::Generator(error.to_string()))?;
    let preprocessing_ns = preprocessing_started.elapsed().as_nanos();
    let chord_started = Instant::now();
    let geometry = sg_oracle::grid::analyze_prepared_geometry(
        prepared,
        &sg_oracle::grid::experiment::InteriorRuns,
    )
    .map_err(|error| Error::Generator(error.to_string()))?;
    let chord_ns = chord_started.elapsed().as_nanos();
    let shared_scope_b_preprocessing = Timings {
        geometry_preprocessing_ns: Some(preprocessing_ns),
        chord_generation_ns: Some(chord_ns),
        ..Timings::default()
    };

    let gate_witness = request
        .algorithms
        .iter()
        .map(|algorithm| (*algorithm, gate_solutions[algorithm.name()].1))
        .collect::<BTreeMap<_, _>>();
    let optimum = explicit.result.optimum_rectangle_count;
    let mut warmups = Vec::new();
    let mut repetitions = BTreeMap::new();
    for &scope in &request.scopes {
        for &algorithm in &request.algorithms {
            let preflight = measure(
                scope,
                algorithm,
                component,
                &geometry,
                optimum,
                gate_witness[&algorithm],
            )?;
            if elapsed(&preflight, scope) > request.stop.max_iteration_ns {
                return Ok(stopped_after_preparation(
                    request,
                    generator_parameter,
                    identity,
                    sizes,
                    structure,
                    shared_scope_b_preprocessing,
                    correctness,
                    oracle_optimum,
                    format!(
                        "preflight iteration exceeded {} ns",
                        request.stop.max_iteration_ns
                    ),
                ));
            }
            let count = repetition_count(elapsed(&preflight, scope), &request.repetitions);
            repetitions.insert((scope, algorithm), count);
            let (warmup_count, converged, cv) = warm_up(
                request,
                scope,
                algorithm,
                component,
                &geometry,
                optimum,
                gate_witness[&algorithm],
            )?;
            warmups.push(WarmupRecord {
                scope,
                algorithm,
                count: warmup_count,
                converged,
                last_five_cv_ppm: cv,
                preflight_ns: elapsed(&preflight, scope),
                measured_repetitions: count,
            });
        }
    }

    let mut runs = Vec::new();
    let mut exact_order = Vec::new();
    let maximum_rounds = repetitions.values().copied().max().unwrap_or(0);
    for iteration in 0..maximum_rounds {
        for &scope in &request.scopes {
            let mut order = request.algorithms.clone();
            deterministic_shuffle(
                &mut order,
                request.seed ^ (iteration as u64).rotate_left(17) ^ scope_tag(scope),
            );
            for (position, algorithm) in order.into_iter().enumerate() {
                if iteration >= repetitions[&(scope, algorithm)] {
                    continue;
                }
                if point_started.elapsed().as_nanos() > request.stop.max_point_ns {
                    return Ok(CampaignResult {
                        schema_version: SCHEMA_VERSION,
                        campaign: CAMPAIGN.to_owned(),
                        generator_version: GENERATOR_VERSION.to_owned(),
                        family: request.family,
                        target_size: request.target_size,
                        generator_parameter,
                        seed: request.seed,
                        canonical_instance_identity: hex(identity),
                        state: PointState::Stopped,
                        message: Some("point time budget exceeded".to_owned()),
                        sizes,
                        structure,
                        shared_scope_b_preprocessing,
                        correctness,
                        oracle_optimum_rectangle_count: oracle_optimum,
                        warmups,
                        runs,
                        exact_measured_order: exact_order,
                    });
                }
                let observation = measure(
                    scope,
                    algorithm,
                    component,
                    &geometry,
                    optimum,
                    gate_witness[&algorithm],
                )?;
                let duration = elapsed(&observation, scope);
                let sample_identity = format!(
                    "{}:{}:{}:{}:{}:{}",
                    CAMPAIGN,
                    request.family.name(),
                    request.target_size,
                    scope_name(scope),
                    algorithm.name(),
                    iteration
                );
                exact_order.push(sample_identity.clone());
                runs.push(IterationRecord {
                    sample_identity,
                    scope,
                    algorithm,
                    iteration,
                    order_position: position,
                    elapsed_ns: duration,
                    timings: observation.timings,
                    optimum_rectangle_count: observation.optimum,
                    matching_size: observation.matching,
                    vertex_cover_size: observation.cover,
                    witness_checksum: hex(observation.witness_checksum),
                    consumed_checksum: hex(observation.consumed_checksum),
                });
                if duration > request.stop.max_iteration_ns {
                    return Ok(CampaignResult {
                        schema_version: SCHEMA_VERSION,
                        campaign: CAMPAIGN.to_owned(),
                        generator_version: GENERATOR_VERSION.to_owned(),
                        family: request.family,
                        target_size: request.target_size,
                        generator_parameter,
                        seed: request.seed,
                        canonical_instance_identity: hex(identity),
                        state: PointState::Stopped,
                        message: Some("measured iteration exceeded time limit".to_owned()),
                        sizes,
                        structure,
                        shared_scope_b_preprocessing,
                        correctness,
                        oracle_optimum_rectangle_count: oracle_optimum,
                        warmups,
                        runs,
                        exact_measured_order: exact_order,
                    });
                }
            }
        }
    }
    Ok(CampaignResult {
        schema_version: SCHEMA_VERSION,
        campaign: CAMPAIGN.to_owned(),
        generator_version: GENERATOR_VERSION.to_owned(),
        family: request.family,
        target_size: request.target_size,
        generator_parameter,
        seed: request.seed,
        canonical_instance_identity: hex(identity),
        state: PointState::Complete,
        message: None,
        sizes,
        structure,
        shared_scope_b_preprocessing,
        correctness,
        oracle_optimum_rectangle_count: oracle_optimum,
        warmups,
        runs,
        exact_measured_order: exact_order,
    })
}

fn validate_request(request: &Request) -> std::result::Result<(), Error> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(Error::SchemaVersion {
            actual: request.schema_version,
        });
    }
    if request.campaign != CAMPAIGN {
        return Err(Error::Campaign);
    }
    if request.target_size == 0 || request.oracle_cell_limit == 0 {
        return Err(Error::ZeroLimit);
    }
    let algorithms = request.algorithms.iter().copied().collect::<BTreeSet<_>>();
    let expected = [
        Algorithm::CompactMrd,
        Algorithm::ExplicitHopcroftKarp,
        Algorithm::ExplicitC0Flow,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if algorithms != expected || request.algorithms.len() != expected.len() {
        return Err(Error::Algorithms);
    }
    let scopes = request.scopes.iter().copied().collect::<BTreeSet<_>>();
    if scopes.len() != 2 || request.scopes.len() != 2 {
        return Err(Error::Scopes);
    }
    if request.warmup.minimum < 5
        || request.warmup.maximum < request.warmup.minimum
        || request.repetitions.fast_minimum < 31
        || request.repetitions.medium_minimum < 15
        || request.repetitions.slow_minimum < 7
        || request.repetitions.maximum < request.repetitions.fast_minimum
        || request.repetitions.maximum > 10_000
    {
        return Err(Error::Rule);
    }
    Ok(())
}

fn generator_parameter(family: Family, target: usize) -> usize {
    match family {
        Family::RepresentationCrossover | Family::CombStaircase => ceil_sqrt(target).max(2),
        _ => target,
    }
}

fn ceil_sqrt(value: usize) -> usize {
    let mut root = 0_usize;
    while root.saturating_mul(root) < value {
        root += 1;
    }
    root
}

fn correctness_failure(records: &[CorrectnessRecord]) -> Option<String> {
    if records
        .iter()
        .any(|record| record.outcome != Outcome::Success)
    {
        return Some("at least one production correctness gate failed".to_owned());
    }
    let optima = records
        .iter()
        .filter_map(|record| record.optimum_rectangle_count)
        .collect::<BTreeSet<_>>();
    if optima.len() != 1 {
        return Some("production optimum counts disagree".to_owned());
    }
    if records.iter().any(|record| {
        record.matching_size.is_none() || record.matching_size != record.vertex_cover_size
    }) {
        return Some("matching and minimum-cover sizes disagree".to_owned());
    }
    None
}

fn structural_measures(sizes: &SizeMeasures, c0: &super::Solved) -> StructuralMeasures {
    let q = sizes.q.unwrap_or_default();
    let k = sizes.explicit_conflict_edge_count_k.unwrap_or_default();
    let compact_nodes = sizes.compressed_network_node_count.unwrap_or_default();
    let compact_arcs = sizes.compressed_network_arc_count.unwrap_or_default();
    let word = std::mem::size_of::<usize>() as u128;
    StructuralMeasures {
        explicit_graph_node_count: Some(q),
        explicit_graph_edge_count: Some(k),
        biclique_count: sizes.biclique_count,
        biclique_incidence_sigma: sizes.biclique_total_vertex_occurrences_sigma,
        compact_node_count: Some(compact_nodes),
        compact_arc_count: Some(compact_arcs),
        explicit_c0_node_count: c0.structure.c0_network_node_count,
        explicit_c0_arc_count: c0.structure.c0_network_arc_count,
        explicit_estimated_structural_bytes: Some((q as u128 + 2 * k as u128) * word),
        compact_estimated_structural_bytes: Some(
            (compact_nodes as u128 + 3 * compact_arcs as u128) * word,
        ),
        max_rss_delta_bytes: None,
    }
}

fn measure(
    scope: Scope,
    algorithm: Algorithm,
    component: &GridComponent<bool>,
    geometry: &sg_oracle::grid::Geometry,
    optimum: usize,
    gate_witness: u64,
) -> std::result::Result<Observation, Error> {
    let observation = match scope {
        Scope::SolveFromCanonicalInstance => measure_scope_a(algorithm, component)?,
        Scope::RepresentationAndSolverKernel => {
            let kernel = measure_scope_b(algorithm, geometry)?;
            let consumed = consume(
                kernel.matching,
                kernel.cover,
                optimum,
                gate_witness,
                kernel.cover_checksum,
            );
            Observation {
                timings: kernel.timings,
                optimum,
                matching: kernel.matching,
                cover: kernel.cover,
                witness_checksum: gate_witness,
                consumed_checksum: consumed,
            }
        }
    };
    if observation.optimum != optimum
        || observation.matching != observation.cover
        || observation.witness_checksum != gate_witness
    {
        return Err(Error::Generator(format!(
            "{} produced an unstable measured result",
            algorithm.name()
        )));
    }
    black_box(&observation);
    Ok(observation)
}

fn measure_scope_a(
    algorithm: Algorithm,
    canonical: &GridComponent<bool>,
) -> std::result::Result<Observation, Error> {
    let started = Instant::now();
    let component = canonical.clone();
    let solved = super::solve_algorithm(algorithm, &component).map_err(Error::Generator)?;
    let total = started.elapsed().as_nanos();
    let witness = witness_checksum(&super::canonical_rectangles(&solved.result.rectangles));
    let matching = solved.structure.matching_size.unwrap_or_default();
    let cover = solved.structure.vertex_cover_size.unwrap_or(matching);
    let consumed = consume(
        matching,
        cover,
        solved.result.optimum_rectangle_count,
        witness,
        0,
    );
    Ok(Observation {
        timings: scope_a_timings(algorithm, &solved.phases, total),
        optimum: solved.result.optimum_rectangle_count,
        matching,
        cover,
        witness_checksum: witness,
        consumed_checksum: consumed,
    })
}

fn scope_a_timings(algorithm: Algorithm, phases: &PhaseTimings, total: u128) -> Timings {
    Timings {
        geometry_preprocessing_ns: phases.geometry_preprocessing_ns,
        chord_generation_ns: phases.chord_generation_ns,
        embedding_ns: phases.embedding_ns,
        explicit_conflict_construction_ns: phases.explicit_conflict_graph_ns,
        biclique_construction_ns: (algorithm == Algorithm::CompactMrd)
            .then_some(phases.biclique_construction_ns)
            .flatten(),
        explicit_network_construction_ns: (algorithm == Algorithm::ExplicitC0Flow)
            .then_some(phases.network_construction_ns)
            .flatten(),
        compressed_network_construction_ns: (algorithm == Algorithm::CompactMrd)
            .then_some(phases.network_construction_ns)
            .flatten(),
        matching_ns: (algorithm == Algorithm::ExplicitHopcroftKarp)
            .then_some(phases.matching_or_flow_ns)
            .flatten(),
        max_flow_ns: (algorithm != Algorithm::ExplicitHopcroftKarp)
            .then_some(phases.matching_or_flow_ns)
            .flatten(),
        vertex_cover_recovery_ns: phases.vertex_cover_recovery_ns,
        chord_selection_ns: phases.chord_selection_ns,
        rectangle_completion_recovery_ns: option_sum(
            phases.geometric_completion_ns,
            phases.rectangle_recovery_ns,
        ),
        verification_ns: phases.verification_ns,
        scope_a_total_ns: Some(total),
        scope_b_total_ns: None,
    }
}

fn measure_scope_b(
    algorithm: Algorithm,
    geometry: &sg_oracle::grid::Geometry,
) -> std::result::Result<KernelObservation, Error> {
    let started = Instant::now();
    let mut timings = Timings::default();
    let (matching, cover, checksum) = match algorithm {
        Algorithm::ExplicitHopcroftKarp => {
            let graph_started = Instant::now();
            let graph = sg_oracle::grid::build_conflict_graph(
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
            )
            .map_err(|error| Error::Generator(error.to_string()))?;
            timings.explicit_conflict_construction_ns = Some(graph_started.elapsed().as_nanos());
            let matching_started = Instant::now();
            let matching = hopcroft_karp(&graph);
            timings.matching_ns = Some(matching_started.elapsed().as_nanos());
            let cover_started = Instant::now();
            let cover = minimum_vertex_cover(&graph, &matching);
            timings.vertex_cover_recovery_ns = Some(cover_started.elapsed().as_nanos());
            (
                matching.size,
                cover.size,
                bool_checksum(&cover.left, &cover.right),
            )
        }
        Algorithm::CompactMrd | Algorithm::ExplicitC0Flow => {
            let embedding_started = Instant::now();
            let embedding = DominanceEmbedding::new_with_backend(
                &geometry.horizontal_chords,
                &geometry.vertical_chords,
                EmbeddingCoordinateBackend::DirectGridParity,
            )
            .map_err(|error| Error::Generator(error.to_string()))?;
            timings.embedding_ns = Some(embedding_started.elapsed().as_nanos());
            let partition = if algorithm == Algorithm::CompactMrd {
                let started = Instant::now();
                let partition = dominance::biclique::experiment::construct(&embedding)
                    .map_err(|error| Error::Generator(error.to_string()))?
                    .partition;
                partition
                    .verify_dominance_blocks(&embedding)
                    .map_err(|error| Error::Generator(error.to_string()))?;
                timings.biclique_construction_ns = Some(started.elapsed().as_nanos());
                partition
            } else {
                let started = Instant::now();
                let graph = embedding
                    .explicit_graph()
                    .map_err(|error| Error::Generator(error.to_string()))?;
                timings.explicit_conflict_construction_ns = Some(started.elapsed().as_nanos());
                let partition = Partition::from_explicit_edges(&graph);
                partition
                    .verify_exact_partition(&graph)
                    .map_err(|error| Error::Generator(error.to_string()))?;
                partition
            };
            let network_started = Instant::now();
            let network = compressed_flow::construct_network(
                geometry.horizontal_chords.len(),
                geometry.vertical_chords.len(),
                &partition,
            )
            .map_err(|error| Error::Generator(error.to_string()))?;
            let network_ns = network_started.elapsed().as_nanos();
            if algorithm == Algorithm::CompactMrd {
                timings.compressed_network_construction_ns = Some(network_ns);
            } else {
                timings.explicit_network_construction_ns = Some(network_ns);
            }
            let flow_started = Instant::now();
            let flow = compressed_flow::execute_flow(&network, &DinicBackend)
                .map_err(|error| Error::Generator(error.to_string()))?;
            timings.max_flow_ns = Some(flow_started.elapsed().as_nanos());
            let cover_started = Instant::now();
            let solution = compressed_flow::recover_vertex_cover(&network, flow)
                .map_err(|error| Error::Generator(error.to_string()))?;
            timings.vertex_cover_recovery_ns = Some(cover_started.elapsed().as_nanos());
            let size = solution.vertex_cover.size;
            (
                size,
                size,
                bool_checksum(&solution.vertex_cover.left, &solution.vertex_cover.right),
            )
        }
        Algorithm::ExactCoverOracle => return Err(Error::Algorithms),
    };
    timings.scope_b_total_ns = Some(started.elapsed().as_nanos());
    Ok(KernelObservation {
        timings,
        matching,
        cover,
        cover_checksum: checksum,
    })
}

#[allow(clippy::too_many_arguments)]
fn warm_up(
    request: &Request,
    scope: Scope,
    algorithm: Algorithm,
    component: &GridComponent<bool>,
    geometry: &sg_oracle::grid::Geometry,
    optimum: usize,
    witness: u64,
) -> std::result::Result<(usize, bool, Option<u64>), Error> {
    let mut durations = Vec::new();
    for count in 1..=request.warmup.maximum {
        let observation = measure(scope, algorithm, component, geometry, optimum, witness)?;
        durations.push(elapsed(&observation, scope));
        if count >= request.warmup.minimum && durations.len() >= 5 {
            let cv = cv_ppm(&durations[durations.len() - 5..]);
            if cv <= request.warmup.cv_threshold_ppm {
                return Ok((count, true, Some(cv)));
            }
        }
    }
    let cv = (durations.len() >= 5).then(|| cv_ppm(&durations[durations.len() - 5..]));
    Ok((durations.len(), false, cv))
}

fn repetition_count(preflight_ns: u128, rule: &RepetitionRule) -> usize {
    let minimum = if preflight_ns < rule.fast_threshold_ns {
        rule.fast_minimum
    } else if preflight_ns <= rule.medium_threshold_ns {
        rule.medium_minimum
    } else {
        rule.slow_minimum
    };
    let target =
        usize::try_from(rule.target_measured_ns / preflight_ns.max(1)).unwrap_or(rule.maximum);
    target.clamp(minimum, rule.maximum)
}

fn cv_ppm(values: &[u128]) -> u64 {
    let count = values.len() as u128;
    let mean = values.iter().sum::<u128>() / count;
    if mean == 0 {
        return 0;
    }
    let variance = values
        .iter()
        .map(|&value| {
            let difference = value.abs_diff(mean);
            difference * difference
        })
        .sum::<u128>()
        / count;
    u64::try_from(integer_sqrt(variance).saturating_mul(1_000_000) / mean).unwrap_or(u64::MAX)
}

fn integer_sqrt(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut current = value;
    let mut next = u128::midpoint(current, value / current);
    while next < current {
        current = next;
        next = u128::midpoint(current, value / current);
    }
    current
}

fn elapsed(observation: &Observation, scope: Scope) -> u128 {
    match scope {
        Scope::SolveFromCanonicalInstance => observation.timings.scope_a_total_ns,
        Scope::RepresentationAndSolverKernel => observation.timings.scope_b_total_ns,
    }
    .expect("selected scope has a total")
}

fn option_sum(left: Option<u128>, right: Option<u128>) -> Option<u128> {
    match (left, right) {
        (None, None) => None,
        (left, right) => Some(left.unwrap_or(0) + right.unwrap_or(0)),
    }
}

fn canonical_component_checksum(component: &GridComponent<bool>) -> u64 {
    let mut hash = fnv_start();
    hash_usize(&mut hash, component.grid_width);
    hash_usize(&mut hash, component.grid_height);
    for cell in &component.cells {
        hash_usize(&mut hash, cell.x);
        hash_usize(&mut hash, cell.y);
    }
    hash
}

fn witness_checksum(rectangles: &[GridRect]) -> u64 {
    let mut hash = fnv_start();
    for rectangle in rectangles {
        for value in [rectangle.x0, rectangle.y0, rectangle.x1, rectangle.y1] {
            hash_usize(&mut hash, value);
        }
    }
    hash
}

fn bool_checksum(left: &[bool], right: &[bool]) -> u64 {
    let mut hash = fnv_start();
    for &value in left.iter().chain(right) {
        hash_byte(&mut hash, u8::from(value));
    }
    hash
}

fn consume(matching: usize, cover: usize, optimum: usize, witness: u64, cover_hash: u64) -> u64 {
    let mut hash = fnv_start();
    hash_usize(&mut hash, matching);
    hash_usize(&mut hash, cover);
    hash_usize(&mut hash, optimum);
    for byte in witness
        .to_le_bytes()
        .into_iter()
        .chain(cover_hash.to_le_bytes())
    {
        hash_byte(&mut hash, byte);
    }
    black_box(hash)
}

const fn fnv_start() -> u64 {
    0xcbf2_9ce4_8422_2325
}

fn hash_usize(hash: &mut u64, value: usize) {
    for byte in value.to_le_bytes() {
        hash_byte(hash, byte);
    }
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
}

fn hex(value: u64) -> String {
    format!("{value:016x}")
}

fn deterministic_shuffle(values: &mut [Algorithm], mut state: u64) {
    for upper in (1..values.len()).rev() {
        state = next_u64(&mut state);
        let modulus = u64::try_from(upper).expect("algorithm count fits u64") + 1;
        let index = usize::try_from(state % modulus).expect("index is bounded by slice length");
        values.swap(upper, index);
    }
}

fn next_u64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

const fn scope_tag(scope: Scope) -> u64 {
    match scope {
        Scope::SolveFromCanonicalInstance => 0xa11c_e001,
        Scope::RepresentationAndSolverKernel => 0xb11c_e002,
    }
}

const fn scope_name(scope: Scope) -> &'static str {
    match scope {
        Scope::SolveFromCanonicalInstance => "scope-a",
        Scope::RepresentationAndSolverKernel => "scope-b",
    }
}

fn empty_result(
    request: &Request,
    generator_parameter: usize,
    identity: u64,
    sizes: SizeMeasures,
    correctness: Vec<CorrectnessRecord>,
    state: PointState,
    message: String,
) -> CampaignResult {
    empty_result_with_structure(
        request,
        generator_parameter,
        identity,
        sizes,
        StructuralMeasures::default(),
        correctness,
        state,
        message,
    )
}

#[allow(clippy::too_many_arguments)]
fn empty_result_with_structure(
    request: &Request,
    generator_parameter: usize,
    identity: u64,
    sizes: SizeMeasures,
    structure: StructuralMeasures,
    correctness: Vec<CorrectnessRecord>,
    state: PointState,
    message: String,
) -> CampaignResult {
    CampaignResult {
        schema_version: SCHEMA_VERSION,
        campaign: CAMPAIGN.to_owned(),
        generator_version: GENERATOR_VERSION.to_owned(),
        family: request.family,
        target_size: request.target_size,
        generator_parameter,
        seed: request.seed,
        canonical_instance_identity: hex(identity),
        state,
        message: Some(message),
        sizes,
        structure,
        shared_scope_b_preprocessing: Timings::default(),
        correctness,
        oracle_optimum_rectangle_count: None,
        warmups: Vec::new(),
        runs: Vec::new(),
        exact_measured_order: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn stopped_after_preparation(
    request: &Request,
    generator_parameter: usize,
    identity: u64,
    sizes: SizeMeasures,
    structure: StructuralMeasures,
    shared_scope_b_preprocessing: Timings,
    correctness: Vec<CorrectnessRecord>,
    oracle_optimum: Option<usize>,
    message: String,
) -> CampaignResult {
    let mut result = empty_result_with_structure(
        request,
        generator_parameter,
        identity,
        sizes,
        structure,
        correctness,
        PointState::Stopped,
        message,
    );
    result.shared_scope_b_preprocessing = shared_scope_b_preprocessing;
    result.oracle_optimum_rectangle_count = oracle_optimum;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> Request {
        Request {
            schema_version: 1,
            campaign: CAMPAIGN.to_owned(),
            family: Family::DenseConflict,
            target_size: 4,
            seed: 42,
            algorithms: vec![
                Algorithm::CompactMrd,
                Algorithm::ExplicitHopcroftKarp,
                Algorithm::ExplicitC0Flow,
            ],
            scopes: vec![
                Scope::SolveFromCanonicalInstance,
                Scope::RepresentationAndSolverKernel,
            ],
            oracle_cell_limit: 40,
            warmup: WarmupRule {
                minimum: 5,
                maximum: 5,
                cv_threshold_ppm: 50_000,
            },
            repetitions: RepetitionRule {
                target_measured_ns: 1,
                fast_threshold_ns: 10_000_000,
                medium_threshold_ns: 100_000_000,
                fast_minimum: 31,
                medium_minimum: 15,
                slow_minimum: 7,
                maximum: 31,
            },
            stop: StopConditions {
                max_explicit_edges: 1_000_000,
                max_iteration_ns: 5_000_000_000,
                max_point_ns: 120_000_000_000,
                host_memory_budget_bytes: 1_000_000_000,
            },
        }
    }

    #[test]
    fn all_algorithms_and_scopes_are_measured_in_process() {
        let result = run(&request()).unwrap();
        assert_eq!(result.state, PointState::Complete);
        assert_eq!(result.warmups.len(), 6);
        assert_eq!(result.runs.len(), 3 * 2 * 31);
        assert!(
            result
                .runs
                .iter()
                .all(|row| row.matching_size == row.vertex_cover_size)
        );
        assert_eq!(
            result
                .runs
                .iter()
                .map(|row| &row.sample_identity)
                .collect::<BTreeSet<_>>()
                .len(),
            result.runs.len()
        );
    }

    #[test]
    fn invalid_algorithm_set_is_rejected() {
        let mut request = request();
        request.algorithms.pop();
        assert!(matches!(run(&request), Err(Error::Algorithms)));
    }
}
