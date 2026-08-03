//! Source-with-target execution isolated from the public result model.

use std::time::{Duration, Instant};

use dominance::{
    biclique::Partition,
    compressed_flow::experiment::source::{Circulation, Solution},
    embedding::DominanceEmbedding,
    formal::{FormalAdmissibleAnalysis, analyze_formal_admissible_family},
};
use graph::{
    ExactRatio,
    source_flow::{Backend, TargetDriver, TargetRun, iteration::DefinitionProjectionFactory},
};
use mrd_domain::{CoordinateRect, FormalRectilinearPolygon};
use sg_oracle::polygon::CoordinateCompressedCompletion;

use super::{
    LayeredError, LayeredResult, SolverProvenance, SourceConfig, TargetCertificate,
    VerificationSummary,
};

/// Timings for a completed source-with-target run. The values are only exposed
/// inside this crate through the experiment boundary; they are deliberately
/// absent from deterministic public solver results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceStages {
    pub(crate) geometry: Duration,
    pub(crate) compressed_representation: Duration,
    pub(crate) source: Duration,
    pub(crate) recovery: Duration,
    pub(crate) verification: Duration,
    pub(crate) total: Duration,
}

/// A source result with its local stage measurements.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRun {
    pub(crate) result: LayeredResult,
    pub(crate) stages: SourceStages,
}

struct Recovery {
    recovered_cost: i128,
    solution: Solution,
    selected_horizontal: Vec<bool>,
    selected_vertical: Vec<bool>,
    rectangles: Vec<CoordinateRect>,
    accepted_updates: usize,
}

pub(crate) fn run_source_with_target(
    polygon: &FormalRectilinearPolygon,
    config: &SourceConfig,
) -> Result<SourceRun, LayeredError> {
    let total_started = Instant::now();

    let geometry_started = Instant::now();
    let analysis = analyze_formal_admissible_family(polygon)
        .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    let geometry = geometry_started.elapsed();

    let compact_started = Instant::now();
    let circulation = compressed_network(&analysis)?;
    let compressed_representation = compact_started.elapsed();

    let source_started = Instant::now();
    let completed = drive(&circulation, config)?;
    let source = source_started.elapsed();

    let recovery_started = Instant::now();
    let recovered = recover(polygon, &analysis, &circulation, &completed)?;
    let recovery = recovery_started.elapsed();

    let verification_started = Instant::now();
    let certificate = verify(&analysis, &recovered, config)?;
    let verification = verification_started.elapsed();

    Ok(SourceRun {
        result: result(&analysis, recovered, config, certificate),
        stages: SourceStages {
            geometry,
            compressed_representation,
            source,
            recovery,
            verification,
            total: total_started.elapsed(),
        },
    })
}

fn compressed_network(analysis: &FormalAdmissibleAnalysis) -> Result<Circulation, LayeredError> {
    let embedding =
        DominanceEmbedding::new(&analysis.families.horizontal, &analysis.families.vertical)
            .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    let partition = Partition::comparability_theorem_8(&embedding)
        .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    partition
        .verify_exact_partition(&analysis.explicit_conflict_graph)
        .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    Circulation::from_partition(
        analysis.families.horizontal.len(),
        analysis.families.vertical.len(),
        &partition,
    )
    .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))
}

fn drive(circulation: &Circulation, config: &SourceConfig) -> Result<TargetRun, LayeredError> {
    let fixed_point = config.fixed_point.into_config()?;
    let kappa = ExactRatio::try_from(config.kappa)?;
    let factory = DefinitionProjectionFactory::new(kappa.clone());
    let mut driver: TargetDriver<DefinitionProjectionFactory> = Backend
        .begin_with_target(
            circulation.network(),
            config.target,
            config.maximum_abs_input,
            fixed_point,
            kappa,
            factory,
        )
        .map_err(|error| LayeredError::Source(error.to_string()))?;
    driver
        .run()
        .map_err(|error| LayeredError::Source(error.to_string()))
}

fn recover(
    polygon: &FormalRectilinearPolygon,
    analysis: &FormalAdmissibleAnalysis,
    circulation: &Circulation,
    completed: &TargetRun,
) -> Result<Recovery, LayeredError> {
    let recovered_cost = completed.recovered.original.cost;
    let solution = circulation
        .recover_certified(&completed.recovered.original)
        .map_err(|error| LayeredError::Source(error.to_string()))?;
    let selected_horizontal = solution
        .vertex_cover
        .left
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let selected_vertical = solution
        .vertex_cover
        .right
        .iter()
        .map(|covered| !covered)
        .collect::<Vec<_>>();
    let completion = CoordinateCompressedCompletion
        .complete_formal(
            polygon,
            &analysis.families.horizontal,
            &analysis.families.vertical,
            &selected_horizontal,
            &selected_vertical,
        )
        .map_err(|error| LayeredError::Source(error.to_string()))?;
    Ok(Recovery {
        recovered_cost,
        solution,
        selected_horizontal,
        selected_vertical,
        rectangles: completion.rectangles,
        accepted_updates: completed.completion.records.len(),
    })
}

fn verify(
    analysis: &FormalAdmissibleAnalysis,
    recovery: &Recovery,
    config: &SourceConfig,
) -> Result<TargetCertificate, LayeredError> {
    if recovery.rectangles.len() != analysis.optimum_rectangle_count {
        return Err(LayeredError::Source(
            "source completion rectangle count disagrees with the optimum".to_owned(),
        ));
    }
    if recovery.recovered_cost > config.target {
        return Err(LayeredError::Source(
            "recovered original cost exceeds the supplied target".to_owned(),
        ));
    }
    let accepted_updates = u64::try_from(recovery.accepted_updates)
        .map_err(|_| LayeredError::Source("source update count overflow".to_owned()))?;
    Ok(TargetCertificate {
        target: config.target,
        recovered_cost: recovery.recovered_cost,
        accepted_updates,
    })
}

fn result(
    analysis: &FormalAdmissibleAnalysis,
    recovery: Recovery,
    config: &SourceConfig,
    certificate: TargetCertificate,
) -> LayeredResult {
    LayeredResult {
        provenance: SolverProvenance::SourceCertifiedAtMost {
            target: config.target,
        },
        objective: analysis.optimum_rectangle_count,
        matching: Some(recovery.solution.matching),
        vertex_cover: Some(recovery.solution.vertex_cover),
        selected_horizontal: Some(recovery.selected_horizontal),
        selected_vertical: Some(recovery.selected_vertical),
        rectangles: Some(recovery.rectangles),
        verification: VerificationSummary {
            objective_verified: true,
            matching_verified: true,
            cover_verified: true,
            rectangles_verified: true,
            source_target_met: recovery.recovered_cost <= config.target,
            no_fallback: true,
        },
        target_certificate: Some(certificate),
    }
}
