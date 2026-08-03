//! Evidence records for the layered public solver.
//!
//! The timing categories intentionally keep reference target provision outside
//! the source run.  A reference-derived target is useful experimental input,
//! not an automatic source-target search or a source-backend capability.

use std::time::{Duration, Instant};

use dominance::{
    biclique::Partition, compressed_flow::experiment::source::Circulation,
    embedding::DominanceEmbedding, formal::analyze_formal_admissible_family,
};
use mrd_domain::{
    FormalRectilinearPolygon, Ornament, OrnamentSegment, OrthogonalLoop, Point, RectilinearPolygon,
};
use serde::{Deserialize, Serialize};
use sg_oracle::polygon::CoordinateCompressedCompletion;

use super::{LayeredError, SolverProvenance, SourceConfig, solve_reference};

/// The isolated concern measured by one record.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// Complete permanent reference-backed solve.
    ReferenceComplete,
    /// Source path with a caller-known or reference-provided inclusive target.
    SourceWithKnownTarget,
    /// Independent certificate verification supplied by the caller.
    CertificateVerification,
    /// Formal-family and chord geometry only.
    GeometryOnly,
    /// Dominance embedding and compact biclique representation only.
    CompressedRepresentation,
    /// Rectangle completion from an already selected admissible family only.
    RecoveryOnly,
}

/// Which input representation produced a measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Input {
    /// The current formal-polygon pipeline.
    PolygonDerived,
    /// Reserved for P11's direct-grid path; no P10 row may claim it ran.
    DirectGrid,
}

/// How the inclusive source target became available to the experiment.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetProvider {
    /// This category has no source target.
    NotApplicable,
    /// The benchmark caller supplied the target.
    CallerSupplied,
    /// A separate reference solve supplied the target for this experiment.
    ReferenceExact { objective: usize },
}

/// Optional source-row configuration for the standard layered benchmark.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceTarget {
    /// Execute the source path under an explicitly caller-supplied target.
    CallerSupplied(i128),
    /// Measure a separate reference solve that supplies the source target.
    ReferenceProvided,
}

/// Honest outcome classification for one measurement.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "detail")]
pub enum Outcome {
    /// The measured operation and its explicit checks completed.
    Verified,
    /// The source path did not produce a certified result and did not fall back.
    SourceUndetermined(String),
    /// The requested operation failed outside the source-undetermined boundary.
    Failed(String),
    /// The requested representation is deliberately not implemented yet.
    Unavailable(String),
}

/// Timings in microseconds. `None` means a stage did not execute, rather than
/// a measured zero duration.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Timings {
    /// Time spent obtaining a reference-provided target, when requested.
    pub target_provider_microseconds: Option<u128>,
    /// Time spent constructing and checking formal chord geometry.
    pub geometry_microseconds: Option<u128>,
    /// Time spent building and checking the compact representation.
    pub compressed_representation_microseconds: Option<u128>,
    /// Time spent in the source-with-target entry point.
    pub source_microseconds: Option<u128>,
    /// Time spent completing selected chords into rectangles.
    pub recovery_microseconds: Option<u128>,
    /// Time spent in an independent certificate verifier.
    pub verification_microseconds: Option<u128>,
    /// End-to-end elapsed time for this row, including only stages it ran.
    pub total_hybrid_microseconds: u128,
}

/// One deterministic, serializable layered benchmark observation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub category: Category,
    pub input: Input,
    pub target_provider: TargetProvider,
    /// Exact source target, encoded in base ten because JSON numbers cannot
    /// represent every valid `i128` without loss.
    pub supplied_target_decimal: Option<String>,
    pub objective: Option<usize>,
    pub outcome: Outcome,
    pub timings: Timings,
}

/// A group of explicitly separated layered benchmark records.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub records: Vec<Record>,
}

impl Report {
    /// Runs the reproducible Figure 3 layered benchmark fixture. The default
    /// has no source row; source execution requires an explicit target policy.
    #[must_use]
    pub fn standard(source_target: Option<SourceTarget>) -> Self {
        let polygon = standard_polygon();
        let mut report = Self::polygon_baseline(&polygon);
        report.push_certificate_verification(certificate_sample);
        match source_target {
            Some(SourceTarget::CallerSupplied(target)) => {
                report.push_source_with_caller_target(&polygon, &SourceConfig::standard(target));
            }
            Some(SourceTarget::ReferenceProvided) => {
                report.push_source_with_reference_target(&polygon, SourceConfig::standard);
            }
            None => {}
        }
        report.push_direct_grid_unavailable(Category::CompressedRepresentation);
        report
    }

    /// Creates the safe polygon-derived categories that need no source target.
    #[must_use]
    pub fn polygon_baseline(polygon: &FormalRectilinearPolygon) -> Self {
        Self {
            records: vec![
                reference_complete(polygon),
                geometry_only(polygon),
                compressed_representation(polygon),
                recovery_only(polygon),
            ],
        }
    }

    /// Adds a source row whose target was supplied by the benchmark caller.
    pub fn push_source_with_caller_target(
        &mut self,
        polygon: &FormalRectilinearPolygon,
        config: &SourceConfig,
    ) {
        self.records
            .push(source_with_caller_target(polygon, config));
    }

    /// Adds a source row whose target was deliberately provided by a separately
    /// measured reference solve. This is an experimental label, not automatic
    /// target search in the source backend.
    pub fn push_source_with_reference_target<F>(
        &mut self,
        polygon: &FormalRectilinearPolygon,
        source_config: F,
    ) where
        F: FnOnce(i128) -> SourceConfig,
    {
        self.records
            .push(source_with_reference_target(polygon, source_config));
    }

    /// Adds an independently supplied certificate-verification observation.
    pub fn push_certificate_verification<F>(&mut self, verification: F)
    where
        F: FnOnce() -> Result<(), LayeredError>,
    {
        self.records.push(certificate_verification(verification));
    }

    /// Records that P11's direct-grid backend was intentionally not measured.
    pub fn push_direct_grid_unavailable(&mut self, category: Category) {
        self.records.push(Record {
            category,
            input: Input::DirectGrid,
            target_provider: TargetProvider::NotApplicable,
            supplied_target_decimal: None,
            objective: None,
            outcome: Outcome::Unavailable(
                "direct-grid parity is reserved for P11 and has no P10 measurement".to_owned(),
            ),
            timings: Timings::default(),
        });
    }
}

/// Measures the complete permanent reference-backed path.
#[must_use]
pub fn reference_complete(polygon: &FormalRectilinearPolygon) -> Record {
    let started = Instant::now();
    match solve_reference(polygon) {
        Ok(result)
            if result.provenance == SolverProvenance::ReferenceExact
                && reference_verification(&result.verification) =>
        {
            Record {
                category: Category::ReferenceComplete,
                input: Input::PolygonDerived,
                target_provider: TargetProvider::NotApplicable,
                supplied_target_decimal: None,
                objective: Some(result.objective),
                outcome: Outcome::Verified,
                timings: total_only(started.elapsed()),
            }
        }
        Ok(_) => failed(
            Category::ReferenceComplete,
            started.elapsed(),
            "reference result failed its provenance or verification contract",
        ),
        Err(error) => failed(
            Category::ReferenceComplete,
            started.elapsed(),
            error.to_string(),
        ),
    }
}

/// Measures formal-family construction independently of representation and
/// rectangle completion.
#[must_use]
pub fn geometry_only(polygon: &FormalRectilinearPolygon) -> Record {
    let started = Instant::now();
    match analyze_formal_admissible_family(polygon) {
        Ok(analysis) => Record {
            category: Category::GeometryOnly,
            input: Input::PolygonDerived,
            target_provider: TargetProvider::NotApplicable,
            supplied_target_decimal: None,
            objective: Some(analysis.optimum_rectangle_count),
            outcome: Outcome::Verified,
            timings: Timings {
                geometry_microseconds: Some(microseconds(started.elapsed())),
                total_hybrid_microseconds: microseconds(started.elapsed()),
                ..Timings::default()
            },
        },
        Err(error) => failed(Category::GeometryOnly, started.elapsed(), error.to_string()),
    }
}

/// Measures compact dominance construction after independently constructing
/// the formal family. The geometry and compact stages are reported separately.
#[must_use]
pub fn compressed_representation(polygon: &FormalRectilinearPolygon) -> Record {
    let started = Instant::now();
    let geometry_started = Instant::now();
    let analysis = match analyze_formal_admissible_family(polygon) {
        Ok(analysis) => analysis,
        Err(error) => {
            return failed(
                Category::CompressedRepresentation,
                started.elapsed(),
                error.to_string(),
            );
        }
    };
    let geometry = geometry_started.elapsed();
    let compact_started = Instant::now();
    let compact = (|| -> Result<(), String> {
        let embedding =
            DominanceEmbedding::new(&analysis.families.horizontal, &analysis.families.vertical)
                .map_err(|error| error.to_string())?;
        let partition =
            Partition::comparability_theorem_8(&embedding).map_err(|error| error.to_string())?;
        partition
            .verify_exact_partition(&analysis.explicit_conflict_graph)
            .map_err(|error| error.to_string())?;
        partition
            .verify_dominance_blocks(&embedding)
            .map_err(|error| error.to_string())?;
        Circulation::from_partition(
            analysis.families.horizontal.len(),
            analysis.families.vertical.len(),
            &partition,
        )
        .map_err(|error| error.to_string())?;
        Ok(())
    })();
    let compact_duration = compact_started.elapsed();
    match compact {
        Ok(()) => Record {
            category: Category::CompressedRepresentation,
            input: Input::PolygonDerived,
            target_provider: TargetProvider::NotApplicable,
            supplied_target_decimal: None,
            objective: Some(analysis.optimum_rectangle_count),
            outcome: Outcome::Verified,
            timings: Timings {
                geometry_microseconds: Some(microseconds(geometry)),
                compressed_representation_microseconds: Some(microseconds(compact_duration)),
                total_hybrid_microseconds: microseconds(started.elapsed()),
                ..Timings::default()
            },
        },
        Err(error) => failed(
            Category::CompressedRepresentation,
            started.elapsed(),
            error.to_string(),
        ),
    }
}

/// Measures rectangle completion from the already selected exact family.
#[must_use]
pub fn recovery_only(polygon: &FormalRectilinearPolygon) -> Record {
    let started = Instant::now();
    let geometry_started = Instant::now();
    let analysis = match analyze_formal_admissible_family(polygon) {
        Ok(analysis) => analysis,
        Err(error) => return failed(Category::RecoveryOnly, started.elapsed(), error.to_string()),
    };
    let geometry = geometry_started.elapsed();
    let recovery_started = Instant::now();
    let completion = CoordinateCompressedCompletion.complete_formal(
        polygon,
        &analysis.families.horizontal,
        &analysis.families.vertical,
        &analysis.selected_horizontal,
        &analysis.selected_vertical,
    );
    let recovery = recovery_started.elapsed();
    match completion {
        Ok(completion) if completion.rectangles.len() == analysis.optimum_rectangle_count => {
            Record {
                category: Category::RecoveryOnly,
                input: Input::PolygonDerived,
                target_provider: TargetProvider::NotApplicable,
                supplied_target_decimal: None,
                objective: Some(analysis.optimum_rectangle_count),
                outcome: Outcome::Verified,
                timings: Timings {
                    geometry_microseconds: Some(microseconds(geometry)),
                    recovery_microseconds: Some(microseconds(recovery)),
                    total_hybrid_microseconds: microseconds(started.elapsed()),
                    ..Timings::default()
                },
            }
        }
        Ok(completion) => failed(
            Category::RecoveryOnly,
            started.elapsed(),
            format!(
                "recovered {} rectangles, expected {}",
                completion.rectangles.len(),
                analysis.optimum_rectangle_count
            ),
        ),
        Err(error) => failed(Category::RecoveryOnly, started.elapsed(), error.to_string()),
    }
}

/// Measures a source run with an explicitly caller-supplied target.
#[must_use]
pub fn source_with_caller_target(
    polygon: &FormalRectilinearPolygon,
    config: &SourceConfig,
) -> Record {
    source_record(
        polygon,
        config,
        TargetProvider::CallerSupplied,
        None,
        Duration::ZERO,
    )
}

/// Measures a source run after a separate reference solve supplies its target.
/// The source path still receives that target as explicit input.
#[must_use]
pub fn source_with_reference_target<F>(
    polygon: &FormalRectilinearPolygon,
    source_config: F,
) -> Record
where
    F: FnOnce(i128) -> SourceConfig,
{
    let total_started = Instant::now();
    let provider_started = Instant::now();
    let reference = match solve_reference(polygon) {
        Ok(reference) => reference,
        Err(error) => {
            return failed(
                Category::SourceWithKnownTarget,
                total_started.elapsed(),
                error.to_string(),
            );
        }
    };
    if reference.provenance != SolverProvenance::ReferenceExact
        || !reference_verification(&reference.verification)
    {
        return failed(
            Category::SourceWithKnownTarget,
            total_started.elapsed(),
            "reference target provider failed its provenance or verification contract",
        );
    }
    let provider_duration = provider_started.elapsed();
    let Some(objective) = i128::try_from(reference.objective)
        .ok()
        .and_then(i128::checked_neg)
    else {
        return failed(
            Category::SourceWithKnownTarget,
            total_started.elapsed(),
            "reference objective cannot be represented as a source target",
        );
    };
    let config = source_config(objective);
    source_record(
        polygon,
        &config,
        TargetProvider::ReferenceExact {
            objective: reference.objective,
        },
        Some(provider_duration),
        total_started.elapsed(),
    )
}

/// Measures an exact certificate-verification callback independently from
/// source execution and target provision.
#[must_use]
pub fn certificate_verification<F>(verification: F) -> Record
where
    F: FnOnce() -> Result<(), LayeredError>,
{
    let started = Instant::now();
    match verification() {
        Ok(()) => Record {
            category: Category::CertificateVerification,
            input: Input::PolygonDerived,
            target_provider: TargetProvider::NotApplicable,
            supplied_target_decimal: None,
            objective: None,
            outcome: Outcome::Verified,
            timings: Timings {
                verification_microseconds: Some(microseconds(started.elapsed())),
                total_hybrid_microseconds: microseconds(started.elapsed()),
                ..Timings::default()
            },
        },
        Err(error) => failed(
            Category::CertificateVerification,
            started.elapsed(),
            error.to_string(),
        ),
    }
}

fn source_record(
    polygon: &FormalRectilinearPolygon,
    config: &SourceConfig,
    target_provider: TargetProvider,
    provider_duration: Option<Duration>,
    prior_duration: Duration,
) -> Record {
    let source_started = Instant::now();
    let outcome = super::execution::run_source_with_target(polygon, config);
    let source_duration = source_started.elapsed();
    match outcome {
        Ok(run)
            if run.result.provenance
                == SolverProvenance::SourceCertifiedAtMost {
                    target: config.target,
                }
                && run.result.verification.verified() =>
        {
            Record {
                category: Category::SourceWithKnownTarget,
                input: Input::PolygonDerived,
                target_provider,
                supplied_target_decimal: Some(config.target.to_string()),
                objective: Some(run.result.objective),
                outcome: Outcome::Verified,
                timings: Timings {
                    target_provider_microseconds: provider_duration.map(microseconds),
                    geometry_microseconds: Some(microseconds(run.stages.geometry)),
                    compressed_representation_microseconds: Some(microseconds(
                        run.stages.compressed_representation,
                    )),
                    source_microseconds: Some(microseconds(run.stages.source)),
                    recovery_microseconds: Some(microseconds(run.stages.recovery)),
                    verification_microseconds: Some(microseconds(run.stages.verification)),
                    total_hybrid_microseconds: microseconds(prior_duration + run.stages.total),
                },
            }
        }
        Ok(_) => source_undetermined(
            config,
            target_provider,
            provider_duration,
            source_duration,
            prior_duration + source_duration,
            "source result failed its provenance or verification contract".to_owned(),
        ),
        Err(error) => source_undetermined(
            config,
            target_provider,
            provider_duration,
            source_duration,
            prior_duration + source_duration,
            error.to_string(),
        ),
    }
}

fn source_undetermined(
    config: &SourceConfig,
    target_provider: TargetProvider,
    provider_duration: Option<Duration>,
    source_duration: Duration,
    total: Duration,
    detail: String,
) -> Record {
    Record {
        category: Category::SourceWithKnownTarget,
        input: Input::PolygonDerived,
        target_provider,
        supplied_target_decimal: Some(config.target.to_string()),
        objective: None,
        outcome: Outcome::SourceUndetermined(detail),
        timings: Timings {
            target_provider_microseconds: provider_duration.map(microseconds),
            source_microseconds: Some(microseconds(source_duration)),
            total_hybrid_microseconds: microseconds(total),
            ..Timings::default()
        },
    }
}

fn failed(category: Category, total: Duration, detail: impl Into<String>) -> Record {
    Record {
        category,
        input: Input::PolygonDerived,
        target_provider: TargetProvider::NotApplicable,
        supplied_target_decimal: None,
        objective: None,
        outcome: Outcome::Failed(detail.into()),
        timings: total_only(total),
    }
}

fn total_only(total: Duration) -> Timings {
    Timings {
        total_hybrid_microseconds: microseconds(total),
        ..Timings::default()
    }
}

fn reference_verification(summary: &super::VerificationSummary) -> bool {
    summary.objective_verified
        && summary.matching_verified
        && summary.cover_verified
        && summary.rectangles_verified
        && summary.no_fallback
        && !summary.source_target_met
}

fn microseconds(duration: Duration) -> u128 {
    duration.as_micros()
}

fn standard_polygon() -> FormalRectilinearPolygon {
    let rectangle = |x0, y0, x1, y1| {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    };
    FormalRectilinearPolygon::new(
        RectilinearPolygon::new(rectangle(0, 0, 12, 12), vec![rectangle(2, 6, 5, 9)])
            .expect("standard layered fixture is a valid ordinary polygon"),
        Ornament {
            isolated_points: vec![Point::new(6, 3), Point::new(6, 9), Point::new(8, 9)],
            segments: vec![
                OrnamentSegment::new(Point::new(10, 0), Point::new(10, 3))
                    .expect("standard layered fixture has a valid ornament segment"),
                OrnamentSegment::new(Point::new(2, 3), Point::new(5, 3))
                    .expect("standard layered fixture has a valid ornament segment"),
                OrnamentSegment::new(Point::new(10, 6), Point::new(12, 6))
                    .expect("standard layered fixture has a valid ornament segment"),
                OrnamentSegment::new(Point::new(10, 9), Point::new(10, 12))
                    .expect("standard layered fixture has a valid ornament segment"),
            ],
        },
    )
    .expect("standard layered fixture is a valid formal polygon")
}

fn certificate_sample() -> Result<(), LayeredError> {
    let mut network = graph::CirculationNetwork::new(2);
    network
        .add_arc(graph::FlowNodeId(0), graph::FlowNodeId(1), 2, 1)
        .map_err(|error| LayeredError::Certificate(error.to_string()))?;
    network
        .add_arc(graph::FlowNodeId(1), graph::FlowNodeId(0), 2, 0)
        .map_err(|error| LayeredError::Certificate(error.to_string()))?;
    let zero = graph::ExactRatio::new(0, 1).map_err(|_| LayeredError::InvalidRatio)?;
    let certificate = graph::source_flow::certificate::DualLowerBoundCertificate::from_potentials(
        &network,
        vec![zero.clone(), zero],
    )
    .map_err(|error| LayeredError::Certificate(error.to_string()))?;
    super::verify_source_infeasible_below(&network, -1, &certificate).map(|_| ())
}

#[cfg(test)]
mod tests {
    use mrd_domain::{
        FormalRectilinearPolygon, Ornament, OrnamentSegment, OrthogonalLoop, Point,
        RectilinearPolygon,
    };

    use super::{
        Category, Input, Outcome, Report, SourceTarget, TargetProvider, certificate_verification,
        compressed_representation, geometry_only, recovery_only, reference_complete,
    };

    fn rectangle(x0: i64, y0: i64, x1: i64, y1: i64) -> OrthogonalLoop {
        OrthogonalLoop::new(vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
        ])
    }

    fn polygon() -> FormalRectilinearPolygon {
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

    #[test]
    fn baseline_categories_are_separate_and_verified() {
        let report = Report::polygon_baseline(&polygon());
        assert_eq!(report.records.len(), 4);
        assert_eq!(report.records[0].category, Category::ReferenceComplete);
        assert_eq!(report.records[1].category, Category::GeometryOnly);
        assert_eq!(
            report.records[2].category,
            Category::CompressedRepresentation
        );
        assert_eq!(report.records[3].category, Category::RecoveryOnly);
        assert!(
            report
                .records
                .iter()
                .all(|record| record.outcome == Outcome::Verified),
            "records: {:#?}",
            report.records
        );
        assert!(
            report
                .records
                .iter()
                .all(|record| record.input == Input::PolygonDerived)
        );
    }

    #[test]
    fn individual_stage_records_keep_only_their_measured_stage() {
        let polygon = polygon();
        let reference = reference_complete(&polygon);
        let geometry = geometry_only(&polygon);
        let compressed = compressed_representation(&polygon);
        let recovery = recovery_only(&polygon);
        assert_eq!(reference.timings.source_microseconds, None);
        assert!(geometry.timings.geometry_microseconds.is_some());
        assert!(
            compressed
                .timings
                .compressed_representation_microseconds
                .is_some()
        );
        assert!(recovery.timings.recovery_microseconds.is_some());
    }

    #[test]
    fn certificate_callback_is_reported_without_source_timing() {
        let record = certificate_verification(|| Ok(()));
        assert_eq!(record.category, Category::CertificateVerification);
        assert_eq!(record.target_provider, TargetProvider::NotApplicable);
        assert_eq!(record.outcome, Outcome::Verified);
        assert!(record.timings.verification_microseconds.is_some());
        assert_eq!(record.timings.source_microseconds, None);
    }

    #[test]
    fn direct_grid_is_explicitly_unavailable_before_p11() {
        let mut report = Report::default();
        report.push_direct_grid_unavailable(Category::CompressedRepresentation);
        let record = report.records.pop().unwrap();
        assert_eq!(record.input, Input::DirectGrid);
        assert!(matches!(record.outcome, Outcome::Unavailable(_)));
    }

    #[test]
    fn standard_report_uses_only_explicit_source_policy() {
        let default = Report::standard(None);
        assert!(
            default
                .records
                .iter()
                .all(|record| record.category != Category::SourceWithKnownTarget)
        );
        let configured = Report::standard(Some(SourceTarget::CallerSupplied(i128::MIN / 2)));
        let source = configured
            .records
            .iter()
            .find(|record| record.category == Category::SourceWithKnownTarget)
            .unwrap();
        assert_eq!(source.target_provider, TargetProvider::CallerSupplied);
        assert!(matches!(source.outcome, Outcome::SourceUndetermined(_)));
    }
}
