//! Independent bounded raster Oracle and grid/polygon differential tests.

use std::collections::BTreeSet;

use rect_core::{
    ColorGrid, CoordinateRect, DoubledPoint, HorizontalChord, PolygonDissectionResult,
    PolygonGeometryBackend, PreparedPolygonContext, RectilinearPolygon, VerticalChord, polygon,
};
use rect_dominance::{
    ConflictRepresentationBackend, PolygonArrangementBackend, PolygonChordBackend,
    PolygonCompletionBackend, PolygonSolveOptions, VerificationMode, solve_polygon,
    solve_polygon_with_options,
};
use rect_oracle_sg::{
    CleanHoleFreeCertificate, CoordinateCompressedCompletion, EffectiveChordEndpointIndex,
    GeneralPolygonPairwiseEnumerator, IndexedPolygonCompletion, IndexedPolygonPairwiseEnumerator,
    PolygonChordEnumerationMetrics, PolygonCompletionResult, PolygonDissectionValidatorBackend,
    PolygonRecoveryBackend, PreparedCoordinateArrangement, SoltanGorpinevichSweepEnumerator,
    SparseOrthogonalSubdivision, SparseSlabValidator, SparseValidatorBackend,
    SubdivisionBuilderBackend, SweepCertificate, classify_clean_polygon, polygon_cut_index,
    validate_polygon_dissection,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RasterLimits {
    pub max_width: usize,
    pub max_height: usize,
    pub max_cells: usize,
}

impl Default for RasterLimits {
    fn default() -> Self {
        Self {
            max_width: 256,
            max_height: 256,
            max_cells: 65_536,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RasterizedPolygon {
    pub origin_x: i64,
    pub origin_y: i64,
    pub grid: ColorGrid<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolygonVerificationInputSummary {
    pub boundary_complexity: usize,
    pub outer_vertices: usize,
    pub hole_count: usize,
    pub hole_vertices: usize,
    pub twice_area: i128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolygonGeometryEvidence {
    pub normalized_polygon: RectilinearPolygon,
    pub reflex_vertices: Vec<rect_core::Point>,
    pub horizontal_chords: Vec<HorizontalChord>,
    pub vertical_chords: Vec<VerticalChord>,
    pub endpoint_index: EffectiveChordEndpointIndex,
    pub clean_certificate: CleanHoleFreeCertificate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolygonRepresentationEvidence {
    pub production: PolygonDissectionResult,
    pub sweep: PolygonDissectionResult,
    pub dominance_4d: PolygonDissectionResult,
    pub auto: PolygonDissectionResult,
    pub clean_path_tree: Option<PolygonDissectionResult>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolygonCompletionEvidence {
    pub reference: PolygonCompletionResult,
    pub line_map_dense: PolygonCompletionResult,
    pub dynamic_dense: PolygonCompletionResult,
    pub indexed: PolygonCompletionResult,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct PolygonValidatorEvidence {
    pub reference_accepts_reference: bool,
    pub reference_accepts_indexed: bool,
    pub indexed_accepts_reference: bool,
    pub indexed_accepts_indexed: bool,
    pub sparse_accepts_reference: bool,
    pub sparse_accepts_indexed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolygonRasterEvidence {
    pub origin_x: i64,
    pub origin_y: i64,
    pub optimum_rectangle_count: usize,
    pub rectangles: Vec<CoordinateRect>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SweepCertificateSummary {
    pub output_record_count: usize,
    pub event_summary_count: usize,
    pub event_trace_truncated: bool,
}

impl From<&SweepCertificate> for SweepCertificateSummary {
    fn from(certificate: &SweepCertificate) -> Self {
        Self {
            output_record_count: certificate.output_records.len(),
            event_summary_count: certificate.event_summaries.len(),
            event_trace_truncated: certificate.event_trace_truncated,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PolygonVerificationReport {
    pub input_summary: PolygonVerificationInputSummary,
    pub reference_geometry: PolygonGeometryEvidence,
    pub indexed_geometry: PolygonGeometryEvidence,
    pub sweep_geometry: PolygonGeometryEvidence,
    pub three_backend_chord_equality: bool,
    pub sweep_metrics: PolygonChordEnumerationMetrics,
    pub sweep_certificate_summary: Option<SweepCertificateSummary>,
    pub sweep_fallback_used: bool,
    pub representation_results: PolygonRepresentationEvidence,
    pub completion_results: PolygonCompletionEvidence,
    pub validator_results: PolygonValidatorEvidence,
    pub raster_oracle: Option<PolygonRasterEvidence>,
    pub disagreements: Vec<String>,
    pub disagreement_classifications: Vec<String>,
}

impl PolygonVerificationReport {
    #[must_use]
    pub fn verified(&self) -> bool {
        self.disagreements.is_empty()
    }
}

/// Runs the complete reference-versus-indexed polygon verification stack.
///
/// # Errors
///
/// Returns [`PolygonVerificationError`] when a backend cannot produce an
/// exact result. Semantic differences are retained in `disagreements`.
#[allow(clippy::too_many_lines)]
pub fn verify_polygon(
    polygon: &RectilinearPolygon,
    raster_limits: Option<RasterLimits>,
) -> Result<PolygonVerificationReport, PolygonVerificationError> {
    let reference_prepared =
        PreparedPolygonContext::new_with_validator(polygon, polygon::Backend::Oracle).map_err(
            |error| PolygonVerificationError::Backend {
                backend: "reference-quadratic",
                message: error.to_string(),
            },
        )?;
    let indexed_prepared =
        PreparedPolygonContext::new_with_validator(polygon, polygon::Backend::Experiment).map_err(
            |error| PolygonVerificationError::Backend {
                backend: "orthogonal-sweep",
                message: error.to_string(),
            },
        )?;
    let reference_families = GeneralPolygonPairwiseEnumerator
        .enumerate_prepared(&reference_prepared)
        .map_err(|error| PolygonVerificationError::Backend {
            backend: "reference-pairwise",
            message: error.to_string(),
        })?;
    let indexed_families = IndexedPolygonPairwiseEnumerator
        .enumerate_prepared(&indexed_prepared)
        .map_err(|error| PolygonVerificationError::Backend {
            backend: "indexed-pairwise",
            message: error.to_string(),
        })?
        .families;
    let sweep_result = SoltanGorpinevichSweepEnumerator
        .enumerate_prepared(&indexed_prepared)
        .map_err(|error| PolygonVerificationError::Backend {
            backend: "sg-sweep",
            message: error.to_string(),
        })?;
    let sweep_families = sweep_result.families.clone();
    let sweep_metrics = sweep_result.metrics.clone();
    let sweep_certificate_summary = sweep_result
        .sweep_certificate
        .as_ref()
        .map(SweepCertificateSummary::from);
    let reference_geometry = geometry_evidence(&reference_prepared, &reference_families)?;
    let indexed_geometry = geometry_evidence(&indexed_prepared, &indexed_families)?;
    let sweep_geometry = geometry_evidence(&indexed_prepared, &sweep_families)?;

    let production = solve_polygon(polygon).map_err(|error| PolygonVerificationError::Backend {
        backend: "production",
        message: error.to_string(),
    })?;
    let audited_options = PolygonSolveOptions {
        verification_mode: VerificationMode::FullyAudited,
        geometry_backend: PolygonGeometryBackend::Indexed,
        validation_backend: polygon::Backend::Experiment,
        chord_backend: PolygonChordBackend::IndexedPairwise,
        completion_backend: PolygonCompletionBackend::IndexedFrontier,
        cut_index_backend: polygon_cut_index::Backend::Experiment,
        recovery_backend: PolygonRecoveryBackend::SparseSubdivision,
        dissection_validator_backend: PolygonDissectionValidatorBackend::SparseSlab,
        subdivision_builder_backend: SubdivisionBuilderBackend::OrthogonalSweep,
        sparse_validator_backend: SparseValidatorBackend::EventSegmentTree,
        arrangement_backend: PolygonArrangementBackend::Indexed,
        representation: ConflictRepresentationBackend::GeneralDominance4D,
    };
    let dominance_4d = solve_polygon_with_options(polygon, audited_options).map_err(|error| {
        PolygonVerificationError::Backend {
            backend: "dominance-4d",
            message: error.to_string(),
        }
    })?;
    let sweep = solve_polygon_with_options(
        polygon,
        PolygonSolveOptions {
            chord_backend: PolygonChordBackend::SoltanGorpinevichSweep,
            ..audited_options
        },
    )
    .map_err(|error| PolygonVerificationError::Backend {
        backend: "sg-sweep-solver",
        message: error.to_string(),
    })?;
    let auto = solve_polygon_with_options(
        polygon,
        PolygonSolveOptions {
            representation: ConflictRepresentationBackend::Auto,
            ..audited_options
        },
    )
    .map_err(|error| PolygonVerificationError::Backend {
        backend: "auto",
        message: error.to_string(),
    })?;
    let clean_path_tree = reference_geometry.clean_certificate.eligible.then(|| {
        solve_polygon_with_options(
            polygon,
            PolygonSolveOptions {
                representation: ConflictRepresentationBackend::CleanHoleFreePathTree,
                ..audited_options
            },
        )
    });
    let clean_path_tree =
        clean_path_tree
            .transpose()
            .map_err(|error| PolygonVerificationError::Backend {
                backend: "clean-path-tree",
                message: error.to_string(),
            })?;

    let selected_horizontal = selected_flags(
        &production,
        "selected_horizontal",
        reference_families.horizontal.len(),
    )?;
    let selected_vertical = selected_flags(
        &production,
        "selected_vertical",
        reference_families.vertical.len(),
    )?;
    let reference_completion = CoordinateCompressedCompletion
        .complete_prepared(
            &reference_prepared,
            &reference_families.horizontal,
            &reference_families.vertical,
            &selected_horizontal,
            &selected_vertical,
        )
        .map_err(|error| PolygonVerificationError::Backend {
            backend: "coordinate-reference",
            message: error.to_string(),
        })?;
    let line_map_dense_completion = IndexedPolygonCompletion
        .complete_prepared_with_backends(
            &indexed_prepared,
            &indexed_families.horizontal,
            &indexed_families.vertical,
            &selected_horizontal,
            &selected_vertical,
            polygon_cut_index::Backend::Oracle,
            PolygonRecoveryBackend::DenseCoordinateArrangement,
            PolygonDissectionValidatorBackend::DenseArrangement,
        )
        .map_err(|error| PolygonVerificationError::Backend {
            backend: "line-map-dense",
            message: error.to_string(),
        })?;
    let dynamic_dense_completion = IndexedPolygonCompletion
        .complete_prepared_with_backends(
            &indexed_prepared,
            &indexed_families.horizontal,
            &indexed_families.vertical,
            &selected_horizontal,
            &selected_vertical,
            polygon_cut_index::Backend::Experiment,
            PolygonRecoveryBackend::DenseCoordinateArrangement,
            PolygonDissectionValidatorBackend::DenseArrangement,
        )
        .map_err(|error| PolygonVerificationError::Backend {
            backend: "dynamic-dense",
            message: error.to_string(),
        })?;
    let indexed_completion = IndexedPolygonCompletion
        .complete_prepared_with_backends(
            &indexed_prepared,
            &indexed_families.horizontal,
            &indexed_families.vertical,
            &selected_horizontal,
            &selected_vertical,
            polygon_cut_index::Backend::Experiment,
            PolygonRecoveryBackend::SparseSubdivision,
            PolygonDissectionValidatorBackend::SparseSlab,
        )
        .map_err(|error| PolygonVerificationError::Backend {
            backend: "indexed-frontier",
            message: error.to_string(),
        })?;
    let horizontal_cuts = indexed_completion
        .selected_horizontal_cuts
        .iter()
        .chain(&indexed_completion.added_horizontal_cuts)
        .copied()
        .collect::<BTreeSet<_>>();
    let vertical_cuts = indexed_completion
        .selected_vertical_cuts
        .iter()
        .chain(&indexed_completion.added_vertical_cuts)
        .copied()
        .collect::<BTreeSet<_>>();
    let arrangement =
        PreparedCoordinateArrangement::new(&indexed_prepared, &horizontal_cuts, &vertical_cuts)
            .map_err(|error| PolygonVerificationError::Backend {
                backend: "indexed-arrangement",
                message: error.to_string(),
            })?;
    let reference_subdivision = SparseOrthogonalSubdivision::new_with_backend(
        &indexed_prepared,
        &horizontal_cuts,
        &vertical_cuts,
        SubdivisionBuilderBackend::ReferenceRangeScan,
    )
    .map_err(|error| PolygonVerificationError::Backend {
        backend: "reference-range-scan-subdivision",
        message: error.to_string(),
    })?;
    let sweep_subdivision = SparseOrthogonalSubdivision::new_with_backend(
        &indexed_prepared,
        &horizontal_cuts,
        &vertical_cuts,
        SubdivisionBuilderBackend::OrthogonalSweep,
    )
    .map_err(|error| PolygonVerificationError::Backend {
        backend: "orthogonal-sweep-subdivision",
        message: error.to_string(),
    })?;
    let reference_sparse_validation = SparseSlabValidator.validate_with_backend(
        indexed_prepared.polygon(),
        &indexed_completion.rectangles,
        SparseValidatorBackend::ReferenceSlabRescan,
    );
    let event_sparse_validation = SparseSlabValidator.validate_with_backend(
        indexed_prepared.polygon(),
        &indexed_completion.rectangles,
        SparseValidatorBackend::EventSegmentTree,
    );
    let validator_results = PolygonValidatorEvidence {
        reference_accepts_reference: validate_polygon_dissection(
            indexed_prepared.polygon(),
            &reference_completion.rectangles,
        )
        .is_ok(),
        reference_accepts_indexed: validate_polygon_dissection(
            indexed_prepared.polygon(),
            &indexed_completion.rectangles,
        )
        .is_ok(),
        indexed_accepts_reference: arrangement
            .validate_rectangles(indexed_prepared.polygon(), &reference_completion.rectangles)
            .is_ok(),
        indexed_accepts_indexed: arrangement
            .validate_rectangles(indexed_prepared.polygon(), &indexed_completion.rectangles)
            .is_ok(),
        sparse_accepts_reference: SparseSlabValidator
            .validate(indexed_prepared.polygon(), &reference_completion.rectangles)
            .is_ok(),
        sparse_accepts_indexed: SparseSlabValidator
            .validate(indexed_prepared.polygon(), &indexed_completion.rectangles)
            .is_ok(),
    };
    let raster_oracle = raster_limits
        .and_then(|limits| bounded_rasterize_polygon(indexed_prepared.polygon(), limits).ok())
        .map(|rasterized| raster_evidence(&rasterized))
        .transpose()?;

    let mut disagreements = Vec::new();
    let three_backend_chord_equality =
        reference_geometry == indexed_geometry && reference_geometry == sweep_geometry;
    if !three_backend_chord_equality {
        disagreements.push("reference, indexed, and sweep geometry differ".to_owned());
    }
    for (name, result) in [
        ("production", &production),
        ("sg-sweep", &sweep),
        ("auto", &auto),
        ("dominance-4d", &dominance_4d),
    ] {
        if result.optimum_rectangle_count != dominance_4d.optimum_rectangle_count
            || result.rectangles != dominance_4d.rectangles
        {
            disagreements.push(format!(
                "{name} representation result differs from dominance-4d"
            ));
        }
    }
    if let Some(path_tree) = &clean_path_tree
        && (path_tree.optimum_rectangle_count != dominance_4d.optimum_rectangle_count
            || path_tree.rectangles != dominance_4d.rectangles)
    {
        disagreements.push("clean path-tree result differs from dominance-4d".to_owned());
    }
    if reference_completion.selected_horizontal_cuts != indexed_completion.selected_horizontal_cuts
        || reference_completion.selected_vertical_cuts != indexed_completion.selected_vertical_cuts
        || reference_completion.added_horizontal_cuts != indexed_completion.added_horizontal_cuts
        || reference_completion.added_vertical_cuts != indexed_completion.added_vertical_cuts
        || reference_completion.rectangles != indexed_completion.rectangles
    {
        disagreements.push("reference and indexed completion differ".to_owned());
    }
    if reference_subdivision.split_junctions != sweep_subdivision.split_junctions
        || reference_subdivision.atomic_segments != sweep_subdivision.atomic_segments
        || reference_subdivision.vertices != sweep_subdivision.vertices
        || reference_subdivision.half_edges != sweep_subdivision.half_edges
        || reference_subdivision.faces != sweep_subdivision.faces
        || reference_subdivision
            .recover_rectangles(indexed_prepared.polygon())
            .map_err(|error| error.to_string())
            != sweep_subdivision
                .recover_rectangles(indexed_prepared.polygon())
                .map_err(|error| error.to_string())
    {
        disagreements.push("reference and sweep subdivisions differ".to_owned());
    }
    if sweep_subdivision.metrics.candidate_pair_tests != 0 {
        disagreements.push("orthogonal sweep reported candidate-pair traversal".to_owned());
    }
    if reference_sparse_validation.as_ref().map(|_| ())
        != event_sparse_validation.as_ref().map(|_| ())
    {
        disagreements.push("reference and event sparse validators differ".to_owned());
    }
    if event_sparse_validation.as_ref().is_ok_and(|metrics| {
        metrics.boundary_edge_scans != 0 || metrics.active_rectangle_resorts != 0
    }) {
        disagreements.push("event validator reported forbidden slab rescans".to_owned());
    }
    for (name, completion) in [
        ("line-map-dense", &line_map_dense_completion),
        ("dynamic-dense", &dynamic_dense_completion),
    ] {
        if completion.selected_horizontal_cuts != indexed_completion.selected_horizontal_cuts
            || completion.selected_vertical_cuts != indexed_completion.selected_vertical_cuts
            || completion.added_horizontal_cuts != indexed_completion.added_horizontal_cuts
            || completion.added_vertical_cuts != indexed_completion.added_vertical_cuts
            || completion.rectangles != indexed_completion.rectangles
        {
            disagreements.push(format!(
                "{name} completion differs from sparse dynamic completion"
            ));
        }
    }
    if indexed_completion.metrics.cut_index.coordinate_line_scans != 0
        || indexed_completion.metrics.cut_index.interval_scans != 0
    {
        disagreements.push("dynamic cut index reported a forbidden linear scan".to_owned());
    }
    let production_trace = &production.diagnostics.execution_trace;
    if production_trace.dense_atomic_cells_materialized
        || production_trace.dense_occupied_array_materialized
        || production_trace.dense_horizontal_barrier_array_materialized
        || production_trace.dense_vertical_barrier_array_materialized
        || production_trace.dense_coverage_difference_array_materialized
    {
        disagreements.push("CompactOnly polygon path materialized a dense arrangement".to_owned());
    }
    if !validator_results.reference_accepts_reference
        || !validator_results.reference_accepts_indexed
        || !validator_results.indexed_accepts_reference
        || !validator_results.indexed_accepts_indexed
        || !validator_results.sparse_accepts_reference
        || !validator_results.sparse_accepts_indexed
    {
        disagreements.push("one or more exact validators rejected a completion".to_owned());
    }
    if let Some(raster) = &raster_oracle
        && (raster.optimum_rectangle_count != dominance_4d.optimum_rectangle_count
            || raster.rectangles != dominance_4d.rectangles)
    {
        disagreements.push("bounded raster Oracle differs from polygon solve".to_owned());
    }
    let disagreement_classifications = disagreements
        .iter()
        .map(|message| {
            if message.contains("geometry") {
                "chord-family".to_owned()
            } else if message.contains("completion") {
                "completion".to_owned()
            } else if message.contains("validator") {
                "validation".to_owned()
            } else if message.contains("raster") {
                "raster-oracle".to_owned()
            } else {
                "downstream-solver".to_owned()
            }
        })
        .collect();

    Ok(PolygonVerificationReport {
        input_summary: PolygonVerificationInputSummary {
            boundary_complexity: indexed_prepared.polygon().boundary_complexity(),
            outer_vertices: indexed_prepared.polygon().outer.vertices.len(),
            hole_count: indexed_prepared.polygon().holes.len(),
            hole_vertices: indexed_prepared.polygon().hole_vertex_count(),
            twice_area: indexed_prepared
                .polygon()
                .twice_signed_area()
                .map_err(|error| PolygonVerificationError::Backend {
                    backend: "area",
                    message: error.to_string(),
                })?,
        },
        reference_geometry,
        indexed_geometry,
        sweep_geometry,
        three_backend_chord_equality,
        sweep_metrics,
        sweep_certificate_summary,
        sweep_fallback_used: false,
        representation_results: PolygonRepresentationEvidence {
            production,
            sweep,
            dominance_4d,
            auto,
            clean_path_tree,
        },
        completion_results: PolygonCompletionEvidence {
            reference: reference_completion,
            line_map_dense: line_map_dense_completion,
            dynamic_dense: dynamic_dense_completion,
            indexed: indexed_completion,
        },
        validator_results,
        raster_oracle,
        disagreements,
        disagreement_classifications,
    })
}

fn geometry_evidence(
    prepared: &PreparedPolygonContext,
    families: &rect_oracle_sg::EffectiveChordFamilies,
) -> Result<PolygonGeometryEvidence, PolygonVerificationError> {
    let endpoint_index = EffectiveChordEndpointIndex::new(
        prepared.boundary_index(),
        &families.horizontal,
        &families.vertical,
    )
    .map_err(|error| PolygonVerificationError::Backend {
        backend: "endpoint-index",
        message: error.to_string(),
    })?;
    Ok(PolygonGeometryEvidence {
        normalized_polygon: prepared.polygon().clone(),
        reflex_vertices: prepared
            .boundary()
            .reflex_vertices
            .iter()
            .map(|vertex| vertex.point)
            .collect(),
        horizontal_chords: families.horizontal.clone(),
        vertical_chords: families.vertical.clone(),
        clean_certificate: classify_clean_polygon(
            prepared.polygon(),
            prepared.boundary(),
            &families.horizontal,
            &families.vertical,
            &endpoint_index,
        ),
        endpoint_index,
    })
}

fn selected_flags(
    result: &PolygonDissectionResult,
    key: &'static str,
    expected_len: usize,
) -> Result<Vec<bool>, PolygonVerificationError> {
    let flags = result
        .certificate
        .as_ref()
        .and_then(|certificate| certificate.payload.get(key))
        .and_then(serde_json::Value::as_array)
        .ok_or(PolygonVerificationError::Certificate { key })?
        .iter()
        .map(|value| {
            value
                .as_bool()
                .ok_or(PolygonVerificationError::Certificate { key })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if flags.len() != expected_len {
        return Err(PolygonVerificationError::Certificate { key });
    }
    Ok(flags)
}

fn raster_evidence(
    rasterized: &RasterizedPolygon,
) -> Result<PolygonRasterEvidence, PolygonVerificationError> {
    let component = rasterized
        .grid
        .four_connected_components()
        .into_iter()
        .find(|component| component.color)
        .ok_or(PolygonVerificationError::RasterForeground)?;
    let result =
        rect_dominance::solve_with_verification_mode(&component, VerificationMode::CompactOnly)
            .map_err(|error| PolygonVerificationError::Backend {
                backend: "bounded-raster",
                message: error.to_string(),
            })?;
    let rectangles = result
        .rectangles
        .into_iter()
        .map(|rectangle| {
            let x0 = rasterized
                .origin_x
                .checked_add(
                    i64::try_from(rectangle.x0)
                        .map_err(|_| PolygonVerificationError::CoordinateOverflow)?,
                )
                .ok_or(PolygonVerificationError::CoordinateOverflow)?;
            let y0 = rasterized
                .origin_y
                .checked_add(
                    i64::try_from(rectangle.y0)
                        .map_err(|_| PolygonVerificationError::CoordinateOverflow)?,
                )
                .ok_or(PolygonVerificationError::CoordinateOverflow)?;
            let x1 = rasterized
                .origin_x
                .checked_add(
                    i64::try_from(rectangle.x1)
                        .map_err(|_| PolygonVerificationError::CoordinateOverflow)?,
                )
                .ok_or(PolygonVerificationError::CoordinateOverflow)?;
            let y1 = rasterized
                .origin_y
                .checked_add(
                    i64::try_from(rectangle.y1)
                        .map_err(|_| PolygonVerificationError::CoordinateOverflow)?,
                )
                .ok_or(PolygonVerificationError::CoordinateOverflow)?;
            CoordinateRect::new(x0, y0, x1, y1)
                .map_err(|_| PolygonVerificationError::CoordinateOverflow)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PolygonRasterEvidence {
        origin_x: rasterized.origin_x,
        origin_y: rasterized.origin_y,
        optimum_rectangle_count: result.optimum_rectangle_count,
        rectangles,
    })
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PolygonVerificationError {
    #[error("polygon verification backend {backend} failed: {message}")]
    Backend {
        backend: &'static str,
        message: String,
    },
    #[error("polygon certificate is missing or has malformed field {key}")]
    Certificate { key: &'static str },
    #[error("bounded rasterization produced no foreground component")]
    RasterForeground,
    #[error("coordinate conversion overflowed while translating raster rectangles")]
    CoordinateOverflow,
}

/// Rasterizes a small integer-coordinate polygon for differential testing.
///
/// This function is an optional bounded Oracle. Production polygon solving
/// never calls it.
///
/// # Errors
///
/// Returns [`RasterOracleError`] if the coordinate bounding box exceeds an
/// explicit width, height, or cell limit.
pub fn bounded_rasterize_polygon(
    polygon: &RectilinearPolygon,
    limits: RasterLimits,
) -> Result<RasterizedPolygon, RasterOracleError> {
    let mut points = polygon
        .loops()
        .flat_map(|boundary_loop| boundary_loop.vertices.iter().copied());
    let first = points.next().ok_or(RasterOracleError::EmptyPolygon)?;
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
    for point in points {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    let width = usize::try_from(i128::from(max_x) - i128::from(min_x))
        .map_err(|_| RasterOracleError::DimensionOverflow)?;
    let height = usize::try_from(i128::from(max_y) - i128::from(min_y))
        .map_err(|_| RasterOracleError::DimensionOverflow)?;
    if width > limits.max_width {
        return Err(RasterOracleError::WidthLimit {
            actual: width,
            limit: limits.max_width,
        });
    }
    if height > limits.max_height {
        return Err(RasterOracleError::HeightLimit {
            actual: height,
            limit: limits.max_height,
        });
    }
    let cell_count = width
        .checked_mul(height)
        .ok_or(RasterOracleError::DimensionOverflow)?;
    if cell_count > limits.max_cells {
        return Err(RasterOracleError::CellLimit {
            actual: cell_count,
            limit: limits.max_cells,
        });
    }
    let mut cells = Vec::with_capacity(cell_count);
    for local_y in 0..height {
        let y = i128::from(min_y)
            .checked_add(i128::try_from(local_y).map_err(|_| RasterOracleError::DimensionOverflow)?)
            .ok_or(RasterOracleError::DimensionOverflow)?;
        for local_x in 0..width {
            let x = i128::from(min_x)
                .checked_add(
                    i128::try_from(local_x).map_err(|_| RasterOracleError::DimensionOverflow)?,
                )
                .ok_or(RasterOracleError::DimensionOverflow)?;
            cells.push(
                polygon.contains_doubled_point_strict(DoubledPoint::new(
                    x.checked_mul(2)
                        .and_then(|value| value.checked_add(1))
                        .ok_or(RasterOracleError::DimensionOverflow)?,
                    y.checked_mul(2)
                        .and_then(|value| value.checked_add(1))
                        .ok_or(RasterOracleError::DimensionOverflow)?,
                )),
            );
        }
    }
    Ok(RasterizedPolygon {
        origin_x: min_x,
        origin_y: min_y,
        grid: ColorGrid::new(width, height, cells)
            .map_err(|_| RasterOracleError::DimensionOverflow)?,
    })
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RasterOracleError {
    #[error("cannot rasterize an empty polygon")]
    EmptyPolygon,
    #[error("polygon raster dimensions overflow usize")]
    DimensionOverflow,
    #[error("polygon raster width {actual} exceeds limit {limit}")]
    WidthLimit { actual: usize, limit: usize },
    #[error("polygon raster height {actual} exceeds limit {limit}")]
    HeightLimit { actual: usize, limit: usize },
    #[error("polygon raster cell count {actual} exceeds limit {limit}")]
    CellLimit { actual: usize, limit: usize },
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rect_core::{
        Boundary, ColorGrid, CoordinateRect, GridComponent, OrthogonalLoop, Point,
        PolygonDissectionResult, RectilinearPolygon,
    };
    use rect_dominance::{
        ChordEnumerator, ConflictRepresentationBackend, VerificationMode, solve_polygon,
        solve_polygon_with_representation,
        solve_with_verification_mode_and_chord_enumerator_and_completion_backend,
    };
    use rect_oracle_sg::{
        CoordinateCompressedCompletion, GridInteriorRunEnumerator, HorizontalCutSegment,
        HorizontalUnitCut, IndexedFrontierCompletion, VerticalCutSegment, VerticalUnitCut,
        analyze_geometry_with, complete_with_prepared_backend,
    };

    use super::{RasterLimits, RasterOracleError, bounded_rasterize_polygon, verify_polygon};

    #[derive(Debug, Default)]
    struct DifferentialCounts {
        inputs: usize,
        components: usize,
        supported_components: usize,
        rejected_components: usize,
        path_tree_components: usize,
        four_d_fallback_components: usize,
    }

    #[allow(clippy::too_many_lines)]
    fn compare_component(
        component: &GridComponent<bool>,
        counts: &mut DifferentialCounts,
    ) -> Result<(), String> {
        counts.components += 1;
        let boundary = Boundary::from_component(component).map_err(|error| error.to_string())?;
        let Ok(polygon) = boundary.to_polygon() else {
            counts.rejected_components += 1;
            return Ok(());
        };
        counts.supported_components += 1;
        let geometry = analyze_geometry_with(component, &GridInteriorRunEnumerator)
            .map_err(|error| error.to_string())?;
        let polygon_families = rect_oracle_sg::GeneralPolygonPairwiseEnumerator
            .enumerate(&polygon)
            .map_err(|error| error.to_string())?;
        if geometry.horizontal_chords != polygon_families.horizontal
            || geometry.vertical_chords != polygon_families.vertical
        {
            return Err("effective chord families differ".to_owned());
        }

        let grid_result = solve_with_verification_mode_and_chord_enumerator_and_completion_backend(
            component,
            VerificationMode::CompactOnly,
            ChordEnumerator::GridInteriorRuns,
            rect_oracle_sg::CompletionBackendKind::IndexedFrontier,
        )
        .map_err(|error| error.to_string())?;
        let polygon_result = solve_polygon(&polygon).map_err(|error| error.to_string())?;
        let auto_result =
            solve_polygon_with_representation(&polygon, ConflictRepresentationBackend::Auto)
                .map_err(|error| error.to_string())?;
        if auto_result.optimum_rectangle_count != polygon_result.optimum_rectangle_count
            || auto_result.rectangles != polygon_result.rectangles
        {
            return Err("Auto and 4D polygon results differ".to_owned());
        }
        match auto_result.diagnostics.conflict_representation.as_deref() {
            Some("path-tree") => counts.path_tree_components += 1,
            Some("dominance-4d") => counts.four_d_fallback_components += 1,
            other => return Err(format!("unexpected Auto representation {other:?}")),
        }
        if grid_result.optimum_rectangle_count != polygon_result.optimum_rectangle_count {
            return Err("minimum rectangle counts differ".to_owned());
        }

        let selected_horizontal = selected_grid_flags(
            &grid_result,
            "selected_horizontal",
            geometry.horizontal_chords.len(),
        );
        let selected_vertical = selected_grid_flags(
            &grid_result,
            "selected_vertical",
            geometry.vertical_chords.len(),
        );
        let polygon_selected_horizontal = selected_polygon_flags(
            &polygon_result,
            "selected_horizontal",
            geometry.horizontal_chords.len(),
        );
        let polygon_selected_vertical = selected_polygon_flags(
            &polygon_result,
            "selected_vertical",
            geometry.vertical_chords.len(),
        );
        if selected_horizontal != polygon_selected_horizontal
            || selected_vertical != polygon_selected_vertical
        {
            return Err("minimum-cover selections differ".to_owned());
        }

        let grid_completion = complete_with_prepared_backend(
            component,
            &geometry.prepared,
            &geometry.horizontal_chords,
            &geometry.vertical_chords,
            &selected_horizontal,
            &selected_vertical,
            &IndexedFrontierCompletion,
        )
        .map_err(|error| error.to_string())?;
        let polygon_completion = CoordinateCompressedCompletion
            .complete(
                &polygon,
                &polygon_families.horizontal,
                &polygon_families.vertical,
                &polygon_selected_horizontal,
                &polygon_selected_vertical,
            )
            .map_err(|error| error.to_string())?;
        if merge_horizontal(&grid_completion.selected_horizontal_unit_cuts)
            != polygon_completion.selected_horizontal_cuts
            || merge_vertical(&grid_completion.selected_vertical_unit_cuts)
                != polygon_completion.selected_vertical_cuts
            || merge_horizontal(&grid_completion.added_horizontal_unit_cuts)
                != polygon_completion.added_horizontal_cuts
            || merge_vertical(&grid_completion.added_vertical_unit_cuts)
                != polygon_completion.added_vertical_cuts
        {
            return Err("selected or added cuts differ".to_owned());
        }

        let grid_rectangles = grid_result
            .rectangles
            .iter()
            .map(|rectangle| {
                CoordinateRect::new(
                    i64::try_from(rectangle.x0).map_err(|error| error.to_string())?,
                    i64::try_from(rectangle.y0).map_err(|error| error.to_string())?,
                    i64::try_from(rectangle.x1).map_err(|error| error.to_string())?,
                    i64::try_from(rectangle.y1).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        if grid_rectangles != polygon_result.rectangles
            || polygon_result.rectangles != polygon_completion.rectangles
        {
            return Err("canonical rectangles differ".to_owned());
        }
        Ok(())
    }

    fn selected_grid_flags(
        result: &rect_core::DissectionResult,
        key: &str,
        len: usize,
    ) -> Vec<bool> {
        let mut flags = vec![false; len];
        for index in result.certificate.as_ref().unwrap().payload[key]
            .as_array()
            .unwrap()
        {
            flags[usize::try_from(index.as_u64().unwrap()).unwrap()] = true;
        }
        flags
    }

    fn selected_polygon_flags(
        result: &PolygonDissectionResult,
        key: &str,
        len: usize,
    ) -> Vec<bool> {
        let flags = result.certificate.as_ref().unwrap().payload[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_bool().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(flags.len(), len);
        flags
    }

    fn merge_horizontal(cuts: &[HorizontalUnitCut]) -> Vec<HorizontalCutSegment> {
        let mut cuts = cuts.to_vec();
        cuts.sort_unstable_by_key(|cut| (cut.y, cut.x));
        let mut result = Vec::<HorizontalCutSegment>::new();
        for cut in cuts {
            let x = i64::try_from(cut.x).unwrap();
            let y = i64::try_from(cut.y).unwrap();
            if let Some(last) = result.last_mut()
                && last.y == y
                && last.right == x
            {
                last.right += 1;
            } else {
                result.push(HorizontalCutSegment {
                    left: x,
                    right: x + 1,
                    y,
                });
            }
        }
        result.sort_unstable();
        result
    }

    fn merge_vertical(cuts: &[VerticalUnitCut]) -> Vec<VerticalCutSegment> {
        let mut cuts = cuts.to_vec();
        cuts.sort_unstable_by_key(|cut| (cut.x, cut.y));
        let mut result = Vec::<VerticalCutSegment>::new();
        for cut in cuts {
            let x = i64::try_from(cut.x).unwrap();
            let y = i64::try_from(cut.y).unwrap();
            if let Some(last) = result.last_mut()
                && last.x == x
                && last.top == y
            {
                last.top += 1;
            } else {
                result.push(VerticalCutSegment {
                    x,
                    bottom: y,
                    top: y + 1,
                });
            }
        }
        result.sort_unstable();
        result
    }

    fn loop_from(points: &[(i64, i64)]) -> OrthogonalLoop {
        OrthogonalLoop::new(points.iter().map(|&(x, y)| Point::new(x, y)).collect())
    }

    #[test]
    fn rasterizes_small_translated_polygon_with_explicit_origin() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[(-3, 5), (1, 5), (1, 6), (-2, 6), (-2, 9), (-3, 9)]),
            vec![],
        )
        .unwrap();
        let raster = bounded_rasterize_polygon(&polygon, RasterLimits::default()).unwrap();
        assert_eq!((raster.origin_x, raster.origin_y), (-3, 5));
        assert_eq!((raster.grid.width, raster.grid.height), (4, 4));
        assert_eq!(raster.grid.cells.iter().filter(|&&cell| cell).count(), 7);
    }

    #[test]
    fn structured_polygon_verification_compares_every_backend() {
        let polygon =
            RectilinearPolygon::new(loop_from(&[(0, 0), (4, 0), (4, 4), (0, 4)]), vec![]).unwrap();
        let report = verify_polygon(&polygon, Some(RasterLimits::default())).unwrap();
        assert!(report.verified(), "{:?}", report.disagreements);
        assert_eq!(report.reference_geometry, report.indexed_geometry);
        assert_eq!(report.reference_geometry, report.sweep_geometry);
        assert!(report.three_backend_chord_equality);
        assert_eq!(report.sweep_metrics.sweep_aligned_pair_iterations, 0);
        assert_eq!(report.sweep_metrics.sweep_all_pair_iterations, 0);
        assert_eq!(report.sweep_metrics.sweep_definition7_fallback_checks, 0);
        assert_eq!(report.sweep_metrics.sweep_full_boundary_scans, 0);
        assert!(!report.sweep_fallback_used);
        assert!(report.sweep_certificate_summary.is_some());
        assert!(report.raster_oracle.is_some());
        assert!(report.validator_results.reference_accepts_reference);
        assert!(report.validator_results.indexed_accepts_indexed);
    }

    #[test]
    fn compact_polygon_default_keeps_dense_arrangement_trace_false() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[(0, 0), (5, 0), (5, 2), (2, 2), (2, 5), (0, 5)]),
            vec![],
        )
        .unwrap();
        let result = solve_polygon(&polygon).unwrap();
        let trace = &result.diagnostics.execution_trace;
        assert!(trace.compact_structure_check_called);
        assert!(!trace.dense_atomic_cells_materialized);
        assert!(!trace.dense_occupied_array_materialized);
        assert!(!trace.dense_horizontal_barrier_array_materialized);
        assert!(!trace.dense_vertical_barrier_array_materialized);
        assert!(!trace.dense_coverage_difference_array_materialized);
        assert_eq!(
            result.diagnostics.polygon_cut_index_backend.as_deref(),
            Some("dynamic-stabbing")
        );
        assert_eq!(
            result.diagnostics.polygon_arrangement_backend.as_deref(),
            Some("sparse-subdivision")
        );
        assert_eq!(
            result.diagnostics.polygon_validator_backend.as_deref(),
            Some("sparse-slab")
        );
    }

    #[test]
    fn rejects_large_coordinate_gap_before_allocating_cells() {
        let polygon = RectilinearPolygon::new(
            loop_from(&[(0, 0), (1_000_000_000, 0), (1_000_000_000, 1), (0, 1)]),
            vec![],
        )
        .unwrap();
        assert!(matches!(
            bounded_rasterize_polygon(&polygon, RasterLimits::default()),
            Err(RasterOracleError::WidthLimit { .. })
        ));
    }

    #[test]
    #[ignore = "release-mode extended v0.9 grid/polygon differential populations"]
    fn extended_grid_polygon_differential_populations_match() {
        use crate::adversarial::{
            clean_complete_bipartite_grid, dense_conflict_grid, endpoint_contact_instances,
            external_oracle_adversarial_instances, path_tree_geometry_families,
            topological_stress_instances,
        };
        use crate::polyomino::enumerate_free_polyominoes;
        use crate::witness::{
            mixed_branching_connected_sum_family, stored_mixed_branching_witnesses,
        };

        let mut counts = DifferentialCounts::default();
        for level in enumerate_free_polyominoes(10) {
            for polyomino in level {
                let instance = polyomino.to_instance(
                    format!("polyomino-{}", polyomino.canonical_key()),
                    "free-polyomino",
                );
                counts.inputs += 1;
                compare_instance(&instance, &mut counts);
            }
        }
        let adversarial_instances = endpoint_contact_instances()
            .into_iter()
            .chain(topological_stress_instances())
            .chain(external_oracle_adversarial_instances())
            .chain(path_tree_geometry_families(12))
            .chain(stored_mixed_branching_witnesses())
            .chain(mixed_branching_connected_sum_family(6))
            .chain([
                dense_conflict_grid(4, 5),
                dense_conflict_grid(8, 8),
                dense_conflict_grid(32, 32),
            ]);
        for instance in adversarial_instances {
            counts.inputs += 1;
            compare_instance(&instance, &mut counts);
        }
        for t in 1..=4 {
            let instance = clean_complete_bipartite_grid(t).unwrap();
            counts.inputs += 1;
            compare_instance(&instance, &mut counts);
        }
        for case in 0..1_000 {
            let instance = random_connected_instance(case);
            counts.inputs += 1;
            compare_instance(&instance, &mut counts);
        }
        assert!(counts.supported_components > 1_000);
        println!("{counts:?}");
    }

    fn compare_instance(
        instance: &crate::adversarial::AdversarialInstance,
        counts: &mut DifferentialCounts,
    ) {
        let grid = ColorGrid::new(instance.width, instance.height, instance.cells.clone()).unwrap();
        for component in grid
            .four_connected_components()
            .into_iter()
            .filter(|component| component.color)
        {
            compare_component(&component, counts)
                .unwrap_or_else(|error| panic!("{}: {error}", instance.name));
        }
    }

    fn random_connected_instance(case: usize) -> crate::adversarial::AdversarialInstance {
        let width = 8;
        let height = 8;
        let mut state = (case as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        let mut cells = vec![false; width * height];
        let mut x = case % width;
        let mut y = (case / width) % height;
        for _ in 0..24 {
            cells[y * width + x] = true;
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            match (state >> 62) as usize {
                0 if x > 0 => x -= 1,
                1 if x + 1 < width => x += 1,
                2 if y > 0 => y -= 1,
                _ if y + 1 < height => y += 1,
                _ => {}
            }
        }
        crate::adversarial::AdversarialInstance {
            name: format!("polygon-random-{case:04}"),
            family: "polygon-random-connected".to_owned(),
            width,
            height,
            cells,
            parameters: BTreeMap::from([("seed".to_owned(), case)]),
        }
    }
}
