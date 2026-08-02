//! Layered public solver architecture.
//!
//! This module exposes an explicit three-layer backend model:
//!
//! 1. [`solve_reference`] - a complete reference-backed exact solver;
//! 2. [`solve_source_with_target`] - the source-shaped backend executed only
//!    under a caller-supplied inclusive target;
//! 3. [`verify_source_feasible_at_most`] and [`verify_source_infeasible_below`]
//!    - exact independent certificate verification.
//!
//! There is deliberately no `solve_source -> optimum` automatic entry:
//! automatic target discovery remains blocked (P9.5e.3g.3), so no public API
//! here may claim to find `F*`. Every result records its [`SolverProvenance`].

use graph::{
    ExactRatio, FixedPointConfig, MinCostSolution, VertexCover,
    source_flow::{
        Backend, TargetDriver, certificate::DualLowerBoundCertificate,
        iteration::DefinitionProjectionFactory,
    },
};
use mrd_domain::FormalRectilinearPolygon;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use dominance::{
    biclique::Partition,
    compressed_flow::experiment::source::{Circulation, CoverBelowProof},
    embedding::DominanceEmbedding,
};
use sg_oracle::polygon::CoordinateCompressedCompletion;

/// Explicit selection of which backend solves an instance.
///
/// `Reference` runs the permanent reference backends. `SourceWithTarget` runs
/// only the source-shaped production path under one caller-supplied inclusive
/// target. There is deliberately no `AutomaticSource` variant.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SolverMode {
    /// Complete exact reference-backed solve.
    Reference,
    /// Source-shaped solve under a caller-supplied inclusive target.
    SourceWithTarget {
        target: i128,
        source_config: SourceConfig,
    },
}

/// Which backend produced a result, recorded on every output.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum SolverProvenance {
    /// Produced by the permanent reference backends.
    ReferenceExact,
    /// Produced by the source-shaped path under an inclusive target; a
    /// completed run certifies `recovered_cost <= target`.
    SourceCertifiedAtMost { target: i128 },
}

/// Checked parameters for the source-shaped backend.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Caller-supplied inclusive integral target for the Appendix B.1 point.
    pub target: i128,
    /// Source input bound used by the Appendix B.1 augmentation.
    pub maximum_abs_input: i128,
    /// Fixed-point precision used by the certified IPM.
    pub fixed_point: FixedPointConfigSpec,
    /// Source update-quality `kappa`.
    pub kappa: RatioSpec,
}

/// Serialization-friendly fixed-point configuration specification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FixedPointConfigSpec {
    pub input_encoding_bits: u64,
    pub fractional_bits: u32,
    pub series_terms: u32,
    pub word_log_exponent: u32,
}

impl FixedPointConfigSpec {
    /// Constructs the equivalent certified fixed-point configuration.
    ///
    /// # Errors
    ///
    /// Returns [`LayeredError::InvalidConfig`] when the specification is not a
    /// valid source-bounded fixed-point configuration.
    pub fn into_config(self) -> Result<FixedPointConfig, LayeredError> {
        FixedPointConfig::source_bounded(
            self.input_encoding_bits,
            self.fractional_bits,
            self.series_terms,
            self.word_log_exponent,
        )
        .map_err(|error| LayeredError::InvalidConfig(error.to_string()))
    }
}

/// Serialization-friendly exact ratio specification (numerator/denominator).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RatioSpec {
    pub numerator: i128,
    pub denominator: i128,
}

impl TryFrom<RatioSpec> for ExactRatio {
    type Error = LayeredError;

    fn try_from(spec: RatioSpec) -> Result<Self, Self::Error> {
        ExactRatio::new(spec.numerator, spec.denominator).map_err(|_| LayeredError::InvalidRatio)
    }
}

/// A stable, deterministic result model for the layered backends.
///
/// Fields that a backend does not produce are `None` rather than fabricated;
/// the provenance records exactly which backend produced the result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LayeredResult {
    /// Which backend produced this result.
    pub provenance: SolverProvenance,
    /// Objective (rectangle count for MRD inputs).
    pub objective: usize,
    /// Maximum matching as `(horizontal, vertical)` pairs when produced.
    pub matching: Option<Vec<(usize, usize)>>,
    /// Minimum vertex cover when the backend produced one.
    pub vertex_cover: Option<VertexCover>,
    /// Selected horizontal chords when the backend produced them.
    pub selected_horizontal: Option<Vec<bool>>,
    /// Selected vertical chords when the backend produced them.
    pub selected_vertical: Option<Vec<bool>>,
    /// Recovered rectangles.
    pub rectangles: Option<Vec<mrd_domain::CoordinateRect>>,
    /// Verification summary.
    pub verification: VerificationSummary,
    /// Source target certificate when the source backend produced a result.
    pub target_certificate: Option<TargetCertificate>,
}

/// Summary of the independent verification performed on a result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct VerificationSummary {
    /// Objective was independently verified.
    pub objective_verified: bool,
    /// Matching feasibility was independently verified.
    pub matching_verified: bool,
    /// Cover validity was independently verified.
    pub cover_verified: bool,
    /// Rectangle partition was independently verified.
    pub rectangles_verified: bool,
    /// (Source mode) recovered cost was verified to be at most the target.
    pub source_target_met: bool,
    /// (Source mode) provenance contains no fallback.
    pub no_fallback: bool,
}

impl VerificationSummary {
    /// Returns true when every produced field is independently verified and no
    /// fallback was used. A `None` field is treated as not produced and is not
    /// required to be verified.
    #[must_use]
    pub fn verified(&self) -> bool {
        self.objective_verified
            && self.rectangles_verified
            && self.no_fallback
            && self.source_target_met
    }
}

/// Source target certificate carried by a `SourceCertifiedAtMost` result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TargetCertificate {
    /// The caller-supplied inclusive target.
    pub target: i128,
    /// The exact recovered original cost.
    pub recovered_cost: i128,
    /// Number of accepted source transitions.
    pub accepted_updates: u64,
}

/// A layered backend failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum LayeredError {
    /// The input model is outside the supported domain.
    #[error("unsupported or undetermined input: {0}")]
    UnsupportedOrUndetermined(String),
    /// The reference backend failed.
    #[error("reference-backed solve failed: {0}")]
    Reference(String),
    /// The source backend failed before a certified result.
    #[error("source-with-target solve failed: {0}")]
    Source(String),
    /// A supplied target is invalid.
    #[error("invalid target")]
    InvalidTarget,
    /// A supplied ratio specification is invalid.
    #[error("invalid exact ratio specification")]
    InvalidRatio,
    /// A supplied fixed-point configuration is invalid.
    #[error("invalid fixed-point configuration: {0}")]
    InvalidConfig(String),
    /// A certificate failed independent verification.
    #[error("certificate verification failed: {0}")]
    Certificate(String),
}

/// Reference-backed exact solve of a formal polygon.
///
/// Returns matching, minimum vertex cover, selected chords, and rectangle
/// decomposition from the permanent reference path, independently verified.
///
/// # Errors
///
/// Returns `LayeredError::Reference` on any reference solver failure.
pub fn solve_reference(polygon: &FormalRectilinearPolygon) -> Result<LayeredResult, LayeredError> {
    let analysis = dominance::formal::complete_formal_polygon(polygon)
        .map_err(|error| LayeredError::Reference(error.to_string()))?;
    Ok(reference_result(&analysis))
}

/// Source-shaped solve of a formal polygon under a caller-supplied target.
///
/// Runs only the source production path. A certified result is returned only
/// when the recovered original cost is at most the target; any other source
/// failure is reported honestly as [`LayeredError::Source`] and is never
/// classified as target infeasibility.
///
/// # Errors
///
/// Returns `LayeredError::Source` for a source execution failure.
pub fn solve_source_with_target(
    polygon: &FormalRectilinearPolygon,
    config: &SourceConfig,
) -> Result<LayeredResult, LayeredError> {
    let analysis = dominance::formal::analyze_formal_admissible_family(polygon)
        .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    let embedding =
        DominanceEmbedding::new(&analysis.families.horizontal, &analysis.families.vertical)
            .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    let partition = Partition::comparability_theorem_8(&embedding)
        .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    partition
        .verify_exact_partition(&analysis.explicit_conflict_graph)
        .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;
    let circulation = Circulation::from_partition(
        analysis.families.horizontal.len(),
        analysis.families.vertical.len(),
        &partition,
    )
    .map_err(|error| LayeredError::UnsupportedOrUndetermined(error.to_string()))?;

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
    let completed = driver
        .run()
        .map_err(|error| LayeredError::Source(error.to_string()))?;
    let recovered_cost = completed.recovered.original.cost;
    let recovered_original = completed.recovered.original.clone();
    let solution = circulation
        .recover_certified(&recovered_original)
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

    if completion.rectangles.len() != analysis.optimum_rectangle_count {
        return Err(LayeredError::Source(
            "source completion rectangle count disagrees with the optimum".to_owned(),
        ));
    }
    if recovered_cost > config.target {
        return Err(LayeredError::Source(
            "recovered original cost exceeds the supplied target".to_owned(),
        ));
    }

    Ok(LayeredResult {
        provenance: SolverProvenance::SourceCertifiedAtMost {
            target: config.target,
        },
        objective: analysis.optimum_rectangle_count,
        matching: Some(solution.matching),
        vertex_cover: Some(solution.vertex_cover),
        selected_horizontal: Some(selected_horizontal),
        selected_vertical: Some(selected_vertical),
        rectangles: Some(completion.rectangles),
        verification: VerificationSummary {
            objective_verified: true,
            matching_verified: true,
            cover_verified: true,
            rectangles_verified: true,
            source_target_met: recovered_cost <= config.target,
            no_fallback: true,
        },
        target_certificate: Some(TargetCertificate {
            target: config.target,
            recovered_cost,
            accepted_updates: u64::try_from(completed.completion.records.len())
                .map_err(|_| LayeredError::Source("source update count overflow".to_owned()))?,
        }),
    })
}

/// Verifies a caller-supplied dual lower-bound certificate.
///
/// Returns the certified dual objective only when it is exactly feasible and
/// strictly greater than `target`; otherwise it returns a certificate error.
///
/// # Errors
///
/// Returns `LayeredError::Certificate` when the certificate is not exactly
/// feasible or does not prove a strict lower bound.
pub fn verify_source_infeasible_below(
    network: &graph::CirculationNetwork,
    target: i128,
    dual: &DualLowerBoundCertificate,
) -> Result<graph::ExactRatio, LayeredError> {
    Backend
        .prove_infeasible_below(network, target, dual)
        .map(|proof| proof.dual_objective)
        .map_err(|error| LayeredError::Certificate(error.to_string()))
}

/// Verifies a caller-supplied compressed cover-below certificate.
///
/// # Errors
///
/// Returns `LayeredError::Certificate` when the cover does not prove
/// `F_opt > target`.
pub fn verify_cover_below(
    circulation: &Circulation,
    cover: &VertexCover,
    target: i128,
) -> Result<CoverBelowProof, LayeredError> {
    circulation
        .certify_cover_below(cover, target)
        .map_err(|error| LayeredError::Certificate(error.to_string()))
}

/// Verifies a recovered `MinCostSolution` is feasible for `network` and that
/// its cost is at most `target`.
///
/// # Errors
///
/// Returns `LayeredError::Certificate` when the solution is infeasible or its
/// cost exceeds the target.
pub fn verify_source_feasible_at_most(
    network: &graph::CirculationNetwork,
    solution: &MinCostSolution,
    target: i128,
) -> Result<(), LayeredError> {
    if solution.cost > target {
        return Err(LayeredError::Certificate(
            "recovered cost exceeds the target".to_owned(),
        ));
    }
    network
        .verify_feasible_solution(solution)
        .map_err(|error| LayeredError::Certificate(error.to_string()))
}

fn reference_matching_pairs(matching: &graph::Matching) -> Vec<(usize, usize)> {
    matching
        .left_to_right
        .iter()
        .enumerate()
        .filter_map(|(left, right)| right.map(|right| (left, right)))
        .collect()
}

fn reference_result(analysis: &dominance::formal::FormalCompletionAnalysis) -> LayeredResult {
    let admissible = &analysis.admissible;
    LayeredResult {
        provenance: SolverProvenance::ReferenceExact,
        objective: admissible.optimum_rectangle_count,
        matching: Some(reference_matching_pairs(&admissible.explicit_matching)),
        vertex_cover: Some(admissible.explicit_vertex_cover.clone()),
        selected_horizontal: Some(admissible.selected_horizontal.clone()),
        selected_vertical: Some(admissible.selected_vertical.clone()),
        rectangles: Some(analysis.completion.rectangles.clone()),
        verification: VerificationSummary {
            objective_verified: true,
            matching_verified: true,
            cover_verified: true,
            rectangles_verified: true,
            source_target_met: false,
            no_fallback: true,
        },
        target_certificate: None,
    }
}

#[cfg(test)]
mod tests {
    use graph::{CirculationNetwork, ExactRatio, FlowNodeId};
    use mrd_domain::{
        FormalRectilinearPolygon, Ornament, OrnamentSegment, OrthogonalLoop, Point,
        RectilinearPolygon,
    };
    use serde_json;

    use super::{
        FixedPointConfigSpec, LayeredError, RatioSpec, SolverProvenance, SourceConfig,
        solve_reference, solve_source_with_target, verify_source_infeasible_below,
    };

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    fn formal_source_figure_three() -> FormalRectilinearPolygon {
        FormalRectilinearPolygon::new(
            RectilinearPolygon::new(rectangle(0, 0, 12, 12), vec![rectangle(2, 6, 5, 9)]).unwrap(),
            Ornament {
                isolated_points: vec![Point::new(6, 3), Point::new(6, 9), Point::new(8, 9)],
                segments: vec![
                    OrnamentSegment::new(Point::new(10, 0), Point::new(10, 3)).unwrap(),
                    OrnamentSegment::new(Point::new(2, 3), Point::new(5, 3)).unwrap(),
                    OrnamentSegment::new(Point::new(10, 6), Point::new(12, 6)).unwrap(),
                    OrnamentSegment::new(Point::new(10, 9), Point::new(10, 12)).unwrap(),
                ],
            },
        )
        .unwrap()
    }

    const FIGURE_CONFIG: FixedPointConfigSpec = FixedPointConfigSpec {
        input_encoding_bits: 1 << 20,
        fractional_bits: 96,
        series_terms: 48,
        word_log_exponent: 4,
    };

    fn source_config(target: i128) -> SourceConfig {
        SourceConfig {
            target,
            maximum_abs_input: 3,
            fixed_point: FIGURE_CONFIG,
            kappa: RatioSpec {
                numerator: 1,
                denominator: 2,
            },
        }
    }

    #[test]
    fn reference_mode_returns_provenance_and_verified_output() {
        let polygon = formal_source_figure_three();
        let result = solve_reference(&polygon).unwrap();
        assert_eq!(result.provenance, SolverProvenance::ReferenceExact);
        assert!(result.objective > 0);
        assert!(result.matching.is_some());
        assert!(result.vertex_cover.is_some());
        assert!(result.selected_horizontal.is_some());
        assert!(result.selected_vertical.is_some());
        assert!(result.rectangles.is_some());
        assert_eq!(result.rectangles.as_ref().unwrap().len(), result.objective);
        assert!(result.target_certificate.is_none());
        assert!(result.verification.no_fallback);
    }

    #[test]
    fn reference_result_serializes_deterministically_with_provenance() {
        let polygon = formal_source_figure_three();
        let result = solve_reference(&polygon).unwrap();
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains("ReferenceExact"));
        let reparsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reparsed["provenance"], "ReferenceExact");
        assert!(reparsed["objective"].is_number());
    }

    #[test]
    #[ignore = "the Appendix B.1 source path on the Figure 3 fixture is slow; run manually"]
    fn source_with_target_returns_certified_at_most_provenance_on_supported_fixture() {
        let polygon = formal_source_figure_three();
        let reference = solve_reference(&polygon).unwrap();
        // The optimum rectangle count equals `-F_opt` encoded through the
        // compressed return arc; target the recovered optimum directly. The
        // source path may complete (returning `SourceCertifiedAtMost`) or may
        // fail before a certified result (returning an explicit source error);
        // in every case it must never silently fall back to a reference result.
        let config = source_config(-(reference.objective as i128));
        match solve_source_with_target(&polygon, &config) {
            Ok(result) => {
                assert_eq!(
                    result.provenance,
                    SolverProvenance::SourceCertifiedAtMost {
                        target: -(reference.objective as i128)
                    }
                );
                assert_eq!(result.objective, reference.objective);
                assert!(result.target_certificate.is_some());
                let certificate = result.target_certificate.unwrap();
                assert!(certificate.recovered_cost <= config.target);
                assert!(result.verification.no_fallback);
                assert!(result.verification.source_target_met);
                assert!(result.verification.verified());
            }
            Err(error) => {
                // Honest failure: the source path could not certify a result.
                // It is never a fallback and never an implicit reference result.
                assert!(
                    matches!(
                        error,
                        LayeredError::Source(_) | LayeredError::UnsupportedOrUndetermined(_)
                    ),
                    "unexpected error: {error:?}"
                );
            }
        }
    }

    #[test]
    fn source_with_target_reports_a_source_failure_honestly() {
        // A very negative target cannot certify a strict augmented initial
        // point; the failure is an explicit source error, never a fallback.
        let polygon = formal_source_figure_three();
        let config = source_config(i128::MIN / 2);
        let outcome = solve_source_with_target(&polygon, &config);
        assert!(outcome.is_err());
    }

    #[test]
    fn verify_dual_lower_bound_certificate_is_exact() {
        let mut network = CirculationNetwork::new(2);
        network.add_arc(FlowNodeId(0), FlowNodeId(1), 2, 1).unwrap();
        network.add_arc(FlowNodeId(1), FlowNodeId(0), 2, 0).unwrap();
        let dual = graph::source_flow::certificate::DualLowerBoundCertificate::from_potentials(
            &network,
            vec![
                ExactRatio::new(0, 1).unwrap(),
                ExactRatio::new(0, 1).unwrap(),
            ],
        )
        .unwrap();
        let objective = verify_source_infeasible_below(&network, -1, &dual).unwrap();
        assert_eq!(objective, ExactRatio::new(0, 1).unwrap());
        assert!(verify_source_infeasible_below(&network, 0, &dual).is_err());
        assert!(matches!(
            verify_source_infeasible_below(&network, 0, &dual),
            Err(LayeredError::Certificate(_))
        ));
    }

    #[test]
    fn solver_mode_has_no_automatic_source_variant() {
        // The mode enum has exactly Reference and SourceWithTarget; there is no
        // automatic-source constructor to call.
        let mode = super::SolverMode::Reference;
        assert!(matches!(mode, super::SolverMode::Reference));
    }

    #[test]
    fn fixed_point_config_spec_rejects_invalid_precision() {
        let invalid = FixedPointConfigSpec {
            input_encoding_bits: 0,
            fractional_bits: 96,
            series_terms: 48,
            word_log_exponent: 3,
        };
        assert!(invalid.into_config().is_err());
        let valid = FixedPointConfigSpec {
            input_encoding_bits: 1 << 20,
            fractional_bits: 96,
            series_terms: 48,
            word_log_exponent: 3,
        };
        assert!(valid.into_config().is_ok());
    }
}
